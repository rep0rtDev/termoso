//! The client half of Mosh's State Synchronization Protocol over UDP: the
//! sender keeps a log of keystrokes/resizes and retransmits the diff the
//! server has not acknowledged; the receiver applies the server's terminal
//! diffs (plain ANSI for the local emulator) in order, acks them, and rides
//! out packet loss, roaming and long silences the way `mosh-client` does.
//!
//! Timers and constants follow the reference implementation (mosh 1.4) so a
//! stock `mosh-server` sees a client it already knows how to talk to.

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use rand::Rng;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::MoshError;
use super::crypto::{self, Key, Session as Cipher};
use super::fragment::{Assembly, Fragment, Fragmenter, HEADER_LEN};
use super::proto::{
    HostInstruction, Instruction, UserInstruction, decode_host_message, encode_user_message,
};
use crate::error::{CoreError, Result};

type MoshResult<T> = std::result::Result<T, MoshError>;
use crate::terminal::{TermEvent, TermEvents, TermSink, TermSize, TerminalSession, event_channel};

const PROTOCOL_VERSION: u32 = 2;
/// `uint64_t(-1)`: the state number that means "I am shutting down".
const SHUTDOWN_NUM: u64 = u64::MAX;

const SEND_INTERVAL_MIN: u64 = 20;
const SEND_INTERVAL_MAX: u64 = 250;
const ACK_INTERVAL: u64 = 3000;
const ACK_DELAY: u64 = 100;
const SEND_MINDELAY: u64 = 8;
const SHUTDOWN_RETRIES: u32 = 16;
const ACTIVE_RETRY_TIMEOUT: u64 = 10_000;
const MAX_SENT_STATES: usize = 32;
const CHAFF_MAX: usize = 16;

const MIN_RTO: u64 = 50;
const MAX_RTO: u64 = 1000;
const PORT_HOP_INTERVAL: u64 = 10_000;
const OLD_SOCKET_AGE: u64 = 60_000;
const MAX_SOCKETS: usize = 10;
const RECV_MTU: usize = 2048;
const DEFAULT_SEND_MTU: usize = 500;
/// Both reference MTU defaults are 1280 minus the IP+UDP headers.
const IPV4_MTU: usize = 1280 - 20 - 8;
const IPV6_MTU: usize = 1280 - 40 - 8;
/// Packet header we add before the fragments: two 16-bit timestamps.
const TIMESTAMP_BYTES: usize = 4;

/// Give up if the server never answers the first packet.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Mention lost contact to the user after this long without a packet.
const SILENCE_NOTICE: u64 = 15_000;

/// How to reach a freshly started `mosh-server`.
pub struct Options {
    pub addr: SocketAddr,
    pub key: Key,
    pub size: TermSize,
}

/// Monotonic millisecond clock shared by the whole connection.
#[derive(Clone, Copy)]
struct Clock(Instant);

impl Clock {
    fn now(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
    fn now16(&self) -> u16 {
        self.now() as u16
    }
}

fn timestamp_diff(new: u16, old: u16) -> u16 {
    new.wrapping_sub(old)
}

struct Rtt {
    srtt: f64,
    rttvar: f64,
    hit: bool,
}

impl Rtt {
    fn new() -> Self {
        Self {
            srtt: 1000.0,
            rttvar: 500.0,
            hit: false,
        }
    }

    fn sample(&mut self, r: f64) {
        if !self.hit {
            self.srtt = r;
            self.rttvar = r / 2.0;
            self.hit = true;
        } else {
            const ALPHA: f64 = 1.0 / 8.0;
            const BETA: f64 = 1.0 / 4.0;
            self.rttvar = (1.0 - BETA) * self.rttvar + BETA * (self.srtt - r).abs();
            self.srtt = (1.0 - ALPHA) * self.srtt + ALPHA * r;
        }
    }

    fn timeout(&self) -> u64 {
        ((self.srtt + 4.0 * self.rttvar).ceil() as u64).clamp(MIN_RTO, MAX_RTO)
    }

    fn send_interval(&self) -> u64 {
        ((self.srtt / 2.0).ceil() as u64).clamp(SEND_INTERVAL_MIN, SEND_INTERVAL_MAX)
    }
}

fn is_msgsize(e: &std::io::Error) -> bool {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let code = Some(90);
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let code = Some(40);
    #[cfg(windows)]
    let code = Some(10040);
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        windows
    )))]
    let code: Option<i32> = None;
    code.is_some() && e.raw_os_error() == code
}

