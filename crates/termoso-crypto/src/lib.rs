//! Termoso zero-knowledge cryptography.
//!
//! Everything a user stores in Termoso is encrypted on the device before it is
//! sent to the server. The server only ever sees:
//!
//! * the OPAQUE registration record (from which the password cannot be recovered),
//! * X25519 *public* keys,
//! * ciphertext envelopes produced by [`aead`].
//!
//! # Key hierarchy
//!
//! ```text
//! master password ──OPAQUE──▶ export_key (64 B, never leaves the device)
//!                                 │ HKDF "termoso/v1/account-kek"
//!                                 ▼
//!                            account KEK (32 B)
//!                                 │ wraps (XChaCha20-Poly1305)
//!                                 ▼
//!                    account X25519 private key ◀── also wrapped by recovery KEK
//!                                 │ opens sealed boxes
//!                                 ▼
//!                 vault keys (32 B, one per personal / team vault)
//!                                 │ encrypts (XChaCha20-Poly1305 + AAD)
//!                                 ▼
//!                   entity fields (hosts, identities, keys, snippets…)
//! ```
//!
//! The recovery key is a 24-word BIP39 mnemonic (256 bits of entropy). It
//! derives a second KEK that wraps the same account private key, and a
//! *verifier* the server stores hashed to authorise password resets.
//!
//! All formats are versioned so they can evolve without breaking old data.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod aead;
pub mod encoding;
pub mod error;
pub mod kdf;
pub mod keys;
pub mod live;
pub mod opaque;
pub mod recovery;
pub mod sealed;

pub use error::CryptoError;

/// Protocol/format version prefix used in AAD labels and envelopes.
pub const PROTOCOL_VERSION: &str = "termoso/v1";

/// Cryptographically secure RNG used throughout the crate.
pub type Rng = rand_core::OsRng;

/// Fill `buf` with random bytes from the OS RNG.
pub fn random_bytes(buf: &mut [u8]) {
    use rand_core::RngCore;
    rand_core::OsRng.fill_bytes(buf);
}
