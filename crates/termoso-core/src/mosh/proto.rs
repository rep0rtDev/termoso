//! The three protobuf messages Mosh puts on the wire, hand-encoded: the
//! schemas are tiny (varints and byte strings only) and stable since
//! protocol version 2, so a generator would cost more than it saves.
//!
//! Wire refresher: every field is `(number << 3 | wire_type)` as a varint,
//! then a varint (type 0) or a length-prefixed blob (type 2). Unknown fields
//! are skipped, like a real parser would.

use super::MoshError;

const VARINT: u8 = 0;
const FIXED64: u8 = 1;
const LEN: u8 = 2;
const FIXED32: u8 = 5;

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn put_uint(out: &mut Vec<u8>, field: u32, v: u64) {
    put_varint(out, u64::from(field << 3 | u32::from(VARINT)));
    put_varint(out, v);
}

fn put_bytes(out: &mut Vec<u8>, field: u32, v: &[u8]) {
    put_varint(out, u64::from(field << 3 | u32::from(LEN)));
    put_varint(out, v.len() as u64);
    out.extend_from_slice(v);
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn done(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn varint(&mut self) -> Result<u64, MoshError> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *self.buf.get(self.pos).ok_or(MoshError::Protobuf)?;
            self.pos += 1;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(MoshError::Protobuf)
    }

    fn bytes(&mut self) -> Result<&'a [u8], MoshError> {
        let len = usize::try_from(self.varint()?).map_err(|_| MoshError::Protobuf)?;
        let end = self.pos.checked_add(len).ok_or(MoshError::Protobuf)?;
        let s = self.buf.get(self.pos..end).ok_or(MoshError::Protobuf)?;
        self.pos = end;
        Ok(s)
    }

    /// Next `(field, wire_type)` pair.
    fn tag(&mut self) -> Result<(u32, u8), MoshError> {
        let t = self.varint()?;
        let field = u32::try_from(t >> 3).map_err(|_| MoshError::Protobuf)?;
        Ok((field, (t & 7) as u8))
    }

    fn skip(&mut self, wire: u8) -> Result<(), MoshError> {
        match wire {
            VARINT => self.varint().map(drop),
            LEN => self.bytes().map(drop),
            FIXED64 => self.advance(8),
            FIXED32 => self.advance(4),
            _ => Err(MoshError::Protobuf),
        }
    }

    fn advance(&mut self, n: usize) -> Result<(), MoshError> {
        let end = self.pos.checked_add(n).ok_or(MoshError::Protobuf)?;
        if end > self.buf.len() {
            return Err(MoshError::Protobuf);
        }
        self.pos = end;
        Ok(())
    }
}

/// `TransportBuffers.Instruction`: one state-sync step.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Instruction {
    pub protocol_version: u32,
    pub old_num: u64,
    pub new_num: u64,
    pub ack_num: u64,
    pub throwaway_num: u64,
    pub diff: Vec<u8>,
    pub chaff: Vec<u8>,
}

impl Instruction {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(48 + self.diff.len() + self.chaff.len());
        put_uint(&mut out, 1, u64::from(self.protocol_version));
        put_uint(&mut out, 2, self.old_num);
        put_uint(&mut out, 3, self.new_num);
        put_uint(&mut out, 4, self.ack_num);
        put_uint(&mut out, 5, self.throwaway_num);
        put_bytes(&mut out, 6, &self.diff);
        put_bytes(&mut out, 7, &self.chaff);
        out
    }

    pub fn decode(buf: &[u8]) -> Result<Self, MoshError> {
        let mut r = Reader::new(buf);
        let mut inst = Self::default();
        while !r.done() {
            let (field, wire) = r.tag()?;
            match (field, wire) {
                (1, VARINT) => {
                    inst.protocol_version =
                        u32::try_from(r.varint()?).map_err(|_| MoshError::Protobuf)?
                }
                (2, VARINT) => inst.old_num = r.varint()?,
                (3, VARINT) => inst.new_num = r.varint()?,
                (4, VARINT) => inst.ack_num = r.varint()?,
                (5, VARINT) => inst.throwaway_num = r.varint()?,
                (6, LEN) => inst.diff = r.bytes()?.to_vec(),
                (7, LEN) => inst.chaff = r.bytes()?.to_vec(),
                (_, wire) => r.skip(wire)?,
            }
        }
        Ok(inst)
    }
}

