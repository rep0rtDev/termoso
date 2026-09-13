//! Multiplayer: share a live terminal with other signed-in users through the
//! server relay, end-to-end encrypted.
//!
//! * The **host** keeps its real terminal and mirrors its output into a
//!   [`HostShare`]; every frame is sealed with a key derived from the link
//!   secret ([`termoso_crypto::live`]) before it reaches the relay.
//! * A **viewer** opens the link and gets a [`ViewerTerminal`] that behaves
//!   like any other [`TerminalSession`]: output arrives as [`TermEvent`]s,
//!   `write` is forwarded only while the host granted remote control.
//!
//! The relay only sees ciphertext and presence; it can neither read the
//! terminal nor forge frames.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use termoso_crypto::keys::SymmetricKey;
use termoso_crypto::live::{self as crypto, Direction, FrameKind, LiveSecret};
use termoso_proto::live::{LiveClientMessage, LiveParticipant, LiveServerMessage};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::api::ApiClient;
use crate::error::{CoreError, Result};
use crate::terminal::{SharedTerminal, TermEvent, TermEvents, TermSize, TerminalSession};

/// URL scheme of invitation links.
pub const SCHEME: &str = "termoso";
const PING_EVERY: Duration = Duration::from_secs(25);
/// Output kept for late joiners.
const REPLAY_BYTES: usize = 128 * 1024;
/// Outbound queue between the terminal pump and the relay task.
const QUEUE: usize = 1024;

/// `termoso://join/<session>?s=<server>#<secret>`.
///
/// The secret travels in the fragment so it is never part of anything a
/// server sees; the server URL is only a hint for a clear mismatch error.
#[derive(Clone)]
pub struct LiveLink {
    /// Relay session id.
    pub session_id: Uuid,
    /// Server the session lives on (as shown by the host).
    pub server: Option<Url>,
    /// Link secret.
    pub secret: LiveSecret,
}

impl std::fmt::Debug for LiveLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveLink")
            .field("session_id", &self.session_id)
            .field("server", &self.server.as_ref().map(Url::as_str))
            .finish_non_exhaustive()
    }
}

impl LiveLink {
    /// Is `s` a multiplayer link?
    pub fn looks_like(s: &str) -> bool {
        let s = s.trim();
        s.starts_with(&format!("{SCHEME}://join/")) || s.starts_with(&format!("{SCHEME}:join/"))
    }

    /// Parse a link; the secret is validated (length) but nothing is contacted.
    pub fn parse(s: &str) -> Result<Self> {
        let u = Url::parse(s.trim()).map_err(|_| CoreError::Invalid("not a link".into()))?;
        if u.scheme() != SCHEME || u.host_str() != Some("join") {
            return Err(CoreError::Invalid("not a multiplayer link".into()));
        }
        let id = u
            .path_segments()
            .and_then(|mut p| p.next().map(str::to_owned))
            .ok_or_else(|| CoreError::Invalid("link has no session id".into()))?;
        let session_id =
            Uuid::parse_str(&id).map_err(|_| CoreError::Invalid("bad session id".into()))?;
        let secret = u
            .fragment()
            .filter(|f| !f.is_empty())
            .ok_or_else(|| CoreError::Invalid("link has no secret".into()))?;
        let secret = LiveSecret::from_b64(secret)
            .map_err(|_| CoreError::Invalid("link secret is malformed".into()))?;
        let server = u
            .query_pairs()
            .find(|(k, _)| k == "s")
            .and_then(|(_, v)| Url::parse(&v).ok());
        Ok(Self {
            session_id,
            server,
            secret,
        })
    }

    /// Render the link.
    pub fn to_url(&self) -> String {
        let mut s = format!("{SCHEME}://join/{}", self.session_id);
        if let Some(server) = &self.server {
            let q: String = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("s", server.as_str())
                .finish();
            s.push('?');
            s.push_str(&q);
        }
        s.push('#');
        s.push_str(&self.secret.to_b64());
        s
    }
}

