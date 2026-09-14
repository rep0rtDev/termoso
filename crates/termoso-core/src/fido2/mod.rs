//! FIDO2 security keys as SSH keys (`sk-ssh-ed25519@openssh.com`,
//! `sk-ecdsa-sha2-nistp256@openssh.com`).
//!
//! The private scalar never leaves the token. What the vault stores is the
//! same thing OpenSSH puts in `id_ed25519_sk`: the public key, the FIDO
//! *key handle* (credential id) and the flags, in OpenSSH private-key format
//! (optionally passphrase-protected). Signing means asking the token for an
//! assertion over the SSH data; the token asks the user to touch it (and
//! enter a PIN when the key was created with user verification).
//!
//! Wire formats follow OpenSSH `PROTOCOL.u2f`. This module holds the parts
//! that do not talk to hardware; the token itself is reached through one of:
//!
//! * [`usb`] (feature `fido2`) – desktop, USB HID through `hidapi`;
//! * [`ctap`] (feature `fido2-ctap`) – our own CTAP2 client over any
//!   [`ctap::CtapTransport`], with [`hid`] (CTAPHID framing) and [`nfc`]
//!   (ISO 7816 APDUs) adapters. Android drives these with the USB-host and
//!   NFC APIs from Kotlin, the packets themselves crossing the bridge as
//!   opaque bytes.

use std::sync::Arc;

use russh::keys::HashAlg;
use russh::keys::agent::AgentIdentity;
use russh::keys::ssh_encoding::Encode;
use russh::keys::ssh_key::private::{KeypairData, SkEcdsaSha2NistP256, SkEd25519};
use russh::keys::ssh_key::public::{Ed25519PublicKey, KeyData};
use russh::keys::ssh_key::sec1::EncodedPoint;
use russh::keys::ssh_key::sec1::consts::U32;
use russh::keys::ssh_key::{Algorithm, PrivateKey, PublicKey, public};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{CoreError, Result};
use crate::keys::{KeyMaterial, material};

#[cfg(feature = "fido2-ctap")]
pub mod ctap;
#[cfg(feature = "fido2-ctap")]
pub mod hid;
#[cfg(feature = "fido2-ctap")]
pub mod nfc;
#[cfg(feature = "fido2-soft")]
pub mod soft;
#[cfg(feature = "fido2")]
pub mod usb;
#[cfg(feature = "fido2-ctap")]
pub mod webauthn;

#[cfg(feature = "fido2")]
pub use usb::{UsbBackend, generate, list_devices, load_resident, sign};

/// `SSH_SK_USER_PRESENCE_REQD` – touch required at every signature.
pub const FLAG_USER_PRESENCE: u8 = 0x01;
/// `SSH_SK_USER_VERIFICATION_REQD` – PIN (or biometric) required at every
/// signature.
pub const FLAG_USER_VERIFICATION: u8 = 0x04;
/// `SSH_SK_RESIDENT_KEY` – credential stored on the token (discoverable).
pub const FLAG_RESIDENT: u8 = 0x20;

/// Default FIDO relying party / application for SSH keys (`ssh-keygen -O application=`).
pub const DEFAULT_APPLICATION: &str = "ssh:";

/// Things that go wrong with a security key, typed so the UI can react
/// (ask for a PIN, tell the user to touch the token…).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Fido2Error {
    /// No authenticator is plugged in (or none we may open).
    #[error("no FIDO2 device found")]
    NoDevice,
    /// The chosen device went away.
    #[error("FIDO2 device not found: {0}")]
    DeviceGone(String),
    /// The token wants a PIN for this operation and none was given.
    #[error("PIN required")]
    PinRequired,
    /// Wrong PIN. `retries` is what the token reports, when it does.
    #[error("invalid PIN")]
    PinInvalid {
        /// Attempts left before the token blocks.
        retries: Option<i32>,
    },
    /// Too many wrong PINs; the token needs a power cycle or reset.
    #[error("PIN blocked")]
    PinBlocked,
    /// The token has no PIN set but the operation needs one.
    #[error("no PIN set on the device")]
    PinNotSet,
    /// Nobody touched the token in time.
    #[error("timed out waiting for a touch")]
    Timeout,
    /// The user declined (or the token refused the operation).
    #[error("operation denied by the device")]
    Denied,
    /// The credential is not on this token (wrong device, or a non-resident
    /// key handle the token cannot unwrap).
    #[error("this key does not belong to the connected device")]
    WrongDevice,
    /// The token cannot do what was asked (algorithm, resident keys…).
    #[error("not supported by the device: {0}")]
    Unsupported(String),
    /// Anything else (transport, parse).
    #[error("{0}")]
    Other(String),
}

