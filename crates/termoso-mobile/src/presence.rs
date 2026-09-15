//! Team presence on mobile: which team-vault hosts this phone is connected
//! to, and who else is on the team's hosts.
//!
//! Every terminal, SFTP browser and tunnel registers a [`Slot`] when it is
//! opened for a saved host; the slot reports "connected" / "gone" and the
//! [`Tracker`] turns the live set into the snapshot the sync engine sends.
//! Only routing metadata leaves the phone (vault, host, protocol, start
//! time); local- and personal-vault hosts are filtered out here.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use chrono::{DateTime, Utc};
use termoso_core::model::Host;
use termoso_core::store::{LocalVaultKind, Store};
use termoso_core::sync::SyncEngine;
use termoso_proto::team::{PresenceEntry, PresenceSession, TeamPresence};
use uuid::Uuid;

struct Live {
    host_id: Uuid,
    protocol: &'static str,
    since: DateTime<Utc>,
}

/// This device's open connections, shared by every session object and the
/// account runtime.
pub(crate) struct Tracker {
    store: Arc<Store>,
    live: Mutex<HashMap<u64, Live>>,
    next: AtomicU64,
    engine: Mutex<Weak<SyncEngine>>,
}

impl Tracker {
    pub(crate) fn new(store: Arc<Store>) -> Arc<Self> {
        Arc::new(Self {
            store,
            live: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            engine: Mutex::new(Weak::new()),
        })
    }

    /// Publish through `engine` from now on (and push the current set once).
    pub(crate) fn attach(&self, engine: &Arc<SyncEngine>) {
        *self.engine.lock().unwrap_or_else(|p| p.into_inner()) = Arc::downgrade(engine);
        self.publish();
    }

    /// A connection to `host_id` that will report when it is up and down.
    pub(crate) fn slot(self: &Arc<Self>, host_id: Uuid, protocol: &'static str) -> Slot {
        Slot {
            tracker: self.clone(),
            key: self.next.fetch_add(1, Ordering::Relaxed),
            host_id,
            protocol,
        }
    }

    fn connected(&self, key: u64, host_id: Uuid, protocol: &'static str) {
        let changed = {
            let mut live = self.live.lock().unwrap_or_else(|p| p.into_inner());
            match live.entry(key) {
                Entry::Occupied(_) => false,
                Entry::Vacant(slot) => {
                    slot.insert(Live {
                        host_id,
                        protocol,
                        since: Utc::now(),
                    });
                    true
                }
            }
        };
        if changed {
            self.publish();
        }
    }

    fn gone(&self, key: u64) {
        let removed = self
            .live
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&key)
            .is_some();
        if removed {
            self.publish();
        }
    }

    /// Team-vault sessions open on this device right now.
    pub(crate) fn collect(&self) -> Vec<PresenceSession> {
        let team_vaults: HashSet<Uuid> = self
            .store
            .vaults()
            .unwrap_or_default()
            .into_iter()
            .filter(|v| v.kind == LocalVaultKind::Team)
            .map(|v| v.id)
            .collect();
        if team_vaults.is_empty() {
            return Vec::new();
        }
        let live = self.live.lock().unwrap_or_else(|p| p.into_inner());
        live.values()
            .filter_map(|l| {
                let host = self.store.require::<Host>(l.host_id).ok()?;
                team_vaults
                    .contains(&host.vault_id)
                    .then(|| PresenceSession {
                        vault_id: host.vault_id,
                        host_id: l.host_id,
                        protocol: l.protocol.to_string(),
                        since: l.since,
                    })
            })
            .collect()
    }

    fn publish(&self) {
        let engine = self
            .engine
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .upgrade();
        if let Some(engine) = engine {
            engine.set_presence(self.collect());
        }
    }
}

/// One connection's registration. Dropping it (the session object going
/// away) counts as "gone".
pub(crate) struct Slot {
    tracker: Arc<Tracker>,
    key: u64,
    host_id: Uuid,
    protocol: &'static str,
}

