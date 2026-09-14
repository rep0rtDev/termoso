//! Splitting a zlib-compressed `Instruction` into MTU-sized fragments and
//! putting them back together. Header: 64-bit fragment id, then a 16-bit
//! word whose high bit marks the last fragment and low 15 bits number it.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

use super::MoshError;
use super::proto::Instruction;

pub const HEADER_LEN: usize = 8 + 2;

/// Upper bound on a reassembled instruction (the reference uses 4 MiB).
const MAX_ASSEMBLED: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub id: u64,
    pub num: u16,
    pub final_: bool,
    pub contents: Vec<u8>,
}

impl Fragment {
    pub fn to_wire(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.contents.len());
        out.extend_from_slice(&self.id.to_be_bytes());
        let combined = (u16::from(self.final_) << 15) | (self.num & 0x7fff);
        out.extend_from_slice(&combined.to_be_bytes());
        out.extend_from_slice(&self.contents);
        out
    }

    pub fn from_wire(buf: &[u8]) -> Result<Self, MoshError> {
        if buf.len() < HEADER_LEN {
            return Err(MoshError::Fragment);
        }
        let id = u64::from_be_bytes(buf[..8].try_into().expect("8 bytes"));
        let combined = u16::from_be_bytes([buf[8], buf[9]]);
        Ok(Self {
            id,
            num: combined & 0x7fff,
            final_: combined & 0x8000 != 0,
            contents: buf[HEADER_LEN..].to_vec(),
        })
    }
}

/// Outgoing side: compress, then cut into `mtu`-sized pieces.
pub struct Fragmenter {
    next_id: u64,
}

impl Fragmenter {
    pub fn new() -> Self {
        Self { next_id: 1 }
    }

    pub fn make(&mut self, inst: &Instruction, mtu: usize) -> Result<Vec<Fragment>, MoshError> {
        let payload = compress(&inst.encode())?;
        let id = self.next_id;
        self.next_id += 1;
        let chunk = mtu.saturating_sub(HEADER_LEN).max(1);
        let pieces: Vec<&[u8]> = if payload.is_empty() {
            vec![&[][..]]
        } else {
            payload.chunks(chunk).collect()
        };
        if pieces.len() > 0x8000 {
            return Err(MoshError::Fragment);
        }
        Ok(pieces
            .iter()
            .enumerate()
            .map(|(i, p)| Fragment {
                id,
                num: i as u16,
                final_: i + 1 == pieces.len(),
                contents: p.to_vec(),
            })
            .collect())
    }
}

/// Incoming side: collects fragments of the current id; a fragment with a
/// new id discards whatever was pending.
pub struct Assembly {
    current_id: Option<u64>,
    pieces: Vec<Option<Vec<u8>>>,
    arrived: usize,
    total: Option<usize>,
}

impl Assembly {
    pub fn new() -> Self {
        Self {
            current_id: None,
            pieces: Vec::new(),
            arrived: 0,
            total: None,
        }
    }

    /// Feed one fragment; returns the decoded instruction once complete.
    pub fn add(&mut self, frag: Fragment) -> Result<Option<Instruction>, MoshError> {
        let idx = usize::from(frag.num);
        if self.current_id != Some(frag.id) {
            self.current_id = Some(frag.id);
            self.pieces.clear();
            self.arrived = 0;
            self.total = None;
        }
        if let Some(total) = self.total
            && idx >= total
        {
            return Err(MoshError::Fragment);
        }
        if self.pieces.len() <= idx {
            self.pieces.resize(idx + 1, None);
        }
        match &self.pieces[idx] {
            Some(existing) if *existing != frag.contents => return Err(MoshError::Fragment),
            Some(_) => {}
            None => {
                self.pieces[idx] = Some(frag.contents);
                self.arrived += 1;
            }
        }
        if frag.final_ {
            if self.pieces.len() > idx + 1 {
                return Err(MoshError::Fragment);
            }
            self.total = Some(idx + 1);
        }
        if self.total != Some(self.arrived) {
            return Ok(None);
        }
        let mut encoded = Vec::new();
        for p in self.pieces.drain(..) {
            encoded.extend_from_slice(&p.ok_or(MoshError::Fragment)?);
        }
        self.arrived = 0;
        self.total = None;
        self.current_id = None;
        Instruction::decode(&decompress(&encoded)?).map(Some)
    }
}

fn compress(data: &[u8]) -> Result<Vec<u8>, MoshError> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data).map_err(|_| MoshError::Compression)?;
    enc.finish().map_err(|_| MoshError::Compression)
}

fn decompress(data: &[u8]) -> Result<Vec<u8>, MoshError> {
    let mut out = Vec::new();
    ZlibDecoder::new(data)
        .take(MAX_ASSEMBLED + 1)
        .read_to_end(&mut out)
        .map_err(|_| MoshError::Compression)?;
    if out.len() as u64 > MAX_ASSEMBLED {
        return Err(MoshError::Compression);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(diff_len: usize) -> Instruction {
        Instruction {
            protocol_version: 2,
            old_num: 1,
            new_num: 2,
            ack_num: 3,
            throwaway_num: 1,
            diff: (0..diff_len).map(|i| (i % 251) as u8).collect(),
            chaff: vec![9; 5],
        }
    }

    #[test]
    fn header_layout_matches_reference() {
        let f = Fragment {
            id: 0x0102030405060708,
            num: 3,
            final_: true,
            contents: b"xyz".to_vec(),
        };
        let wire = f.to_wire();
        assert_eq!(&wire[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&wire[8..10], &[0x80, 0x03]);
        assert_eq!(Fragment::from_wire(&wire).unwrap(), f);
        assert!(Fragment::from_wire(&wire[..9]).is_err());
    }

    #[test]
    fn small_instruction_is_one_fragment() {
        let mut fr = Fragmenter::new();
        let frags = fr.make(&inst(10), 500).unwrap();
        assert_eq!(frags.len(), 1);
        assert!(frags[0].final_);
        let mut asm = Assembly::new();
        assert_eq!(asm.add(frags[0].clone()).unwrap(), Some(inst(10)));
    }

    #[test]
    fn large_instruction_reassembles_out_of_order_and_ignores_duplicates() {
        let mut fr = Fragmenter::new();
        let frags = fr.make(&inst(20_000), 100).unwrap();
        assert!(frags.len() > 3);
        let mut asm = Assembly::new();
        let mut order: Vec<Fragment> = frags.iter().rev().cloned().collect();
        order.insert(1, frags[0].clone()); // duplicate
        let mut result = None;
        for f in order {
            if let Some(i) = asm.add(f).unwrap() {
                result = Some(i);
            }
        }
        assert_eq!(result, Some(inst(20_000)));
    }

    #[test]
    fn new_id_drops_a_partial_assembly() {
        let mut fr = Fragmenter::new();
        let a = fr.make(&inst(5_000), 100).unwrap();
        let b = fr.make(&inst(3), 100).unwrap();
        let mut asm = Assembly::new();
        assert!(asm.add(a[0].clone()).unwrap().is_none());
        assert_eq!(asm.add(b[0].clone()).unwrap(), Some(inst(3)));
    }

    #[test]
    fn conflicting_duplicate_is_rejected() {
        let mut fr = Fragmenter::new();
        let frags = fr.make(&inst(5_000), 100).unwrap();
        let mut asm = Assembly::new();
        asm.add(frags[0].clone()).unwrap();
        let mut bad = frags[0].clone();
        bad.contents[0] ^= 0xff;
        assert!(asm.add(bad).is_err());
    }
}