impl Fido2Error {
    /// Stable kind for the UI.
    pub fn kind(&self) -> &'static str {
        match self {
            Fido2Error::NoDevice => "fido2_no_device",
            Fido2Error::DeviceGone(_) => "fido2_device_gone",
            Fido2Error::PinRequired => "fido2_pin_required",
            Fido2Error::PinInvalid { .. } => "fido2_pin_invalid",
            Fido2Error::PinBlocked => "fido2_pin_blocked",
            Fido2Error::PinNotSet => "fido2_pin_not_set",
            Fido2Error::Timeout => "fido2_timeout",
            Fido2Error::Denied => "fido2_denied",
            Fido2Error::WrongDevice => "fido2_wrong_device",
            Fido2Error::Unsupported(_) => "fido2_unsupported",
            Fido2Error::Other(_) => "fido2",
        }
    }

    /// Typed error for a CTAP2 status byte (`CTAP2_ERR_*`).
    pub fn from_status(code: u8) -> Self {
        match code {
            0x22 | 0x2E => Fido2Error::WrongDevice,
            0x19 => Fido2Error::Other("this device is already registered".into()),
            0x26 => Fido2Error::Unsupported("algorithm".into()),
            0x2B | 0x2C => Fido2Error::Unsupported("option".into()),
            0x27 | 0x2D => Fido2Error::Denied,
            0x05 | 0x2F | 0x3A => Fido2Error::Timeout,
            0x31 | 0x33 | 0x3F => Fido2Error::PinInvalid { retries: None },
            0x32 | 0x34 | 0x3C => Fido2Error::PinBlocked,
            0x35 => Fido2Error::PinNotSet,
            0x36 => Fido2Error::PinRequired,
            0x06 => Fido2Error::Other("device busy".into()),
            0x28 => Fido2Error::Other("key store full".into()),
            0x3B => Fido2Error::Other("user presence required".into()),
            0x40 => Fido2Error::Other("PIN token lacks permission".into()),
            other => Fido2Error::Other(format!("CTAP error 0x{other:02x}")),
        }
    }
}

/// Key type a token can mint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkAlgorithm {
    /// `sk-ssh-ed25519@openssh.com` (not every token supports it).
    Ed25519,
    /// `sk-ecdsa-sha2-nistp256@openssh.com` (every FIDO2 token supports it).
    #[default]
    EcdsaP256,
}

impl SkAlgorithm {
    /// COSE algorithm identifier (`-8` EdDSA, `-7` ES256).
    pub fn cose_alg(self) -> i64 {
        match self {
            SkAlgorithm::Ed25519 => -8,
            SkAlgorithm::EcdsaP256 => -7,
        }
    }

    /// SSH algorithm of the resulting key.
    pub fn ssh_algorithm(self) -> Algorithm {
        match self {
            SkAlgorithm::Ed25519 => Algorithm::SkEd25519,
            SkAlgorithm::EcdsaP256 => Algorithm::SkEcdsaSha2NistP256,
        }
    }
}

