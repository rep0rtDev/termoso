//! FIDO2 security keys on the phone. Kotlin owns the hardware handles (a
//! `UsbDeviceConnection` on the CTAPHID interface, an NFC `IsoDep` tag) and
//! hands them to Rust as a [`Fido2UsbLink`] / [`Fido2NfcLink`]: raw 64-byte
//! reports or APDUs cross the bridge, nothing else. CTAP2, PIN protocol,
//! credential handling and the OpenSSH `sk-*` wrapping all happen here, so
//! the PIN and the key handle never sit in Kotlin memory longer than the
//! text field that collected them.
//!
//! Attached tokens live in one process-wide [`Fido2Devices`] registry; the
//! SSH connection path reaches them through [`PhoneBackend`].

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::Duration;

use russh::keys::ssh_key::PrivateKey;
use termoso_client::keychain;
use termoso_core::error::CoreError;
use termoso_core::fido2::ctap::{self, Authenticator};
use termoso_core::fido2::hid::{HidPackets, HidTransport};
use termoso_core::fido2::nfc::{Apdu, NfcTransport};
use termoso_core::fido2::webauthn;
use termoso_core::fido2::{
    Fido2Device, Fido2Error, GenerateOptions, SecurityKeyInfo, SkAlgorithm, SkBackend,
};
use termoso_core::keys::KeyMaterial;
use termoso_core::model::SshKey;
use termoso_core::store::Store;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::dto::KeyItem;
use crate::error::{MobileError, Result};

// ---------------------------------------------------------------------------
// Foreign transports

/// A CTAPHID interface Kotlin opened with the USB host API. Both calls
/// block on the bulk/interrupt transfer and are made from a Rust worker
/// thread, never the main thread.
#[uniffi::export(with_foreign)]
pub trait Fido2UsbLink: Send + Sync {
    /// Send one 64-byte report on the interrupt OUT endpoint.
    fn write_report(&self, packet: Vec<u8>) -> Result<()>;
    /// Wait up to `timeout_ms` for a report on the interrupt IN endpoint;
    /// `None` when nothing arrived in time.
    fn read_report(&self, timeout_ms: u64) -> Result<Option<Vec<u8>>>;
}

/// An ISO-DEP tag Kotlin is holding (`IsoDep.connect()` done).
#[uniffi::export(with_foreign)]
pub trait Fido2NfcLink: Send + Sync {
    /// `IsoDep.transceive`: one command APDU in, response APDU (with the
    /// status word) out.
    fn transceive(&self, apdu: Vec<u8>) -> Result<Vec<u8>>;
    /// `IsoDep.isExtendedLengthApduSupported()`.
    fn extended_length(&self) -> bool;
}

/// Progress of a token operation Kotlin started.
#[uniffi::export(with_foreign)]
pub trait Fido2Listener: Send + Sync {
    /// The token is about to ask for a touch; show the hint.
    fn on_touch(&self);
}

struct UsbPackets(Arc<dyn Fido2UsbLink>);

impl HidPackets for UsbPackets {
    fn write(&mut self, packet: &[u8]) -> std::result::Result<(), Fido2Error> {
        self.0
            .write_report(packet.to_vec())
            .map_err(|e| Fido2Error::DeviceGone(e.to_string()))
    }

    fn read(&mut self, timeout: Duration) -> std::result::Result<Option<Vec<u8>>, Fido2Error> {
        self.0
            .read_report(timeout.as_millis().min(u64::MAX as u128) as u64)
            .map_err(|e| Fido2Error::DeviceGone(e.to_string()))
    }
}

struct NfcApdu(Arc<dyn Fido2NfcLink>);

impl Apdu for NfcApdu {
    fn transceive(&mut self, apdu: &[u8]) -> std::result::Result<Vec<u8>, Fido2Error> {
        self.0
            .transceive(apdu.to_vec())
            .map_err(|e| Fido2Error::DeviceGone(e.to_string()))
    }

    fn extended_length(&self) -> bool {
        self.0.extended_length()
    }
}

#[derive(Clone)]
enum Link {
    Usb(Arc<dyn Fido2UsbLink>),
    Nfc(Arc<dyn Fido2NfcLink>),
}

/// One open authenticator, whichever wire it hangs off.
enum Auth {
    Usb(Authenticator<HidTransport<UsbPackets>>),
    Nfc(Authenticator<NfcTransport<NfcApdu>>),
}

impl Auth {
    fn open(link: &Link) -> std::result::Result<Self, Fido2Error> {
        Ok(match link {
            Link::Usb(l) => Auth::Usb(Authenticator::open(HidTransport::open(UsbPackets(
                l.clone(),
            ))?)?),
            Link::Nfc(l) => Auth::Nfc(Authenticator::open(NfcTransport::open(NfcApdu(
                l.clone(),
            ))?)?),
        })
    }

