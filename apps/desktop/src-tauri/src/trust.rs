//! Known-host pins: list, forget, import/export in OpenSSH format. Trust
//! decisions for new/changed keys stay in the connection prompt flow
//! (`prompts.rs`); this façade never accepts a key on the UI's behalf.

use chrono::{DateTime, Utc};
use serde::Serialize;
use termoso_core::hostkey::KnownHosts;
use termoso_core::model::KnownHost;
use termoso_core::store::Store;
use uuid::Uuid;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownHostCard {
    pub id: Uuid,
    pub vault_id: Uuid,
    /// `host` or `[host]:port`.
    pub hostname: String,
    pub key_type: String,
    pub fingerprint: String,
    pub public_key: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: usize,
}

fn known_hosts(state: &AppState) -> Result<KnownHosts> {
    Ok(KnownHosts::new(
        state.store.clone(),
        state.store.local_vault()?.id,
    ))
}

fn card(e: termoso_core::model::Entity<KnownHost>) -> KnownHostCard {
    KnownHostCard {
        id: e.id,
        vault_id: e.vault_id,
        hostname: e.data.hostname,
        key_type: e.data.key_type,
        fingerprint: e.data.fingerprint,
        public_key: e.data.public_key,
        updated_at: e.updated_at,
    }
}

pub fn list(store: &Store) -> Result<Vec<KnownHostCard>> {
    let mut out: Vec<KnownHostCard> = store
        .list::<KnownHost>(None)?
        .into_iter()
        .map(card)
        .collect();
    out.sort_by(|a, b| {
        a.hostname
            .cmp(&b.hostname)
            .then_with(|| a.key_type.cmp(&b.key_type))
    });
    Ok(out)
}

/// Forget one pin.
pub fn forget(store: &Store, id: Uuid) -> Result<()> {
    store.require::<KnownHost>(id)?;
    store.delete(id)?;
    Ok(())
}

/// Forget every pin for `hostname` (canonical `host` / `[host]:port` form).
pub fn forget_host(store: &Store, hostname: &str) -> Result<usize> {
    let target = hostname.trim().to_ascii_lowercase();
    if target.is_empty() {
        return Err(DesktopError::invalid("hostname is required"));
    }
    let mut n = 0;
    for e in store.list::<KnownHost>(None)? {
        if e.data.hostname == target {
            store.delete(e.id)?;
            n += 1;
        }
    }
    Ok(n)
}

pub fn import_openssh(state: &AppState, contents: &str) -> Result<ImportReport> {
    if contents.len() > 4 * 1024 * 1024 {
        return Err(DesktopError::invalid("known_hosts file is too large"));
    }
    let added = known_hosts(state)?.import_openssh(contents)?;
    Ok(ImportReport { added })
}

pub fn import_file(state: &AppState, path: &str) -> Result<ImportReport> {
    let contents = std::fs::read_to_string(path)?;
    import_openssh(state, &contents)
}

pub fn export_openssh(state: &AppState) -> Result<String> {
    Ok(known_hosts(state)?.export_openssh()?)
}

pub fn export_file(state: &AppState, path: &str) -> Result<usize> {
    let text = export_openssh(state)?;
    let n = text.lines().count();
    std::fs::write(path, text)?;
    Ok(n)
}

/// Default OpenSSH `known_hosts` of the current user, when it exists.
pub fn default_openssh_path() -> Option<String> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)?;
    let p = home.join(".ssh").join("known_hosts");
    p.exists().then(|| p.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use termoso_core::termoso_crypto::keys::SymmetricKey;

    const KEY: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH7t5lVvmEL4L7DMiC5J2RzzAz76OMaRqQmDH/anAAbv";

    fn store() -> Store {
        Store::open_in_memory(SymmetricKey::generate()).expect("store")
    }

    #[test]
    fn list_forget_by_host() {
        let store = Arc::new(store());
        let vault = store.local_vault().unwrap().id;
        let kh = KnownHosts::new(store.clone(), vault);
        assert_eq!(
            kh.import_openssh(&format!("example.com {KEY}\n[db.local]:2222 {KEY}\n"))
                .unwrap(),
            2
        );
        let all = list(&store).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].hostname, "[db.local]:2222");
        assert!(all[0].fingerprint.starts_with("SHA256:"));
        assert_eq!(forget_host(&store, "EXAMPLE.COM").unwrap(), 1);
        assert_eq!(forget_host(&store, "nope").unwrap(), 0);
        forget(&store, all[0].id).unwrap();
        assert!(list(&store).unwrap().is_empty());
        assert!(forget(&store, all[0].id).is_err());
    }
}
