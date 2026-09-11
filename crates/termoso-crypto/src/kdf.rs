//! HKDF-SHA256 based key derivation with domain-separated labels.

use hkdf::Hkdf;
use sha2::Sha256;

use crate::keys::SymmetricKey;
use crate::{CryptoError, PROTOCOL_VERSION};

/// Fixed salt used for all HKDF derivations (domain separation lives in `info`).
const SALT: &[u8] = b"termoso-hkdf-salt-v1";

/// Derivation labels. Adding a label is backwards compatible; never change an
/// existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// Account key-encryption-key derived from the OPAQUE export key.
    AccountKek,
    /// KEK derived from the recovery mnemonic.
    RecoveryKek,
    /// Verifier derived from the recovery mnemonic (sent to the server on reset).
    RecoveryVerifier,
    /// Key used by the client to encrypt its local database.
    LocalDatabase,
    /// Server-side key for encrypting secrets at rest (TOTP seeds, OPAQUE seed).
    ServerAtRest,
}

impl Label {
    fn info(self) -> Vec<u8> {
        let name = match self {
            Label::AccountKek => "account-kek",
            Label::RecoveryKek => "recovery-kek",
            Label::RecoveryVerifier => "recovery-verifier",
            Label::LocalDatabase => "local-database",
            Label::ServerAtRest => "server-at-rest",
        };
        format!("{PROTOCOL_VERSION}/{name}").into_bytes()
    }
}

/// Derive a 32-byte key from `ikm` for the given label.
pub fn derive_key(ikm: &[u8], label: Label) -> Result<SymmetricKey, CryptoError> {
    derive_key_with_context(ikm, label, &[])
}

/// Derive a 32-byte key from `ikm` for the given label and an extra context
/// (e.g. a vault id) appended to the info string.
pub fn derive_key_with_context(
    ikm: &[u8],
    label: Label,
    context: &[u8],
) -> Result<SymmetricKey, CryptoError> {
    let hk = Hkdf::<Sha256>::new(Some(SALT), ikm);
    let mut info = label.info();
    if !context.is_empty() {
        info.push(b'/');
        info.extend_from_slice(context);
    }
    let mut out = [0u8; 32];
    hk.expand(&info, &mut out).map_err(|_| CryptoError::Kdf)?;
    Ok(SymmetricKey::from_bytes(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_domain_separated() {
        let ikm = [7u8; 64];
        let a = derive_key(&ikm, Label::AccountKek).unwrap();
        let b = derive_key(&ikm, Label::RecoveryKek).unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
        let c = derive_key_with_context(&ikm, Label::AccountKek, b"x").unwrap();
        assert_ne!(a.as_bytes(), c.as_bytes());
    }

    #[test]
    fn deterministic() {
        let ikm = b"some input keying material";
        let a = derive_key(ikm, Label::LocalDatabase).unwrap();
        let b = derive_key(ikm, Label::LocalDatabase).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }
}
