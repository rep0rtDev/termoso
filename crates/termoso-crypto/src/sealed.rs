//! libsodium-compatible sealed boxes (X25519 + XSalsa20-Poly1305) used to hand
//! a vault key to a member: the vault key is sealed to the member's account
//! public key and can only be opened with the member's private key.

use crypto_box::aead::OsRng;
use crypto_box::{PublicKey, SalsaBox};

use crate::encoding::{b64, unb64};
use crate::keys::{KeyPair, SymmetricKey};
use crate::CryptoError;

/// Seal `plaintext` to `recipient`. Returns base64.
pub fn seal_b64(recipient: &PublicKey, plaintext: &[u8]) -> Result<String, CryptoError> {
    let ct = recipient
        .seal(&mut OsRng, plaintext)
        .map_err(|_| CryptoError::Encrypt)?;
    Ok(b64(&ct))
}

/// Open a base64 sealed box with our key pair.
pub fn open_b64(recipient: &KeyPair, sealed: &str) -> Result<Vec<u8>, CryptoError> {
    let ct = unb64(sealed)?;
    recipient
        .secret()
        .unseal(&ct)
        .map_err(|_| CryptoError::Decrypt)
}

/// Seal a vault key to a member.
pub fn seal_vault_key(
    recipient: &PublicKey,
    vault_key: &SymmetricKey,
) -> Result<String, CryptoError> {
    seal_b64(recipient, vault_key.as_bytes())
}

/// Open a sealed vault key.
pub fn open_vault_key(recipient: &KeyPair, sealed: &str) -> Result<SymmetricKey, CryptoError> {
    SymmetricKey::from_slice(&open_b64(recipient, sealed)?)
}

/// Authenticated (non-anonymous) box between two parties — used later for
/// multiplayer / device-to-device messages. Returns `nonce ‖ ciphertext` base64.
pub fn box_b64(
    sender: &KeyPair,
    recipient: &PublicKey,
    plaintext: &[u8],
) -> Result<String, CryptoError> {
    use crypto_box::aead::{Aead, AeadCore};
    let sbox = SalsaBox::new(recipient, sender.secret());
    let nonce = SalsaBox::generate_nonce(&mut OsRng);
    let ct = sbox
        .encrypt(&nonce, plaintext)
        .map_err(|_| CryptoError::Encrypt)?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Ok(b64(&out))
}

/// Open a message produced by [`box_b64`].
pub fn unbox_b64(
    recipient: &KeyPair,
    sender: &PublicKey,
    data: &str,
) -> Result<Vec<u8>, CryptoError> {
    use crypto_box::aead::Aead;
    let bytes = unb64(data)?;
    if bytes.len() < 24 {
        return Err(CryptoError::Envelope);
    }
    let (nonce, ct) = bytes.split_at(24);
    let sbox = SalsaBox::new(sender, recipient.secret());
    sbox.decrypt(nonce.into(), ct)
        .map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_roundtrip() {
        let member = KeyPair::generate();
        let vk = SymmetricKey::generate();
        let pk = crate::keys::public_key_from_b64(&member.public_b64()).unwrap();
        let sealed = seal_vault_key(&pk, &vk).unwrap();
        let opened = open_vault_key(&member, &sealed).unwrap();
        assert_eq!(opened.as_bytes(), vk.as_bytes());

        let other = KeyPair::generate();
        assert!(open_vault_key(&other, &sealed).is_err());
    }

    #[test]
    fn box_roundtrip() {
        let a = KeyPair::generate();
        let b = KeyPair::generate();
        let b_pk = crate::keys::public_key_from_b64(&b.public_b64()).unwrap();
        let a_pk = crate::keys::public_key_from_b64(&a.public_b64()).unwrap();
        let msg = box_b64(&a, &b_pk, b"hello").unwrap();
        assert_eq!(unbox_b64(&b, &a_pk, &msg).unwrap(), b"hello");
    }
}