/// The encrypted datagram layer with port hopping and RTT tracking.
struct Connection {
    remote: SocketAddr,
    sockets: Vec<(Arc<UdpSocket>, JoinHandle<()>)>,
    datagrams: mpsc::Receiver<Vec<u8>>,
    datagram_tx: mpsc::Sender<Vec<u8>>,
    cipher: Cipher,
    clock: Clock,
    mtu: usize,
    rtt: Rtt,
    expected_receiver_seq: u64,
    saved_timestamp: Option<(u16, u64)>,
    last_heard: Option<u64>,
    last_port_choice: u64,
    last_roundtrip_success: u64,
    send_error: Option<String>,
}

impl Connection {
    async fn open(remote: SocketAddr, key: &Key, clock: Clock) -> MoshResult<Self> {
        let (datagram_tx, datagrams) = mpsc::channel(256);
        let mut c = Self {
            remote,
            sockets: Vec::new(),
            datagrams,
            datagram_tx,
            cipher: Cipher::new(key, true),
            clock,
            mtu: if remote.is_ipv4() { IPV4_MTU } else { IPV6_MTU },
            rtt: Rtt::new(),
            expected_receiver_seq: 0,
            saved_timestamp: None,
            last_heard: None,
            last_port_choice: clock.now(),
            last_roundtrip_success: 0,
            send_error: None,
        };
        c.hop_port().await?;
        Ok(c)
    }

    async fn hop_port(&mut self) -> MoshResult<()> {
        let local: SocketAddr = match self.remote.ip() {
            IpAddr::V4(_) => "0.0.0.0:0".parse().expect("literal"),
            IpAddr::V6(_) => "[::]:0".parse().expect("literal"),
        };
        let sock = UdpSocket::bind(local)
            .await
            .map_err(|e| MoshError::Socket(e.to_string()))?;
        sock.connect(self.remote)
            .await
            .map_err(|e| MoshError::Socket(e.to_string()))?;
        let sock = Arc::new(sock);
        let tx = self.datagram_tx.clone();
        let reader = {
            let sock = sock.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; RECV_MTU];
                loop {
                    match sock.recv(&mut buf).await {
                        Ok(n) => {
                            if tx.send(buf[..n].to_vec()).await.is_err() {
                                break;
                            }
                        }
                        // ICMP port-unreachable surfaces here on Linux; the
                        // server may simply not be up yet, so keep listening.
                        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {}
                        Err(_) => break,
                    }
                }
            })
        };
        self.sockets.push((sock, reader));
        self.last_port_choice = self.clock.now();
        if self.sockets.len() > MAX_SOCKETS {
            let (_, task) = self.sockets.remove(0);
            task.abort();
        }
        Ok(())
    }

    fn prune_sockets(&mut self) {
        if self.sockets.len() > 1 && self.clock.now() - self.last_port_choice > OLD_SOCKET_AGE {
            let keep = self.sockets.pop().expect("non-empty");
            for (_, task) in self.sockets.drain(..) {
                task.abort();
            }
            self.sockets.push(keep);
        }
    }

    async fn maybe_hop(&mut self) {
        let now = self.clock.now();
        if now.saturating_sub(self.last_port_choice) > PORT_HOP_INTERVAL
            && now.saturating_sub(self.last_roundtrip_success) > PORT_HOP_INTERVAL
            && let Err(e) = self.hop_port().await
        {
            self.send_error = Some(e.to_string());
        }
        self.prune_sockets();
    }

    /// Payload budget for one fragment.
    fn payload_mtu(&self) -> usize {
        self.mtu
            .saturating_sub(TIMESTAMP_BYTES + crypto::ADDED_BYTES)
            .max(HEADER_LEN + 1)
    }

    fn send(&mut self, payload: &[u8]) -> MoshResult<()> {
        let now = self.clock.now();
        let reply = match self.saved_timestamp.take() {
            Some((ts, at)) if now.saturating_sub(at) < 1000 => ts.wrapping_add((now - at) as u16),
            _ => 0xFFFF,
        };
        let mut pkt = Vec::with_capacity(TIMESTAMP_BYTES + payload.len());
        pkt.extend_from_slice(&self.clock.now16().to_be_bytes());
        pkt.extend_from_slice(&reply.to_be_bytes());
        pkt.extend_from_slice(payload);
        let wire = self.cipher.encrypt(&pkt)?;
        let sock = &self.sockets.last().expect("at least one socket").0;
        match sock.try_send(&wire) {
            Ok(_) => {
                self.send_error = None;
            }
            Err(e) if is_msgsize(&e) => {
                self.mtu = DEFAULT_SEND_MTU;
                self.send_error = Some(format!("send: {e}"));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => {
                self.send_error = Some(format!("send: {e}"));
            }
        }
        Ok(())
    }

    /// Decrypt one datagram from the server; `None` for anything we ignore.
    fn receive(&mut self, wire: &[u8]) -> Option<Vec<u8>> {
        let (direction_seq, plain) = self.cipher.decrypt(wire).ok()?;
        if direction_seq & crypto::DIRECTION_MASK == 0 || plain.len() < TIMESTAMP_BYTES {
            return None;
        }
        let seq = direction_seq & crypto::SEQUENCE_MASK;
        let now = self.clock.now();
        if seq >= self.expected_receiver_seq {
            self.expected_receiver_seq = seq + 1;
            let ts = u16::from_be_bytes([plain[0], plain[1]]);
            let reply = u16::from_be_bytes([plain[2], plain[3]]);
            if ts != 0xFFFF {
                self.saved_timestamp = Some((ts, now));
            }
            if reply != 0xFFFF {
                let r = timestamp_diff(self.clock.now16(), reply);
                if r < 5000 {
                    self.rtt.sample(f64::from(r));
                }
            }
            self.last_heard = Some(now);
        }
        Some(plain[TIMESTAMP_BYTES..].to_vec())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        for (_, task) in &self.sockets {
            task.abort();
        }
    }
}