/// A connected authenticator, as the UI lists it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fido2Device {
    /// Opaque path used to pick this device again.
    pub path: String,
    /// Product name as reported over USB (or by the NFC reader).
    pub product: String,
    /// USB vendor id (0 over NFC).
    pub vendor_id: u16,
    /// USB product id (0 over NFC).
    pub product_id: u16,
    /// Authenticator AAGUID (hex), when readable.
    pub aaguid: Option<String>,
    /// Whether a client PIN is set (`None` when the token does not support PINs).
    pub pin_set: Option<bool>,
    /// Token can store resident (discoverable) credentials.
    pub resident_keys: bool,
    /// Algorithms the token can generate.
    pub algorithms: Vec<SkAlgorithm>,
    /// CTAP versions (`FIDO_2_0`, `FIDO_2_1`, `U2F_V2`…).
    pub versions: Vec<String>,
}

/// Parameters for [`generate`] / [`ctap::generate_with`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateOptions {
    /// Device path from [`Fido2Device::path`]; `None` = the only connected one.
    #[serde(default)]
    pub device: Option<String>,
    /// Key type.
    #[serde(default)]
    pub algorithm: SkAlgorithm,
    /// FIDO application / relying party id. Defaults to `ssh:`.
    #[serde(default)]
    pub application: Option<String>,
    /// Store the credential on the token so it can be loaded on another
    /// machine (`ssh-keygen -O resident`).
    #[serde(default)]
    pub resident: bool,
    /// Ask for a touch at every signature (`SSH_SK_USER_PRESENCE_REQD`).
    /// OpenSSH default is on.
    #[serde(default = "default_true")]
    pub user_presence: bool,
    /// Ask for the PIN at every signature (`-O verify-required`).
    #[serde(default)]
    pub user_verification: bool,
    /// Client PIN, needed when the token has one set (and always for
    /// resident / verify-required keys).
    #[serde(default)]
    pub pin: Option<Zeroizing<String>>,
    /// User name recorded with a resident credential (`-O user=`).
    #[serde(default)]
    pub user: Option<String>,
    /// Key comment.
    #[serde(default)]
    pub comment: String,
    /// Passphrase protecting the stored key handle, like OpenSSH does.
    #[serde(default)]
    pub passphrase: Option<Zeroizing<String>>,
}

fn default_true() -> bool {
    true
}

impl GenerateOptions {
    /// Application (relying party id) after defaulting and validation:
    /// OpenSSH insists on the `ssh:` prefix.
    pub fn application(&self) -> Result<&str> {
        let application = self
            .application
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .unwrap_or(DEFAULT_APPLICATION);
        if !application.starts_with("ssh:") {
            return Err(CoreError::Invalid(
                "FIDO application must start with \"ssh:\"".into(),
            ));
        }
        Ok(application)
    }

    /// User name recorded with the credential (`termoso` when unset).
    pub fn user_name(&self) -> &str {
        self.user
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .unwrap_or("termoso")
    }

    /// User id: OpenSSH uses an all-zero 32-byte id for non-resident keys
    /// and the user name for resident ones.
    pub fn user_id(&self) -> Vec<u8> {
        if self.resident {
            let mut id = self.user_name().as_bytes().to_vec();
            id.resize(id.len().clamp(1, 64), 0);
            id
        } else {
            vec![0u8; 32]
        }
    }

    /// OpenSSH flags byte for the stored handle.
    pub fn flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.user_presence {
            flags |= FLAG_USER_PRESENCE;
        }
        if self.user_verification {
            flags |= FLAG_USER_VERIFICATION;
        }
        if self.resident {
            flags |= FLAG_RESIDENT;
        }
        flags
    }

    /// PIN when one was given and it is not blank.
    pub fn pin(&self) -> Option<&str> {
        self.pin
            .as_deref()
            .map(|p| p.as_str())
            .filter(|p| !p.is_empty())
    }
}

/// Public key as a token reports it (from the COSE key in the attested
/// credential data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CosePublicKey {
    /// EdDSA over Ed25519, 32-byte point.
    Ed25519([u8; 32]),
    /// ES256 – P-256 point coordinates.
    P256 {
        /// Affine x.
        x: [u8; 32],
        /// Affine y.
        y: [u8; 32],
    },
}

impl CosePublicKey {
    /// Algorithm this key belongs to.
    pub fn algorithm(&self) -> SkAlgorithm {
        match self {
            CosePublicKey::Ed25519(_) => SkAlgorithm::Ed25519,
            CosePublicKey::P256 { .. } => SkAlgorithm::EcdsaP256,
        }
    }
}