    fn info(&self) -> &ctap::Info {
        match self {
            Auth::Usb(a) => a.info(),
            Auth::Nfc(a) => a.info(),
        }
    }

    fn generate(&mut self, opts: &GenerateOptions) -> termoso_core::error::Result<KeyMaterial> {
        match self {
            Auth::Usb(a) => ctap::generate_with(a, opts),
            Auth::Nfc(a) => ctap::generate_with(a, opts),
        }
    }

    fn sign(
        &mut self,
        key: &PrivateKey,
        data: &[u8],
        pin: Option<&str>,
    ) -> termoso_core::error::Result<Vec<u8>> {
        match self {
            Auth::Usb(a) => ctap::sign_with(a, key, data, pin),
            Auth::Nfc(a) => ctap::sign_with(a, key, data, pin),
        }
    }

    fn load_resident(
        &mut self,
        pin: &str,
        passphrase: Option<&str>,
    ) -> termoso_core::error::Result<Vec<KeyMaterial>> {
        match self {
            Auth::Usb(a) => ctap::load_resident_with(a, pin, passphrase),
            Auth::Nfc(a) => ctap::load_resident_with(a, pin, passphrase),
        }
    }

    fn pin_retries(&mut self) -> Option<i32> {
        match self {
            Auth::Usb(a) => a.pin_retries(),
            Auth::Nfc(a) => a.pin_retries(),
        }
    }

    fn webauthn_assert(
        &mut self,
        options: &webauthn::RequestOptions,
        origin: &str,
        pin: Option<&str>,
    ) -> termoso_core::error::Result<serde_json::Value> {
        Ok(match self {
            Auth::Usb(a) => webauthn::assert(a, options, origin, pin)?,
            Auth::Nfc(a) => webauthn::assert(a, options, origin, pin)?,
        })
    }