/// Anything the UI wants to know about a share (either side).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveEvent {
    /// Who is in the session (host first).
    Participants {
        /// Everyone connected, host included.
        participants: Vec<LiveParticipant>,
    },
    /// Viewer: the host granted / revoked remote control.
    Control {
        /// Whether our keystrokes now reach the host terminal.
        can_write: bool,
    },
    /// Viewer: the host terminal changed size.
    Resize {
        /// Columns.
        cols: u16,
        /// Rows.
        rows: u16,
    },
    /// Viewer: the host tab title.
    Title {
        /// New title.
        title: String,
    },
    /// The session is over.
    Ended {
        /// `stopped`, `disconnected` or `error`.
        reason: String,
        /// Human-readable explanation.
        message: String,
    },
}

/// Host → viewers metadata frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cols: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rows: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

// ---- relay socket ----------------------------------------------------------

enum Incoming {
    Msg(LiveServerMessage),
    Frame(Bytes),
    Closed,
}

/// Result of the auth handshake.
#[derive(Debug, Clone)]
pub struct Joined {
    /// Our user id as the server sees it.
    pub user_id: Uuid,
    /// Whether we are the host.
    pub is_host: bool,
    /// Current participants.
    pub participants: Vec<LiveParticipant>,
}

/// One authenticated relay connection, split into a reader and a writer.
struct Socket {
    out: mpsc::Sender<Message>,
    inbox: mpsc::Receiver<Incoming>,
    task: JoinHandle<()>,
}

