//! XChaCha20-Poly1305 authenticated encryption with versioned envelopes.
//!
//! Envelope layout (binary, base64 for transport):
//!
//! ```text
//! ┌─────────┬──────────────┬──────────────────────────┐
//! │ version │ nonce (24 B) │ ciphertext ‖ tag (16 B)  │
//! │  0x01   │              │                          │
//! └─────────┴──────────────┴──────────────────────────┘
//! ```
//!
//! Every encryption binds *associated data* describing where the ciphertext
//! lives (entity kind, entity id, field name). This prevents a malicious server
//! from swapping ciphertexts between fields or records without detection.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

use crate::encoding::{b64, unb64};
use crate::keys::SymmetricKey;
use crate::{CryptoError, PROTOCOL_VERSION};

/// Current envelope version.
pub const ENVELOPE_V1: u8 = 0x01;
/// XChaCha20 nonce length.
pub const NONCE_LEN: usize = 24;
/// Poly1305 tag length.
pub const TAG_LEN: usize = 16;

/// Associated data label. Build with the helper constructors so the format
/// stays consistent across clients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aad(Vec<u8>);

impl Aad {
    /// Arbitrary label under the protocol namespace.
    pub fn label(parts: &[&str]) -> Self {
        let mut s = String::from(PROTOCOL_VERSION);
        for p in parts {
            s.push('/');
            s.push_str(p);
        }
        Self(s.into_bytes())
    }

    /// AAD for a field of a synced entity: `termoso/v1/entity/<kind>/<id>/<field>`.
    pub fn entity_field(kind: &str, entity_id: &str, field: &str) -> Self {
        Self::label(&["entity", kind, entity_id, field])
    }

    /// AAD for a whole encrypted entity payload: `termoso/v1/entity/<kind>/<id>`.
    pub fn entity(kind: &str, entity_id: &str) -> Self {
        Self::label(&["entity", kind, entity_id])
    }

    /// AAD for the account private key wrapped by the account KEK.
    pub fn account_private_key() -> Self {
        Self::label(&["account-private-key"])
    }

    /// AAD for the account private key wrapped by the recovery KEK.
    pub fn recovery_private_key() -> Self {
        Self::label(&["recovery-private-key"])
    }

    /// AAD for server-side secrets at rest (`termoso/v1/server/<purpose>`).
    pub fn server(purpose: &str) -> Self {
        Self::label(&["server", purpose])
    }

    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Encrypt `plaintext` under `key`, binding `aad`. Returns a binary envelope.
pub fn encrypt(key: &SymmetricKey, aad: &Aad, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let mut nonce = [0u8; NONCE_LEN];
    crate::random_bytes(&mut nonce);
    let ct = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| CryptoError::Encrypt)?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
    out.push(ENVELOPE_V1);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Decrypt a binary envelope produced by [`encrypt`].
pub fn decrypt(key: &SymmetricKey, aad: &Aad, envelope: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if envelope.len() < 1 + NONCE_LEN + TAG_LEN {
        return Err(CryptoError::Envelope);
    }
    let (version, rest) = envelope.split_first().ok_or(CryptoError::Envelope)?;
    if *version != ENVELOPE_V1 {
        return Err(CryptoError::Envelope);
    }
    let (nonce, ct) = rest.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ct,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| CryptoError::Decrypt)
}

/// Encrypt and return a base64 envelope (for JSON fields).
pub fn encrypt_b64(key: &SymmetricKey, aad: &Aad, plaintext: &[u8]) -> Result<String, CryptoError> {
    encrypt(key, aad, plaintext).map(|v| b64(&v))
}

/// Decrypt a base64 envelope.
pub fn decrypt_b64(key: &SymmetricKey, aad: &Aad, envelope: &str) -> Result<Vec<u8>, CryptoError> {
    decrypt(key, aad, &unb64(envelope)?)
}

/// Encrypt a UTF-8 string field.
pub fn encrypt_str(key: &SymmetricKey, aad: &Aad, value: &str) -> Result<String, CryptoError> {
    encrypt_b64(key, aad, value.as_bytes())
}

/// Decrypt to a UTF-8 string field.
pub fn decrypt_str(key: &SymmetricKey, aad: &Aad, envelope: &str) -> Result<String, CryptoError> {
    let bytes = decrypt_b64(key, aad, envelope)?;
    String::from_utf8(bytes).map_err(|_| CryptoError::Decrypt)
}

/// Wrap a symmetric key with another key (key-encryption-key).
pub fn wrap_key(kek: &SymmetricKey, aad: &Aad, key: &SymmetricKey) -> Result<String, CryptoError> {
    encrypt_b64(kek, aad, key.as_bytes())
}

/// Unwrap a symmetric key.
pub fn unwrap_key(
    kek: &SymmetricKey,
    aad: &Aad,
    wrapped: &str,
) -> Result<SymmetricKey, CryptoError> {
    let bytes = decrypt_b64(kek, aad, wrapped)?;
    SymmetricKey::from_slice(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let k = SymmetricKey::generate();
        let aad = Aad::entity_field("host", "abc", "address");
        let ct = encrypt_str(&k, &aad, "10.0.0.1").unwrap();
        assert_eq!(decrypt_str(&k, &aad, &ct).unwrap(), "10.0.0.1");
    }

    #[test]
    fn aad_mismatch_fails() {
        let k = SymmetricKey::generate();
        let ct = encrypt_str(&k, &Aad::entity_field("host", "1", "address"), "x").unwrap();
        assert!(decrypt_str(&k, &Aad::entity_field("host", "2", "address"), &ct).is_err());
        assert!(decrypt_str(&k, &Aad::entity_field("host", "1", "label"), &ct).is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let k1 = SymmetricKey::generate();
        let k2 = SymmetricKey::generate();
        let aad = Aad::label(&["t"]);
        let ct = encrypt(&k1, &aad, b"secret").unwrap();
        assert!(decrypt(&k2, &aad, &ct).is_err());
    }

    #[test]
    fn tamper_fails() {
        let k = SymmetricKey::generate();
        let aad = Aad::label(&["t"]);
        let mut ct = encrypt(&k, &aad, b"secret").unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 1;
        assert!(decrypt(&k, &aad, &ct).is_err());
        ct[0] = 0x02;
        assert!(matches!(decrypt(&k, &aad, &ct), Err(CryptoError::Envelope)));
    }

    #[test]
    fn key_wrapping() {
        let kek = SymmetricKey::generate();
        let key = SymmetricKey::generate();
        let aad = Aad::account_private_key();
        let w = wrap_key(&kek, &aad, &key).unwrap();
        assert_eq!(
            unwrap_key(&kek, &aad, &w).unwrap().as_bytes(),
            key.as_bytes()
        );
    }
}
