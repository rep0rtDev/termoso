//! SSH ID: the account's public handle listing device-bound SSH keys.
//!
//! Every signed-in device generates its own key pair per algorithm and
//! publishes only the public halves under the handle; servers fetch them
//! from `/sshid/{handle}`. The private halves never leave this store: they
//! live in encrypted metadata, are not part of any vault or sync payload
//! and are wiped when the account signs out.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use termoso_proto::sshid::{DeviceKeyUpload, PutDeviceKeysRequest, SshIdKeyType};
use zeroize::Zeroizing;

use crate::error::Result;
use crate::keys::{self, KeyAlgorithm};
use crate::store::Store;

const KEYS_META: &str = "sshid.device_keys";
const HANDLE_META: &str = "sshid.handle";

/// Algorithms generated for every device. Hardware-backed types are
/// attached separately as FIDO2 keys.
pub const DEVICE_KEY_TYPES: [SshIdKeyType; 3] = [
    SshIdKeyType::Ed25519,
    SshIdKeyType::Ecdsa,
    SshIdKeyType::Rsa,
];

const RSA_BITS: usize = 3072;

/// One device-bound key pair. Kept only in encrypted local metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKey {
    /// Algorithm family (one key per type and device).
    pub key_type: SshIdKeyType,
    /// `-----BEGIN OPENSSH PRIVATE KEY-----` block, unencrypted (the store
    /// itself is encrypted with the master key).
    pub private_key: Zeroizing<String>,
    /// `authorized_keys` line.
    pub public_key: String,
    /// Generation time.
    pub created_at: DateTime<Utc>,
}

impl DeviceKey {
    /// `SHA256:…` fingerprint of the public key.
    pub fn fingerprint(&self) -> String {
        keys::parse_public(&self.public_key)
            .map(|p| p.fingerprint)
            .unwrap_or_default()
    }
}

/// Algorithm behind a device key type; `None` for hardware-backed types.
pub fn algorithm_of(t: SshIdKeyType) -> Option<KeyAlgorithm> {
    match t {
        SshIdKeyType::Ed25519 => Some(KeyAlgorithm::Ed25519),
        SshIdKeyType::Ecdsa => Some(KeyAlgorithm::EcdsaP256),
        SshIdKeyType::Rsa => Some(KeyAlgorithm::Rsa { bits: RSA_BITS }),
        SshIdKeyType::EcdsaSk | SshIdKeyType::Ed25519Sk => None,
    }
}

/// Device keys held locally, if any.
pub fn device_keys(store: &Store) -> Result<Vec<DeviceKey>> {
    Ok(match store.secret_meta(KEYS_META)? {
        Some(json) => serde_json::from_str(&json)?,
        None => Vec::new(),
    })
}

fn save(store: &Store, keys: &[DeviceKey]) -> Result<()> {
    store.set_secret_meta(KEYS_META, &serde_json::to_string(keys)?)
}

fn generate(t: SshIdKeyType, handle: &str) -> Result<Option<DeviceKey>> {
    let Some(alg) = algorithm_of(t) else {
        return Ok(None);
    };
    let comment = format!("{handle}@termoso");
    let material = keys::generate(alg, &comment, None)?;
    Ok(Some(DeviceKey {
        key_type: t,
        private_key: material.private_key,
        public_key: material.public_key,
        created_at: Utc::now(),
    }))
}

/// Make sure a key of every device type exists, generating the missing
/// ones. Returns the full set. CPU-bound (RSA); call off the async runtime.
pub fn ensure_device_keys(store: &Store, handle: &str) -> Result<Vec<DeviceKey>> {
    let mut keys = device_keys(store)?;
    let mut changed = false;
    for t in DEVICE_KEY_TYPES {
        if keys.iter().any(|k| k.key_type == t) {
            continue;
        }
        if let Some(k) = generate(t, handle)? {
            keys.push(k);
            changed = true;
        }
    }
    if changed {
        save(store, &keys)?;
    }
    Ok(keys)
}

/// A fresh key of every device type, not yet stored: publish them first and
/// keep them with [`save_device_keys`] only once the server accepted the
/// set, so a refused or abandoned rotation leaves the working keys in place.
/// CPU-bound (RSA); call off the async runtime.
pub fn new_device_keys(handle: &str) -> Result<Vec<DeviceKey>> {
    let mut keys = Vec::new();
    for t in DEVICE_KEY_TYPES {
        if let Some(k) = generate(t, handle)? {
            keys.push(k);
        }
    }
    Ok(keys)
}

/// Replace the stored device keys with `keys`.
pub fn save_device_keys(store: &Store, keys: &[DeviceKey]) -> Result<()> {
    save(store, keys)
}

