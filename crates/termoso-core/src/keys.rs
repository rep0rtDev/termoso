//! SSH key generation, import (OpenSSH / PEM / PKCS#8 / PuTTY), export and
//! passphrase handling. Nothing here touches the store; callers put the
//! resulting text into an `SshKey` entity.

use base64::Engine;
use chrono::{DateTime, Utc};
use russh::keys::ssh_key::private::{EcdsaKeypair, Ed25519Keypair, RsaKeypair};
use russh::keys::ssh_key::{
    Algorithm, Certificate, EcdsaCurve, HashAlg, LineEnding, PrivateKey, PublicKey,
};
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

/// Parsed OpenSSH certificate summary for the UI (no secrets involved:
/// certificates are public documents).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateInfo {
    /// Certificate algorithm (`ssh-ed25519-cert-v01@openssh.com`).
    pub cert_type: String,
    /// `user` or `host`.
    pub kind: String,
    /// Key ID set by the CA.
    pub key_id: String,
    /// Serial number.
    pub serial: u64,
    /// Principals (usernames / hostnames). Empty = any.
    pub principals: Vec<String>,
    /// Start of validity, `None` = unbounded.
    pub valid_after: Option<DateTime<Utc>>,
    /// End of validity, `None` = forever.
    pub valid_before: Option<DateTime<Utc>>,
    /// `SHA256:` fingerprint of the certified public key.
    pub fingerprint: String,
    /// `SHA256:` fingerprint of the CA key.
    pub ca_fingerprint: String,
    /// CA key algorithm.
    pub ca_key_type: String,
    /// Whether the certificate is valid right now (signature + time window).
    pub valid_now: bool,
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

/// Whether the text looks like a PuTTY `.ppk` file.
pub fn is_ppk(text: &str) -> bool {
    text.trim_start().starts_with("PuTTY-User-Key-File-")
}

