//! Base64 helpers (standard alphabet, padded) used for JSON transport.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::CryptoError;

/// Encode bytes as standard base64.
pub fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// Decode standard base64.
pub fn unb64(s: &str) -> Result<Vec<u8>, CryptoError> {
    STANDARD.decode(s.trim()).map_err(|_| CryptoError::Base64)
}

/// Decode base64 into a fixed-size array.
pub fn unb64_array<const N: usize>(s: &str) -> Result<[u8; N], CryptoError> {
    let v = unb64(s)?;
    v.try_into().map_err(|_| CryptoError::Key)
}

/// Serde helpers to (de)serialize `Vec<u8>` fields as base64 strings.
pub mod serde_b64 {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Serialize bytes as base64.
    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::b64(bytes))
    }

    /// Deserialize base64 into bytes.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        super::unb64(&s).map_err(serde::de::Error::custom)
    }
}
