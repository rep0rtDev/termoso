//! Multiplayer (live terminal sharing) secrets and frame encryption.
//!
//! The invitation link carries a random 32-byte *secret*. Both ends derive:
//!
//! * a **join token** the viewer shows the relay (the server stores its hash),
//! * a **stream key** that encrypts every terminal frame end-to-end.
//!
//! Frames are XChaCha20-Poly1305 envelopes ([`crate::aead`]) whose AAD binds
//! the session id and the direction, so a relayed viewer frame can never be
//! replayed as host output and frames cannot be moved between sessions.
//!
//! Plaintext layout: one tag byte ([`FrameKind`]) followed by the payload.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::CryptoError;
use crate::aead::{self, Aad};
use crate::kdf::{self, Label};
use crate::keys::SymmetricKey;

/// Secret length in bytes.
pub const SECRET_LEN: usize = 32;

/// Which way an encrypted frame travels; part of the AAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Host → viewers.
    FromHost,
    /// Viewer → host.
    FromViewer,
}

impl Direction {
    fn label(self) -> &'static str {
        match self {
            Direction::FromHost => "host",
            Direction::FromViewer => "viewer",
        }
    }
}

/// What a decrypted frame carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// Terminal output bytes (host → viewers).
    Output,
    /// Keyboard input bytes (viewer → host).
    Input,
    /// JSON metadata: title, size (host → viewers).
    Meta,
}

impl FrameKind {
    fn tag(self) -> u8 {
        match self {
            FrameKind::Output => 0,
            FrameKind::Input => 1,
            FrameKind::Meta => 2,
        }
    }

    fn from_tag(t: u8) -> Option<Self> {
        Some(match t {
            0 => FrameKind::Output,
            1 => FrameKind::Input,
            2 => FrameKind::Meta,
            _ => return None,
        })
    }
}

/// The link secret and everything derived from it.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct LiveSecret([u8; SECRET_LEN]);

impl std::fmt::Debug for LiveSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LiveSecret(…)")
    }
}

impl LiveSecret {
    /// Fresh random secret.
    pub fn generate() -> Self {
        let mut b = [0u8; SECRET_LEN];
        crate::random_bytes(&mut b);
        Self(b)
    }

    /// Parse the URL-safe base64 form found in a link.
    pub fn from_b64(s: &str) -> Result<Self, CryptoError> {
        let v = URL_SAFE_NO_PAD
            .decode(s.trim())
            .map_err(|_| CryptoError::Base64)?;
        let b: [u8; SECRET_LEN] = v.try_into().map_err(|_| CryptoError::Envelope)?;
        Ok(Self(b))
    }

    /// URL-safe base64 for the link.
    pub fn to_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    /// Token the viewer presents to the relay (URL-safe base64).
    pub fn join_token(&self) -> Result<String, CryptoError> {
        Ok(URL_SAFE_NO_PAD.encode(kdf::derive_key(&self.0, Label::LiveJoin)?.as_bytes()))
    }

    /// Key encrypting terminal frames; never leaves the clients.
    pub fn stream_key(&self, session_id: &str) -> Result<SymmetricKey, CryptoError> {
        kdf::derive_key_with_context(&self.0, Label::LiveStream, session_id.as_bytes())
    }
}

fn aad(session_id: &str, dir: Direction) -> Aad {
    Aad::label(&["live", session_id, dir.label()])
}

/// Encrypt one frame.
pub fn seal_frame(
    key: &SymmetricKey,
    session_id: &str,
    dir: Direction,
    kind: FrameKind,
    payload: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let mut pt = Vec::with_capacity(1 + payload.len());
    pt.push(kind.tag());
    pt.extend_from_slice(payload);
    aead::encrypt(key, &aad(session_id, dir), &pt)
}

/// Decrypt one frame produced by [`seal_frame`] for the same direction.
pub fn open_frame(
    key: &SymmetricKey,
    session_id: &str,
    dir: Direction,
    envelope: &[u8],
) -> Result<(FrameKind, Vec<u8>), CryptoError> {
    let mut pt = aead::decrypt(key, &aad(session_id, dir), envelope)?;
    if pt.is_empty() {
        return Err(CryptoError::Envelope);
    }
    let kind = FrameKind::from_tag(pt[0]).ok_or(CryptoError::Envelope)?;
    pt.remove(0);
    Ok((kind, pt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_direction_binding() {
        let s = LiveSecret::generate();
        let s2 = LiveSecret::from_b64(&s.to_b64()).unwrap();
        assert_eq!(s.join_token().unwrap(), s2.join_token().unwrap());
        let k = s.stream_key("sid").unwrap();
        let env = seal_frame(&k, "sid", Direction::FromHost, FrameKind::Output, b"ls\r\n").unwrap();
        let (kind, pt) = open_frame(&k, "sid", Direction::FromHost, &env).unwrap();
        assert_eq!(kind, FrameKind::Output);
        assert_eq!(pt, b"ls\r\n");
        assert!(open_frame(&k, "sid", Direction::FromViewer, &env).is_err());
        assert!(open_frame(&k, "other", Direction::FromHost, &env).is_err());
        let other = s.stream_key("other").unwrap();
        assert_ne!(k.as_bytes(), other.as_bytes());
        assert_ne!(k.as_bytes(), s.join_token().unwrap().as_bytes());
    }
}