/// Public half of a PuTTY key, readable without the passphrase (PPK keeps
/// `Public-Lines` in clear, like OpenSSH).
fn ppk_public(text: &str) -> Option<PublicKey> {
    let mut lines = text.lines().map(str::trim);
    let mut comment = String::new();
    let mut blob = String::new();
    while let Some(line) = lines.next() {
        if let Some(c) = line.strip_prefix("Comment:") {
            comment = c.trim().to_string();
        } else if let Some(n) = line.strip_prefix("Public-Lines:") {
            let n: usize = n.trim().parse().ok()?;
            for _ in 0..n {
                blob.push_str(lines.next()?);
            }
        }
    }
    if blob.is_empty() {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(blob)
        .ok()?;
    let mut key = PublicKey::from_bytes(&bytes).ok()?;
    key.set_comment(comment);
    Some(key)
}

/// Inspect a stored key without decrypting it (fingerprint needs the public
/// part, which OpenSSH and PuTTY keep in clear even when encrypted).
pub fn inspect(private_key: &str) -> Result<KeyInfo> {
    let text = private_key.trim();
    if let Ok(key) = PrivateKey::from_openssh(text) {
        return Ok(public_info(key.public_key(), key.is_encrypted()));
    }
    // Non-OpenSSH formats: try to load unencrypted; otherwise report encrypted
    // with what little we know.
    match crate::ssh::load_private_key(text, None) {
        Ok(key) => Ok(public_info(key.public_key(), false)),
        Err(CoreError::Key(msg)) if msg.contains("encrypted") => Ok(match ppk_public(text) {
            Some(pk) => public_info(&pk, true),
            None => KeyInfo {
                key_type: String::new(),
                bits: 0,
                fingerprint: String::new(),
                public_key: String::new(),
                comment: String::new(),
                encrypted: true,
            },
        }),
        Err(e) => Err(e),
    }
}

fn parse_certificate(text: &str) -> Result<Certificate> {
    let text = text.trim();
    if text.is_empty() {
        return Err(CoreError::Invalid("empty certificate".into()));
    }
    if text.starts_with("-----BEGIN") {
        return Err(CoreError::Invalid(
            "expected an OpenSSH certificate (*-cert.pub), not a private key or X.509 PEM".into(),
        ));
    }
    Certificate::from_openssh(text).map_err(|e| CoreError::Invalid(format!("certificate: {e}")))
}

fn unix_time(ts: u64, unbounded: u64) -> Option<DateTime<Utc>> {
    if ts == unbounded {
        return None;
    }
    DateTime::<Utc>::from_timestamp(i64::try_from(ts).ok()?, 0)
}

/// Parse and summarise an OpenSSH certificate. The CA signature is verified
/// against the CA key embedded in the certificate, so a truncated or edited
/// certificate is rejected here rather than by the server.
pub fn inspect_certificate(text: &str) -> Result<CertificateInfo> {
    let cert = parse_certificate(text)?;
    cert.verify_signature()
        .map_err(|e| CoreError::Invalid(format!("certificate signature: {e}")))?;
    let now = u64::try_from(Utc::now().timestamp()).unwrap_or(0);
    let valid_now = cert.valid_after() <= now && now < cert.valid_before();
    Ok(CertificateInfo {
        cert_type: cert.algorithm().to_certificate_type(),
        kind: if cert.cert_type().is_host() {
            "host".into()
        } else {
            "user".into()
        },
        key_id: cert.key_id().to_string(),
        serial: cert.serial(),
        principals: cert.valid_principals().to_vec(),
        valid_after: unix_time(cert.valid_after(), 0),
        valid_before: unix_time(cert.valid_before(), u64::MAX),
        fingerprint: cert.public_key().fingerprint(HashAlg::Sha256).to_string(),
        ca_fingerprint: cert
            .signature_key()
            .fingerprint(HashAlg::Sha256)
            .to_string(),
        ca_key_type: cert.signature_key().algorithm().as_str().to_string(),
        valid_now,
    })
}

/// Whether `certificate` certifies the key with `public_line`.
pub fn certificate_matches(certificate: &str, public_line: &str) -> Result<bool> {
    let cert = parse_certificate(certificate)?;
    let key =
        PublicKey::from_openssh(public_line.trim()).map_err(|e| CoreError::Key(e.to_string()))?;
    Ok(cert.public_key() == key.key_data())
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

    // Throwaway fixtures generated with puttygen / ssh-keygen for tests only.
    const PPK_V3: &str = include_str!("../testdata/keys/ed25519_v3.ppk");
    const PPK_V2: &str = include_str!("../testdata/keys/ed25519_v2.ppk");
    const PPK_V3_ENC: &str = include_str!("../testdata/keys/ed25519_v3_encrypted.ppk");
    const PPK_V2_RSA_ENC: &str = include_str!("../testdata/keys/rsa_v2_encrypted.ppk");
    const OPENSSH_PUB: &str = include_str!("../testdata/keys/ed25519_openssh.pub");
    const RSA_PUB: &str = include_str!("../testdata/keys/rsa_openssh.pub");
    const CERT: &str = include_str!("../testdata/keys/ed25519-cert.pub");
    const CERT_EXPIRED: &str = include_str!("../testdata/keys/ed25519-expired-cert.pub");
    const RSA_CERT: &str = include_str!("../testdata/keys/rsa-cert.pub");

    #[test]
    fn imports_putty_ppk_v2_and_v3() {
        let expected = parse_public(OPENSSH_PUB).unwrap();
        for ppk in [PPK_V3, PPK_V2] {
            assert!(is_ppk(ppk));
            let k = import(ppk, None).unwrap();
            assert_eq!(k.info.key_type, "ssh-ed25519");
            assert_eq!(k.info.fingerprint, expected.fingerprint);
            assert_eq!(k.info.comment, "user@c5");
            assert!(!k.info.encrypted);
            // Normalised to OpenSSH on the way into the vault.
            assert!(
                k.private_key
                    .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----")
            );
            assert_eq!(inspect(ppk).unwrap().fingerprint, expected.fingerprint);
        }
        assert!(!is_ppk(OPENSSH_PUB));
    }

    #[test]
    fn encrypted_ppk_needs_the_right_passphrase() {
        let expected = parse_public(OPENSSH_PUB).unwrap();
        // Public half is readable without the passphrase.
        let peek = inspect(PPK_V3_ENC).unwrap();
        assert!(peek.encrypted);
        assert_eq!(peek.fingerprint, expected.fingerprint);
        assert_eq!(peek.key_type, "ssh-ed25519");
        assert_eq!(peek.comment, "user@c5");

        match import(PPK_V3_ENC, None) {
            Err(CoreError::Key(m)) => assert!(m.contains("passphrase required"), "{m}"),
            other => panic!("{other:?}"),
        }
        match import(PPK_V3_ENC, Some("nope")) {
            Err(CoreError::Key(m)) => assert!(m.contains("wrong passphrase"), "{m}"),
            other => panic!("{other:?}"),
        }
        let k = import(PPK_V3_ENC, Some("pw")).unwrap();
        assert_eq!(k.info.fingerprint, expected.fingerprint);
        assert!(k.info.encrypted, "re-encrypted with the same passphrase");
        let plain = change_passphrase(&k.private_key, Some("pw"), None).unwrap();
        assert!(!plain.info.encrypted);

        // v2 RSA (SHA-1 KDF path).
        let rsa = import(PPK_V2_RSA_ENC, Some("pw")).unwrap();
        assert_eq!(rsa.info.key_type, "ssh-rsa");
        assert_eq!(
            rsa.info.fingerprint,
            parse_public(RSA_PUB).unwrap().fingerprint
        );
        assert!(inspect(PPK_V2_RSA_ENC).unwrap().encrypted);
        assert!(import(PPK_V2_RSA_ENC, Some("bad")).is_err());
    }

    #[test]
    fn wrong_openssh_passphrase_is_reported_as_such() {
        let k = generate(KeyAlgorithm::Ed25519, "c", Some("pw")).unwrap();
        match import(&k.private_key, Some("bad")) {
            Err(CoreError::Key(m)) => assert!(m.contains("wrong passphrase"), "{m}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn inspects_and_matches_certificates() {
        let info = inspect_certificate(CERT).unwrap();
        assert_eq!(info.cert_type, "ssh-ed25519-cert-v01@openssh.com");
        assert_eq!(info.kind, "user");
        assert_eq!(info.key_id, "user-cert");
        assert_eq!(info.serial, 7);
        assert_eq!(info.principals, vec!["root", "ubuntu"]);
        assert!(info.valid_after.is_none() && info.valid_before.is_none());
        assert!(info.valid_now);
        assert_eq!(
            info.fingerprint,
            parse_public(OPENSSH_PUB).unwrap().fingerprint
        );
        assert_eq!(info.ca_key_type, "ssh-ed25519");
        assert!(info.ca_fingerprint.starts_with("SHA256:"));

        let expired = inspect_certificate(CERT_EXPIRED).unwrap();
        assert!(!expired.valid_now);
        assert!(expired.valid_before.is_some());

        assert!(certificate_matches(CERT, OPENSSH_PUB).unwrap());
        assert!(!certificate_matches(CERT, RSA_PUB).unwrap());
        assert!(certificate_matches(RSA_CERT, RSA_PUB).unwrap());

        // Garbage, a plain public key, a PEM block and a tampered signature.
        assert!(matches!(
            inspect_certificate(""),
            Err(CoreError::Invalid(_))
        ));
        assert!(inspect_certificate(OPENSSH_PUB).is_err());
        assert!(
            inspect_certificate("-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----")
                .is_err()
        );
        let mut parts: Vec<&str> = CERT.split_whitespace().collect();
        let tampered = {
            let mut b = parts[1].to_string();
            let n = b.len() - 8;
            let c = b.as_bytes()[n];
            b.replace_range(n..n + 1, if c == b'A' { "B" } else { "A" });
            b
        };
        parts[1] = &tampered;
        assert!(inspect_certificate(&parts.join(" ")).is_err());
    }
}
