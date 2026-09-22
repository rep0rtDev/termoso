//! libsodium-compatible sealed boxes (X25519 + XSalsa20-Poly1305) used to hand
//! a vault key to a member: the vault key is sealed to the member's account
//! public key and can only be opened with the member's private key.

use crypto_box::aead::OsRng;
use crypto_box::{PublicKey, SalsaBox};

use crate::CryptoError;
use crate::encoding::{b64, unb64};
use crate::keys::{KeyPair, SymmetricKey};

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

/// Open a sealed vault key in either format (see [`open_vault_key_checked`]).
pub fn open_vault_key(recipient: &KeyPair, sealed: &str) -> Result<SymmetricKey, CryptoError> {
    open_vault_key_checked(recipient, sealed).map(|(k, _)| k)
}

/// How a vault key was sealed to us.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealedKeyOrigin {
    /// Anonymous sealed box: anyone who knows our public key could have
    /// produced it, including the server.
    Anonymous,
    /// Authenticated box from our own key pair to itself: only a holder of
    /// our account private key could have produced it.
    SelfAuthenticated,
}

const SELF_TAG: u8 = 0x01;
const NONCE_LEN: usize = 24;
const SELF_SEALED_LEN: usize = 1 + NONCE_LEN + 32 + 16;
const ANON_SEALED_LEN: usize = 32 + 32 + 16;

/// Seal a vault key to ourselves so that other devices of the same account
/// can verify it was produced by a holder of the account private key: a
/// `crypto_box` from our key pair to our own public key, encoded as
/// `0x01 ‖ nonce ‖ ciphertext` in base64. Used for the personal vault, whose
/// key must never be replaced by anything the server made up.
pub fn seal_vault_key_self(me: &KeyPair, vault_key: &SymmetricKey) -> Result<String, CryptoError> {
    use crypto_box::aead::{Aead, AeadCore};
    let sbox = SalsaBox::new(me.public(), me.secret());
    let nonce = SalsaBox::generate_nonce(&mut OsRng);
    let ct = sbox
        .encrypt(&nonce, vault_key.as_bytes().as_slice())
        .map_err(|_| CryptoError::Encrypt)?;
    let mut out = Vec::with_capacity(SELF_SEALED_LEN);
    out.push(SELF_TAG);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(b64(&out))
}

/// Open a sealed vault key and report which format it used. The two formats
/// have distinct lengths (80 bytes anonymous, 73 bytes self-authenticated),
/// so a caller can require [`SealedKeyOrigin::SelfAuthenticated`] where the
/// server must not be able to substitute a key.
pub fn open_vault_key_checked(
    recipient: &KeyPair,
    sealed: &str,
) -> Result<(SymmetricKey, SealedKeyOrigin), CryptoError> {
    use crypto_box::aead::Aead;
    let bytes = unb64(sealed)?;
    match bytes.len() {
        ANON_SEALED_LEN => {
            let key = recipient
                .secret()
                .unseal(&bytes)
                .map_err(|_| CryptoError::Decrypt)?;
            Ok((SymmetricKey::from_slice(&key)?, SealedKeyOrigin::Anonymous))
        }
        SELF_SEALED_LEN if bytes[0] == SELF_TAG => {
            let (nonce, ct) = bytes[1..].split_at(NONCE_LEN);
            let sbox = SalsaBox::new(recipient.public(), recipient.secret());
            let key = sbox
                .decrypt(nonce.into(), ct)
                .map_err(|_| CryptoError::Decrypt)?;
            Ok((
                SymmetricKey::from_slice(&key)?,
                SealedKeyOrigin::SelfAuthenticated,
            ))
        }
        _ => Err(CryptoError::Envelope),
    }
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
        assert_eq!(
            open_vault_key_checked(&member, &sealed).unwrap().1,
            SealedKeyOrigin::Anonymous
        );
    }

    #[test]
    fn self_sealed_roundtrip_and_origin() {
        let me = KeyPair::generate();
        let vk = SymmetricKey::generate();
        let sealed = seal_vault_key_self(&me, &vk).unwrap();
        assert_eq!(unb64(&sealed).unwrap().len(), SELF_SEALED_LEN);
        let (opened, origin) = open_vault_key_checked(&me, &sealed).unwrap();
        assert_eq!(opened.as_bytes(), vk.as_bytes());
        assert_eq!(origin, SealedKeyOrigin::SelfAuthenticated);
        assert_eq!(
            open_vault_key(&me, &sealed).unwrap().as_bytes(),
            vk.as_bytes()
        );

        // Someone who only knows our public key cannot produce it.
        let other = KeyPair::generate();
        assert!(open_vault_key_checked(&other, &sealed).is_err());
        let forged = {
            use crypto_box::aead::{Aead, AeadCore};
            let sbox = SalsaBox::new(me.public(), other.secret());
            let nonce = SalsaBox::generate_nonce(&mut OsRng);
            let ct = sbox.encrypt(&nonce, vk.as_bytes().as_slice()).unwrap();
            let mut out = vec![SELF_TAG];
            out.extend_from_slice(&nonce);
            out.extend_from_slice(&ct);
            b64(&out)
        };
        assert!(open_vault_key_checked(&me, &forged).is_err());

        // Tampering with the tag or length is rejected as malformed.
        let mut bytes = unb64(&sealed).unwrap();
        bytes[0] = 0x02;
        assert!(matches!(
            open_vault_key_checked(&me, &b64(&bytes)),
            Err(CryptoError::Envelope)
        ));
        bytes.push(0);
        assert!(matches!(
            open_vault_key_checked(&me, &b64(&bytes)),
            Err(CryptoError::Envelope)
        ));
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
