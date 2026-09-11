//! SSH key generation, import (OpenSSH / PEM / PKCS#8 / PuTTY), export and
//! passphrase handling. Nothing here touches the store; callers put the
//! resulting text into an `SshKey` entity.

use russh::keys::ssh_key::private::{EcdsaKeypair, Ed25519Keypair, RsaKeypair};
use russh::keys::ssh_key::{Algorithm, EcdsaCurve, LineEnding, PrivateKey, PublicKey};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{CoreError, Result};

/// Key type to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KeyAlgorithm {
    /// Ed25519 (default).
    #[default]
    Ed25519,
    /// RSA with the given modulus size (2048–8192).
    Rsa {
        /// Bits.
        bits: usize,
    },
    /// ECDSA P-256.
    EcdsaP256,
    /// ECDSA P-384.
    EcdsaP384,
    /// ECDSA P-521.
    EcdsaP521,
}

/// Parsed key summary for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    /// Algorithm name (`ssh-ed25519`, `ssh-rsa`, `ecdsa-sha2-nistp256`).
    pub key_type: String,
    /// Modulus size for RSA / curve size for ECDSA / 256 for Ed25519.
    pub bits: usize,
    /// `SHA256:` fingerprint.
    pub fingerprint: String,
    /// `authorized_keys` line.
    pub public_key: String,
    /// Comment.
    pub comment: String,
    /// Whether the private key is passphrase-protected.
    pub encrypted: bool,
}

/// A freshly generated or imported key, in OpenSSH text form.
#[derive(Clone, Serialize, Deserialize)]
pub struct KeyMaterial {
    /// `-----BEGIN OPENSSH PRIVATE KEY-----` block.
    pub private_key: Zeroizing<String>,
    /// `authorized_keys` line.
    pub public_key: String,
    /// Summary.
    pub info: KeyInfo,
}

impl std::fmt::Debug for KeyMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyMaterial")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

fn bits(key: &PublicKey) -> usize {
    match key.algorithm() {
        Algorithm::Rsa { .. } => key
            .key_data()
            .rsa()
            .map(|r| r.n().as_positive_bytes().map_or(0, |b| b.len() * 8))
            .unwrap_or(0),
        Algorithm::Ecdsa { curve } => match curve {
            EcdsaCurve::NistP256 => 256,
            EcdsaCurve::NistP384 => 384,
            EcdsaCurve::NistP521 => 521,
        },
        Algorithm::Ed25519 | Algorithm::SkEd25519 => 256,
        _ => 0,
    }
}

/// Summarise a public key.
pub fn public_info(key: &PublicKey, encrypted: bool) -> KeyInfo {
    KeyInfo {
        key_type: key.algorithm().as_str().to_string(),
        bits: bits(key),
        fingerprint: crate::hostkey::fingerprint(key),
        public_key: key.to_openssh().unwrap_or_default(),
        comment: key.comment().to_string(),
        encrypted,
    }
}

fn material(key: &PrivateKey, passphrase: Option<&str>) -> Result<KeyMaterial> {
    let public = key.public_key().clone();
    let to_write = match passphrase {
        Some(p) if !p.is_empty() => key
            .encrypt(&mut rand::rng(), p.as_bytes())
            .map_err(|e| CoreError::Key(e.to_string()))?,
        _ => key.clone(),
    };
    let private_key = to_write
        .to_openssh(LineEnding::LF)
        .map_err(|e| CoreError::Key(e.to_string()))?;
    let encrypted = to_write.is_encrypted();
    Ok(KeyMaterial {
        private_key,
        public_key: public
            .to_openssh()
            .map_err(|e| CoreError::Key(e.to_string()))?,
        info: public_info(&public, encrypted),
    })
}

/// Generate a new key. `comment` becomes the public-key comment.
pub fn generate(
    algorithm: KeyAlgorithm,
    comment: &str,
    passphrase: Option<&str>,
) -> Result<KeyMaterial> {
    let mut rng = rand::rng();
    let mut key: PrivateKey = match algorithm {
        KeyAlgorithm::Ed25519 => Ed25519Keypair::random(&mut rng).into(),
        KeyAlgorithm::Rsa { bits } => {
            if !(2048..=8192).contains(&bits) || bits % 8 != 0 {
                return Err(CoreError::Invalid("RSA size must be 2048–8192 bits".into()));
            }
            RsaKeypair::random(&mut rng, bits)
                .map_err(|e| CoreError::Key(e.to_string()))?
                .into()
        }
        KeyAlgorithm::EcdsaP256 => EcdsaKeypair::random(&mut rng, EcdsaCurve::NistP256)
            .map_err(|e| CoreError::Key(e.to_string()))?
            .into(),
        KeyAlgorithm::EcdsaP384 => EcdsaKeypair::random(&mut rng, EcdsaCurve::NistP384)
            .map_err(|e| CoreError::Key(e.to_string()))?
            .into(),
        KeyAlgorithm::EcdsaP521 => EcdsaKeypair::random(&mut rng, EcdsaCurve::NistP521)
            .map_err(|e| CoreError::Key(e.to_string()))?
            .into(),
    };
    key.set_comment(comment);
    material(&key, passphrase)
}