/// A state of the user-input log the server may hold: the log prefix of
/// length `len`, numbered `num`.
#[derive(Debug, Clone, Copy)]
struct SentState {
    num: u64,
    timestamp: u64,
    len: usize,
}

/// `TransportSender<UserStream>`.
struct Sender {
    clock: Clock,
    log: VecDeque<UserInstruction>,
    /// Absolute index of `log[0]`; the server is known to have everything
    /// before it.
    base: usize,
    sent: VecDeque<SentState>,
    assumed: usize,
    ack_num: u64,
    pending_data_ack: bool,
    next_ack_time: u64,
    next_send_time: Option<u64>,
    mindelay_clock: Option<u64>,
    last_heard: u64,
    shutdown_in_progress: bool,
    shutdown_tries: u32,
    shutdown_start: u64,
    fragmenter: Fragmenter,
    last_ack_sent: u64,
}

impl Sender {
    fn new(clock: Clock) -> Self {
        let now = clock.now();
        Self {
            clock,
            log: VecDeque::new(),
            base: 0,
            sent: VecDeque::from([SentState {
                num: 0,
                timestamp: now,
                len: 0,
            }]),
            assumed: 0,
            ack_num: 0,
            pending_data_ack: false,
            next_ack_time: now,
            next_send_time: Some(now),
            mindelay_clock: None,
            last_heard: 0,
            shutdown_in_progress: false,
            shutdown_tries: 0,
            shutdown_start: 0,
            fragmenter: Fragmenter::new(),
            last_ack_sent: 0,
        }
    }

    fn current_len(&self) -> usize {
        self.base + self.log.len()
    }

    fn push(&mut self, inst: UserInstruction) {
        self.log.push_back(inst);
    }

    fn diff_from(&self, len: usize) -> Vec<u8> {
        if len >= self.current_len() {
            return Vec::new();
        }
        let skip = len.saturating_sub(self.base);
        let items: Vec<UserInstruction> = self.log.iter().skip(skip).cloned().collect();
        encode_user_message(&items)
    }

    fn update_assumed_receiver_state(&mut self, rto: u64) {
        let now = self.clock.now();
        self.assumed = 0;
        for i in 1..self.sent.len() {
            if now.saturating_sub(self.sent[i].timestamp) < rto + ACK_DELAY {
                self.assumed = i;
            } else {
                return;
            }
        }
    }

    /// Forget log entries every sent state already includes.
    fn rationalize_states(&mut self) {
        let known = self.sent.front().expect("never empty").len;
        while self.base < known && !self.log.is_empty() {
            self.log.pop_front();
            self.base += 1;
        }
    }