/// Wipe the device keys and the cached handle (sign-out, SSH ID deleted).
pub fn forget(store: &Store) -> Result<()> {
    store.delete_meta(KEYS_META)?;
    store.delete_meta(HANDLE_META)
}

/// Public halves as the server expects them.
pub fn upload_request(keys: &[DeviceKey]) -> PutDeviceKeysRequest {
    PutDeviceKeysRequest {
        keys: keys
            .iter()
            .map(|k| DeviceKeyUpload {
                key_type: k.key_type,
                public_key: k.public_key.clone(),
            })
            .collect(),
    }
}

/// Handle of the account's SSH ID as last seen from the server, so
/// connections can fall back to it as the username while offline.
pub fn handle(store: &Store) -> Result<Option<String>> {
    store.meta(HANDLE_META)
}

/// Remember (or forget) the handle after talking to the server.
pub fn set_handle(store: &Store, handle: Option<&str>) -> Result<()> {
    match handle {
        Some(h) => store.set_meta(HANDLE_META, h),
        None => store.delete_meta(HANDLE_META),
    }
}

/// Order the device keys for authentication: the preferred type first,
/// then the rest in the default order.
pub fn ordered(mut keys: Vec<DeviceKey>, preferred: Option<SshIdKeyType>) -> Vec<DeviceKey> {
    let rank = |t: SshIdKeyType| {
        if Some(t) == preferred {
            0
        } else {
            1 + DEVICE_KEY_TYPES.iter().position(|d| *d == t).unwrap_or(9)
        }
    };
    keys.sort_by_key(|k| rank(k.key_type));
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_crypto::keys::SymmetricKey;

    fn store() -> Store {
        Store::open_in_memory(SymmetricKey::generate()).unwrap()
    }

    #[test]
    fn generates_once_and_rotates() {
        let s = store();
        assert!(device_keys(&s).unwrap().is_empty());
        let first = ensure_device_keys(&s, "alice").unwrap();
        assert_eq!(first.len(), 3);
        for k in &first {
            assert_eq!(k.public_key.split(' ').next(), Some(k.key_type.wire_name()));
            assert!(k.public_key.ends_with("alice@termoso"));
            assert!(
                k.private_key
                    .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----")
            );
            assert!(k.fingerprint().starts_with("SHA256:"));
        }
        let again = ensure_device_keys(&s, "alice").unwrap();
        assert_eq!(
            again.iter().map(|k| &k.public_key).collect::<Vec<_>>(),
            first.iter().map(|k| &k.public_key).collect::<Vec<_>>()
        );
        let rotated = new_device_keys("alice").unwrap();
        assert_eq!(rotated.len(), 3);
        assert!(
            rotated
                .iter()
                .zip(&first)
                .all(|(a, b)| a.public_key != b.public_key)
        );
        assert_eq!(
            device_keys(&s)
                .unwrap()
                .iter()
                .map(|k| &k.public_key)
                .collect::<Vec<_>>(),
            first.iter().map(|k| &k.public_key).collect::<Vec<_>>(),
            "unsaved rotation must not touch the stored keys"
        );
        save_device_keys(&s, &rotated).unwrap();
        assert_eq!(
            device_keys(&s)
                .unwrap()
                .iter()
                .map(|k| &k.public_key)
                .collect::<Vec<_>>(),
            rotated.iter().map(|k| &k.public_key).collect::<Vec<_>>()
        );
        let req = upload_request(&rotated);
        assert_eq!(req.keys.len(), 3);
        assert!(req.keys.iter().all(|k| !k.public_key.contains("PRIVATE")));

        set_handle(&s, Some("alice")).unwrap();
        assert_eq!(handle(&s).unwrap().as_deref(), Some("alice"));
        forget(&s).unwrap();
        assert!(device_keys(&s).unwrap().is_empty());
        assert!(handle(&s).unwrap().is_none());
    }

    #[test]
    fn private_keys_are_encrypted_at_rest() {
        let s = store();
        ensure_device_keys(&s, "bob").unwrap();
        let raw = s.meta(KEYS_META).unwrap().unwrap();
        assert!(!raw.contains("PRIVATE KEY"));
        assert!(!raw.contains("ssh-ed25519"));
    }

    #[test]
    fn preferred_type_goes_first() {
        let s = store();
        let keys = ensure_device_keys(&s, "carol").unwrap();
        let o = ordered(keys.clone(), Some(SshIdKeyType::Rsa));
        assert_eq!(o[0].key_type, SshIdKeyType::Rsa);
        assert_eq!(o[1].key_type, SshIdKeyType::Ed25519);
        let o = ordered(keys, None);
        assert_eq!(o[0].key_type, SshIdKeyType::Ed25519);
    }
}
