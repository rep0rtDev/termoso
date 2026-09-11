//! Key types: zeroizing symmetric keys and X25519 key pairs.

use crypto_box::aead::OsRng;
use crypto_box::{PublicKey, SecretKey};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::encoding::{b64, unb64_array};
use crate::CryptoError;

/// Length of every symmetric key in Termoso.
pub const KEY_LEN: usize = 32;

/// A 256-bit symmetric key that is zeroed when dropped.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SymmetricKey([u8; KEY_LEN]);

impl SymmetricKey {
    /// Generate a fresh random key.
    pub fn generate() -> Self {
        let mut k = [0u8; KEY_LEN];
        crate::random_bytes(&mut k);
        Self(k)
    }

    /// Wrap raw bytes.
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Parse from a slice (must be exactly 32 bytes).
    pub fn from_slice(bytes: &[u8]) -> Result<Self, CryptoError> {
        let arr: [u8; KEY_LEN] = bytes.try_into().map_err(|_| CryptoError::Key)?;
        Ok(Self(arr))
    }

    /// Parse from base64.
    pub fn from_b64(s: &str) -> Result<Self, CryptoError> {
        Ok(Self(unb64_array::<KEY_LEN>(s)?))
    }

    /// Raw key bytes.
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Base64 representation (only for wrapping / transport inside another envelope).
    pub fn to_b64(&self) -> String {
        b64(&self.0)
    }
}

impl std::fmt::Debug for SymmetricKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SymmetricKey(<redacted>)")
    }
}

/// X25519 key pair used for sealed-box key distribution.
pub struct KeyPair {
    secret: SecretKey,
    public: PublicKey,
}

impl KeyPair {
    /// Generate a new random key pair.
    pub fn generate() -> Self {
        let secret = SecretKey::generate(&mut OsRng);
        let public = secret.public_key();
        Self { secret, public }
    }

    /// Rebuild from a 32-byte private key.
    pub fn from_secret_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let arr: [u8; 32] = bytes.try_into().map_err(|_| CryptoError::Key)?;
        let secret = SecretKey::from(arr);
        let public = secret.public_key();
        Ok(Self { secret, public })
    }

    /// Private key bytes (handle with care; wrap before storing).
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.secret.to_bytes()
    }

    /// Public key.
    pub fn public(&self) -> &PublicKey {
        &self.public
    }

    /// Public key bytes.
    pub fn public_bytes(&self) -> [u8; 32] {
        *self.public.as_bytes()
    }

    /// Public key as base64 (what the server stores).
    pub fn public_b64(&self) -> String {
        b64(self.public.as_bytes())
    }

    pub(crate) fn secret(&self) -> &SecretKey {
        &self.secret
    }
}

impl std::fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeyPair(pub={})", self.public_b64())
    }
}

/// Parse a base64 X25519 public key.
pub fn public_key_from_b64(s: &str) -> Result<PublicKey, CryptoError> {
    let arr = unb64_array::<32>(s)?;
    Ok(PublicKey::from(arr))
}