impl Drop for Socket {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Socket {
    async fn connect(
        api: &ApiClient,
        session_id: Uuid,
        join_token: Option<String>,
    ) -> Result<(Self, Joined)> {
        let token = api.token().ok_or(CoreError::NotSignedIn)?;
        let url = api.live_ws_url(session_id)?;
        let (mut ws, _) = tokio_tungstenite::connect_async(url.as_str())
            .await
            .map_err(|e| CoreError::Ws(e.to_string()))?;
        let auth = serde_json::to_string(&LiveClientMessage::Auth { token, join_token })?;
        ws.send(Message::Text(auth.into()))
            .await
            .map_err(|e| CoreError::Ws(e.to_string()))?;
        let joined = loop {
            match ws.next().await {
                Some(Ok(Message::Text(t))) => match serde_json::from_str(&t)? {
                    LiveServerMessage::Hello {
                        user_id,
                        is_host,
                        participants,
                        ..
                    } => {
                        break Joined {
                            user_id,
                            is_host,
                            participants,
                        };
                    }
                    LiveServerMessage::Error { code, message } => {
                        return Err(server_error(code, message));
                    }
                    _ => continue,
                },
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => continue,
                Some(Ok(_)) | None => {
                    return Err(CoreError::Ws("relay closed during handshake".into()));
                }
                Some(Err(e)) => return Err(CoreError::Ws(e.to_string())),
            }
        };

        let (out, mut out_rx) = mpsc::channel::<Message>(QUEUE);
        let (inbox_tx, inbox) = mpsc::channel::<Incoming>(QUEUE);
        let task = tokio::spawn(async move {
            let mut ping = tokio::time::interval(PING_EVERY);
            ping.tick().await;
            loop {
                tokio::select! {
                    m = out_rx.recv() => match m {
                        Some(m) => {
                            if ws.send(m).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    },
                    f = ws.next() => match f {
                        Some(Ok(Message::Text(t))) => match serde_json::from_str::<LiveServerMessage>(&t) {
                            Ok(LiveServerMessage::Pong) => {}
                            Ok(m) => {
                                if inbox_tx.send(Incoming::Msg(m)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => tracing::debug!("live: bad frame from relay: {e}"),
                        },
                        Some(Ok(Message::Binary(b))) => {
                            if inbox_tx.send(Incoming::Frame(b)).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                        Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    },
                    _ = ping.tick() => {
                        let p = serde_json::to_string(&LiveClientMessage::Ping).unwrap_or_default();
                        if ws.send(Message::Text(p.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = inbox_tx.send(Incoming::Closed).await;
            let _ = ws.close(None).await;
        });
        Ok((Self { out, inbox, task }, joined))
    }

    async fn send_msg(&self, m: &LiveClientMessage) -> Result<()> {
        let s = serde_json::to_string(m)?;
        self.out
            .send(Message::Text(s.into()))
            .await
            .map_err(|_| CoreError::Ws("relay connection is gone".into()))
    }

    async fn send_frame(&self, frame: Vec<u8>) -> Result<()> {
        self.out
            .send(Message::Binary(frame.into()))
            .await
            .map_err(|_| CoreError::Ws("relay connection is gone".into()))
    }

    async fn recv(&mut self) -> Incoming {
        self.inbox.recv().await.unwrap_or(Incoming::Closed)
    }
}

fn server_error(code: String, message: String) -> CoreError {
    let status = match code.as_str() {
        "unauthorized" | "auth_required" => 401,
        "forbidden" | "multiplayer_disabled" => 403,
        "not_found" => 404,
        "ended" => 410,
        _ => 400,
    };
    CoreError::Api {
        status,
        code,
        message,
    }
}

// ---- host ------------------------------------------------------------------

enum HostCmd {
    Output(Bytes),
    Resize(TermSize),
    Title(String),
    Control { user_id: Uuid, enabled: bool },
    Stop,
}

/// A terminal being shared. Dropping it ends the share (the relay ends the
/// session as soon as the host connection goes away).
pub struct HostShare {
    session_id: Uuid,
    link: String,
    user_id: Uuid,
    cmds: mpsc::Sender<HostCmd>,
    task: JoinHandle<()>,
}

impl Drop for HostShare {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl HostShare {
    /// Register a session on the server, connect to the relay and start
    /// forwarding. `term` receives input from viewers holding remote control.
    pub async fn start(
        api: Arc<ApiClient>,
        term: SharedTerminal,
        size: TermSize,
        title: String,
        events: mpsc::Sender<LiveEvent>,
    ) -> Result<Self> {
        let secret = LiveSecret::generate();
        let created = api.create_live_session(secret.join_token()?).await?;
        let session_id = created.id;
        let key = secret.stream_key(&session_id.to_string())?;
        let link = LiveLink {
            session_id,
            server: Some(api.server_url().clone()),
            secret,
        }
        .to_url();

        let (socket, joined) = match Socket::connect(&api, session_id, None).await {
            Ok(v) => v,
            Err(e) => {
                let _ = api.stop_live_session(session_id).await;
                return Err(e);
            }
        };
        let _ = events
            .send(LiveEvent::Participants {
                participants: joined.participants.clone(),
            })
            .await;

        let (cmds, cmd_rx) = mpsc::channel(QUEUE);
        let task = tokio::spawn(host_loop(
            HostLoop {
                api: api.clone(),
                session_id,
                key,
                term,
                size,
                title,
                events,
                seen: joined.participants.iter().map(|p| p.user_id).collect(),
            },
            socket,
            cmd_rx,
        ));
        Ok(Self {
            session_id,
            link,
            user_id: joined.user_id,
            cmds,
            task,
        })
    }

    /// Relay session id.
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Invitation link (contains the secret — show it, do not log it).
    pub fn link(&self) -> &str {
        &self.link
    }

    /// Our user id.
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    /// Cheap handle for the terminal output pump.
    pub fn publisher(&self) -> Publisher {
        Publisher {
            session_id: self.session_id,
            cmds: self.cmds.clone(),
        }
    }

    /// The shared terminal was resized.
    pub fn resized(&self, size: TermSize) {
        let _ = self.cmds.try_send(HostCmd::Resize(size));
    }

    /// The tab title changed.
    pub fn retitled(&self, title: String) {
        let _ = self.cmds.try_send(HostCmd::Title(title));
    }

    /// Grant or revoke remote control for a viewer.
    pub fn set_control(&self, user_id: Uuid, enabled: bool) -> Result<()> {
        self.cmds
            .try_send(HostCmd::Control { user_id, enabled })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => CoreError::Ws("relay is busy".into()),
                mpsc::error::TrySendError::Closed(_) => CoreError::Ws("share has ended".into()),
            })
    }

    /// Stop sharing: tells the server, which notifies every viewer. Resolves
    /// once the relay loop has wound down (or after a short grace period).
    pub async fn stop(mut self) {
        let _ = self.cmds.send(HostCmd::Stop).await;
        let task = std::mem::replace(&mut self.task, tokio::spawn(async {}));
        let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
    }
}

/// Feeds terminal output into a [`HostShare`] from wherever the bytes flow.
#[derive(Clone)]
pub struct Publisher {
    session_id: Uuid,
    cmds: mpsc::Sender<HostCmd>,
}

impl Publisher {
    /// Mirror terminal output. Never blocks the terminal: when the relay
    /// cannot keep up the chunk is dropped and viewers fall behind.
    pub fn publish(&self, bytes: Bytes) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.cmds.try_send(HostCmd::Output(bytes))
        {
            tracing::debug!(session = %self.session_id, "live: relay queue full, dropping output");
        }
    }
}

struct HostLoop {
    api: Arc<ApiClient>,
    session_id: Uuid,
    key: SymmetricKey,
    term: SharedTerminal,
    size: TermSize,
    title: String,
    events: mpsc::Sender<LiveEvent>,
    seen: HashSet<Uuid>,
}

impl HostLoop {
    fn seal(&self, kind: FrameKind, payload: &[u8]) -> Result<Vec<u8>> {
        Ok(crypto::seal_frame(
            &self.key,
            &self.session_id.to_string(),
            Direction::FromHost,
            kind,
            payload,
        )?)
    }

    fn meta(&self) -> Result<Vec<u8>> {
        let m = Meta {
            cols: Some(self.size.cols),
            rows: Some(self.size.rows),
            title: Some(self.title.clone()),
        };
        self.seal(FrameKind::Meta, &serde_json::to_vec(&m)?)
    }
}

async fn host_loop(mut h: HostLoop, mut socket: Socket, mut cmds: mpsc::Receiver<HostCmd>) {
    let mut replay: Vec<u8> = Vec::with_capacity(REPLAY_BYTES);
    let sid = h.session_id.to_string();
    let ended = loop {
        tokio::select! {
            cmd = cmds.recv() => match cmd {
                Some(HostCmd::Output(b)) => {
                    replay.extend_from_slice(&b);
                    if replay.len() > REPLAY_BYTES {
                        let cut = replay.len() - REPLAY_BYTES;
                        replay.drain(..cut);
                    }
                    match h.seal(FrameKind::Output, &b) {
                        Ok(f) => {
                            if socket.send_frame(f).await.is_err() {
                                break ("disconnected", "Lost connection to the relay");
                            }
                        }
                        Err(e) => tracing::warn!("live: seal failed: {e}"),
                    }
                }
                Some(HostCmd::Resize(size)) => {
                    h.size = size;
                    if let Ok(f) = h.meta() {
                        let _ = socket.send_frame(f).await;
                    }
                }
                Some(HostCmd::Title(title)) => {
                    h.title = title;
                    if let Ok(f) = h.meta() {
                        let _ = socket.send_frame(f).await;
                    }
                }
                Some(HostCmd::Control { user_id, enabled }) => {
                    let _ = socket
                        .send_msg(&LiveClientMessage::Control { user_id, enabled })
                        .await;
                }
                Some(HostCmd::Stop) | None => {
                    if let Err(e) = h.api.stop_live_session(h.session_id).await {
                        tracing::debug!("live: stop on server failed: {e}");
                    }
                    break ("stopped", "Multiplayer stopped");
                }
            },
            inc = socket.recv() => match inc {
                Incoming::Frame(b) => {
                    match crypto::open_frame(&h.key, &sid, Direction::FromViewer, &b) {
                        Ok((FrameKind::Input, data)) => {
                            if let Err(e) = h.term.write(&data).await {
                                tracing::debug!("live: viewer input not delivered: {e}");
                            }
                        }
                        Ok(_) => {}
                        Err(e) => tracing::debug!("live: dropping viewer frame: {e}"),
                    }
                }
                Incoming::Msg(LiveServerMessage::Participants { participants }) => {
                    // Late joiners get the current size and recent output
                    // through a direct (encrypted) frame.
                    for p in participants.iter().filter(|p| !p.is_host) {
                        if h.seen.insert(p.user_id) {
                            use base64::Engine;
                            let std = base64::engine::general_purpose::STANDARD;
                            if let Ok(f) = h.meta() {
                                let _ = socket
                                    .send_msg(&LiveClientMessage::Direct {
                                        user_id: p.user_id,
                                        data: std.encode(f),
                                    })
                                    .await;
                            }
                            if !replay.is_empty()
                                && let Ok(f) = h.seal(FrameKind::Output, &replay)
                            {
                                let _ = socket
                                    .send_msg(&LiveClientMessage::Direct {
                                        user_id: p.user_id,
                                        data: std.encode(f),
                                    })
                                    .await;
                            }
                        }
                    }
                    let _ = h.events.send(LiveEvent::Participants { participants }).await;
                }
                Incoming::Msg(LiveServerMessage::Ended) => break ("stopped", "Multiplayer stopped"),
                Incoming::Msg(LiveServerMessage::Error { message, .. }) => {
                    tracing::warn!("live: relay error: {message}");
                    break ("error", "Relay error");
                }
                Incoming::Msg(_) => {}
                Incoming::Closed => break ("disconnected", "Lost connection to the relay"),
            }
        }
    };
    let _ = h
        .events
        .send(LiveEvent::Ended {
            reason: ended.0.into(),
            message: ended.1.into(),
        })
        .await;
}

// ---- viewer ----------------------------------------------------------------

/// Read-only (until granted control) terminal mirroring a shared session.
pub struct ViewerTerminal {
    session_id: String,
    key: SymmetricKey,
    frames: mpsc::Sender<Vec<u8>>,
    can_write: Arc<AtomicBool>,
    cancel: CancellationToken,
}

impl ViewerTerminal {
    /// Whether the host granted remote control.
    pub fn can_write(&self) -> bool {
        self.can_write.load(Ordering::Relaxed)
    }
}

#[async_trait::async_trait]
impl TerminalSession for ViewerTerminal {
    async fn write(&self, data: &[u8]) -> Result<()> {
        if !self.can_write() {
            return Ok(());
        }
        let f = crypto::seal_frame(
            &self.key,
            &self.session_id,
            Direction::FromViewer,
            FrameKind::Input,
            data,
        )?;
        self.frames
            .send(f)
            .await
            .map_err(|_| CoreError::Ws("share has ended".into()))
    }

    async fn resize(&self, _size: TermSize) -> Result<()> {
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        self.cancel.cancel();
        Ok(())
    }

    fn kind(&self) -> &'static str {
        "multiplayer"
    }
}

/// Joined session as seen by a viewer.
pub struct ViewerJoin {
    /// Terminal to attach to a tab.
    pub term: SharedTerminal,
    /// Output / lifecycle stream for the tab.
    pub events: TermEvents,
    /// Our user id.
    pub user_id: Uuid,
    /// Participants at join time (host included).
    pub participants: Vec<LiveParticipant>,
}

/// Open a link. Fails with the server's error when the link is wrong, the
/// session has ended, or the team disabled multiplayer.
pub async fn join(
    api: Arc<ApiClient>,
    link: &LiveLink,
    live_events: mpsc::Sender<LiveEvent>,
) -> Result<ViewerJoin> {
    if let Some(server) = &link.server
        && server.host_str() != api.server_url().host_str()
    {
        return Err(CoreError::Invalid(format!(
            "This session is hosted on {}, but you are signed in to {}",
            server.host_str().unwrap_or("another server"),
            api.server_url().host_str().unwrap_or("a different server")
        )));
    }
    let session_id = link.session_id;
    let key = link.secret.stream_key(&session_id.to_string())?;
    let (socket, joined) =
        Socket::connect(&api, session_id, Some(link.secret.join_token()?)).await?;
    if joined.is_host {
        return Err(CoreError::Invalid(
            "You are the host of this session on another device".into(),
        ));
    }

    let (events_tx, events) = crate::terminal::event_channel();
    let (frames, frames_rx) = mpsc::channel(QUEUE);
    let can_write = Arc::new(AtomicBool::new(false));
    let cancel = CancellationToken::new();
    let term = Arc::new(ViewerTerminal {
        session_id: session_id.to_string(),
        key: key.clone(),
        frames,
        can_write: can_write.clone(),
        cancel: cancel.clone(),
    });
    tokio::spawn(viewer_loop(
        session_id.to_string(),
        key,
        socket,
        frames_rx,
        events_tx,
        live_events,
        can_write,
        cancel,
    ));
    Ok(ViewerJoin {
        term,
        events,
        user_id: joined.user_id,
        participants: joined.participants,
    })
}

#[allow(clippy::too_many_arguments)]
async fn viewer_loop(
    sid: String,
    key: SymmetricKey,
    mut socket: Socket,
    mut frames: mpsc::Receiver<Vec<u8>>,
    term_events: crate::terminal::TermSink,
    live_events: mpsc::Sender<LiveEvent>,
    can_write: Arc<AtomicBool>,
    cancel: CancellationToken,
) {
    let ended: Option<(&str, String)> = loop {
        tokio::select! {
            _ = cancel.cancelled() => break None,
            f = frames.recv() => match f {
                Some(f) => {
                    if socket.send_frame(f).await.is_err() {
                        break Some(("disconnected", "Lost connection to the relay".into()));
                    }
                }
                None => break None,
            },
            inc = socket.recv() => match inc {
                Incoming::Frame(b) => match crypto::open_frame(&key, &sid, Direction::FromHost, &b) {
                    Ok((FrameKind::Output, data)) => {
                        if term_events.send(TermEvent::Output(Bytes::from(data))).await.is_err() {
                            break None;
                        }
                    }
                    Ok((FrameKind::Meta, data)) => {
                        if let Ok(m) = serde_json::from_slice::<Meta>(&data) {
                            if let (Some(cols), Some(rows)) = (m.cols, m.rows) {
                                let _ = live_events.send(LiveEvent::Resize { cols, rows }).await;
                            }
                            if let Some(title) = m.title {
                                let _ = live_events.send(LiveEvent::Title { title }).await;
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => tracing::debug!("live: dropping host frame: {e}"),
                },
                Incoming::Msg(LiveServerMessage::Participants { participants }) => {
                    let _ = live_events.send(LiveEvent::Participants { participants }).await;
                }
                Incoming::Msg(LiveServerMessage::Control { can_write: w }) => {
                    can_write.store(w, Ordering::Relaxed);
                    let _ = live_events.send(LiveEvent::Control { can_write: w }).await;
                }
                Incoming::Msg(LiveServerMessage::Ended) => {
                    break Some(("stopped", "The host stopped multiplayer".into()));
                }
                Incoming::Msg(LiveServerMessage::Error { message, .. }) => {
                    break Some(("error", message));
                }
                Incoming::Msg(_) => {}
                Incoming::Closed => break Some(("disconnected", "Lost connection to the relay".into())),
            }
        }
    };
    if let Some((reason, message)) = ended {
        let _ = live_events
            .send(LiveEvent::Ended {
                reason: reason.into(),
                message: message.clone(),
            })
            .await;
        let _ = term_events
            .send(TermEvent::Notice(format!("\r\n[{message}]\r\n")))
            .await;
    }
    let _ = term_events.send(TermEvent::Closed).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_round_trips_and_keeps_secret_in_fragment() {
        let link = LiveLink {
            session_id: Uuid::new_v4(),
            server: Some(Url::parse("https://cloud.example.org/").unwrap()),
            secret: LiveSecret::generate(),
        };
        let s = link.to_url();
        assert!(LiveLink::looks_like(&s));
        let (before, frag) = s.split_once('#').unwrap();
        assert!(
            !before.contains(frag),
            "secret must only be in the fragment"
        );
        let parsed = LiveLink::parse(&s).unwrap();
        assert_eq!(parsed.session_id, link.session_id);
        assert_eq!(parsed.secret.to_b64(), link.secret.to_b64());
        assert_eq!(
            parsed.server.as_ref().map(Url::as_str),
            Some("https://cloud.example.org/")
        );
    }

    #[test]
    fn link_rejects_garbage() {
        assert!(LiveLink::parse("termoso://host/abc").is_err());
        assert!(LiveLink::parse(&format!("termoso://join/{}", Uuid::new_v4())).is_err());
        assert!(LiveLink::parse(&format!("termoso://join/{}#short", Uuid::new_v4())).is_err());
        assert!(!LiveLink::looks_like("ssh://root@example"));
    }

    #[test]
    fn meta_frames_are_optional_fields() {
        let m: Meta = serde_json::from_str(r#"{"title":"x"}"#).unwrap();
        assert_eq!(m.title.as_deref(), Some("x"));
        assert!(m.cols.is_none());
    }
}