    fn calculate_timers(&mut self, rto: u64, send_interval: u64) {
        let now = self.clock.now();
        self.update_assumed_receiver_state(rto);
        self.rationalize_states();
        if self.pending_data_ack && self.next_ack_time > now + ACK_DELAY {
            self.next_ack_time = now + ACK_DELAY;
        }
        let current = self.current_len();
        let back = *self.sent.back().expect("never empty");
        let front = *self.sent.front().expect("never empty");
        let assumed = self.sent[self.assumed];
        let heard_recently = self.last_heard + ACTIVE_RETRY_TIMEOUT > now;
        if current != back.len {
            let md = *self.mindelay_clock.get_or_insert(now);
            self.next_send_time = Some((md + SEND_MINDELAY).max(back.timestamp + send_interval));
        } else if current != assumed.len && heard_recently {
            let mut t = back.timestamp + send_interval;
            if let Some(md) = self.mindelay_clock {
                t = t.max(md + SEND_MINDELAY);
            }
            self.next_send_time = Some(t);
        } else if current != front.len && heard_recently {
            self.next_send_time = Some(back.timestamp + rto + ACK_DELAY);
        } else {
            self.next_send_time = None;
        }
        if self.shutdown_in_progress || self.ack_num == SHUTDOWN_NUM {
            self.next_ack_time = back.timestamp + send_interval;
        }
    }

    /// Milliseconds until the next thing to do.
    fn wait_time(&mut self, rto: u64, send_interval: u64) -> u64 {
        self.calculate_timers(rto, send_interval);
        let mut wake = self.next_ack_time;
        if let Some(t) = self.next_send_time {
            wake = wake.min(t);
        }
        wake.saturating_sub(self.clock.now())
    }

    fn tick(&mut self, conn: &mut Connection) -> MoshResult<()> {
        self.calculate_timers(conn.rtt.timeout(), conn.rtt.send_interval());
        let now = self.clock.now();
        let send_due = self.next_send_time.is_some_and(|t| now >= t);
        if now < self.next_ack_time && !send_due {
            return Ok(());
        }
        let diff = self.diff_from(self.sent[self.assumed].len);
        if diff.is_empty() {
            if now >= self.next_ack_time {
                self.send_empty_ack(conn)?;
                self.mindelay_clock = None;
            }
            if send_due {
                self.next_send_time = None;
                self.mindelay_clock = None;
            }
        } else if send_due || now >= self.next_ack_time {
            self.send_to_receiver(conn, diff)?;
            self.mindelay_clock = None;
        }
        Ok(())
    }

    fn send_empty_ack(&mut self, conn: &mut Connection) -> MoshResult<()> {
        let now = self.clock.now();
        let back = *self.sent.back().expect("never empty");
        let new_num = if self.shutdown_in_progress {
            SHUTDOWN_NUM
        } else {
            back.num + 1
        };
        self.add_sent_state(now, new_num, self.current_len());
        self.send_in_fragments(conn, Vec::new(), new_num)?;
        self.next_ack_time = now + ACK_INTERVAL;
        self.next_send_time = None;
        Ok(())
    }

    fn send_to_receiver(&mut self, conn: &mut Connection, diff: Vec<u8>) -> MoshResult<()> {
        let now = self.clock.now();
        let back = *self.sent.back().expect("never empty");
        let current = self.current_len();
        let new_num = if current == back.len {
            back.num
        } else {
            back.num + 1
        };
        let new_num = if self.shutdown_in_progress {
            SHUTDOWN_NUM
        } else {
            new_num
        };
        if new_num == back.num {
            self.sent.back_mut().expect("never empty").timestamp = now;
        } else {
            self.add_sent_state(now, new_num, current);
        }
        self.send_in_fragments(conn, diff, new_num)?;
        self.assumed = self.sent.len() - 1;
        self.next_ack_time = now + ACK_INTERVAL;
        self.next_send_time = None;
        Ok(())
    }

    fn add_sent_state(&mut self, timestamp: u64, num: u64, len: usize) {
        self.sent.push_back(SentState {
            num,
            timestamp,
            len,
        });
        if self.sent.len() > MAX_SENT_STATES {
            // Drop one from the middle so the oldest (acked) and the newest
            // both survive.
            self.sent.remove(self.sent.len() - 16);
            self.assumed = self.assumed.min(self.sent.len() - 1);
        }
    }

