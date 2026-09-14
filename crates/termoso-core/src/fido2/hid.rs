//! CTAPHID framing (CTAP 2.1 §11.2) over 64-byte HID reports supplied by
//! the platform – on Android, `UsbDeviceConnection.bulkTransfer` on the
//! interrupt endpoints of the FIDO interface.

use std::time::{Duration, Instant};

use super::Fido2Error;
use super::ctap::CtapTransport;

/// HID report size every FIDO authenticator uses.
pub const PACKET_SIZE: usize = 64;
const INIT_DATA: usize = PACKET_SIZE - 7;
const CONT_DATA: usize = PACKET_SIZE - 5;
const BROADCAST_CID: u32 = 0xFFFF_FFFF;

const CMD_INIT: u8 = 0x06;
const CMD_CBOR: u8 = 0x10;
const CMD_CANCEL: u8 = 0x11;
const CMD_KEEPALIVE: u8 = 0x3B;
const CMD_ERROR: u8 = 0x3F;

/// Raw HID report I/O. `write` sends one `PACKET_SIZE` report (no report
/// id); `read` returns the next report or `None` when `timeout` passes.
pub trait HidPackets: Send {
    /// Send one report.
    fn write(&mut self, packet: &[u8]) -> Result<(), Fido2Error>;
    /// Receive one report, `None` on timeout.
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, Fido2Error>;
}

/// CTAPHID channel on top of [`HidPackets`].
pub struct HidTransport<P: HidPackets> {
    io: P,
    cid: u32,
    /// How long to wait for the user (touch / PIN on the token).
    pub user_timeout: Duration,
}

impl<P: HidPackets> HidTransport<P> {
    /// Allocate a channel (`CTAPHID_INIT`).
    pub fn open(mut io: P) -> Result<Self, Fido2Error> {
        let mut nonce = [0u8; 8];
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut nonce);
        write_message(&mut io, BROADCAST_CID, CMD_INIT, &nonce)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let (cid, cmd, data) = read_message(&mut io, BROADCAST_CID, deadline)?;
            if cid != BROADCAST_CID || cmd != CMD_INIT {
                continue;
            }
            if data.len() < 17 {
                return Err(Fido2Error::Other("short CTAPHID_INIT response".into()));
            }
            if data[..8] != nonce {
                // Another client's INIT; keep waiting for ours.
                continue;
            }
            let cid = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
            if data[12] != 2 {
                return Err(Fido2Error::Unsupported(format!(
                    "CTAPHID protocol version {}",
                    data[12]
                )));
            }
            // Capability flags: WINK 0x01, CBOR 0x04, NMSG 0x08.
            if data[16] & 0x04 == 0 {
                return Err(Fido2Error::Unsupported("device speaks U2F only".into()));
            }
            return Ok(Self {
                io,
                cid,
                user_timeout: Duration::from_secs(60),
            });
        }
    }

    /// Channel id the device handed out.
    pub fn channel(&self) -> u32 {
        self.cid
    }

    /// Give the report I/O back.
    pub fn into_inner(self) -> P {
        self.io
    }
}

impl<P: HidPackets> CtapTransport for HidTransport<P> {
    fn cbor(&mut self, payload: &[u8]) -> Result<Vec<u8>, Fido2Error> {
        write_message(&mut self.io, self.cid, CMD_CBOR, payload)?;
        let deadline = Instant::now() + self.user_timeout;
        loop {
            let (cid, cmd, data) = match read_message(&mut self.io, self.cid, deadline) {
                Ok(m) => m,
                Err(Fido2Error::Timeout) => {
                    let _ = write_message(&mut self.io, self.cid, CMD_CANCEL, &[]);
                    return Err(Fido2Error::Timeout);
                }
                Err(e) => return Err(e),
            };
            if cid != self.cid {
                continue;
            }
            match cmd {
                CMD_KEEPALIVE => continue,
                CMD_CBOR => return Ok(data),
                CMD_ERROR => return Err(hid_error(data.first().copied().unwrap_or(0))),
                other => {
                    return Err(Fido2Error::Other(format!(
                        "unexpected CTAPHID command 0x{other:02x}"
                    )));
                }
            }
        }
    }
}

fn hid_error(code: u8) -> Fido2Error {
    match code {
        0x06 => Fido2Error::Other("device busy (another client holds it)".into()),
        0x2D => Fido2Error::Denied,
        0x05 => Fido2Error::Timeout,
        other => Fido2Error::Other(format!("CTAPHID error 0x{other:02x}")),
    }
}