/// What the client tells the server (`ClientBuffers.UserMessage`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserInstruction {
    Keystroke(Vec<u8>),
    Resize { width: i32, height: i32 },
}

/// Encode a `UserMessage`. Consecutive keystrokes are merged into one
/// instruction, as the reference client does.
pub fn encode_user_message(instructions: &[UserInstruction]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pending: Vec<u8> = Vec::new();
    let flush = |out: &mut Vec<u8>, pending: &mut Vec<u8>| {
        if pending.is_empty() {
            return;
        }
        let mut keystroke = Vec::new();
        put_bytes(&mut keystroke, 4, pending);
        let mut inst = Vec::new();
        put_bytes(&mut inst, 2, &keystroke);
        put_bytes(out, 1, &inst);
        pending.clear();
    };
    for i in instructions {
        match i {
            UserInstruction::Keystroke(k) => pending.extend_from_slice(k),
            UserInstruction::Resize { width, height } => {
                flush(&mut out, &mut pending);
                let mut resize = Vec::new();
                put_uint(&mut resize, 5, i64::from(*width) as u64);
                put_uint(&mut resize, 6, i64::from(*height) as u64);
                let mut inst = Vec::new();
                put_bytes(&mut inst, 3, &resize);
                put_bytes(&mut out, 1, &inst);
            }
        }
    }
    flush(&mut out, &mut pending);
    out
}

/// What the server tells the client (`HostBuffers.HostMessage`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostInstruction {
    /// Terminal bytes to feed the local emulator.
    HostBytes(Vec<u8>),
    /// The server's framebuffer now has this size.
    Resize { width: i32, height: i32 },
    /// Server echo acknowledgement (used for predictive echo; informational).
    EchoAck(u64),
}

fn decode_i32(v: u64) -> i32 {
    v as i64 as i32
}