    fn send_in_fragments(
        &mut self,
        conn: &mut Connection,
        diff: Vec<u8>,
        new_num: u64,
    ) -> MoshResult<()> {
        let mut chaff = vec![0u8; usize::from(rand::random::<u8>()) % (CHAFF_MAX + 1)];
        rand::rng().fill_bytes(&mut chaff);
        let inst = Instruction {
            protocol_version: PROTOCOL_VERSION,
            old_num: self.sent[self.assumed].num,
            new_num,
            ack_num: self.ack_num,
            throwaway_num: self.sent.front().expect("never empty").num,
            diff,
            chaff,
        };
        if new_num == SHUTDOWN_NUM {
            self.shutdown_tries += 1;
        }
        self.last_ack_sent = inst.ack_num;
        for frag in self.fragmenter.make(&inst, conn.payload_mtu())? {
            conn.send(&frag.to_wire())?;
        }
        Ok(())
    }

    fn process_acknowledgment_through(&mut self, ack_num: u64) {
        if self.sent.iter().any(|s| s.num == ack_num) {
            while self.sent.front().is_some_and(|s| s.num < ack_num) {
                self.sent.pop_front();
            }
            self.assumed = self.assumed.min(self.sent.len() - 1);
        }
    }

    fn acked_timestamp(&self) -> u64 {
        self.sent.front().expect("never empty").timestamp
    }

    fn start_shutdown(&mut self) {
        if !self.shutdown_in_progress {
            self.shutdown_in_progress = true;
            self.shutdown_start = self.clock.now();
        }
    }

    fn shutdown_acknowledged(&self) -> bool {
        self.sent.front().expect("never empty").num == SHUTDOWN_NUM
    }

    fn shutdown_ack_timed_out(&self) -> bool {
        self.shutdown_in_progress
            && (self.shutdown_tries >= SHUTDOWN_RETRIES
                || self.clock.now().saturating_sub(self.shutdown_start) >= ACTIVE_RETRY_TIMEOUT)
    }

    fn counterparty_shutdown_ack_sent(&self) -> bool {
        self.last_ack_sent == SHUTDOWN_NUM
    }
}

enum Command {
    Write(Vec<u8>),
    Resize(TermSize),
    Close,
}

/// Handle the terminal layer holds; the protocol runs in a background task.
pub struct MoshTerminal {
    commands: mpsc::UnboundedSender<Command>,
    closed: AtomicBool,
}

#[async_trait]
impl TerminalSession for MoshTerminal {
    async fn write(&self, data: &[u8]) -> Result<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(CoreError::Closed);
        }
        self.commands
            .send(Command::Write(data.to_vec()))
            .map_err(|_| CoreError::Closed)
    }

    async fn resize(&self, size: TermSize) -> Result<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(CoreError::Closed);
        }
        self.commands
            .send(Command::Resize(size))
            .map_err(|_| CoreError::Closed)
    }

    async fn close(&self) -> Result<()> {
        if self.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let _ = self.commands.send(Command::Close);
        Ok(())
    }

    fn kind(&self) -> &'static str {
        "mosh"
    }
}

/// Everything the protocol task owns.
struct Client {
    clock: Clock,
    conn: Connection,
    sender: Sender,
    assembly: Assembly,
    /// Number of the server state we currently display.
    remote_num: u64,
    remote_timestamp: u64,
    server_size: Option<(i32, i32)>,
    events: TermSink,
    silent_since_notice: bool,
}

impl Client {
    /// Apply one decrypted server instruction.
    async fn process(&mut self, inst: Instruction) -> MoshResult<()> {
        if inst.protocol_version != PROTOCOL_VERSION {
            return Err(MoshError::Version(inst.protocol_version));
        }
        self.sender.process_acknowledgment_through(inst.ack_num);
        self.conn.last_roundtrip_success = self.sender.acked_timestamp();
        if inst.new_num != SHUTDOWN_NUM && inst.new_num <= self.remote_num {
            return Ok(());
        }
        if inst.old_num != self.remote_num {
            // Relative to a state we no longer hold; the server falls back
            // to our acked state once its timer expires.
            return Ok(());
        }
        let now = self.clock.now();
        if !inst.diff.is_empty() {
            let mut out = Vec::new();
            for host in decode_host_message(&inst.diff)? {
                match host {
                    HostInstruction::HostBytes(b) => out.extend_from_slice(&b),
                    HostInstruction::Resize { width, height } => {
                        self.server_size = Some((width, height));
                    }
                    HostInstruction::EchoAck(_) => {}
                }
            }
            if !out.is_empty()
                && self
                    .events
                    .send(TermEvent::Output(Bytes::from(out)))
                    .await
                    .is_err()
            {
                return Err(MoshError::Closed);
            }
            self.sender.pending_data_ack = true;
        }
        self.remote_num = inst.new_num;
        self.remote_timestamp = now;
        self.sender.ack_num = inst.new_num;
        self.sender.last_heard = now;
        Ok(())
    }