    fn webauthn_register(
        &mut self,
        options: &webauthn::CreationOptions,
        origin: &str,
        pin: Option<&str>,
    ) -> termoso_core::error::Result<serde_json::Value> {
        Ok(match self {
            Auth::Usb(a) => webauthn::register(a, options, origin, "usb", pin)?,
            Auth::Nfc(a) => webauthn::register(a, options, origin, "nfc", pin)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Records for Kotlin

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Fido2Transport {
    Usb,
    Nfc,
}

/// An attached token as the picker shows it. Everything here is what the
/// token itself advertises in `getInfo` plus the USB descriptor.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Fido2DeviceCard {
    /// Id Kotlin chose when attaching (`usb:<deviceName>`, `nfc:<tag>`).
    pub id: String,
    pub transport: Fido2Transport,
    /// Product name from the USB descriptor (or what Kotlin passed for NFC).
    pub product: String,
    pub vendor_id: u16,
    pub product_id: u16,
    /// Authenticator AAGUID (hex), when the token reports one.
    pub aaguid: Option<String>,
    /// A client PIN is set; `None` when the token has no PIN support at all.
    pub pin_set: Option<bool>,
    /// Can store resident (discoverable) credentials.
    pub resident_keys: bool,
    /// Can make Ed25519 credentials (every token does P-256).
    pub ed25519: bool,
    /// CTAP versions (`FIDO_2_0`, `FIDO_2_1`, `U2F_V2`…).
    pub versions: Vec<String>,
}

fn card_of(id: &str, transport: Fido2Transport, d: Fido2Device) -> Fido2DeviceCard {
    Fido2DeviceCard {
        id: id.to_string(),
        transport,
        product: d.product,
        vendor_id: d.vendor_id,
        product_id: d.product_id,
        aaguid: d.aaguid,
        pin_set: d.pin_set,
        resident_keys: d.resident_keys,
        ed25519: d.algorithms.contains(&SkAlgorithm::Ed25519),
        versions: d.versions,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SkKeyAlgorithm {
    Ed25519,
    EcdsaP256,
}

impl From<SkKeyAlgorithm> for SkAlgorithm {
    fn from(a: SkKeyAlgorithm) -> Self {
        match a {
            SkKeyAlgorithm::Ed25519 => SkAlgorithm::Ed25519,
            SkKeyAlgorithm::EcdsaP256 => SkAlgorithm::EcdsaP256,
        }
    }
}

/// `ssh-keygen -t ed25519-sk …` on the phone.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Fido2GenerateDraft {
    pub vault_id: String,
    /// Token from [`Fido2Devices::list`]; `None` = the only attached one.
    pub device_id: Option<String>,
    pub label: String,
    pub algorithm: SkKeyAlgorithm,
    /// FIDO application, `ssh:` when empty (`-O application=`).
    pub application: String,
    /// Store the credential on the token (`-O resident`).
    pub resident: bool,
    /// Touch at every signature (OpenSSH default: on).
    pub user_presence: bool,
    /// PIN at every signature (`-O verify-required`).
    pub user_verification: bool,
    /// Client PIN; needed when the token has one set and always for
    /// resident / verify-required keys.
    pub pin: Option<String>,
    /// User name recorded with a resident credential (`-O user=`).
    pub user: String,
    pub comment: String,
    /// Passphrase protecting the stored handle, like OpenSSH does.
    pub passphrase: Option<String>,
    pub remember_passphrase: bool,
}

/// `ssh-keygen -K`: pull the resident SSH credentials off a token.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Fido2LoadDraft {
    pub vault_id: String,
    pub device_id: Option<String>,
    pub pin: String,
    pub passphrase: Option<String>,
    pub remember_passphrase: bool,
}

/// What a stored `sk-*` key requires — the key detail card. Nothing here
/// is secret: the "private key" of a security key is the public key plus
/// the credential handle; the token keeps the signing key.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SecurityKeyCard {
    /// FIDO application (`ssh:`…).
    pub application: String,
    /// `None` when the handle is passphrase-protected and the passphrase is
    /// not remembered — the flags live inside the encrypted block.
    pub resident: Option<bool>,
    pub user_presence: Option<bool>,
    pub user_verification: Option<bool>,
    /// Credential id (hex); `None` while the handle is encrypted.
    pub credential_id: Option<String>,
}

impl From<SecurityKeyInfo> for SecurityKeyCard {
    fn from(i: SecurityKeyInfo) -> Self {
        Self {
            application: i.application,
            resident: i.flags.map(|f| f.resident),
            user_presence: i.flags.map(|f| f.user_presence),
            user_verification: i.flags.map(|f| f.user_verification),
            credential_id: i.credential_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Registry

struct Attached {
    card: Fido2DeviceCard,
    link: Link,
    /// One CTAP exchange at a time per token.
    busy: Mutex<()>,
}

/// The tokens Kotlin has attached. One per process: a phone has one USB
/// port and one NFC field, and the SSH path must find the token whoever
/// started the connection.
#[derive(uniffi::Object)]
pub struct Fido2Devices {
    attached: Mutex<BTreeMap<String, Arc<Attached>>>,
}

static REGISTRY: LazyLock<Arc<Fido2Devices>> = LazyLock::new(|| {
    Arc::new(Fido2Devices {
        attached: Mutex::new(BTreeMap::new()),
    })
});

/// The process-wide registry.
pub fn registry() -> Arc<Fido2Devices> {
    REGISTRY.clone()
}

fn non_empty(s: Option<String>) -> Option<Zeroizing<String>> {
    s.filter(|s| !s.is_empty()).map(Zeroizing::new)
}

impl Fido2Devices {
    fn attached(&self) -> MutexGuard<'_, BTreeMap<String, Arc<Attached>>> {
        self.attached.lock().expect("fido2 registry poisoned")
    }

    fn insert(
        &self,
        id: String,
        transport: Fido2Transport,
        product: &str,
        vendor_id: u16,
        product_id: u16,
        link: Link,
    ) -> Result<Fido2DeviceCard> {
        if id.trim().is_empty() {
            return Err(MobileError::invalid("device id must not be empty"));
        }
        let auth = Auth::open(&link).map_err(CoreError::from)?;
        let card = card_of(
            &id,
            transport,
            auth.info().describe(&id, product, vendor_id, product_id),
        );
        self.attached().insert(
            id,
            Arc::new(Attached {
                card: card.clone(),
                link,
                busy: Mutex::new(()),
            }),
        );
        Ok(card)
    }

    /// The token to use: `device_id` when given, else the only one attached.
    fn pick(&self, device_id: Option<&str>) -> Result<Arc<Attached>> {
        let attached = self.attached();
        match device_id {
            Some(id) => attached
                .get(id)
                .cloned()
                .ok_or_else(|| CoreError::from(Fido2Error::DeviceGone(id.to_string())).into()),
            None => {
                let mut it = attached.values();
                match (it.next(), it.next()) {
                    (Some(a), None) => Ok(a.clone()),
                    (None, _) => Err(CoreError::from(Fido2Error::NoDevice).into()),
                    (Some(_), Some(_)) => Err(MobileError::invalid(
                        "several security keys attached: pick one",
                    )),
                }
            }
        }
    }

    /// Run one CTAP operation against `device`, holding its lock.
    fn with_device<R>(
        &self,
        device: &Attached,
        f: impl FnOnce(&mut Auth) -> termoso_core::error::Result<R>,
    ) -> termoso_core::error::Result<R> {
        let _busy = device.busy.lock().expect("fido2 device poisoned");
        let mut auth = Auth::open(&device.link)?;
        f(&mut auth)
    }

    /// Sign for the SSH path: the named token, or every attached one until
    /// one recognises the handle (what OpenSSH does with several tokens).
    pub(crate) fn sign(
        &self,
        device_id: Option<&str>,
        key: &PrivateKey,
        data: &[u8],
        pin: Option<&str>,
    ) -> termoso_core::error::Result<Vec<u8>> {
        let candidates: Vec<Arc<Attached>> = match device_id {
            Some(id) => vec![
                self.attached()
                    .get(id)
                    .cloned()
                    .ok_or_else(|| Fido2Error::DeviceGone(id.to_string()))?,
            ],
            None => self.attached().values().cloned().collect(),
        };
        if candidates.is_empty() {
            return Err(Fido2Error::NoDevice.into());
        }
        self.first_recognising(&candidates, |auth| auth.sign(key, data, pin))
    }

    /// The named token, or every attached one.
    fn candidates(
        &self,
        device_id: Option<&str>,
    ) -> termoso_core::error::Result<Vec<Arc<Attached>>> {
        let candidates: Vec<Arc<Attached>> = match device_id {
            Some(id) => vec![
                self.attached()
                    .get(id)
                    .cloned()
                    .ok_or_else(|| Fido2Error::DeviceGone(id.to_string()))?,
            ],
            None => self.attached().values().cloned().collect(),
        };
        if candidates.is_empty() {
            return Err(Fido2Error::NoDevice.into());
        }
        Ok(candidates)
    }

    /// Run `f` on each candidate until one does not answer "not my
    /// credential" (what OpenSSH does with several tokens).
    fn first_recognising<R>(
        &self,
        candidates: &[Arc<Attached>],
        mut f: impl FnMut(&mut Auth) -> termoso_core::error::Result<R>,
    ) -> termoso_core::error::Result<R> {
        let mut last = None;
        for device in candidates {
            match self.with_device(device, &mut f) {
                Err(CoreError::Fido2(Fido2Error::WrongDevice)) if candidates.len() > 1 => {
                    last = Some(CoreError::Fido2(Fido2Error::WrongDevice));
                }
                other => return other,
            }
        }
        Err(last.unwrap_or_else(|| Fido2Error::NoDevice.into()))
    }

    /// WebAuthn assertion for `options` (the relying party's request
    /// options JSON) as seen from `origin`: the named token, or whichever
    /// attached one holds one of the allowed credentials. Blocking.
    pub(crate) fn webauthn_assert(
        &self,
        device_id: Option<&str>,
        options: &serde_json::Value,
        origin: &str,
        pin: Option<&str>,
    ) -> termoso_core::error::Result<serde_json::Value> {
        let options = webauthn::RequestOptions::parse(options)?;
        let candidates = self.candidates(device_id)?;
        self.first_recognising(&candidates, |auth| {
            auth.webauthn_assert(&options, origin, pin)
        })
    }

    /// WebAuthn registration for `options` (the relying party's creation
    /// options JSON) as seen from `origin`. Blocking.
    pub(crate) fn webauthn_register(
        &self,
        device_id: Option<&str>,
        options: &serde_json::Value,
        origin: &str,
        pin: Option<&str>,
    ) -> Result<serde_json::Value> {
        let options = webauthn::CreationOptions::parse(options).map_err(CoreError::from)?;
        let device = self.pick(device_id)?;
        Ok(self.with_device(&device, |auth| {
            auth.webauthn_register(&options, origin, pin)
        })?)
    }
}

#[uniffi::export]
impl Fido2Devices {
    /// The registry (the same object every time).
    #[uniffi::constructor]
    pub fn shared() -> Arc<Self> {
        registry()
    }

    /// Register a USB token. Talks to it right away (CTAPHID INIT +
    /// getInfo), so call off the main thread; fails for U2F-only tokens.
    pub fn attach_usb(
        &self,
        id: String,
        product: String,
        vendor_id: u16,
        product_id: u16,
        link: Arc<dyn Fido2UsbLink>,
    ) -> Result<Fido2DeviceCard> {
        self.insert(
            id,
            Fido2Transport::Usb,
            &product,
            vendor_id,
            product_id,
            Link::Usb(link),
        )
    }

    /// Register an NFC token that is in the field right now. Selects the
    /// FIDO applet and reads getInfo; call off the main thread.
    pub fn attach_nfc(&self, id: String, link: Arc<dyn Fido2NfcLink>) -> Result<Fido2DeviceCard> {
        self.insert(
            id,
            Fido2Transport::Nfc,
            "NFC security key",
            0,
            0,
            Link::Nfc(link),
        )
    }

    /// Forget a token (USB detached, tag left the field, Activity gone).
    /// An operation already running on it fails with `fido2_device_gone`
    /// as soon as the link errors.
    pub fn detach(&self, id: String) {
        self.attached().remove(&id);
    }

    /// Forget every token.
    pub fn detach_all(&self) {
        self.attached().clear();
    }

    pub fn list(&self) -> Vec<Fido2DeviceCard> {
        self.attached().values().map(|a| a.card.clone()).collect()
    }

    /// PIN attempts left on a token, when it says. Blocking.
    pub fn pin_retries(&self, device_id: String) -> Result<Option<i32>> {
        let device = self.pick(Some(&device_id))?;
        Ok(self.with_device(&device, |auth| Ok(auth.pin_retries()))?)
    }
}

// ---------------------------------------------------------------------------
// Key operations (need the vault)

fn label_of(label: &str) -> Result<String> {
    let l = label.trim();
    if l.is_empty() {
        return Err(MobileError::invalid("label must not be empty"));
    }
    Ok(l.to_string())
}

/// Create a credential on the token and store the `sk-*` handle in the
/// vault. Blocking: waits for the touch. `listener.on_touch` fires just
/// before the token is asked.
pub(crate) fn generate(
    devices: &Fido2Devices,
    store: &Store,
    draft: Fido2GenerateDraft,
    listener: Option<Arc<dyn Fido2Listener>>,
) -> Result<KeyItem> {
    let vault_id = Uuid::parse_str(&draft.vault_id)?;
    let label = label_of(&draft.label)?;
    let passphrase = non_empty(draft.passphrase);
    let comment = if draft.comment.trim().is_empty() {
        label.clone()
    } else {
        draft.comment.trim().to_string()
    };
    let opts = GenerateOptions {
        device: draft.device_id.clone(),
        algorithm: draft.algorithm.into(),
        application: Some(draft.application).filter(|a| !a.trim().is_empty()),
        resident: draft.resident,
        user_presence: draft.user_presence,
        user_verification: draft.user_verification,
        pin: non_empty(draft.pin),
        user: Some(draft.user).filter(|u| !u.trim().is_empty()),
        comment,
        passphrase: passphrase.clone(),
    };
    // Validate before touching the token.
    opts.application()?;
    let device = devices.pick(draft.device_id.as_deref())?;
    if let Some(l) = &listener {
        l.on_touch();
    }
    let material = devices.with_device(&device, |auth| auth.generate(&opts))?;
    let passphrase = passphrase.as_deref().map(|p| p.as_str());
    Ok(keychain::store_security_key(
        store,
        vault_id,
        label,
        &material,
        passphrase,
        draft.remember_passphrase,
    )?
    .into())
}

/// Load the resident SSH credentials from a token into the vault
/// (`ssh-keygen -K`). Credentials already present are skipped. Blocking.
pub(crate) fn load_resident(
    devices: &Fido2Devices,
    store: &Store,
    draft: Fido2LoadDraft,
    listener: Option<Arc<dyn Fido2Listener>>,
) -> Result<Vec<KeyItem>> {
    let vault_id = Uuid::parse_str(&draft.vault_id)?;
    if draft.pin.is_empty() {
        return Err(CoreError::from(Fido2Error::PinRequired).into());
    }
    let passphrase = non_empty(draft.passphrase);
    let device = devices.pick(draft.device_id.as_deref())?;
    if let Some(l) = &listener {
        l.on_touch();
    }
    let pin = Zeroizing::new(draft.pin);
    let found = devices.with_device(&device, |auth| {
        auth.load_resident(&pin, passphrase.as_deref().map(|p| p.as_str()))
    })?;
    let existing: std::collections::HashSet<String> = store
        .list::<SshKey>(Some(vault_id))?
        .into_iter()
        .filter_map(|k| termoso_core::keys::inspect(&k.data.private_key).ok())
        .map(|i| i.fingerprint)
        .collect();
    let mut out = Vec::new();
    for (n, material) in found.iter().enumerate() {
        if existing.contains(&material.info.fingerprint) {
            continue;
        }
        let label = if material.info.comment.trim().is_empty() {
            format!("Resident key {}", n + 1)
        } else {
            material.info.comment.trim().to_string()
        };
        out.push(
            keychain::store_security_key(
                store,
                vault_id,
                label,
                material,
                passphrase.as_deref().map(|p| p.as_str()),
                draft.remember_passphrase,
            )?
            .into(),
        );
    }
    Ok(out)
}

/// Security-key facts about a stored key; `None` for ordinary keys.
pub(crate) fn describe(store: &Store, id: Uuid) -> Result<Option<SecurityKeyCard>> {
    let key = store.require::<SshKey>(id)?;
    Ok(
        termoso_core::fido2::describe(&key.data.private_key, key.data.passphrase.as_deref())
            .map(Into::into),
    )
}

// ---------------------------------------------------------------------------
// SSH backend

/// [`SkBackend`] over the registry: whatever token is attached when the
/// server asks for the signature.
#[derive(Debug, Clone, Default)]
pub(crate) struct PhoneBackend {
    /// Restrict to one token; `None` = try every attached one.
    pub device: Option<String>,
}

impl SkBackend for PhoneBackend {
    fn sign(
        &self,
        key: &PrivateKey,
        data: &[u8],
        pin: Option<&str>,
    ) -> termoso_core::error::Result<Vec<u8>> {
        registry().sign(self.device.as_deref(), key, data, pin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use termoso_core::fido2::soft::{HidWire, NfcTag, SoftToken, Touch};
    use termoso_crypto::keys::SymmetricKey;

    /// A USB link over the soft token's CTAPHID wire.
    struct UsbCable(Mutex<HidWire>);

    impl Fido2UsbLink for UsbCable {
        fn write_report(&self, packet: Vec<u8>) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .write(&packet)
                .map_err(CoreError::from)?;
            Ok(())
        }
        fn read_report(&self, timeout_ms: u64) -> Result<Option<Vec<u8>>> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .read(Duration::from_millis(timeout_ms))
                .map_err(CoreError::from)?)
        }
    }

    /// A link that fails like a yanked cable.
    struct Yanked;

    impl Fido2UsbLink for Yanked {
        fn write_report(&self, _packet: Vec<u8>) -> Result<()> {
            Err(MobileError::Other {
                kind: "io".into(),
                detail: "ENODEV".into(),
            })
        }
        fn read_report(&self, _timeout_ms: u64) -> Result<Option<Vec<u8>>> {
            Err(MobileError::Other {
                kind: "io".into(),
                detail: "ENODEV".into(),
            })
        }
    }

    /// An NFC link over the soft token's applet.
    struct Tag(Mutex<NfcTag>);

    impl Fido2NfcLink for Tag {
        fn transceive(&self, apdu: Vec<u8>) -> Result<Vec<u8>> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .transceive(&apdu)
                .map_err(CoreError::from)?)
        }
        fn extended_length(&self) -> bool {
            self.0.lock().unwrap().extended_length()
        }
    }

    #[derive(Default)]
    struct Touches(AtomicUsize);

    impl Fido2Listener for Touches {
        fn on_touch(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn fresh() -> Fido2Devices {
        Fido2Devices {
            attached: Mutex::new(BTreeMap::new()),
        }
    }

    fn usb(token: &SoftToken) -> Arc<dyn Fido2UsbLink> {
        Arc::new(UsbCable(Mutex::new(token.hid())))
    }

    fn nfc(token: &SoftToken) -> Arc<dyn Fido2NfcLink> {
        Arc::new(Tag(Mutex::new(token.nfc())))
    }

    fn store() -> (Store, String) {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let vault = s.local_vault().unwrap().id.to_string();
        (s, vault)
    }

    fn draft(vault: &str, device: &str) -> Fido2GenerateDraft {
        Fido2GenerateDraft {
            vault_id: vault.to_string(),
            device_id: Some(device.to_string()),
            label: "YubiKey".into(),
            algorithm: SkKeyAlgorithm::Ed25519,
            application: String::new(),
            resident: false,
            user_presence: true,
            user_verification: false,
            pin: Some("123456".into()),
            user: String::new(),
            comment: String::new(),
            passphrase: None,
            remember_passphrase: false,
        }
    }

    fn sk_error(e: MobileError) -> String {
        match e {
            MobileError::SecurityKey { kind, .. } => kind,
            other => panic!("expected a security-key error, got {other:?}"),
        }
    }

    #[test]
    fn attach_describes_and_detach_forgets() {
        let devices = fresh();
        let token = SoftToken::default();
        let card = devices
            .insert(
                "usb:1".into(),
                Fido2Transport::Usb,
                "Soft key",
                0x1050,
                0x0407,
                Link::Usb(usb(&token)),
            )
            .unwrap();
        assert_eq!(card.transport, Fido2Transport::Usb);
        assert_eq!((card.vendor_id, card.product_id), (0x1050, 0x0407));
        assert_eq!(card.pin_set, Some(true));
        assert!(card.resident_keys && card.ed25519);
        assert!(card.versions.iter().any(|v| v.starts_with("FIDO_2")));

        let nfc_card = devices
            .insert(
                "nfc:1".into(),
                Fido2Transport::Nfc,
                "NFC security key",
                0,
                0,
                Link::Nfc(nfc(&token)),
            )
            .unwrap();
        assert_eq!(nfc_card.transport, Fido2Transport::Nfc);
        assert_eq!(devices.list().len(), 2);
        assert!(matches!(
            devices.pick(None),
            Err(MobileError::Invalid { .. })
        ));

        devices.detach("nfc:1".into());
        assert_eq!(devices.list().len(), 1);
        assert_eq!(devices.pick(None).unwrap().card.id, "usb:1");
        assert_eq!(devices.pin_retries("usb:1".into()).unwrap(), Some(8));
        devices.detach_all();
        assert!(devices.list().is_empty());
        assert_eq!(
            sk_error(devices.pick(None).map(|_| ()).unwrap_err()),
            "fido2_no_device"
        );
        assert_eq!(
            sk_error(devices.pick(Some("usb:1")).map(|_| ()).unwrap_err()),
            "fido2_device_gone"
        );
    }

    #[test]
    fn attach_rejects_dead_link_and_empty_id() {
        let devices = fresh();
        let e = devices
            .insert(
                "usb:9".into(),
                Fido2Transport::Usb,
                "x",
                0,
                0,
                Link::Usb(Arc::new(Yanked)),
            )
            .unwrap_err();
        assert!(sk_error(e).starts_with("fido2_"));
        let token = SoftToken::default();
        assert!(matches!(
            devices.insert(
                "  ".into(),
                Fido2Transport::Usb,
                "x",
                0,
                0,
                Link::Usb(usb(&token))
            ),
            Err(MobileError::Invalid { .. })
        ));
        assert!(devices.list().is_empty());
    }

    #[test]
    fn generate_stores_handle_and_signs_over_usb_and_nfc() {
        let devices = fresh();
        let token = SoftToken::default();
        devices
            .insert(
                "usb:1".into(),
                Fido2Transport::Usb,
                "Soft",
                1,
                2,
                Link::Usb(usb(&token)),
            )
            .unwrap();
        devices
            .insert(
                "nfc:1".into(),
                Fido2Transport::Nfc,
                "Soft",
                0,
                0,
                Link::Nfc(nfc(&token)),
            )
            .unwrap();
        let (s, vault) = store();
        let touches = Arc::new(Touches::default());

        let ed = generate(&devices, &s, draft(&vault, "usb:1"), Some(touches.clone())).unwrap();
        assert_eq!(ed.key_type, "sk-ssh-ed25519@openssh.com");
        assert!(ed.security_key && !ed.encrypted);
        assert!(ed.public_key.starts_with("sk-ssh-ed25519@openssh.com "));
        assert_eq!(touches.0.load(Ordering::SeqCst), 1);

        let mut d = draft(&vault, "nfc:1");
        d.label = "ECDSA".into();
        d.algorithm = SkKeyAlgorithm::EcdsaP256;
        d.passphrase = Some("pp".into());
        d.remember_passphrase = true;
        let ec = generate(&devices, &s, d, None).unwrap();
        assert_eq!(ec.key_type, "sk-ecdsa-sha2-nistp256@openssh.com");
        assert!(ec.encrypted && ec.has_passphrase);
        assert_eq!(token.credential_count(), 2);

        // The vault holds a handle, never the scalar: describe() sees it.
        let info = describe(&s, Uuid::parse_str(&ed.id).unwrap())
            .unwrap()
            .unwrap();
        assert!(info.credential_id.is_some());
        assert_eq!(info.application, "ssh:");
        assert_eq!(info.user_presence, Some(true));
        assert_eq!(info.user_verification, Some(false));
        assert_eq!(info.resident, Some(false));

        // Sign with each token through the registry path used by SSH.
        let key = s
            .require::<SshKey>(Uuid::parse_str(&ed.id).unwrap())
            .unwrap();
        let pk = PrivateKey::from_openssh(&key.data.private_key).unwrap();
        let sig = devices.sign(Some("nfc:1"), &pk, b"hello", None).unwrap();
        assert!(!sig.is_empty());
        let sig2 = devices.sign(None, &pk, b"hello", None).unwrap();
        assert!(!sig2.is_empty());

        // Ordinary keys are not security keys.
        let plain = keychain::generate(
            &s,
            &keychain::GenerateForm {
                vault_id: Uuid::parse_str(&vault).unwrap(),
                label: "plain".into(),
                algorithm: termoso_core::keys::KeyAlgorithm::Ed25519,
                comment: String::new(),
                passphrase: None,
                remember_passphrase: false,
            },
        )
        .unwrap();
        assert!(describe(&s, plain.id).unwrap().is_none());
        assert!(!KeyItem::from(plain).security_key);
    }

    #[test]
    fn sign_skips_strangers_and_reports_missing_token() {
        let devices = fresh();
        let mine = SoftToken::default();
        let stranger = SoftToken::default();
        devices
            .insert(
                "usb:a".into(),
                Fido2Transport::Usb,
                "A",
                0,
                0,
                Link::Usb(usb(&stranger)),
            )
            .unwrap();
        devices
            .insert(
                "usb:b".into(),
                Fido2Transport::Usb,
                "B",
                0,
                0,
                Link::Usb(usb(&mine)),
            )
            .unwrap();
        let (s, vault) = store();
        let item = generate(&devices, &s, draft(&vault, "usb:b"), None).unwrap();
        let key = s
            .require::<SshKey>(Uuid::parse_str(&item.id).unwrap())
            .unwrap();
        let pk = PrivateKey::from_openssh(&key.data.private_key).unwrap();

        // No device named: the stranger says WrongDevice, ours signs.
        devices.sign(None, &pk, b"x", None).unwrap();
        assert_eq!(
            sk_error(
                devices
                    .sign(Some("usb:a"), &pk, b"x", None)
                    .unwrap_err()
                    .into()
            ),
            "fido2_wrong_device"
        );
        devices.detach("usb:b".into());
        assert_eq!(
            sk_error(devices.sign(None, &pk, b"x", None).unwrap_err().into()),
            "fido2_wrong_device"
        );
        devices.detach_all();
        assert_eq!(
            sk_error(devices.sign(None, &pk, b"x", None).unwrap_err().into()),
            "fido2_no_device"
        );
        assert_eq!(
            sk_error(
                devices
                    .sign(Some("usb:b"), &pk, b"x", None)
                    .unwrap_err()
                    .into()
            ),
            "fido2_device_gone"
        );
    }

    #[test]
    fn pin_and_touch_errors_are_typed() {
        let devices = fresh();
        let token = SoftToken::default();
        devices
            .insert(
                "usb:1".into(),
                Fido2Transport::Usb,
                "Soft",
                0,
                0,
                Link::Usb(usb(&token)),
            )
            .unwrap();
        let (s, vault) = store();

        // Verification without a PIN: the token wants one.
        let mut d = draft(&vault, "usb:1");
        d.user_verification = true;
        d.pin = None;
        assert_eq!(
            sk_error(generate(&devices, &s, d, None).unwrap_err()),
            "fido2_pin_required"
        );

        // Wrong PIN reports retries.
        let mut d = draft(&vault, "usb:1");
        d.user_verification = true;
        d.pin = Some("000000".into());
        match generate(&devices, &s, d, None).unwrap_err() {
            MobileError::SecurityKey { kind, retries, .. } => {
                assert_eq!(kind, "fido2_pin_invalid");
                assert_eq!(retries, Some(7));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(devices.pin_retries("usb:1".into()).unwrap(), Some(7));

        // The user walks away / declines.
        token.configure(|c| c.touch = Touch::Timeout);
        assert_eq!(
            sk_error(generate(&devices, &s, draft(&vault, "usb:1"), None).unwrap_err()),
            "fido2_timeout"
        );
        token.configure(|c| c.touch = Touch::Deny);
        assert_eq!(
            sk_error(generate(&devices, &s, draft(&vault, "usb:1"), None).unwrap_err()),
            "fido2_denied"
        );
        assert!(s.list::<SshKey>(None).unwrap().is_empty());

        // Bad input never reaches the token.
        let before = token.commands().len();
        let mut d = draft(&vault, "usb:1");
        d.label = "  ".into();
        assert!(matches!(
            generate(&devices, &s, d, None),
            Err(MobileError::Invalid { .. })
        ));
        let mut d = draft(&vault, "usb:1");
        d.application = "https://not-ssh".into();
        assert!(generate(&devices, &s, d, None).is_err());
        assert_eq!(token.commands().len(), before);
    }

    #[test]
    fn resident_keys_load_once() {
        let devices = fresh();
        let token = SoftToken::default();
        devices
            .insert(
                "nfc:1".into(),
                Fido2Transport::Nfc,
                "Soft",
                0,
                0,
                Link::Nfc(nfc(&token)),
            )
            .unwrap();
        let (s, vault) = store();
        let mut d = draft(&vault, "nfc:1");
        d.resident = true;
        d.user = "laptop".into();
        let first = generate(&devices, &s, d, None).unwrap();
        let mut d = draft(&vault, "nfc:1");
        d.resident = true;
        d.label = "second".into();
        d.algorithm = SkKeyAlgorithm::EcdsaP256;
        generate(&devices, &s, d, None).unwrap();

        let load = |pin: &str| Fido2LoadDraft {
            vault_id: vault.clone(),
            device_id: Some("nfc:1".into()),
            pin: pin.into(),
            passphrase: None,
            remember_passphrase: false,
        };
        assert_eq!(
            sk_error(load_resident(&devices, &s, load(""), None).unwrap_err()),
            "fido2_pin_required"
        );
        // Both already in the vault: nothing new.
        assert!(
            load_resident(&devices, &s, load("123456"), None)
                .unwrap()
                .is_empty()
        );

        // A fresh vault imports both, labelled by the token-side user name.
        let (s2, vault2) = store();
        let mut d = load("123456");
        d.vault_id = vault2;
        let got = load_resident(&devices, &s2, d, None).unwrap();
        assert_eq!(got.len(), 2);
        assert!(got.iter().any(|k| k.label == "laptop"));
        assert!(got.iter().all(|k| k.security_key));
        assert!(got.iter().any(|k| k.fingerprint == first.fingerprint));
        assert_eq!(s2.list::<SshKey>(None).unwrap().len(), 2);
    }
}