/// What the token hands back for a new (or enumerated) credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    /// Credential id – the OpenSSH *key handle*.
    pub id: Vec<u8>,
    /// Its public key.
    pub public_key: CosePublicKey,
}

/// One assertion: authenticator data (`rpIdHash ‖ flags ‖ counter …`) and
/// the raw signature (Ed25519: 64 bytes; ES256: DER).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssertionData {
    /// Authenticator data as signed.
    pub auth_data: Vec<u8>,
    /// Signature over `auth_data ‖ clientDataHash`.
    pub signature: Vec<u8>,
}

/// Whether `key` is a security-key type.
pub fn is_security_key(key: &PublicKey) -> bool {
    matches!(
        key.algorithm(),
        Algorithm::SkEd25519 | Algorithm::SkEcdsaSha2NistP256
    )
}

/// Whether the stored key-type label names a security key (`sk-…`).
pub fn is_sk_type(key_type: &str) -> bool {
    key_type.starts_with("sk-") || key_type.ends_with("-sk")
}

/// Wrap a token credential as an OpenSSH `sk-*` private key (public key,
/// flags and handle – no secret).
pub fn sk_private_key(
    algorithm: SkAlgorithm,
    cred: &Credential,
    application: &str,
    flags: u8,
    comment: &str,
) -> Result<PrivateKey> {
    let handle = cred.id.clone();
    if handle.is_empty() {
        return Err(Fido2Error::Other("token returned an empty credential id".into()).into());
    }
    let data = match (algorithm, &cred.public_key) {
        (SkAlgorithm::Ed25519, CosePublicKey::Ed25519(raw)) => {
            let pk = Ed25519PublicKey::try_from(&raw[..])?;
            KeypairData::SkEd25519(SkEd25519::new(
                public::SkEd25519::new(pk, application),
                flags,
                handle,
            )?)
        }
        (SkAlgorithm::EcdsaP256, CosePublicKey::P256 { x, y }) => {
            let mut raw = Vec::with_capacity(65);
            raw.push(0x04);
            raw.extend_from_slice(x);
            raw.extend_from_slice(y);
            let point = EncodedPoint::<U32>::from_bytes(&raw)
                .map_err(|_| Fido2Error::Other("bad P-256 point".into()))?;
            KeypairData::SkEcdsaSha2NistP256(SkEcdsaSha2NistP256::new(
                public::SkEcdsaSha2NistP256::new(point, application),
                flags,
                handle,
            )?)
        }
        (want, got) => {
            return Err(Fido2Error::Unsupported(format!(
                "asked for {want:?}, token returned {:?}",
                got.algorithm()
            ))
            .into());
        }
    };
    Ok(PrivateKey::new(data, comment)?)
}

/// Wrap a credential and serialise it as vault material.
pub fn sk_material(
    algorithm: SkAlgorithm,
    cred: &Credential,
    application: &str,
    flags: u8,
    comment: &str,
    passphrase: Option<&str>,
) -> Result<KeyMaterial> {
    let key = sk_private_key(algorithm, cred, application, flags, comment)?;
    material(&key, passphrase)
}

/// What a stored `sk-*` key tells us about the credential it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkHandle {
    /// FIDO application / relying party id.
    pub application: String,
    /// Credential id.
    pub key_handle: Vec<u8>,
    /// OpenSSH flags byte.
    pub flags: u8,
    /// SSH algorithm.
    pub algorithm: Algorithm,
}

impl SkHandle {
    /// Read the handle out of a decrypted `sk-*` private key.
    pub fn of(key: &PrivateKey) -> Result<Self> {
        match key.key_data() {
            KeypairData::SkEd25519(sk) => Ok(Self {
                application: sk.public().application().to_string(),
                key_handle: sk.key_handle().to_vec(),
                flags: sk.flags(),
                algorithm: Algorithm::SkEd25519,
            }),
            KeypairData::SkEcdsaSha2NistP256(sk) => Ok(Self {
                application: sk.public().application().to_string(),
                key_handle: sk.key_handle().to_vec(),
                flags: sk.flags(),
                algorithm: Algorithm::SkEcdsaSha2NistP256,
            }),
            _ => Err(CoreError::Key("not a security key".into())),
        }
    }