/// Import any supported private-key text. Encrypted keys need `passphrase`;
/// the output is always OpenSSH format, re-encrypted with the same
/// passphrase (so the vault stores a normalised blob).
pub fn import(text: &str, passphrase: Option<&str>) -> Result<KeyMaterial> {
    let text = text.trim();
    if text.is_empty() {
        return Err(CoreError::Invalid("empty key".into()));
    }
    let key = crate::ssh::load_private_key(text, passphrase)?;
    material(&key, passphrase)
}

/// Inspect a stored key without decrypting it (fingerprint needs the public
/// part, which OpenSSH keeps in clear even when encrypted).
pub fn inspect(private_key: &str) -> Result<KeyInfo> {
    let text = private_key.trim();
    if let Ok(key) = PrivateKey::from_openssh(text) {
        return Ok(public_info(key.public_key(), key.is_encrypted()));
    }
    // Non-OpenSSH formats: try to load unencrypted; otherwise report encrypted
    // with what little we know.
    match crate::ssh::load_private_key(text, None) {
        Ok(key) => Ok(public_info(key.public_key(), false)),
        Err(CoreError::Key(msg)) if msg.contains("encrypted") => Ok(KeyInfo {
            key_type: String::new(),
            bits: 0,
            fingerprint: String::new(),
            public_key: String::new(),
            comment: String::new(),
            encrypted: true,
        }),
        Err(e) => Err(e),
    }
}

/// Change (or remove, with `None`) the passphrase of a stored key.
pub fn change_passphrase(
    private_key: &str,
    old: Option<&str>,
    new: Option<&str>,
) -> Result<KeyMaterial> {
    let key = crate::ssh::load_private_key(private_key.trim(), old)?;
    material(&key, new)
}

/// Export a private key as unencrypted PEM/OpenSSH for other tools.
pub fn export_openssh(
    private_key: &str,
    passphrase: Option<&str>,
    export_passphrase: Option<&str>,
) -> Result<Zeroizing<String>> {
    let key = crate::ssh::load_private_key(private_key.trim(), passphrase)?;
    Ok(material(&key, export_passphrase)?.private_key)
}

/// Parse an `authorized_keys`-style public line.
pub fn parse_public(line: &str) -> Result<KeyInfo> {
    let key = PublicKey::from_openssh(line.trim()).map_err(|e| CoreError::Key(e.to_string()))?;
    Ok(public_info(&key, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_import_roundtrip_with_passphrase() {
        let k = generate(KeyAlgorithm::Ed25519, "me@laptop", Some("pw")).unwrap();
        assert_eq!(k.info.key_type, "ssh-ed25519");
        assert!(k.info.encrypted);
        assert!(k.public_key.ends_with("me@laptop"));

        let info = inspect(&k.private_key).unwrap();
        assert_eq!(info.fingerprint, k.info.fingerprint);
        assert!(info.encrypted);

        assert!(matches!(
            import(&k.private_key, None),
            Err(CoreError::Key(_))
        ));
        let again = import(&k.private_key, Some("pw")).unwrap();
        assert_eq!(again.info.fingerprint, k.info.fingerprint);

        let plain = change_passphrase(&k.private_key, Some("pw"), None).unwrap();
        assert!(!plain.info.encrypted);
        assert_eq!(plain.info.fingerprint, k.info.fingerprint);
    }

    #[test]
    fn ecdsa_and_rsa_sizes() {
        for (algo, bits, key_type) in [
            (KeyAlgorithm::EcdsaP256, 256, "ecdsa-sha2-nistp256"),
            (KeyAlgorithm::EcdsaP384, 384, "ecdsa-sha2-nistp384"),
            (KeyAlgorithm::EcdsaP521, 521, "ecdsa-sha2-nistp521"),
        ] {
            let e = generate(algo, "c", Some("pw")).unwrap();
            assert_eq!(e.info.bits, bits);
            assert_eq!(e.info.key_type, key_type);
            assert!(e.public_key.starts_with(key_type));
            let again = import(&e.private_key, Some("pw")).unwrap();
            assert_eq!(again.info.fingerprint, e.info.fingerprint);
            assert_eq!(parse_public(&e.public_key).unwrap().bits, bits);
        }
        let r = generate(KeyAlgorithm::Rsa { bits: 2048 }, "", None).unwrap();
        assert_eq!(r.info.bits, 2048);
        assert!(matches!(
            generate(KeyAlgorithm::Rsa { bits: 1024 }, "", None),
            Err(CoreError::Invalid(_))
        ));
    }

    #[test]
    fn imports_pkcs8_pem() {
        // ed25519 PKCS#8 as produced by `openssl genpkey -algorithm ed25519`.
        let pem = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIIBz2/qVjjxjMvdRdoT9xV4BgFkbK0sFbmVYo9GYrZ0d\n-----END PRIVATE KEY-----\n";
        let k = import(pem, None).unwrap();
        assert_eq!(k.info.key_type, "ssh-ed25519");
        assert!(
            k.private_key
                .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----")
        );
    }

    #[test]
    fn parses_public_line() {
        let k = generate(KeyAlgorithm::Ed25519, "c", None).unwrap();
        let p = parse_public(&k.public_key).unwrap();
        assert_eq!(p.fingerprint, k.info.fingerprint);
        assert!(parse_public("garbage").is_err());
    }
}
