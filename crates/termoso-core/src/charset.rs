//! Charset conversion for byte-oriented consoles (serial, and the mobile
//! USB-UART bridge): device output → UTF-8 for the terminal, UTF-8 keyboard
//! input → the line's charset.

use encoding_rs::{Decoder, Encoder, EncoderResult, Encoding, UTF_8};

use crate::error::{CoreError, Result};

/// Resolve a charset label. Empty means UTF-8; UTF-16 is refused because a
/// byte-oriented console cannot carry it.
pub fn charset(label: &str) -> Result<&'static Encoding> {
    let label = label.trim();
    if label.is_empty() {
        return Ok(UTF_8);
    }
    match Encoding::for_label(label.as_bytes()) {
        Some(e) if e.is_ascii_compatible() => Ok(e),
        Some(e) => Err(CoreError::Invalid(format!(
            "charset {} cannot be used on a serial line",
            e.name()
        ))),
        None => Err(CoreError::Invalid(format!("unknown charset {label:?}"))),
    }
}

/// Streaming charset → UTF-8 conversion for device output. UTF-8 passes
/// through untouched so the terminal keeps its own decoder state.
pub struct Transcoder {
    decoder: Option<Decoder>,
}

impl Transcoder {
    /// Decoder for `enc`; UTF-8 needs none.
    pub fn new(enc: &'static Encoding) -> Self {
        Self {
            decoder: (enc != UTF_8).then(|| enc.new_decoder()),
        }
    }

    /// Convert one chunk of device output to UTF-8 (stateful across calls).
    pub fn decode(&mut self, input: &[u8]) -> bytes::Bytes {
        match &mut self.decoder {
            None => bytes::Bytes::copy_from_slice(input),
            Some(d) => {
                let mut out = String::with_capacity(
                    d.max_utf8_buffer_length(input.len())
                        .unwrap_or(input.len() * 3),
                );
                let _ = d.decode_to_string(input, &mut out, false);
                bytes::Bytes::from(out)
            }
        }
    }
}

/// UTF-8 → charset for keyboard input; characters the charset lacks become `?`.
pub fn encode_input(encoder: &mut Option<Encoder>, text: &[u8]) -> Vec<u8> {
    let Some(enc) = encoder else {
        return text.to_vec();
    };
    let s = String::from_utf8_lossy(text);
    let mut out = Vec::with_capacity(
        enc.max_buffer_length_from_utf8_if_no_unmappables(s.len())
            .unwrap_or(s.len() * 2),
    );
    let mut rest: &str = &s;
    loop {
        let (res, read) = enc.encode_from_utf8_to_vec_without_replacement(rest, &mut out, false);
        rest = &rest[read..];
        match res {
            EncoderResult::InputEmpty => break,
            EncoderResult::OutputFull => out.reserve(rest.len() * 4 + 16),
            EncoderResult::Unmappable(_) => out.push(b'?'),
        }
    }
    out
}
