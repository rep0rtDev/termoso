//! Recovery key: a 24-word BIP39 mnemonic (256-bit entropy).
//!
//! * `recovery KEK`      – wraps the account private key (second copy).
//! * `recovery verifier` – 32 bytes the client sends to the server to prove it
//!   holds the recovery key when resetting the password. The server stores only
//!   `blake3(verifier)`.

use bip39::{Language, Mnemonic};
use zeroize::Zeroizing;

use crate::kdf::{derive_key, Label};
use crate::keys::SymmetricKey;
use crate::CryptoError;

/// Number of words in a recovery phrase.
pub const WORD_COUNT: usize = 24;

/// A parsed recovery phrase.
pub struct RecoveryKey {
    mnemonic: Mnemonic,
}

impl RecoveryKey {
    /// Generate a fresh recovery key.
    pub fn generate() -> Self {
        let mut entropy = Zeroizing::new([0u8; 32]);
        crate::random_bytes(entropy.as_mut());
        let mnemonic = Mnemonic::from_entropy_in(Language::English, entropy.as_ref())
            .expect("32 bytes of entropy is valid");
        Self { mnemonic }
    }

    /// Parse a phrase typed by the user (whitespace/case tolerant).
    pub fn parse(phrase: &str) -> Result<Self, CryptoError> {
        let normalized = phrase
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, &normalized)
            .map_err(|_| CryptoError::Recovery)?;
        if mnemonic.word_count() != WORD_COUNT {
            return Err(CryptoError::Recovery);
        }
        Ok(Self { mnemonic })
    }

    /// The phrase to show the user (once!).
    pub fn phrase(&self) -> String {
        self.mnemonic.words().collect::<Vec<_>>().join(" ")
    }

    /// Words as a list (for grid display).
    pub fn words(&self) -> Vec<&'static str> {
        self.mnemonic.words().collect()
    }

    fn entropy(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.mnemonic.to_entropy())
    }

    /// KEK used to wrap the account private key.
    pub fn kek(&self) -> Result<SymmetricKey, CryptoError> {
        derive_key(&self.entropy(), Label::RecoveryKek)
    }

    /// Verifier presented to the server during recovery.
    pub fn verifier(&self) -> Result<[u8; 32], CryptoError> {
        Ok(*derive_key(&self.entropy(), Label::RecoveryVerifier)?.as_bytes())
    }

    /// Base64 verifier.
    pub fn verifier_b64(&self) -> Result<String, CryptoError> {
        Ok(crate::encoding::b64(&self.verifier()?))
    }
}

/// What the server stores for a verifier: hex-encoded blake3 hash.
pub fn hash_verifier(verifier: &[u8]) -> String {
    blake3::hash(verifier).to_hex().to_string()
}

/// Constant-time check of a presented verifier against the stored hash.
pub fn check_verifier(verifier: &[u8], stored_hash_hex: &str) -> bool {
    use subtle::ConstantTimeEq;
    let computed = blake3::hash(verifier);
    let Ok(stored) = blake3::Hash::from_hex(stored_hash_hex) else {
        return false;
    };
    computed.as_bytes().ct_eq(stored.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_parse_roundtrip() {
        let rk = RecoveryKey::generate();
        let phrase = rk.phrase();
        assert_eq!(phrase.split(' ').count(), 24);
        let parsed = RecoveryKey::parse(&phrase.to_uppercase()).unwrap();
        assert_eq!(
            parsed.kek().unwrap().as_bytes(),
            rk.kek().unwrap().as_bytes()
        );
        assert_eq!(parsed.verifier().unwrap(), rk.verifier().unwrap());
    }

    #[test]
    fn verifier_hash() {
        let rk = RecoveryKey::generate();
        let v = rk.verifier().unwrap();
        let h = hash_verifier(&v);
        assert!(check_verifier(&v, &h));
        assert!(!check_verifier(&[0u8; 32], &h));
        assert!(!check_verifier(&v, "nothex"));
    }

    #[test]
    fn rejects_short_phrase() {
        assert!(RecoveryKey::parse("abandon abandon abandon").is_err());
    }
}
