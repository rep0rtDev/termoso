//! Desktop backend: CTAP2 over USB HID through `hidapi` (the
//! `ctap-hid-fido2` crate). Blocking – callers run it off the async runtime.

use ctap_hid_fido2::fidokey::get_assertion::get_assertion_params::Assertion;
use ctap_hid_fido2::fidokey::make_credential::make_credential_params::{
    Attestation, CredentialSupportedKeyType,
};
use ctap_hid_fido2::fidokey::{GetAssertionArgsBuilder, MakeCredentialArgsBuilder};
use ctap_hid_fido2::public_key::PublicKeyType;
use ctap_hid_fido2::public_key_credential_user_entity::PublicKeyCredentialUserEntity;
use ctap_hid_fido2::{FidoKeyHid, FidoKeyHidFactory, HidInfo, HidParam, LibCfg};
use russh::keys::ssh_key::PrivateKey;

use super::{
    AssertionData, CosePublicKey, Credential, FLAG_RESIDENT, FLAG_USER_PRESENCE, Fido2Device,
    Fido2Error, GenerateOptions, SkAlgorithm, SkBackend, SkHandle, signature_blob, sk_material,
};
use crate::error::Result;
use crate::keys::KeyMaterial;

impl Fido2Error {
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
            Some(0x26) | Some(0x2B) | Some(0x2C) => Fido2Error::Unsupported(text),
            Some(0x3B) => Fido2Error::PinRequired,
            Some(code) => match Fido2Error::from_status(code) {
                Fido2Error::Other(_) => Fido2Error::Other(text),
                typed => typed,
            },
            None if text.contains("not found") || text.contains("Failed to open device") => {
                Fido2Error::NoDevice
            }
            None => Fido2Error::Other(text),
        }
    }
}

impl SkAlgorithm {
    fn cose(self) -> CredentialSupportedKeyType {
        match self {
            SkAlgorithm::Ed25519 => CredentialSupportedKeyType::Ed25519,
            SkAlgorithm::EcdsaP256 => CredentialSupportedKeyType::Ecdsa256,
        }
    }
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

/// The library's attestation / enumerated credential as our [`Credential`].
fn credential(id: &[u8], der: &[u8], key_type: &PublicKeyType) -> Result<Credential> {
    let public_key = match key_type {
        PublicKeyType::Ed25519 => {
            let raw = der
                .len()
                .checked_sub(32)
                .and_then(|i| der.get(i..))
                .ok_or_else(|| Fido2Error::Other("bad Ed25519 public key".into()))?;
            CosePublicKey::Ed25519(raw.try_into().expect("32 bytes"))
        }
        PublicKeyType::Ecdsa256 => {
            let raw = der
                .len()
                .checked_sub(65)
                .and_then(|i| der.get(i..))
                .filter(|p| p.first() == Some(&0x04))
                .ok_or_else(|| Fido2Error::Other("bad P-256 public key".into()))?;
            CosePublicKey::P256 {
                x: raw[1..33].try_into().expect("32 bytes"),
                y: raw[33..65].try_into().expect("32 bytes"),
            }
        }
        PublicKeyType::Unknown => {
            return Err(Fido2Error::Unsupported("unknown public key type".into()).into());
        }
    };
    Ok(Credential {
        id: id.to_vec(),
        public_key,
    })
}

fn attested(att: &Attestation) -> Result<Credential> {
    credential(
        &att.credential_descriptor.id,
        &att.credential_publickey.der,
        &att.credential_publickey.key_type,
    )
}

/// Create a new credential on the token and wrap it as an OpenSSH `sk-*`
/// key. Blocking: waits for the user to touch the token.
pub fn generate(opts: &GenerateOptions) -> Result<KeyMaterial> {
    let application = opts.application()?;
    let device = open(opts.device.as_deref())?;
    let challenge: [u8; 32] = rand::random();
    let user_name = opts.user_name();
    let user_id = opts.user_id();
    let user_entity =
        PublicKeyCredentialUserEntity::new(Some(&user_id), Some(user_name), Some(user_name));

    let mut builder = MakeCredentialArgsBuilder::new(application, &challenge)
        .key_type(opts.algorithm.cose())
        .user_entity(&user_entity);
    if opts.resident {
        builder = builder.resident_key();
    }
    builder = match opts.pin() {
        Some(p) => builder.pin(p),
        None => builder.without_pin_and_uv(),
    };
    let attestation = device
        .make_credential_with_args(&builder.build())
        .map_err(|e| Fido2Error::from_ctap(&e))?;
    sk_material(
        opts.algorithm,
        &attested(&attestation)?,
        application,
        opts.flags(),
        &opts.comment,
        opts.passphrase.as_deref().map(|p| p.as_str()),
    )
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
            let algorithm = match c.public_key.key_type {
                PublicKeyType::Ed25519 => SkAlgorithm::Ed25519,
                PublicKeyType::Ecdsa256 => SkAlgorithm::EcdsaP256,
                PublicKeyType::Unknown => continue,
            };
            let cred = credential(
                &c.public_key_credential_descriptor.id,
                &c.public_key.der,
                &c.public_key.key_type,
            )?;
            let user = c.public_key_credential_user_entity.name.clone();
            let flags = FLAG_USER_PRESENCE | FLAG_RESIDENT;
            out.push(sk_material(
                algorithm,
                &cred,
                &application,
                flags,
                &user,
                passphrase,
            )?);
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
    let handle = SkHandle::of(key)?;
    let dev = open(device)?;
    // The library hashes the challenge (SHA-256) into clientDataHash, which
    // is exactly what OpenSSH signs: `message = SHA256(data)`.
    let mut builder =
        GetAssertionArgsBuilder::new(&handle.application, data).credential_id(&handle.key_handle);
    builder = match pin {
        Some(p) if !p.is_empty() => builder.pin(p),
        _ if handle.wants_uv() => return Err(Fido2Error::PinRequired.into()),
        _ => builder.without_pin_and_uv(),
    };
    if !handle.wants_up() {
        builder = builder.without_up();
    }
    let assertions = dev
        .get_assertion_with_args(&builder.build())
        .map_err(|e| Fido2Error::from_ctap(&e))?;
    let assertion: Assertion = assertions
        .into_iter()
        .next()
        .ok_or_else(|| Fido2Error::Other("token returned no assertion".into()))?;
    signature_blob(
        handle.algorithm,
        &AssertionData {
            auth_data: assertion.auth_data,
            signature: assertion.signature,
        },
    )
}

/// [`SkBackend`] over the plugged-in USB tokens.
#[derive(Debug, Clone, Default)]
pub struct UsbBackend {
    /// Restrict to one device (path from [`list_devices`]); `None` = the
    /// only connected one.
    pub device: Option<String>,
}

impl SkBackend for UsbBackend {
    fn sign(&self, key: &PrivateKey, data: &[u8], pin: Option<&str>) -> Result<Vec<u8>> {
        sign(key, data, pin, self.device.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                Err(crate::error::CoreError::Fido2(Fido2Error::NoDevice))
            ));
        }
    }
}