impl Slot {
    /// The connection is fully up (shell open / SFTP ready / tunnel bound).
    pub(crate) fn connected(&self) {
        self.tracker
            .connected(self.key, self.host_id, self.protocol);
    }

    /// The connection ended, failed or is reconnecting.
    pub(crate) fn gone(&self) {
        self.tracker.gone(self.key);
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.gone();
    }
}

// ---- what teammates see -------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PresenceSessionCard {
    pub vault_id: String,
    pub host_id: String,
    /// `ssh`, `mosh`, `telnet`, `sftp`, `forward`.
    pub protocol: String,
    /// RFC 3339.
    pub since: String,
}

/// One teammate device and what it is connected to.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PresenceEntryCard {
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub device_id: String,
    pub device_name: String,
    /// `android`, `linux`, `windows`, `macos`, …
    pub platform: String,
    pub sessions: Vec<PresenceSessionCard>,
    /// RFC 3339.
    pub seen_at: String,
    /// This is the signed-in user (possibly another of their devices).
    pub me: bool,
}

/// Who is on the team's hosts right now. `entries` is empty while the team
/// has presence switched off.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TeamPresenceCard {
    pub enabled: bool,
    pub entries: Vec<PresenceEntryCard>,
}

pub(crate) fn card(p: TeamPresence, me: Option<Uuid>) -> TeamPresenceCard {
    TeamPresenceCard {
        enabled: p.enabled,
        entries: p.entries.into_iter().map(|e| entry_card(e, me)).collect(),
    }
}

fn entry_card(e: PresenceEntry, me: Option<Uuid>) -> PresenceEntryCard {
    PresenceEntryCard {
        me: me == Some(e.user_id),
        user_id: e.user_id.to_string(),
        email: e.email,
        display_name: e.display_name,
        device_id: e.device_id.to_string(),
        device_name: e.device_name,
        platform: e.platform,
        sessions: e
            .sessions
            .into_iter()
            .map(|s| PresenceSessionCard {
                vault_id: s.vault_id.to_string(),
                host_id: s.host_id.to_string(),
                protocol: s.protocol,
                since: s.since.to_rfc3339(),
            })
            .collect(),
        seen_at: e.seen_at.to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_crypto::keys::SymmetricKey;
    use termoso_proto::vault::VaultRole;

    fn host(store: &Store, vault: Uuid, label: &str) -> Uuid {
        store
            .insert(
                vault,
                &Host {
                    label: label.into(),
                    address: label.into(),
                    ..Default::default()
                },
            )
            .expect("host")
    }

    #[test]
    fn team_hosts_only_and_slots_track_lifecycle() {
        let store = Arc::new(Store::open_in_memory(SymmetricKey::generate()).expect("store"));
        let local = store.local_vault().expect("local vault").id;
        let team = Uuid::new_v4();
        store
            .upsert_vault(
                team,
                LocalVaultKind::Team,
                "Ops",
                Some(Uuid::new_v4()),
                VaultRole::Editor,
                Some(&SymmetricKey::generate()),
                1,
            )
            .expect("team vault");
        let local_host = host(&store, local, "a.local");
        let team_host = host(&store, team, "b.team");

        let tracker = Tracker::new(store);
        let a = tracker.slot(local_host, "ssh");
        let b = tracker.slot(team_host, "ssh");
        let b2 = tracker.slot(team_host, "sftp");
        a.connected();
        b.connected();
        assert_eq!(
            tracker.collect().len(),
            1,
            "local-vault host is never reported"
        );
        b2.connected();
        let mut protocols: Vec<String> =
            tracker.collect().into_iter().map(|s| s.protocol).collect();
        protocols.sort();
        assert_eq!(protocols, ["sftp", "ssh"]);
        b.gone();
        assert_eq!(tracker.collect().len(), 1);
        drop(b2);
        assert!(
            tracker.collect().is_empty(),
            "dropping a slot counts as gone"
        );
        b.connected();
        b.connected();
        assert_eq!(tracker.collect().len(), 1, "connected is idempotent");
    }
}
