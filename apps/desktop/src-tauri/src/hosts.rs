//! Host list / editor façade. The UI edits a flat `HostForm`; Rust maps it to
//! the `host` + inline `ssh_config` + inline `identity` entities (Termius
//! layout) and resolves group inheritance for display.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use termoso_core::model::{Entity, Group, Host, Identity, SshConfig, Tag};
use termoso_core::store::Store;
use uuid::Uuid;

use crate::error::{DesktopError, Result};

/// What a host card / list row shows. Effective values already account for
/// group inheritance.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCard {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    pub address: String,
    pub group_id: Option<Uuid>,
    pub group_path: Vec<String>,
    /// `ssh` | `telnet`.
    pub protocol: String,
    pub username: String,
    pub port: u16,
    pub tags: Vec<String>,
    pub os_name: Option<String>,
    pub notes: String,
    pub sort_order: i32,
    pub updated_at: DateTime<Utc>,
    pub dirty: bool,
}

/// Flat editor model. `None` for optional fields means "inherit / unset".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostForm {
    pub id: Option<Uuid>,
    pub vault_id: Uuid,
    pub label: String,
    pub address: String,
    pub group_id: Option<Uuid>,
    pub port: Option<u16>,
    pub username: String,
    /// `None` keeps the stored password when editing; `Some("")` clears it.
    pub password: Option<String>,
    pub ssh_key_id: Option<Uuid>,
    /// Reference an existing (visible) identity instead of the inline one.
    pub identity_id: Option<Uuid>,
    pub tag_ids: Vec<Uuid>,
    pub notes: String,
    pub os_name: Option<String>,
    pub agent_forwarding: bool,
    pub startup_snippet_id: Option<Uuid>,
    pub host_chain_id: Option<Uuid>,
    pub proxy_id: Option<Uuid>,
    /// Set when the stored inline identity has a password (UI shows a mask).
    #[serde(default)]
    pub has_password: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupNode {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    pub parent_id: Option<Uuid>,
    pub sort_order: i32,
    pub host_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    pub color: Option<String>,
}

pub fn cards(store: &Store, vault_id: Option<Uuid>) -> Result<Vec<HostCard>> {
    let hosts: Vec<Entity<Host>> = store.list(vault_id)?;
    let mut out = Vec::with_capacity(hosts.len());
    for h in hosts {
        let r = store.resolve_host(h.id)?;
        let protocol = if r.telnet.is_some() && h.data.ssh_config_id.is_none() {
            "telnet"
        } else {
            "ssh"
        };
        let port = match protocol {
            "telnet" => r.telnet.as_ref().and_then(|t| t.port).unwrap_or(23),
            _ => r.port(),
        };
        let username = r.username();
        out.push(HostCard {
            id: h.id,
            vault_id: h.vault_id,
            label: h.data.label.clone(),
            address: h.data.address.clone(),
            group_id: h.data.group_id,
            group_path: r.group_path,
            protocol: protocol.to_string(),
            username,
            port,
            tags: r.tags,
            os_name: h.data.os_name.clone(),
            notes: h.data.notes.clone(),
            sort_order: h.data.sort_order,
            updated_at: h.updated_at,
            dirty: h.dirty,
        });
    }
    out.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    Ok(out)
}

pub fn groups(store: &Store, vault_id: Option<Uuid>) -> Result<Vec<GroupNode>> {
    let groups: Vec<Entity<Group>> = store.list(vault_id)?;
    let hosts: Vec<Entity<Host>> = store.list(vault_id)?;
    let mut out: Vec<GroupNode> = groups
        .into_iter()
        .map(|g| GroupNode {
            host_count: hosts
                .iter()
                .filter(|h| h.data.group_id == Some(g.id))
                .count(),
            id: g.id,
            vault_id: g.vault_id,
            label: g.data.label,
            parent_id: g.data.parent_id,
            sort_order: g.data.sort_order,
        })
        .collect();
    out.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    Ok(out)
}