/// Split `data` into an init packet and continuation packets.
pub fn frame(cid: u32, cmd: u8, data: &[u8]) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    let mut init = Vec::with_capacity(PACKET_SIZE);
    init.extend_from_slice(&cid.to_be_bytes());
    init.push(0x80 | cmd);
    init.extend_from_slice(&(data.len() as u16).to_be_bytes());
    let first = data.len().min(INIT_DATA);
    init.extend_from_slice(&data[..first]);
    init.resize(PACKET_SIZE, 0);
    packets.push(init);
    for (seq, chunk) in data[first..].chunks(CONT_DATA).enumerate() {
        let mut p = Vec::with_capacity(PACKET_SIZE);
        p.extend_from_slice(&cid.to_be_bytes());
        p.push(seq as u8);
        p.extend_from_slice(chunk);
        p.resize(PACKET_SIZE, 0);
        packets.push(p);
    }
    packets
}

fn write_message<P: HidPackets>(
    io: &mut P,
    cid: u32,
    cmd: u8,
    data: &[u8],
) -> Result<(), Fido2Error> {
    if data.len() > 7609 {
        return Err(Fido2Error::Other("CTAPHID message too large".into()));
    }
    for p in frame(cid, cmd, data) {
        io.write(&p)?;
    }
    Ok(())
}

/// Reassembles one message. Packets on other channels are dropped; a
/// stray continuation packet before an init packet is ignored.
pub struct Reassembler {
    cid: u32,
    cmd: u8,
    len: usize,
    data: Vec<u8>,
    seq: u8,
    started: bool,
}

impl Reassembler {
    /// Collect packets for channel `cid`.
    pub fn new(cid: u32) -> Self {
        Self {
            cid,
            cmd: 0,
            len: 0,
            data: Vec::new(),
            seq: 0,
            started: false,
        }
    }

    /// Feed one report; `Some((cmd, data))` when the message is complete.
    pub fn push(&mut self, packet: &[u8]) -> Result<Option<(u8, Vec<u8>)>, Fido2Error> {
        if packet.len() < 5 {
            return Err(Fido2Error::Other("short HID report".into()));
        }
        let cid = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);
        if cid != self.cid {
            return Ok(None);
        }
        if packet[4] & 0x80 != 0 {
            if packet.len() < 7 {
                return Err(Fido2Error::Other("short HID report".into()));
            }
            self.cmd = packet[4] & 0x7F;
            self.len = u16::from_be_bytes([packet[5], packet[6]]) as usize;
            self.data.clear();
            self.seq = 0;
            self.started = true;
            let take = self.len.min(packet.len() - 7);
            self.data.extend_from_slice(&packet[7..7 + take]);
        } else {
            if !self.started {
                return Ok(None);
            }
            if packet[4] != self.seq {
                self.started = false;
                return Err(Fido2Error::Other("CTAPHID sequence error".into()));
            }
            self.seq = self.seq.wrapping_add(1);
            let take = (self.len - self.data.len()).min(packet.len() - 5);
            self.data.extend_from_slice(&packet[5..5 + take]);
        }
        if self.data.len() >= self.len {
            self.started = false;
            return Ok(Some((self.cmd, std::mem::take(&mut self.data))));
        }
        Ok(None)
    }
}

fn read_message<P: HidPackets>(
    io: &mut P,
    cid: u32,
    deadline: Instant,
) -> Result<(u32, u8, Vec<u8>), Fido2Error> {
    let mut asm = Reassembler::new(cid);
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(Fido2Error::Timeout);
        }
        let step = (deadline - now).min(Duration::from_millis(500));
        let Some(packet) = io.read(step)? else {
            continue;
        };
        if let Some((cmd, data)) = asm.push(&packet)? {
            return Ok((cid, cmd, data));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_and_reassemble_round_trip() {
        for len in [0usize, 1, 57, 58, 57 + 59, 57 + 59 + 1, 1200] {
            let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            let packets = frame(0x0102_0304, CMD_CBOR, &data);
            let expect = 1 + len.saturating_sub(INIT_DATA).div_ceil(CONT_DATA);
            assert_eq!(packets.len(), expect, "len {len}");
            assert!(packets.iter().all(|p| p.len() == PACKET_SIZE));
            let mut asm = Reassembler::new(0x0102_0304);
            let mut out = None;
            for p in &packets {
                assert!(out.is_none());
                out = asm.push(p).unwrap();
            }
            assert_eq!(out, Some((CMD_CBOR, data)));
        }
    }

    #[test]
    fn other_channels_and_stray_continuations_are_ignored() {
        let mut asm = Reassembler::new(1);
        let mut other = frame(2, CMD_CBOR, &[1, 2, 3]);
        assert_eq!(asm.push(&other.remove(0)).unwrap(), None);
        let mut cont = vec![0, 0, 0, 1, 0x00];
        cont.resize(PACKET_SIZE, 9);
        assert_eq!(asm.push(&cont).unwrap(), None);
        let mut ours = frame(1, CMD_CBOR, &[7; 100]);
        assert_eq!(asm.push(&ours.remove(0)).unwrap(), None);
        // Wrong sequence number.
        ours[0][4] = 5;
        assert!(asm.push(&ours[0]).is_err());
    }
}
