//! SSH ID for the phone: the account handle, this device's passkeys and the
//! keys published under the handle. Private device keys are generated and
//! kept in the encrypted store by `termoso_core::sshid`; the server and
//! Kotlin only ever see public keys, fingerprints and metadata. Same
//! publish / rotate / delete semantics as the desktop `sshid` module.

use std::sync::Arc;

use termoso_core::api::ApiClient;
use termoso_core::keys;
use termoso_core::model::SshKey;
use termoso_core::ssh::AuthMethod;
use termoso_core::sshid as core;
use termoso_core::store::Store;
use termoso_proto::sshid::{SshIdKeyType, SshIdProfile, normalize_handle};

use crate::account::AccountRuntime;
use crate::dto::{millis, parse_id};
use crate::error::{MobileError, Result};

/// Passkey types an SSH ID can hold (mirrors the wire enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SshIdKeyKind {
    Ed25519,
    Ecdsa,
    Rsa,
    EcdsaSk,
    Ed25519Sk,
}

impl From<SshIdKeyType> for SshIdKeyKind {
    fn from(t: SshIdKeyType) -> Self {
        match t {
            SshIdKeyType::Ed25519 => Self::Ed25519,
            SshIdKeyType::Ecdsa => Self::Ecdsa,
            SshIdKeyType::Rsa => Self::Rsa,
            SshIdKeyType::EcdsaSk => Self::EcdsaSk,
            SshIdKeyType::Ed25519Sk => Self::Ed25519Sk,
        }
    }
}

impl From<SshIdKeyKind> for SshIdKeyType {
    fn from(k: SshIdKeyKind) -> Self {
        match k {
            SshIdKeyKind::Ed25519 => Self::Ed25519,
            SshIdKeyKind::Ecdsa => Self::Ecdsa,
            SshIdKeyKind::Rsa => Self::Rsa,
            SshIdKeyKind::EcdsaSk => Self::EcdsaSk,
            SshIdKeyKind::Ed25519Sk => Self::Ed25519Sk,
        }
    }
}

/// Label shown in the UI and used in the public URL (`ED25519`, `ECDSA-SK`…).
#[uniffi::export]
pub fn sshid_type_label(kind: SshIdKeyKind) -> String {
    SshIdKeyType::from(kind).url_name().to_string()
}

/// Whether a passkey type needs a FIDO2 token to sign.
#[uniffi::export]
pub fn sshid_type_is_hardware(kind: SshIdKeyKind) -> bool {
    SshIdKeyType::from(kind).is_hardware()
}

/// Whether `input` is an acceptable handle (3–32 chars, `a-z0-9-_`, starts
/// with a letter or digit). A leading `@` and upper case are tolerated.
#[uniffi::export]
pub fn sshid_handle_valid(input: String) -> bool {
    normalize_handle(&input).is_some()
}

/// `curl -fs <url>[/<TYPE>] >> ~/.ssh/authorized_keys` for the given
/// passkey type (`None` = the server default list, ED25519).
#[uniffi::export]
pub fn sshid_provision_command(url: String, kind: Option<SshIdKeyKind>) -> String {
    let url = match kind {
        Some(k) if k != SshIdKeyKind::Ed25519 => {
            format!("{url}/{}", SshIdKeyType::from(k).url_name())
        }
        _ => url,
    };
    format!("curl -fs {url} >> ~/.ssh/authorized_keys")
}

/// One of this device's passkeys.
#[derive(Debug, Clone, uniffi::Record)]
pub struct DeviceKeyCard {
    pub key_type: SshIdKeyKind,
    pub fingerprint: String,
    /// `authorized_keys` line.
    pub public_key: String,
    /// The server currently lists this key for the device.
    pub published: bool,
}

/// A key published under the handle (any device, or a FIDO2 token).
#[derive(Debug, Clone, uniffi::Record)]
pub struct SshIdKeyCard {
    pub id: String,
    pub key_type: SshIdKeyKind,
    pub public_key: String,
    pub fingerprint: String,
    /// Device holding the private key; `None` for FIDO2 keys.
    pub device_id: Option<String>,
    /// Device name or the FIDO2 key's label.
    pub label: String,
    pub current_device: bool,
    pub hardware: bool,
    pub updated_at: i64,
}

/// Everything the SSH ID screen needs.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SshIdView {
    pub signed_in: bool,
    /// `None` until the account sets up a handle.
    pub handle: Option<String>,
    /// Public URL of the default key list (`…/sshid/<handle>`).
    pub url: Option<String>,
    pub created_at: Option<i64>,
    /// Published keys, newest first.
    pub keys: Vec<SshIdKeyCard>,
    /// This device's passkeys.
    pub device_keys: Vec<DeviceKeyCard>,
}