pub fn tags(store: &Store, vault_id: Option<Uuid>) -> Result<Vec<TagInfo>> {
    let tags: Vec<Entity<Tag>> = store.list(vault_id)?;
    Ok(tags
        .into_iter()
        .map(|t| TagInfo {
            id: t.id,
            vault_id: t.vault_id,
            label: t.data.label,
            color: t.data.color,
        })
        .collect())
}

/// Load the editor model for an existing host. Inline (hidden) identities are
/// flattened into the form; visible ones are referenced by id.
pub fn form(store: &Store, id: Uuid) -> Result<HostForm> {
    let host = store.require::<Host>(id)?;
    let ssh = match host.data.ssh_config_id {
        Some(c) => store.get::<SshConfig>(c)?.map(|e| e.data),
        None => None,
    };
    let identity = match ssh.as_ref().and_then(|s| s.identity_id) {
        Some(i) => store.get::<Identity>(i)?,
        None => None,
    };
    let (identity_id, username, ssh_key_id, has_password) = match &identity {
        Some(i) if i.data.is_visible => (Some(i.id), String::new(), None, false),
        Some(i) => (
            None,
            i.data.username.clone(),
            i.data.ssh_key_id,
            i.data.password.as_deref().is_some_and(|p| !p.is_empty()),
        ),
        None => (None, String::new(), None, false),
    };
    Ok(HostForm {
        id: Some(host.id),
        vault_id: host.vault_id,
        label: host.data.label,
        address: host.data.address,
        group_id: host.data.group_id,
        port: ssh.as_ref().and_then(|s| s.port),
        username,
        password: None,
        ssh_key_id,
        identity_id,
        tag_ids: host.data.tag_ids,
        notes: host.data.notes,
        os_name: host.data.os_name,
        agent_forwarding: ssh.as_ref().is_some_and(|s| s.agent_forwarding),
        startup_snippet_id: host.data.startup_snippet_id,
        host_chain_id: ssh.as_ref().and_then(|s| s.host_chain_id),
        proxy_id: ssh.as_ref().and_then(|s| s.proxy_id),
        has_password,
    })
}

/// Create or update a host with its inline ssh_config / identity.
pub fn save(store: &Store, f: &HostForm) -> Result<HostCard> {
    let label = f.label.trim();
    let address = f.address.trim();
    if address.is_empty() {
        return Err(DesktopError::invalid("address is required"));
    }
    if let Some(gid) = f.group_id {
        let g = store.require::<Group>(gid)?;
        if g.vault_id != f.vault_id {
            return Err(DesktopError::invalid("group belongs to another vault"));
        }
    }

    let existing = match f.id {
        Some(id) => store.get::<Host>(id)?,
        None => None,
    };
    let existing_ssh = match existing.as_ref().and_then(|h| h.data.ssh_config_id) {
        Some(c) => store.get::<SshConfig>(c)?,
        None => None,
    };
    let existing_inline_identity = match existing_ssh.as_ref().and_then(|s| s.data.identity_id) {
        Some(i) => store.get::<Identity>(i)?.filter(|e| !e.data.is_visible),
        None => None,
    };

    // Identity: referenced visible one, or an inline hidden one.
    let identity_id = if let Some(vis) = f.identity_id {
        let i = store.require::<Identity>(vis)?;
        if !i.data.is_visible {
            return Err(DesktopError::invalid("identity is not selectable"));
        }
        if let Some(old) = &existing_inline_identity {
            store.delete(old.id)?;
        }
        Some(vis)
    } else {
        let username = f.username.trim().to_string();
        let has_key = f.ssh_key_id.is_some();
        let password = match (&f.password, &existing_inline_identity) {
            (Some(p), _) if p.is_empty() => None,
            (Some(p), _) => Some(p.clone()),
            (None, Some(old)) => old.data.password.clone(),
            (None, None) => None,
        };
        if username.is_empty() && password.is_none() && !has_key {
            if let Some(old) = &existing_inline_identity {
                store.delete(old.id)?;
            }
            None
        } else {
            let data = Identity {
                label: if label.is_empty() {
                    address.to_string()
                } else {
                    label.to_string()
                },
                username,
                password,
                ssh_key_id: f.ssh_key_id,
                ssh_certificate_id: None,
                is_visible: false,
            };
            let id = match &existing_inline_identity {
                Some(old) => {
                    store.update(old.id, &data)?;
                    old.id
                }
                None => store.insert(f.vault_id, &data)?,
            };
            Some(id)
        }
    };

    // Inline ssh_config, keeping fields the form does not edit.
    let mut ssh = existing_ssh
        .as_ref()
        .map(|e| e.data.clone())
        .unwrap_or_default();
    ssh.port = f.port.filter(|p| *p != 0);
    ssh.identity_id = identity_id;
    ssh.agent_forwarding = f.agent_forwarding;
    ssh.host_chain_id = f.host_chain_id;
    ssh.proxy_id = f.proxy_id;
    let ssh_config_id = match &existing_ssh {
        Some(e) => {
            store.update(e.id, &ssh)?;
            e.id
        }
        None => store.insert(f.vault_id, &ssh)?,
    };

    let mut host = existing
        .as_ref()
        .map(|e| e.data.clone())
        .unwrap_or_default();
    host.label = if label.is_empty() {
        address.to_string()
    } else {
        label.to_string()
    };
    host.address = address.to_string();
    host.group_id = f.group_id;
    host.ssh_config_id = Some(ssh_config_id);
    host.tag_ids = f.tag_ids.clone();
    host.notes = f.notes.clone();
    host.os_name = f.os_name.clone().filter(|s| !s.is_empty());
    host.startup_snippet_id = f.startup_snippet_id;
    let host_id = match &existing {
        Some(e) => {
            store.update(e.id, &host)?;
            e.id
        }
        None => store.insert(f.vault_id, &host)?,
    };

    cards(store, Some(f.vault_id))?
        .into_iter()
        .find(|c| c.id == host_id)
        .ok_or_else(|| DesktopError::not_found(format!("host {host_id}")))
}

