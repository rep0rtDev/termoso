//! Server key pins per host, as shown in the host editor.
//!
//! A pin is a `known_host` entity; pins in a team vault sync to every member,
//! so an admin can hand the whole team the trusted fingerprint of a server
//! before anyone connects. [`termoso_core::hostkey::KnownHosts::check`]
//! already consults pins from every unlocked vault; this module only
//! lists and edits them for one `host:port`.

use std::sync::Arc;

use serde::Serialize;
use termoso_core::hostkey::{self, KnownHosts};
use termoso_core::model::KnownHost;
use termoso_core::store::{LocalVaultKind, Store};
use uuid::Uuid;

use crate::error::{ClientError, Result};

/// One pinned key for a `host:port`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyPin {
    pub id: Uuid,
    /// Vault holding the pin; team vaults share it with all members.
    pub vault_id: Uuid,
    pub key_type: String,
    /// `SHA256:…`
    pub fingerprint: String,
    /// `<type> <base64>`, ready for `known_hosts` / `ssh-keyscan` comparison.
    pub public_key: String,
}

/// Pins for `host:port` across all unlocked vaults.
pub fn pins(store: &Arc<Store>, host: &str, port: u16) -> Result<Vec<HostKeyPin>> {
    if host.trim().is_empty() {
        return Ok(Vec::new());
    }
    let kh = KnownHosts::new(store.clone(), store.local_vault()?.id);
    let mut out: Vec<HostKeyPin> = kh
        .for_host(host, port)?
        .into_iter()
        .map(|e| HostKeyPin {
            id: e.id,
            vault_id: e.vault_id,
            public_key: format!("{} {}", e.data.key_type, e.data.public_key),
            key_type: e.data.key_type,
            fingerprint: e.data.fingerprint,
        })
        .collect();
    out.sort_by(|a, b| a.key_type.cmp(&b.key_type));
    Ok(out)
}

/// Pin server keys for `host:port` into `vault_id`.
///
/// With `public_key` (an OpenSSH `<type> <base64>` line, e.g. from
/// `ssh-keyscan`) that key is pinned. Without it, the keys already trusted
/// for the host in *other* vaults (typically accepted on first connection)
/// are copied into `vault_id`, so a key verified once by an admin becomes
/// the team's pin.
pub fn pin(
    store: &Arc<Store>,
    vault_id: Uuid,
    host: &str,
    port: u16,
    public_key: Option<&str>,
) -> Result<Vec<HostKeyPin>> {
    let host = host.trim();
    if host.is_empty() {
        return Err(ClientError::invalid("address is required"));
    }
    let vault = store.vault(vault_id)?;
    if !vault.unlocked {
        return Err(ClientError::forbidden("vault is locked"));
    }
    if vault.kind == LocalVaultKind::Team && !vault.role.can_write() {
        return Err(ClientError::forbidden(
            "only editors and managers can pin server keys in a team vault",
        ));
    }
    let kh = KnownHosts::new(store.clone(), vault_id);
    match public_key {
        Some(line) => {
            let line = line.trim();
            if line.is_empty() || line.len() > 16 * 1024 {
                return Err(ClientError::invalid("public key is required"));
            }
            let key = hostkey::parse_public_key(line).map_err(|_| {
                ClientError::invalid(
                    "expected an OpenSSH public key line: <type> <base64>, e.g. from ssh-keyscan",
                )
            })?;
            kh.trust(host, port, &key)?;
        }
        None => {
            let others: Vec<KnownHost> = kh
                .for_host(host, port)?
                .into_iter()
                .filter(|e| e.vault_id != vault_id)
                .map(|e| e.data)
                .collect();
            if others.is_empty() {
                return Err(ClientError::not_found(
                    "no trusted key for this host yet: connect once and accept its key, or paste the server's public key",
                ));
            }
            for k in others {
                let key = hostkey::parse_public_key(&format!("{} {}", k.key_type, k.public_key))?;
                kh.trust(host, port, &key)?;
            }
        }
    }
    pins(store, host, port)
}

/// Remove one pin.
pub fn unpin(store: &Arc<Store>, id: Uuid) -> Result<()> {
    let e = store.require::<KnownHost>(id)?;
    let vault = store.vault(e.vault_id)?;
    if vault.kind == LocalVaultKind::Team && !vault.role.can_write() {
        return Err(ClientError::forbidden(
            "only editors and managers can unpin server keys in a team vault",
        ));
    }
    store.delete(id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_core::termoso_crypto::keys::SymmetricKey;
    use termoso_proto::vault::VaultRole;

    const KEY: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH7t5lVvmEL4L7DMiC5J2RzzAz76OMaRqQmDH/anAAbv";

    fn store_with_team(role: VaultRole) -> (Arc<Store>, Uuid) {
        let store = Arc::new(Store::open_in_memory(SymmetricKey::generate()).unwrap());
        let team = Uuid::new_v4();
        store
            .upsert_vault(
                team,
                LocalVaultKind::Team,
                "Team",
                Some(Uuid::new_v4()),
                role,
                Some(&SymmetricKey::generate()),
                1,
            )
            .unwrap();
        (store, team)
    }

    #[test]
    fn paste_then_copy_into_team_vault() {
        let (store, team) = store_with_team(VaultRole::Editor);
        let local = store.local_vault().unwrap().id;
        assert!(pins(&store, "db.internal", 2222).unwrap().is_empty());
        assert!(
            pin(&store, team, "db.internal", 2222, None).is_err(),
            "nothing to copy"
        );
        assert!(pin(&store, team, "db.internal", 2222, Some("garbage")).is_err());

        // First-connection trust lands in the local vault …
        let local_pins = pin(&store, local, "DB.internal", 2222, Some(KEY)).unwrap();
        assert_eq!(local_pins.len(), 1);
        assert_eq!(local_pins[0].vault_id, local);
        assert_eq!(local_pins[0].public_key, KEY);
        assert!(local_pins[0].fingerprint.starts_with("SHA256:"));

        // … and the admin promotes it to the team.
        let all = pin(&store, team, "db.internal", 2222, None).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|p| p.vault_id == team));
        // Idempotent: the team pin is replaced, not duplicated.
        let all = pin(&store, team, "db.internal", 2222, None).unwrap();
        assert_eq!(all.len(), 2);

        let team_pin = all.iter().find(|p| p.vault_id == team).unwrap();
        unpin(&store, team_pin.id).unwrap();
        assert_eq!(pins(&store, "db.internal", 2222).unwrap().len(), 1);
        assert!(unpin(&store, team_pin.id).is_err());
    }

    #[test]
    fn viewers_cannot_pin_in_team_vault() {
        let (store, team) = store_with_team(VaultRole::Viewer);
        let err = pin(&store, team, "h", 22, Some(KEY)).unwrap_err();
        assert_eq!(err.kind, "forbidden");
        assert!(pin(&store, Uuid::new_v4(), "h", 22, Some(KEY)).is_err());
    }
}
