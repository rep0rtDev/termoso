//! FIDO2 security keys as SSH keys (`sk-ssh-ed25519@openssh.com`,
//! `sk-ecdsa-sha2-nistp256@openssh.com`), talking CTAP2 to the authenticator
//! over USB HID.
//!
//! The private scalar never leaves the token. What the vault stores is the
//! same thing OpenSSH puts in `id_ed25519_sk`: the public key, the FIDO
//! *key handle* (credential id) and the flags, in OpenSSH private-key format
//! (optionally passphrase-protected). Signing means asking the token for an
//! assertion over the SSH data; the token asks the user to touch it (and
//! enter a PIN when the key was created with user verification).
//!
//! Wire formats follow OpenSSH `PROTOCOL.u2f`.

use std::sync::Arc;

use ctap_hid_fido2::fidokey::get_assertion::get_assertion_params::Assertion;
use ctap_hid_fido2::fidokey::make_credential::make_credential_params::{
    Attestation, CredentialSupportedKeyType,
};
use ctap_hid_fido2::fidokey::{GetAssertionArgsBuilder, MakeCredentialArgsBuilder};
use ctap_hid_fido2::public_key::PublicKeyType;
use ctap_hid_fido2::public_key_credential_user_entity::PublicKeyCredentialUserEntity;
use ctap_hid_fido2::{FidoKeyHid, FidoKeyHidFactory, HidInfo, HidParam, LibCfg};
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

    /// Map the CTAP status codes the library reports as text
    /// (`response_status err = 0x36 CTAP2_ERR_PIN_REQUIRED …`).
    fn from_ctap(e: &dyn std::fmt::Display) -> Self {
        let text = e.to_string();
        let code = text
            .split("err = 0x")
            .nth(1)
            .and_then(|rest| rest.get(..2))
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match code {
            Some(0x22) | Some(0x2E) => Fido2Error::WrongDevice,
            Some(0x26) | Some(0x2B) | Some(0x2C) => Fido2Error::Unsupported(text),
            Some(0x27) | Some(0x2D) => Fido2Error::Denied,
            Some(0x05) | Some(0x2F) | Some(0x3A) => Fido2Error::Timeout,
            Some(0x31) | Some(0x33) => Fido2Error::PinInvalid { retries: None },
            Some(0x32) | Some(0x34) => Fido2Error::PinBlocked,
            Some(0x35) => Fido2Error::PinNotSet,
            Some(0x36) | Some(0x3B) => Fido2Error::PinRequired,
            _ if text.contains("not found") || text.contains("Failed to open device") => {
                Fido2Error::NoDevice
            }
            _ => Fido2Error::Other(text),
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
    fn cose(self) -> CredentialSupportedKeyType {
        match self {
            SkAlgorithm::Ed25519 => CredentialSupportedKeyType::Ed25519,
            SkAlgorithm::EcdsaP256 => CredentialSupportedKeyType::Ecdsa256,
        }
    }
}

/// A connected authenticator, as the UI lists it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fido2Device {
    /// Opaque OS path used to pick this device again.
    pub path: String,
    /// Product name as reported over USB.
    pub product: String,
    /// USB vendor id.
    pub vendor_id: u16,
    /// USB product id.
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

/// Parameters for [`generate`].
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

fn lib_cfg() -> LibCfg {
    let mut cfg = LibCfg::init();
    cfg.enable_log = false;
    cfg.enable_keep_alive_msg = false;
    cfg
}

fn open(path: Option<&str>) -> std::result::Result<FidoKeyHid, Fido2Error> {
    let devices = ctap_hid_fido2::get_fidokey_devices();
    if devices.is_empty() {
        return Err(Fido2Error::NoDevice);
    }
    let param = match path {
        Some(p) => devices
            .iter()
            .find(|d| device_path(d) == p)
            .map(|d| d.param.clone())
            .ok_or_else(|| Fido2Error::DeviceGone(p.to_string()))?,
        None => devices
            .into_iter()
            .next()
            .map(|d| d.param)
            .ok_or(Fido2Error::NoDevice)?,
    };
    FidoKeyHidFactory::create_by_params(&[param], &lib_cfg()).map_err(|e| Fido2Error::from_ctap(&e))
}

fn device_path(d: &HidInfo) -> String {
    match &d.param {
        HidParam::Path(p) => p.clone(),
        HidParam::VidPid { vid, pid } => format!("{vid:04x}:{pid:04x}"),
    }
}

/// Every FIDO authenticator currently plugged in. Reads `getInfo` from each
/// so the UI can offer only what the token supports. Blocking (USB I/O).
pub fn list_devices() -> Vec<Fido2Device> {
    ctap_hid_fido2::get_fidokey_devices()
        .into_iter()
        .map(|d| {
            let path = device_path(&d);
            let mut dev = Fido2Device {
                path,
                product: d.product_string.clone(),
                vendor_id: d.vid,
                product_id: d.pid,
                aaguid: None,
                pin_set: None,
                resident_keys: false,
                algorithms: vec![SkAlgorithm::EcdsaP256],
                versions: Vec::new(),
            };
            if let Ok(key) =
                FidoKeyHidFactory::create_by_params(std::slice::from_ref(&d.param), &lib_cfg())
                && let Ok(info) = key.get_info()
            {
                dev.aaguid = Some(hex::encode(&info.aaguid));
                dev.pin_set = info
                    .options
                    .iter()
                    .find(|(k, _)| k == "clientPin")
                    .map(|(_, v)| *v);
                dev.resident_keys = info.options.iter().any(|(k, v)| k == "rk" && *v);
                dev.versions = info.versions.clone();
                let has = |alg: &str| info.algorithms.iter().any(|(_, a)| a == alg);
                // Tokens that predate CTAP 2.1 do not advertise algorithms;
                // ECDSA P-256 is mandatory, Ed25519 is opt-in.
                if has("-8") || has("EdDSA") {
                    dev.algorithms.push(SkAlgorithm::Ed25519);
                }
            }
            dev
        })
        .collect()
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

/// Create a new credential on the token and wrap it as an OpenSSH `sk-*`
/// key. Blocking: waits for the user to touch the token.
pub fn generate(opts: &GenerateOptions) -> Result<KeyMaterial> {
    let application = opts
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
    let device = open(opts.device.as_deref())?;
    let challenge: [u8; 32] = rand::random();
    let user_name = opts
        .user
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .unwrap_or("termoso");
    // OpenSSH uses an all-zero 32-byte user id for non-resident keys and
    // the user name for resident ones.
    let user_id: Vec<u8> = if opts.resident {
        let mut id = user_name.as_bytes().to_vec();
        id.resize(id.len().clamp(1, 64), 0);
        id
    } else {
        vec![0u8; 32]
    };
    let user_entity =
        PublicKeyCredentialUserEntity::new(Some(&user_id), Some(user_name), Some(user_name));

    let pin = opts.pin.as_deref().map(|p| p.as_str());
    let mut builder = MakeCredentialArgsBuilder::new(application, &challenge)
        .key_type(opts.algorithm.cose())
        .user_entity(&user_entity);
    if opts.resident {
        builder = builder.resident_key();
    }
    builder = match pin {
        Some(p) if !p.is_empty() => builder.pin(p),
        _ => builder.without_pin_and_uv(),
    };
    let attestation = device
        .make_credential_with_args(&builder.build())
        .map_err(|e| Fido2Error::from_ctap(&e))?;

    let mut flags = 0u8;
    if opts.user_presence {
        flags |= FLAG_USER_PRESENCE;
    }
    if opts.user_verification {
        flags |= FLAG_USER_VERIFICATION;
    }
    if opts.resident {
        flags |= FLAG_RESIDENT;
    }
    let key = sk_private_key(
        opts.algorithm,
        &attestation,
        application,
        flags,
        &opts.comment,
    )?;
    material(&key, opts.passphrase.as_deref().map(|p| p.as_str()))
}

fn sk_private_key(
    algorithm: SkAlgorithm,
    att: &Attestation,
    application: &str,
    flags: u8,
    comment: &str,
) -> Result<PrivateKey> {
    let handle = att.credential_descriptor.id.clone();
    if handle.is_empty() {
        return Err(Fido2Error::Other("token returned an empty credential id".into()).into());
    }
    let der = &att.credential_publickey.der;
    let data = match (algorithm, att.credential_publickey.key_type.clone()) {
        (SkAlgorithm::Ed25519, PublicKeyType::Ed25519) => {
            let raw = der
                .len()
                .checked_sub(32)
                .and_then(|i| der.get(i..))
                .ok_or_else(|| Fido2Error::Other("bad Ed25519 public key".into()))?;
            let pk = Ed25519PublicKey::try_from(raw)?;
            KeypairData::SkEd25519(SkEd25519::new(
                public::SkEd25519::new(pk, application),
                flags,
                handle,
            )?)
        }
        (SkAlgorithm::EcdsaP256, PublicKeyType::Ecdsa256) => {
            let raw = der
                .len()
                .checked_sub(65)
                .and_then(|i| der.get(i..))
                .filter(|p| p.first() == Some(&0x04))
                .ok_or_else(|| Fido2Error::Other("bad P-256 public key".into()))?;
            let point = EncodedPoint::<U32>::from_bytes(raw)
                .map_err(|_| Fido2Error::Other("bad P-256 point".into()))?;
            KeypairData::SkEcdsaSha2NistP256(SkEcdsaSha2NistP256::new(
                public::SkEcdsaSha2NistP256::new(point, application),
                flags,
                handle,
            )?)
        }
        (want, got) => {
            return Err(Fido2Error::Unsupported(format!(
                "asked for {want:?}, token returned {got:?}"
            ))
            .into());
        }
    };
    Ok(PrivateKey::new(data, comment)?)
}

/// Resident credentials on the token for the `ssh:` application, wrapped as
/// OpenSSH keys (`ssh-keygen -K`). Needs the PIN. Blocking.
pub fn load_resident(
    device: Option<&str>,
    pin: &str,
    passphrase: Option<&str>,
) -> Result<Vec<KeyMaterial>> {
    let dev = open(device)?;
    let rps = dev
        .credential_management_enumerate_rps(Some(pin))
        .map_err(|e| Fido2Error::from_ctap(&e))?;
    let mut out = Vec::new();
    for rp in rps {
        let application = rp.public_key_credential_rp_entity.id.clone();
        if !application.starts_with("ssh:") {
            continue;
        }
        let creds = dev
            .credential_management_enumerate_credentials(Some(pin), &rp.rpid_hash)
            .map_err(|e| Fido2Error::from_ctap(&e))?;
        for c in creds {
            let (algorithm, key_type) = match c.public_key.key_type {
                PublicKeyType::Ed25519 => (SkAlgorithm::Ed25519, PublicKeyType::Ed25519),
                PublicKeyType::Ecdsa256 => (SkAlgorithm::EcdsaP256, PublicKeyType::Ecdsa256),
                PublicKeyType::Unknown => continue,
            };
            let att = Attestation {
                credential_descriptor: c.public_key_credential_descriptor.clone(),
                credential_publickey: ctap_hid_fido2::public_key::PublicKey::with_der(
                    &c.public_key.der,
                    key_type,
                ),
                ..Default::default()
            };
            let user = c.public_key_credential_user_entity.name.clone();
            let flags = FLAG_USER_PRESENCE | FLAG_RESIDENT;
            let key = sk_private_key(algorithm, &att, &application, flags, &user)?;
            out.push(material(&key, passphrase)?);
        }
    }
    Ok(out)
}

/// Sign `data` with the security key behind `key` (an `sk-*` OpenSSH key:
/// public part, flags and handle). Returns the SSH signature blob
/// (`string alg, string sig, byte flags, uint32 counter`). Blocking: waits
/// for the touch.
pub fn sign(
    key: &PrivateKey,
    data: &[u8],
    pin: Option<&str>,
    device: Option<&str>,
) -> Result<Vec<u8>> {
    let (application, handle, flags, alg) = match key.key_data() {
        KeypairData::SkEd25519(sk) => (
            sk.public().application().to_string(),
            sk.key_handle().to_vec(),
            sk.flags(),
            Algorithm::SkEd25519,
        ),
        KeypairData::SkEcdsaSha2NistP256(sk) => (
            sk.public().application().to_string(),
            sk.key_handle().to_vec(),
            sk.flags(),
            Algorithm::SkEcdsaSha2NistP256,
        ),
        _ => return Err(CoreError::Key("not a security key".into())),
    };
    let dev = open(device)?;
    // The library hashes the challenge (SHA-256) into clientDataHash, which
    // is exactly what OpenSSH signs: `message = SHA256(data)`.
    let mut builder = GetAssertionArgsBuilder::new(&application, data).credential_id(&handle);
    let wants_uv = flags & FLAG_USER_VERIFICATION != 0;
    builder = match pin {
        Some(p) if !p.is_empty() => builder.pin(p),
        _ if wants_uv => return Err(Fido2Error::PinRequired.into()),
        _ => builder.without_pin_and_uv(),
    };
    if flags & FLAG_USER_PRESENCE == 0 {
        builder = builder.without_up();
    }
    let assertions = dev
        .get_assertion_with_args(&builder.build())
        .map_err(|e| Fido2Error::from_ctap(&e))?;
    let assertion = assertions
        .into_iter()
        .next()
        .ok_or_else(|| Fido2Error::Other("token returned no assertion".into()))?;
    signature_blob(alg, &assertion)
}

fn signature_blob(alg: Algorithm, a: &Assertion) -> Result<Vec<u8>> {
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

/// [`russh::auth::Signer`] that signs with the security key. Used by
/// `authenticate_publickey_with` so `russh` never sees private material
/// (there is none to see).
pub struct SkSigner {
    /// The `sk-*` key (public part + handle).
    pub key: Arc<PrivateKey>,
    /// Client PIN when the key requires user verification (or the token
    /// insists).
    pub pin: Option<Zeroizing<String>>,
    /// Restrict to one device; `None` = the only connected one.
    pub device: Option<String>,
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
        let device = self.device.clone();
        if let Some(f) = &self.on_touch {
            f();
        }
        async move {
            let signed = tokio::task::spawn_blocking(move || {
                let blob = sign(
                    &key,
                    &to_sign,
                    pin.as_deref().map(|p| p.as_str()),
                    device.as_deref(),
                )?;
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
    fn from_byte(flags: u8) -> Self {
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

    fn fake_attestation(alg: SkAlgorithm) -> Attestation {
        let mut att = Attestation::default();
        att.credential_descriptor.id = vec![7u8; 40];
        att.credential_publickey = match alg {
            SkAlgorithm::Ed25519 => {
                // SPKI header (12 bytes) + 32-byte key.
                let mut der = vec![
                    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
                ];
                der.extend_from_slice(&[9u8; 32]);
                ctap_hid_fido2::public_key::PublicKey::with_der(&der, PublicKeyType::Ed25519)
            }
            SkAlgorithm::EcdsaP256 => {
                // Generator point of P-256 so the SEC1 decode is happy.
                let g = hex::decode(
                    "046B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296\
                     4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5",
                )
                .unwrap();
                let mut der = vec![0u8; 26];
                der.extend_from_slice(&g);
                ctap_hid_fido2::public_key::PublicKey::with_der(&der, PublicKeyType::Ecdsa256)
            }
        };
        att
    }

    #[test]
    fn sk_keys_round_trip_through_openssh_format() {
        for (alg, name) in [
            (SkAlgorithm::Ed25519, "sk-ssh-ed25519@openssh.com"),
            (SkAlgorithm::EcdsaP256, "sk-ecdsa-sha2-nistp256@openssh.com"),
        ] {
            let att = fake_attestation(alg);
            let key =
                sk_private_key(alg, &att, "ssh:", FLAG_USER_PRESENCE | FLAG_RESIDENT, "c").unwrap();
            assert_eq!(key.algorithm().as_str(), name);
            let m = material(&key, Some("pw")).unwrap();
            assert!(m.info.encrypted);
            assert!(m.public_key.starts_with(name));
            let back = crate::ssh::load_private_key(&m.private_key, Some("pw")).unwrap();
            assert!(is_security_key(back.public_key()));
            match back.key_data() {
                KeypairData::SkEd25519(sk) => {
                    assert_eq!(sk.key_handle(), &[7u8; 40]);
                    assert_eq!(sk.flags(), FLAG_USER_PRESENCE | FLAG_RESIDENT);
                }
                KeypairData::SkEcdsaSha2NistP256(sk) => {
                    assert_eq!(sk.key_handle(), &[7u8; 40]);
                    assert_eq!(sk.public().application(), "ssh:");
                }
                _ => panic!("wrong key data"),
            }
            let _ = key.to_openssh(LineEnding::LF).unwrap();
        }
    }

    #[test]
    fn describe_reads_flags_and_handle_only_when_decryptable() {
        let att = fake_attestation(SkAlgorithm::Ed25519);
        let key = sk_private_key(
            SkAlgorithm::Ed25519,
            &att,
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
            let att = fake_attestation(alg);
            let key = sk_private_key(alg, &att, "ssh:", FLAG_USER_PRESENCE, "c").unwrap();
            // The private half is the public key data, one flags byte, the
            // handle and the empty reserved field — no scalar anywhere.
            let public_len = key.public_key().key_data().encoded_len().unwrap();
            let private_len = key.key_data().encoded_len().unwrap();
            assert_eq!(private_len, public_len + 1 + (4 + 40) + 4);
            let handle = match key.key_data() {
                KeypairData::SkEd25519(sk) => sk.key_handle(),
                KeypairData::SkEcdsaSha2NistP256(sk) => sk.key_handle(),
                other => panic!("not a security key: {other:?}"),
            };
            assert_eq!(handle, &[7u8; 40][..]);
        }
    }

    #[test]
    fn wrong_algorithm_from_token_is_rejected() {
        let att = fake_attestation(SkAlgorithm::Ed25519);
        let err = sk_private_key(SkAlgorithm::EcdsaP256, &att, "ssh:", 1, "").unwrap_err();
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
        let a = Assertion {
            signature: vec![5u8; 64],
            auth_data,
            ..Default::default()
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
    fn ctap_status_mapping() {
        let e = "response_status err = 0x36 CTAP2_ERR_PIN_REQUIRED  PIN is required";
        assert_eq!(Fido2Error::from_ctap(&e), Fido2Error::PinRequired);
        let e = "response_status err = 0x31 CTAP2_ERR_PIN_INVALID   PIN Invalid.";
        assert_eq!(
            Fido2Error::from_ctap(&e),
            Fido2Error::PinInvalid { retries: None }
        );
        let e = "FIDO device not found.";
        assert_eq!(Fido2Error::from_ctap(&e), Fido2Error::NoDevice);
    }

    #[test]
    fn no_device_is_typed() {
        // No token in CI: listing is empty and generate says so honestly.
        let opts = GenerateOptions {
            device: None,
            algorithm: SkAlgorithm::EcdsaP256,
            application: None,
            resident: false,
            user_presence: true,
            user_verification: false,
            pin: None,
            user: None,
            comment: String::new(),
            passphrase: None,
        };
        if list_devices().is_empty() {
            assert!(matches!(
                generate(&opts),
                Err(CoreError::Fido2(Fido2Error::NoDevice))
            ));
        }
    }
}