    /// Touch required for every signature.
    pub fn wants_up(&self) -> bool {
        self.flags & FLAG_USER_PRESENCE != 0
    }

    /// PIN required for every signature.
    pub fn wants_uv(&self) -> bool {
        self.flags & FLAG_USER_VERIFICATION != 0
    }
}

/// Build the SSH signature blob (`string alg, string sig, byte flags,
/// uint32 counter`) from an assertion.
pub fn signature_blob(alg: Algorithm, a: &AssertionData) -> Result<Vec<u8>> {
    // authData = rpIdHash(32) || flags(1) || signCount(4) [|| …]
    let auth_flags = *a
        .auth_data
        .get(32)
        .ok_or_else(|| Fido2Error::Other("short authenticator data".into()))?;
    let counter = a
        .auth_data
        .get(33..37)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| Fido2Error::Other("short authenticator data".into()))?;
    let sig: Vec<u8> = match alg {
        Algorithm::SkEd25519 => {
            if a.signature.len() != 64 {
                return Err(Fido2Error::Other("bad Ed25519 signature length".into()).into());
            }
            a.signature.clone()
        }
        Algorithm::SkEcdsaSha2NistP256 => {
            let (r, s) = der_ecdsa(&a.signature)?;
            let mut out = Vec::with_capacity(r.len() + s.len() + 8);
            r.encode(&mut out)?;
            s.encode(&mut out)?;
            out
        }
        _ => return Err(CoreError::Key("not a security key".into())),
    };
    let mut blob = Vec::new();
    alg.as_str().encode(&mut blob)?;
    sig.as_slice().encode(&mut blob)?;
    blob.push(auth_flags);
    blob.extend_from_slice(&counter.to_be_bytes());
    Ok(blob)
}

/// `SEQUENCE { INTEGER r, INTEGER s }` → (r, s) as minimal big-endian
/// two's-complement, which is also the SSH `mpint` encoding.
fn der_ecdsa(sig: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    fn int(b: &[u8]) -> Option<(&[u8], &[u8])> {
        let (&tag, rest) = b.split_first()?;
        if tag != 0x02 {
            return None;
        }
        let (&len, rest) = rest.split_first()?;
        let len = len as usize;
        if len >= 0x80 || rest.len() < len {
            return None;
        }
        Some((&rest[..len], &rest[len..]))
    }
    let bad = || Fido2Error::Other("bad ECDSA signature".into());
    let (&tag, rest) = sig.split_first().ok_or_else(bad)?;
    if tag != 0x30 {
        return Err(bad().into());
    }
    let (&len, body) = rest.split_first().ok_or_else(bad)?;
    let body = if len == 0x81 {
        let (&l, b) = body.split_first().ok_or_else(bad)?;
        b.get(..l as usize).ok_or_else(bad)?
    } else {
        body.get(..len as usize).ok_or_else(bad)?
    };
    let (r, rest) = int(body).ok_or_else(bad)?;
    let (s, rest) = int(rest).ok_or_else(bad)?;
    if !rest.is_empty() {
        return Err(bad().into());
    }
    Ok((r.to_vec(), s.to_vec()))
}

/// Something that can reach the token holding a stored `sk-*` key and make
/// it sign. Desktop: USB HID via `hidapi`; Android: whatever device the app
/// currently holds (USB host or NFC tag). Blocking – called off the runtime.
pub trait SkBackend: Send + Sync {
    /// SSH signature blob (`signature_blob`) over `data` for `key`.
    fn sign(&self, key: &PrivateKey, data: &[u8], pin: Option<&str>) -> Result<Vec<u8>>;
}