fn same_key(a: &str, b: &str) -> bool {
    let head = |s: &str| {
        let mut it = s.split_whitespace();
        (it.next().map(str::to_string), it.next().map(str::to_string))
    };
    head(a) == head(b)
}

fn fingerprint(public_key: &str) -> String {
    keys::parse_public(public_key)
        .map(|p| p.fingerprint)
        .unwrap_or_default()
}

pub(crate) fn view_of(store: &Store, profile: Option<SshIdProfile>) -> Result<SshIdView> {
    let local = core::device_keys(store)?;
    let device_keys = local
        .iter()
        .map(|k| DeviceKeyCard {
            key_type: k.key_type.into(),
            fingerprint: k.fingerprint(),
            public_key: k.public_key.clone(),
            published: profile.as_ref().is_some_and(|p| {
                p.keys
                    .iter()
                    .any(|s| s.current_device && same_key(&s.public_key, &k.public_key))
            }),
        })
        .collect();
    let keys = profile
        .as_ref()
        .map(|p| {
            p.keys
                .iter()
                .map(|k| SshIdKeyCard {
                    id: k.id.to_string(),
                    key_type: k.key_type.into(),
                    fingerprint: fingerprint(&k.public_key),
                    public_key: k.public_key.clone(),
                    device_id: k.device_id.map(|d| d.to_string()),
                    label: k.label.clone(),
                    current_device: k.current_device,
                    hardware: k.key_type.is_hardware(),
                    updated_at: millis(k.updated_at),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(SshIdView {
        signed_in: true,
        handle: profile.as_ref().map(|p| p.handle.clone()),
        url: profile.as_ref().map(|p| p.url.clone()),
        created_at: profile.as_ref().map(|p| millis(p.created_at)),
        keys,
        device_keys,
    })
}

/// Generate any missing device key and publish the set when the server
/// does not list exactly what this device holds.
async fn publish(
    api: &Arc<ApiClient>,
    store: &Arc<Store>,
    profile: SshIdProfile,
) -> Result<SshIdProfile> {
    let handle = profile.handle.clone();
    let s = store.clone();
    let keys = tokio::task::spawn_blocking(move || core::ensure_device_keys(&s, &handle))
        .await
        .map_err(|e| MobileError::invalid(e.to_string()))??;
    let mine: Vec<&str> = profile
        .keys
        .iter()
        .filter(|k| k.current_device)
        .map(|k| k.public_key.as_str())
        .collect();
    let in_sync = mine.len() == keys.len()
        && keys
            .iter()
            .all(|k| mine.iter().any(|m| same_key(m, &k.public_key)));
    if in_sync {
        return Ok(profile);
    }
    Ok(api
        .put_sshid_device_keys(&core::upload_request(&keys))
        .await?)
}

async fn load(api: &Arc<ApiClient>, store: &Arc<Store>) -> Result<Option<SshIdProfile>> {
    let profile = match api.sshid().await? {
        Some(p) => Some(publish(api, store, p).await?),
        None => None,
    };
    core::set_handle(store, profile.as_ref().map(|p| p.handle.as_str()))?;
    Ok(profile)
}

impl AccountRuntime {
    /// Current state; keeps the device keys published while at it. Signed
    /// out: only the locally held (stale) device keys, if any.
    pub async fn sshid_view(&self) -> Result<SshIdView> {
        let store = self.store_arc();
        let api = match self.api().await {
            Ok(api) => api,
            Err(_) => {
                let mut v = view_of(&store, None)?;
                v.signed_in = false;
                return Ok(v);
            }
        };
        let profile = load(&api, &store).await?;
        view_of(&store, profile)
    }

    /// Re-publish this device's keys (after sign-in, in the background).
    pub(crate) async fn sshid_refresh(&self) -> Result<()> {
        let api = self.api().await?;
        load(&api, &self.store_arc()).await?;
        Ok(())
    }

    /// Claim a handle and publish this device's keys under it.
    pub async fn sshid_create(&self, handle: String) -> Result<SshIdView> {
        let api = self.api().await?;
        let handle = normalize_handle(&handle).ok_or_else(|| {
            MobileError::invalid(
                "handle must be 3–32 characters: lowercase letters, digits, - or _",
            )
        })?;
        let store = self.store_arc();
        let profile = api.create_sshid(&handle).await?;
        let profile = publish(&api, &store, profile).await?;
        core::set_handle(&store, Some(&profile.handle))?;
        view_of(&store, Some(profile))
    }

    /// Delete the SSH ID: every published key goes with it on the server,
    /// the device keys and any FIDO2 handles are wiped here.
    pub async fn sshid_delete(&self) -> Result<SshIdView> {
        let api = self.api().await?;
        let store = self.store_arc();
        api.delete_sshid().await?;
        core::forget(&store)?;
        for k in store.list::<SshKey>(None)? {
            if k.data.ssh_id {
                store.delete(k.id)?;
            }
        }
        view_of(&store, None)
    }

    /// Replace this device's passkeys with fresh ones and publish them.
    pub async fn sshid_rotate(&self) -> Result<SshIdView> {
        let api = self.api().await?;
        let store = self.store_arc();
        let profile = api
            .sshid()
            .await?
            .ok_or_else(|| MobileError::invalid("SSH ID is not set up"))?;
        let handle = profile.handle.clone();
        let s = store.clone();
        let keys = tokio::task::spawn_blocking(move || core::rotate_device_keys(&s, &handle))
            .await
            .map_err(|e| MobileError::invalid(e.to_string()))??;
        let profile = api
            .put_sshid_device_keys(&core::upload_request(&keys))
            .await?;
        view_of(&store, Some(profile))
    }

    /// Remove a published key. Another device's passkeys come back when it
    /// syncs, so revoke the device instead; FIDO2 keys stay gone.
    pub async fn sshid_remove_key(&self, id: String) -> Result<SshIdView> {
        let id = parse_id(&id)?;
        let api = self.api().await?;
        let store = self.store_arc();
        let before = api.sshid().await?;
        let removed = before
            .as_ref()
            .and_then(|p| p.keys.iter().find(|k| k.id == id))
            .map(|k| k.public_key.clone());
        api.remove_sshid_key(id).await?;
        if let Some(pk) = removed {
            for k in store.list::<SshKey>(None)? {
                if k.data.ssh_id
                    && k.data
                        .public_key
                        .as_deref()
                        .is_some_and(|p| same_key(p, &pk))
                {
                    store.delete(k.id)?;
                }
            }
        }
        let profile = api.sshid().await?;
        view_of(&store, profile)
    }
}

/// Authentication methods for an identity that logs in with SSH ID: this
/// device's passkeys, preferred type first. FIDO2 keys attached to the SSH
/// ID need a token and are not offered on the phone yet. Empty when the
/// device holds no SSH ID keys.
pub(crate) fn auth_methods(
    store: &Store,
    preferred: Option<SshIdKeyType>,
) -> Result<Vec<AuthMethod>> {
    let device = core::ordered(core::device_keys(store)?, preferred);
    Ok(device
        .into_iter()
        .filter(|k| keys::inspect(&k.private_key).is_ok())
        .map(|k| AuthMethod::Key {
            private_key: k.private_key,
            passphrase: None,
            certificate: None,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_crypto::keys::SymmetricKey;

    #[test]
    fn provision_commands_and_labels() {
        let url = "https://x.test/sshid/alice".to_string();
        assert_eq!(
            sshid_provision_command(url.clone(), None),
            "curl -fs https://x.test/sshid/alice >> ~/.ssh/authorized_keys"
        );
        assert_eq!(
            sshid_provision_command(url.clone(), Some(SshIdKeyKind::Ed25519)),
            "curl -fs https://x.test/sshid/alice >> ~/.ssh/authorized_keys"
        );
        assert_eq!(
            sshid_provision_command(url, Some(SshIdKeyKind::EcdsaSk)),
            "curl -fs https://x.test/sshid/alice/ECDSA-SK >> ~/.ssh/authorized_keys"
        );
        assert_eq!(sshid_type_label(SshIdKeyKind::Rsa), "RSA");
        assert!(sshid_type_is_hardware(SshIdKeyKind::Ed25519Sk));
        assert!(!sshid_type_is_hardware(SshIdKeyKind::Ecdsa));
        assert!(sshid_handle_valid("@Alice_1".into()));
        assert!(!sshid_handle_valid("ab".into()));
        assert!(!sshid_handle_valid("-abc".into()));
    }

    #[test]
    fn auth_methods_follow_device_keys() {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        assert!(auth_methods(&s, None).unwrap().is_empty());
        core::ensure_device_keys(&s, "alice").unwrap();
        let m = auth_methods(&s, Some(SshIdKeyType::Rsa)).unwrap();
        assert_eq!(m.len(), 3);
        match &m[0] {
            AuthMethod::Key { private_key, .. } => {
                assert_eq!(keys::inspect(private_key).unwrap().key_type, "ssh-rsa");
            }
            _ => panic!("expected a key"),
        }
        let v = view_of(&s, None).unwrap();
        assert_eq!(v.device_keys.len(), 3);
        assert!(v.device_keys.iter().all(|k| !k.published));
        assert!(
            v.device_keys
                .iter()
                .all(|k| k.fingerprint.starts_with("SHA256:"))
        );
        assert!(v.handle.is_none() && v.keys.is_empty());
    }
}