    async fn datagram(&mut self, wire: Vec<u8>) -> MoshResult<()> {
        let Some(payload) = self.conn.receive(&wire) else {
            return Ok(());
        };
        let frag = match Fragment::from_wire(&payload) {
            Ok(f) => f,
            Err(_) => return Ok(()),
        };
        match self.assembly.add(frag) {
            Ok(Some(inst)) => self.process(inst).await,
            Ok(None) => Ok(()),
            Err(MoshError::Fragment) | Err(MoshError::Compression) | Err(MoshError::Protobuf) => {
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn notice(&mut self, text: String) {
        let _ = self.events.send(TermEvent::Notice(text)).await;
    }
}

/// Connect to a bootstrapped `mosh-server` and wait for its first screen.
///
/// Returns once the server has answered, so callers can treat an `Err` as
/// "not reachable over UDP" (firewall, NAT, wrong address) rather than
/// showing an empty terminal that never fills.
pub async fn connect(opts: Options) -> Result<(Arc<MoshTerminal>, TermEvents)> {
    let clock = Clock(Instant::now());
    let conn = Connection::open(opts.addr, &opts.key, clock).await?;
    let (events, rx) = event_channel();
    let mut sender = Sender::new(clock);
    sender.push(UserInstruction::Resize {
        width: i32::from(opts.size.cols),
        height: i32::from(opts.size.rows),
    });
    let mut client = Client {
        clock,
        conn,
        sender,
        assembly: Assembly::new(),
        remote_num: 0,
        remote_timestamp: 0,
        server_size: None,
        events,
        silent_since_notice: false,
    };
    let (commands, mut cmd_rx) = mpsc::unbounded_channel();
    let deadline = tokio::time::Instant::now() + CONNECT_TIMEOUT;
    while client.remote_num == 0 {
        let wait = client
            .sender
            .wait_time(client.conn.rtt.timeout(), client.conn.rtt.send_interval());
        let wake = tokio::time::Instant::now() + Duration::from_millis(wait.min(250));
        tokio::select! {
            d = client.conn.datagrams.recv() => {
                let Some(d) = d else { return Err(MoshError::Socket("socket reader stopped".into()).into()) };
                client.datagram(d).await?;
            }
            _ = tokio::time::sleep_until(wake) => {}
            _ = tokio::time::sleep_until(deadline) => {
                return Err(MoshError::NoReply {
                    addr: opts.addr.to_string(),
                    detail: client.conn.send_error.clone(),
                }
                .into());
            }
        }
        client.sender.tick(&mut client.conn)?;
        client.conn.maybe_hop().await;
    }
    let terminal = Arc::new(MoshTerminal {
        commands,
        closed: AtomicBool::new(false),
    });
    tokio::spawn(async move {
        let result = run(&mut client, &mut cmd_rx).await;
        let ev = match result {
            Ok(()) => TermEvent::Exit {
                code: None,
                signal: None,
            },
            Err(MoshError::Closed) => TermEvent::Closed,
            Err(e) => TermEvent::Error(e.to_string()),
        };
        let _ = client.events.send(ev).await;
    });
    Ok((terminal, rx))
}

/// Steady-state loop; returns `Ok` on a clean shutdown handshake.
async fn run(
    client: &mut Client,
    commands: &mut mpsc::UnboundedReceiver<Command>,
) -> MoshResult<()> {
    loop {
        let wait = client
            .sender
            .wait_time(client.conn.rtt.timeout(), client.conn.rtt.send_interval());
        let wake = tokio::time::Instant::now() + Duration::from_millis(wait.min(250));
        tokio::select! {
            d = client.conn.datagrams.recv() => {
                let Some(d) = d else { return Err(MoshError::Socket("socket reader stopped".into())) };
                client.datagram(d).await?;
            }
            cmd = commands.recv() => {
                match cmd {
                    Some(Command::Write(bytes)) => {
                        client.sender.push(UserInstruction::Keystroke(bytes));
                        // Coalesce a burst of input into one instruction.
                        while let Ok(Command::Write(more)) = commands.try_recv() {
                            client.sender.push(UserInstruction::Keystroke(more));
                        }
                    }
                    Some(Command::Resize(size)) => client.sender.push(UserInstruction::Resize {
                        width: i32::from(size.cols),
                        height: i32::from(size.rows),
                    }),
                    Some(Command::Close) | None => client.sender.start_shutdown(),
                }
            }
            _ = tokio::time::sleep_until(wake) => {}
        }
        if client.sender.shutdown_in_progress
            && (client.sender.shutdown_acknowledged() || client.sender.shutdown_ack_timed_out())
        {
            return Err(MoshError::Closed);
        }
        if client.sender.counterparty_shutdown_ack_sent() {
            return Ok(());
        }
        let now = client.clock.now();
        let silence = now.saturating_sub(client.remote_timestamp);
        if silence > SILENCE_NOTICE && !client.silent_since_notice {
            client.silent_since_notice = true;
            client
                .notice(format!(
                    "mosh: no contact with the server for {} s — still trying",
                    silence / 1000
                ))
                .await;
        } else if silence <= SILENCE_NOTICE && client.silent_since_notice {
            client.silent_since_notice = false;
            client.notice("mosh: connection restored".to_string()).await;
        }
        client.sender.tick(&mut client.conn)?;
        client.conn.maybe_hop().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> Clock {
        Clock(Instant::now())
    }

    #[test]
    fn rtt_follows_rfc6298_and_clamps() {
        let mut r = Rtt::new();
        assert_eq!(r.timeout(), MAX_RTO);
        assert_eq!(r.send_interval(), SEND_INTERVAL_MAX);
        r.sample(10.0);
        assert_eq!(r.timeout(), MIN_RTO);
        assert_eq!(r.send_interval(), SEND_INTERVAL_MIN);
        r.sample(200.0);
        assert!(r.srtt > 10.0 && r.srtt < 200.0);
        assert_eq!(timestamp_diff(5, 65530), 11);
    }

    #[test]
    fn sender_diffs_from_the_assumed_state_and_prunes_on_ack() {
        let mut s = Sender::new(clock());
        assert!(s.diff_from(0).is_empty());
        s.push(UserInstruction::Keystroke(b"a".to_vec()));
        s.push(UserInstruction::Keystroke(b"b".to_vec()));
        assert_eq!(
            s.diff_from(0),
            encode_user_message(&[UserInstruction::Keystroke(b"ab".to_vec())])
        );
        s.add_sent_state(1, 1, 1);
        s.add_sent_state(2, 2, 2);
        assert_eq!(
            s.diff_from(1),
            encode_user_message(&[UserInstruction::Keystroke(b"b".to_vec())])
        );
        s.process_acknowledgment_through(7); // unknown ack: nothing happens
        assert_eq!(s.sent.len(), 3);
        s.process_acknowledgment_through(1);
        assert_eq!(s.sent.front().unwrap().num, 1);
        s.rationalize_states();
        assert_eq!((s.base, s.log.len()), (1, 1));
        assert_eq!(
            s.diff_from(1),
            encode_user_message(&[UserInstruction::Keystroke(b"b".to_vec())])
        );
        assert!(s.diff_from(2).is_empty());
    }

    #[test]
    fn sent_states_are_bounded() {
        let mut s = Sender::new(clock());
        for i in 1..=100 {
            s.add_sent_state(i, i, 0);
        }
        assert_eq!(s.sent.len(), MAX_SENT_STATES);
        assert_eq!(s.sent.front().unwrap().num, 0);
        assert_eq!(s.sent.back().unwrap().num, 100);
    }

    #[test]
    fn shutdown_bookkeeping() {
        let mut s = Sender::new(clock());
        assert!(!s.shutdown_ack_timed_out());
        s.start_shutdown();
        assert!(s.shutdown_in_progress);
        assert!(!s.shutdown_acknowledged());
        s.shutdown_tries = SHUTDOWN_RETRIES;
        assert!(s.shutdown_ack_timed_out());
        s.add_sent_state(1, SHUTDOWN_NUM, 0);
        s.process_acknowledgment_through(SHUTDOWN_NUM);
        assert!(s.shutdown_acknowledged());
    }
}