pub fn decode_host_message(buf: &[u8]) -> Result<Vec<HostInstruction>, MoshError> {
    let mut out = Vec::new();
    let mut r = Reader::new(buf);
    while !r.done() {
        let (field, wire) = r.tag()?;
        if field != 1 || wire != LEN {
            r.skip(wire)?;
            continue;
        }
        let mut inst = Reader::new(r.bytes()?);
        while !inst.done() {
            let (field, wire) = inst.tag()?;
            match (field, wire) {
                (2, LEN) => {
                    let mut hb = Reader::new(inst.bytes()?);
                    while !hb.done() {
                        let (f, w) = hb.tag()?;
                        if (f, w) == (4, LEN) {
                            out.push(HostInstruction::HostBytes(hb.bytes()?.to_vec()));
                        } else {
                            hb.skip(w)?;
                        }
                    }
                }
                (3, LEN) => {
                    let mut rs = Reader::new(inst.bytes()?);
                    let (mut width, mut height) = (0, 0);
                    while !rs.done() {
                        let (f, w) = rs.tag()?;
                        match (f, w) {
                            (5, VARINT) => width = decode_i32(rs.varint()?),
                            (6, VARINT) => height = decode_i32(rs.varint()?),
                            _ => rs.skip(w)?,
                        }
                    }
                    out.push(HostInstruction::Resize { width, height });
                }
                (7, LEN) => {
                    let mut ea = Reader::new(inst.bytes()?);
                    while !ea.done() {
                        let (f, w) = ea.tag()?;
                        if (f, w) == (8, VARINT) {
                            out.push(HostInstruction::EchoAck(ea.varint()?));
                        } else {
                            ea.skip(w)?;
                        }
                    }
                }
                (_, wire) => inst.skip(wire)?,
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_round_trips_including_shutdown_sentinel() {
        let inst = Instruction {
            protocol_version: 2,
            old_num: 5,
            new_num: u64::MAX,
            ack_num: 300,
            throwaway_num: 4,
            diff: b"diff".to_vec(),
            chaff: vec![0, 1, 2],
        };
        assert_eq!(Instruction::decode(&inst.encode()).unwrap(), inst);
    }

    #[test]
    fn instruction_matches_protoc_encoding() {
        // protoc --encode=TransportBuffers.Instruction transportinstruction.proto
        // <<< 'protocol_version: 2 old_num: 0 new_num: 1 ack_num: 0
        //      throwaway_num: 0 diff: "" chaff: "ab"'
        let inst = Instruction {
            protocol_version: 2,
            new_num: 1,
            chaff: b"ab".to_vec(),
            ..Default::default()
        };
        assert_eq!(
            inst.encode(),
            [
                0x08, 2, 0x10, 0, 0x18, 1, 0x20, 0, 0x28, 0, 0x32, 0, 0x3a, 2, b'a', b'b'
            ]
        );
    }

    #[test]
    fn user_message_merges_keystrokes_and_encodes_resize() {
        let msg = encode_user_message(&[
            UserInstruction::Keystroke(b"a".to_vec()),
            UserInstruction::Keystroke(b"b".to_vec()),
            UserInstruction::Resize {
                width: 80,
                height: 24,
            },
            UserInstruction::Keystroke(b"c".to_vec()),
        ]);
        // instruction{ keystroke{ keys:"ab" } } instruction{ resize{80,24} } instruction{ keystroke{ "c" } }
        let expected: Vec<u8> = vec![
            0x0a, 6, 0x12, 4, 0x22, 2, b'a', b'b', // keystroke "ab"
            0x0a, 6, 0x1a, 4, 0x28, 80, 0x30, 24, // resize
            0x0a, 5, 0x12, 3, 0x22, 1, b'c', // keystroke "c"
        ];
        assert_eq!(msg, expected);
    }

    #[test]
    fn host_message_decodes_all_extensions_and_skips_unknown() {
        let mut buf = Vec::new();
        // instruction{ hostbytes{ hoststring:"hi" } }
        let mut hb = Vec::new();
        put_bytes(&mut hb, 4, b"hi");
        let mut inst = Vec::new();
        put_bytes(&mut inst, 2, &hb);
        put_uint(&mut inst, 9, 7); // unknown extension
        put_bytes(&mut buf, 1, &inst);
        // instruction{ resize{ 100, 40 } }
        let mut rs = Vec::new();
        put_uint(&mut rs, 5, 100);
        put_uint(&mut rs, 6, 40);
        let mut inst = Vec::new();
        put_bytes(&mut inst, 3, &rs);
        put_bytes(&mut buf, 1, &inst);
        // instruction{ echoack{ 12 } }
        let mut ea = Vec::new();
        put_uint(&mut ea, 8, 12);
        let mut inst = Vec::new();
        put_bytes(&mut inst, 7, &ea);
        put_bytes(&mut buf, 1, &inst);
        assert_eq!(
            decode_host_message(&buf).unwrap(),
            vec![
                HostInstruction::HostBytes(b"hi".to_vec()),
                HostInstruction::Resize {
                    width: 100,
                    height: 40
                },
                HostInstruction::EchoAck(12),
            ]
        );
    }

    #[test]
    fn truncated_input_is_an_error_not_a_panic() {
        assert!(Instruction::decode(&[0x32, 5, b'a']).is_err());
        assert!(decode_host_message(&[0x0a]).is_err());
        assert!(Instruction::decode(&[0x08]).is_err());
    }
}