/// [`russh::Signer`] that signs with the security key. Used by
/// `authenticate_publickey_with` so `russh` never sees private material
/// (there is none to see).
pub struct SkSigner {
    /// The `sk-*` key (public part + handle).
    pub key: Arc<PrivateKey>,
    /// Client PIN when the key requires user verification (or the token
    /// insists).
    pub pin: Option<Zeroizing<String>>,
    /// How to reach the token.
    pub backend: Arc<dyn SkBackend>,
    /// Called right before each signature request ("touch your key").
    pub on_touch: Option<Box<dyn Fn() + Send + Sync>>,
}

impl russh::Signer for SkSigner {
    type Error = CoreError;

    fn auth_sign(
        &mut self,
        _key: &AgentIdentity,
        _hash_alg: Option<HashAlg>,
        to_sign: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<Vec<u8>>> + Send {
        let key = self.key.clone();
        let pin = self.pin.clone();
        let backend = self.backend.clone();
        if let Some(f) = &self.on_touch {
            f();
        }
        async move {
            let signed = tokio::task::spawn_blocking(move || {
                let blob = backend.sign(&key, &to_sign, pin.as_deref().map(|p| p.as_str()))?;
                let mut out = to_sign;
                blob.as_slice().encode(&mut out)?;
                Ok::<_, CoreError>(out)
            })
            .await
            .map_err(|e| CoreError::Ssh(format!("fido2 signer: {e}")))??;
            Ok(signed)
        }
    }
}

impl From<russh::SendError> for CoreError {
    fn from(_: russh::SendError) -> Self {
        CoreError::Ssh("connection closed while signing".into())
    }
}

/// Requirements baked into a stored `sk-*` handle (OpenSSH's flags byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkFlags {
    /// Credential lives on the token (discoverable).
    pub resident: bool,
    /// Touch required for every signature.
    pub user_presence: bool,
    /// PIN required for every signature.
    pub user_verification: bool,
}

impl SkFlags {
    /// Split OpenSSH's flags byte.
    pub fn from_byte(flags: u8) -> Self {
        Self {
            resident: flags & FLAG_RESIDENT != 0,
            user_presence: flags & FLAG_USER_PRESENCE != 0,
            user_verification: flags & FLAG_USER_VERIFICATION != 0,
        }
    }
}

/// Public facts about a stored `sk-*` key (what the UI shows on the card).
/// Nothing in here is secret: the stored "private key" of a security key is
/// only the public key plus the credential handle — the token keeps the
/// signing key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityKeyInfo {
    /// FIDO application / relying party id (`ssh:`…).
    pub application: String,
    /// `None` when the handle is passphrase-protected and the passphrase
    /// was not supplied — the flags live inside the encrypted block.
    pub flags: Option<SkFlags>,
    /// Credential id (hex); `None` when the handle is still encrypted.
    pub credential_id: Option<String>,
}

/// Describe a stored OpenSSH key when it is a security key; `None` for
/// ordinary keys or unparsable text. With the right passphrase the flags
/// and handle are read from the decrypted block; otherwise only the public
/// half is described.
pub fn describe(private_key: &str, passphrase: Option<&str>) -> Option<SecurityKeyInfo> {
    let mut key = PrivateKey::from_openssh(private_key.trim()).ok()?;
    if !is_security_key(key.public_key()) {
        return None;
    }
    if key.is_encrypted()
        && let Some(p) = passphrase.filter(|p| !p.is_empty())
        && let Ok(clear) = key.decrypt(p)
    {
        key = clear;
    }
    let (application, details) = match key.key_data() {
        KeypairData::SkEd25519(sk) => (
            sk.public().application().to_string(),
            Some((sk.flags(), sk.key_handle().to_vec())),
        ),
        KeypairData::SkEcdsaSha2NistP256(sk) => (
            sk.public().application().to_string(),
            Some((sk.flags(), sk.key_handle().to_vec())),
        ),
        _ => match key.public_key().key_data() {
            KeyData::SkEd25519(pk) => (pk.application().to_string(), None),
            KeyData::SkEcdsaSha2NistP256(pk) => (pk.application().to_string(), None),
            _ => return None,
        },
    };
    Some(SecurityKeyInfo {
        application,
        flags: details.as_ref().map(|(f, _)| SkFlags::from_byte(*f)),
        credential_id: details.map(|(_, h)| hex::encode(h)),
    })
}