/// Delete a host together with its inline ssh_config and hidden identity.
pub fn delete(store: &Store, id: Uuid) -> Result<()> {
    let Some(host) = store.get::<Host>(id)? else {
        return Ok(());
    };
    if let Some(cid) = host.data.ssh_config_id
        && let Some(cfg) = store.get::<SshConfig>(cid)?
    {
        if let Some(iid) = cfg.data.identity_id
            && let Some(i) = store.get::<Identity>(iid)?
            && !i.data.is_visible
        {
            store.delete(i.id)?;
        }
        store.delete(cfg.id)?;
    }
    store.delete(id)?;
    Ok(())
}

pub fn save_group(
    store: &Store,
    vault_id: Uuid,
    id: Option<Uuid>,
    label: &str,
    parent_id: Option<Uuid>,
) -> Result<GroupNode> {
    let label = label.trim();
    if label.is_empty() {
        return Err(DesktopError::invalid("group name is required"));
    }
    if let Some(pid) = parent_id {
        if Some(pid) == id {
            return Err(DesktopError::invalid("a group cannot be its own parent"));
        }
        store.require::<Group>(pid)?;
    }
    let mut data = match id {
        Some(id) => store.require::<Group>(id)?.data,
        None => Group::default(),
    };
    data.label = label.to_string();
    data.parent_id = parent_id;
    let gid = match id {
        Some(id) => {
            store.update(id, &data)?;
            id
        }
        None => store.insert(vault_id, &data)?,
    };
    groups(store, Some(vault_id))?
        .into_iter()
        .find(|g| g.id == gid)
        .ok_or_else(|| DesktopError::not_found(format!("group {gid}")))
}