/// Public key data of an `sk-*` private key, for `authenticate_publickey_with`.
pub fn public_key_of(key: &PrivateKey) -> PublicKey {
    let data: KeyData = key.public_key().key_data().clone();
    PublicKey::new(data, key.comment().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::ssh_key::LineEnding;

    fn fake_credential(alg: SkAlgorithm) -> Credential {
        let public_key = match alg {
            SkAlgorithm::Ed25519 => CosePublicKey::Ed25519([9u8; 32]),
            SkAlgorithm::EcdsaP256 => {
                // Generator point of P-256 so the SEC1 decode is happy.
                let g = hex::decode(
                    "6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296\
                     4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5",
                )
                .unwrap();
                CosePublicKey::P256 {
                    x: g[..32].try_into().unwrap(),
                    y: g[32..].try_into().unwrap(),
                }
            }
        };
        Credential {
            id: vec![7u8; 40],
            public_key,
        }
    }

    #[test]
    fn sk_keys_round_trip_through_openssh_format() {
        for (alg, name) in [
            (SkAlgorithm::Ed25519, "sk-ssh-ed25519@openssh.com"),
            (SkAlgorithm::EcdsaP256, "sk-ecdsa-sha2-nistp256@openssh.com"),
        ] {
            let cred = fake_credential(alg);
            let key = sk_private_key(alg, &cred, "ssh:", FLAG_USER_PRESENCE | FLAG_RESIDENT, "c")
                .unwrap();
            assert_eq!(key.algorithm().as_str(), name);
            let m = material(&key, Some("pw")).unwrap();
            assert!(m.info.encrypted);
            assert!(m.public_key.starts_with(name));
            let back = crate::ssh::load_private_key(&m.private_key, Some("pw")).unwrap();
            assert!(is_security_key(back.public_key()));
            let handle = SkHandle::of(&back).unwrap();
            assert_eq!(handle.key_handle, vec![7u8; 40]);
            assert_eq!(handle.flags, FLAG_USER_PRESENCE | FLAG_RESIDENT);
            assert_eq!(handle.application, "ssh:");
            assert!(handle.wants_up() && !handle.wants_uv());
            let _ = key.to_openssh(LineEnding::LF).unwrap();
        }
    }

    #[test]
    fn describe_reads_flags_and_handle_only_when_decryptable() {
        let cred = fake_credential(SkAlgorithm::Ed25519);
        let key = sk_private_key(
            SkAlgorithm::Ed25519,
            &cred,
            "ssh:termoso",
            FLAG_USER_PRESENCE | FLAG_USER_VERIFICATION,
            "c",
        )
        .unwrap();
        let clear = material(&key, None).unwrap();
        let info = describe(&clear.private_key, None).unwrap();
        assert_eq!(info.application, "ssh:termoso");
        assert_eq!(
            info.credential_id.as_deref(),
            Some(hex::encode([7u8; 40]).as_str())
        );
        let flags = info.flags.unwrap();
        assert!(flags.user_presence && flags.user_verification && !flags.resident);

        let sealed = material(&key, Some("pw")).unwrap();
        let locked = describe(&sealed.private_key, None).unwrap();
        assert_eq!(locked.application, "ssh:termoso");
        assert!(locked.flags.is_none() && locked.credential_id.is_none());
        let opened = describe(&sealed.private_key, Some("pw")).unwrap();
        assert_eq!(opened, info);
        assert!(
            describe(&sealed.private_key, Some("nope"))
                .unwrap()
                .flags
                .is_none()
        );

        // Ordinary keys are not security keys.
        let plain = crate::keys::generate(crate::keys::KeyAlgorithm::Ed25519, "c", None).unwrap();
        assert!(describe(&plain.private_key, None).is_none());
    }

    /// The stored handle of a security key must never carry signing material:
    /// the OpenSSH `sk-*` private block is public key + flags + handle only.
    #[test]
    fn stored_sk_handle_has_no_private_scalar() {
        for alg in [SkAlgorithm::Ed25519, SkAlgorithm::EcdsaP256] {
            let cred = fake_credential(alg);
            let key = sk_private_key(alg, &cred, "ssh:", FLAG_USER_PRESENCE, "c").unwrap();
            // The private half is the public key data, one flags byte, the
            // handle and the empty reserved field — no scalar anywhere.
            let public_len = key.public_key().key_data().encoded_len().unwrap();
            let private_len = key.key_data().encoded_len().unwrap();
            assert_eq!(private_len, public_len + 1 + (4 + 40) + 4);
            assert_eq!(SkHandle::of(&key).unwrap().key_handle, vec![7u8; 40]);
        }
    }

    #[test]
    fn wrong_algorithm_from_token_is_rejected() {
        let cred = fake_credential(SkAlgorithm::Ed25519);
        let err = sk_private_key(SkAlgorithm::EcdsaP256, &cred, "ssh:", 1, "").unwrap_err();
        assert!(matches!(err, CoreError::Fido2(Fido2Error::Unsupported(_))));
    }

    #[test]
    fn der_signature_to_mpints() {
        // r = 0x00ff…, s = 0x01
        let sig = [0x30, 0x08, 0x02, 0x03, 0x00, 0xff, 0x01, 0x02, 0x01, 0x01];
        let (r, s) = der_ecdsa(&sig).unwrap();
        assert_eq!(r, vec![0x00, 0xff, 0x01]);
        assert_eq!(s, vec![0x01]);
        assert!(der_ecdsa(&[0x31, 0x00]).is_err());
    }

    #[test]
    fn signature_blob_layout() {
        let mut auth_data = vec![0u8; 32];
        auth_data.push(0x01);
        auth_data.extend_from_slice(&42u32.to_be_bytes());
        let a = AssertionData {
            signature: vec![5u8; 64],
            auth_data,
        };
        let blob = signature_blob(Algorithm::SkEd25519, &a).unwrap();
        let name = b"sk-ssh-ed25519@openssh.com";
        assert_eq!(&blob[..4], &(name.len() as u32).to_be_bytes());
        assert_eq!(&blob[4..4 + name.len()], name);
        let i = 4 + name.len();
        assert_eq!(&blob[i..i + 4], &64u32.to_be_bytes());
        assert_eq!(blob[i + 4 + 64], 0x01);
        assert_eq!(&blob[i + 5 + 64..], &42u32.to_be_bytes());
    }

    #[test]
    fn status_mapping() {
        assert_eq!(Fido2Error::from_status(0x36), Fido2Error::PinRequired);
        assert_eq!(
            Fido2Error::from_status(0x31),
            Fido2Error::PinInvalid { retries: None }
        );
        assert_eq!(Fido2Error::from_status(0x2E), Fido2Error::WrongDevice);
        assert_eq!(Fido2Error::from_status(0x2F), Fido2Error::Timeout);
        assert!(matches!(
            Fido2Error::from_status(0x77),
            Fido2Error::Other(ref m) if m.contains("0x77")
        ));
    }

    #[test]
    fn generate_options_defaults() {
        let opts: GenerateOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(opts.application().unwrap(), "ssh:");
        assert_eq!(opts.user_name(), "termoso");
        assert_eq!(opts.user_id(), vec![0u8; 32]);
        assert_eq!(opts.flags(), FLAG_USER_PRESENCE);
        assert!(opts.pin().is_none());

        let opts: GenerateOptions = serde_json::from_str(
            r#"{"application":"web:nope","resident":true,"user":"me","userVerification":true,"pin":""}"#,
        )
        .unwrap();
        assert!(opts.application().is_err());
        assert_eq!(opts.user_id(), b"me".to_vec());
        assert_eq!(
            opts.flags(),
            FLAG_USER_PRESENCE | FLAG_USER_VERIFICATION | FLAG_RESIDENT
        );
        assert!(opts.pin().is_none());
    }
}