/// Delete a group; its hosts and sub-groups move to the parent.
pub fn delete_group(store: &Store, id: Uuid) -> Result<()> {
    let Some(g) = store.get::<Group>(id)? else {
        return Ok(());
    };
    let hosts: Vec<Entity<Host>> = store.list(Some(g.vault_id))?;
    for mut h in hosts.into_iter().filter(|h| h.data.group_id == Some(id)) {
        h.data.group_id = g.data.parent_id;
        store.update(h.id, &h.data)?;
    }
    let groups: Vec<Entity<Group>> = store.list(Some(g.vault_id))?;
    for mut c in groups.into_iter().filter(|c| c.data.parent_id == Some(id)) {
        c.data.parent_id = g.data.parent_id;
        store.update(c.id, &c.data)?;
    }
    store.delete(id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_core::termoso_crypto::keys::SymmetricKey;

    fn store() -> Store {
        Store::open_in_memory(SymmetricKey::generate()).expect("store")
    }

    fn new_form(vault_id: Uuid) -> HostForm {
        HostForm {
            id: None,
            vault_id,
            label: "prod".into(),
            address: "10.0.0.1".into(),
            group_id: None,
            port: Some(2222),
            username: "deploy".into(),
            password: Some("s3cret".into()),
            ssh_key_id: None,
            identity_id: None,
            tag_ids: vec![],
            notes: String::new(),
            os_name: None,
            agent_forwarding: false,
            startup_snippet_id: None,
            host_chain_id: None,
            proxy_id: None,
            has_password: false,
        }
    }

    #[test]
    fn save_creates_inline_config_and_identity() {
        let s = store();
        let vault = s.local_vault().unwrap().id;
        let card = save(&s, &new_form(vault)).unwrap();
        assert_eq!(card.username, "deploy");
        assert_eq!(card.port, 2222);
        assert_eq!(card.protocol, "ssh");

        let f = form(&s, card.id).unwrap();
        assert!(f.has_password);
        assert!(f.password.is_none());
        assert_eq!(f.username, "deploy");

        let identities: Vec<Entity<Identity>> = s.list(Some(vault)).unwrap();
        assert_eq!(identities.len(), 1);
        assert!(!identities[0].data.is_visible);
    }

    #[test]
    fn edit_keeps_password_unless_cleared() {
        let s = store();
        let vault = s.local_vault().unwrap().id;
        let card = save(&s, &new_form(vault)).unwrap();
        let mut f = form(&s, card.id).unwrap();
        f.username = "ops".into();
        save(&s, &f).unwrap();
        let r = s.resolve_host(card.id).unwrap();
        assert_eq!(r.identity.unwrap().data.password.as_deref(), Some("s3cret"));

        f.password = Some(String::new());
        save(&s, &f).unwrap();
        let r = s.resolve_host(card.id).unwrap();
        assert_eq!(r.identity.unwrap().data.password, None);
    }

    #[test]
    fn group_inheritance_shows_in_card() {
        let s = store();
        let vault = s.local_vault().unwrap().id;
        let cfg_id = s
            .insert(
                vault,
                &SshConfig {
                    port: Some(2200),
                    ..Default::default()
                },
            )
            .unwrap();
        let g = save_group(&s, vault, None, "dc1", None).unwrap();
        let mut gdata = s.require::<Group>(g.id).unwrap().data;
        gdata.ssh_config_id = Some(cfg_id);
        s.update(g.id, &gdata).unwrap();

        let mut f = new_form(vault);
        f.group_id = Some(g.id);
        f.port = None;
        let card = save(&s, &f).unwrap();
        assert_eq!(card.port, 2200);
        assert_eq!(card.group_path, vec!["dc1".to_string()]);
        assert_eq!(groups(&s, Some(vault)).unwrap()[0].host_count, 1);

        delete_group(&s, g.id).unwrap();
        let card = cards(&s, Some(vault)).unwrap().remove(0);
        assert_eq!(card.group_id, None);
        assert_eq!(card.port, 22);
    }

    #[test]
    fn delete_removes_inline_entities() {
        let s = store();
        let vault = s.local_vault().unwrap().id;
        let card = save(&s, &new_form(vault)).unwrap();
        delete(&s, card.id).unwrap();
        assert!(cards(&s, Some(vault)).unwrap().is_empty());
        let cfgs: Vec<Entity<SshConfig>> = s.list(Some(vault)).unwrap();
        let ids: Vec<Entity<Identity>> = s.list(Some(vault)).unwrap();
        assert!(cfgs.is_empty() && ids.is_empty());
    }
}
