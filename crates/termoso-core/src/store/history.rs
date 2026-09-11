//! Command / connection history (autocomplete, "recent" list). Stored
//! encrypted like entities; synced through `/history/*` when signed in.

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use termoso_crypto::aead::{self, Aad};
use termoso_proto::sync::{HistoryEntry, HistoryKind};
use uuid::Uuid;

use super::{Store, parse_time, parse_uuid};
use crate::error::{CoreError, Result};

/// A shell command the user typed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandHistory {
    /// Host it was typed on (`None` for local terminal).
    pub host_id: Option<Uuid>,
    /// The command line.
    pub command: String,
}

/// A connection that was made.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionHistory {
    /// Host (if it was a saved host).
    pub host_id: Option<Uuid>,
    /// Label shown in "recent".
    pub label: String,
    /// `user@address:port`.
    pub target: String,
    /// Protocol (`ssh`, `sftp`, `telnet`, `local`, `serial`).
    pub protocol: String,
    /// Seconds the session lasted (`None` while open).
    pub duration_secs: Option<u64>,
    /// Ended with an error.
    pub error: Option<String>,
}

/// Decrypted history record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem<T> {
    /// Id.
    pub id: Uuid,
    /// When.
    pub created_at: DateTime<Utc>,
    /// Payload.
    pub data: T,
}

fn kind_str(k: HistoryKind) -> &'static str {
    match k {
        HistoryKind::Command => "command",
        HistoryKind::Connection => "connection",
    }
}

fn parse_kind(s: &str) -> Result<HistoryKind> {
    Ok(match s {
        "command" => HistoryKind::Command,
        "connection" => HistoryKind::Connection,
        other => return Err(CoreError::Invalid(format!("history kind {other}"))),
    })
}

fn aad(kind: HistoryKind, id: Uuid) -> Aad {
    Aad::label(&["history", kind_str(kind), &id.to_string()])
}

impl Store {
    /// Vault whose key protects history: the personal vault when signed in,
    /// otherwise the local vault.
    fn history_vault(&self) -> Result<Uuid> {
        if let Some(p) = self.personal_vault()?
            && p.unlocked
        {
            return Ok(p.id);
        }
        Ok(self.local_vault()?.id)
    }

    fn record_history<T: Serialize>(&self, kind: HistoryKind, data: &T) -> Result<Uuid> {
        let vault_id = self.history_vault()?;
        let key = self.vault_key(vault_id)?;
        let key_version = self.vault(vault_id)?.key_version;
        let id = Uuid::new_v4();
        let ct = aead::encrypt_str(&key, &aad(kind, id), &serde_json::to_string(data)?)?;
        let dirty = self.vault(vault_id)?.kind.is_synced();
        self.conn().execute(
            "INSERT INTO history (id, kind, vault_id, data, key_version, created_at, seq, deleted, dirty)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7)",
            params![
                id.to_string(),
                kind_str(kind),
                vault_id.to_string(),
                ct,
                key_version,
                Utc::now().to_rfc3339(),
                dirty
            ],
        )?;
        Ok(id)
    }

    /// Record a command.
    pub fn record_command(&self, data: &CommandHistory) -> Result<Uuid> {
        if data.command.trim().is_empty() {
            return Err(CoreError::Invalid("empty command".into()));
        }
        self.record_history(HistoryKind::Command, data)
    }

    /// Record a connection.
    pub fn record_connection(&self, data: &ConnectionHistory) -> Result<Uuid> {
        self.record_history(HistoryKind::Connection, data)
    }

    fn list_history<T: serde::de::DeserializeOwned>(
        &self,
        kind: HistoryKind,
        limit: usize,
    ) -> Result<Vec<HistoryItem<T>>> {
        let rows: Vec<(String, String, String, String)> = {
            let conn = self.conn();
            let mut st = conn.prepare(
                "SELECT id, vault_id, data, created_at FROM history
                 WHERE kind = ?1 AND deleted = 0 ORDER BY created_at DESC LIMIT ?2",
            )?;
            st.query_map(params![kind_str(kind), limit as i64], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut out = Vec::with_capacity(rows.len());
        for (id, vault_id, data, created_at) in rows {
            let id = parse_uuid(&id)?;
            let Ok(key) = self.vault_key(parse_uuid(&vault_id)?) else {
                continue;
            };
            let pt = aead::decrypt_str(&key, &aad(kind, id), &data)?;
            out.push(HistoryItem {
                id,
                created_at: parse_time(&created_at)?,
                data: serde_json::from_str(&pt)?,
            });
        }
        Ok(out)
    }

    /// Most recent commands.
    pub fn commands(&self, limit: usize) -> Result<Vec<HistoryItem<CommandHistory>>> {
        self.list_history(HistoryKind::Command, limit)
    }

    /// Most recent connections.
    pub fn connections(&self, limit: usize) -> Result<Vec<HistoryItem<ConnectionHistory>>> {
        self.list_history(HistoryKind::Connection, limit)
    }

    /// Distinct commands matching a prefix, most recent first (autocomplete).
    pub fn complete_command(&self, prefix: &str, limit: usize) -> Result<Vec<String>> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for item in self.commands(2000)? {
            if item.data.command.starts_with(prefix) && seen.insert(item.data.command.clone()) {
                out.push(item.data.command);
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    /// Delete all history of a kind (or all). Synced rows become tombstones.
    pub fn clear_history(&self, kind: Option<HistoryKind>) -> Result<()> {
        let conn = self.conn();
        let filter = kind
            .map(|k| format!(" AND kind = '{}'", kind_str(k)))
            .unwrap_or_default();
        conn.execute(
            &format!("UPDATE history SET deleted = 1, dirty = 1, data = '' WHERE seq > 0{filter}"),
            [],
        )?;
        conn.execute(&format!("DELETE FROM history WHERE seq = 0{filter}"), [])?;
        Ok(())
    }

    /// Rows to push (`dirty = 1`) as wire entries.
    pub fn dirty_history(&self) -> Result<Vec<HistoryEntry>> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT id, kind, data, key_version, created_at, seq, deleted FROM history WHERE dirty = 1",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i32>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, bool>(6)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (id, kind, data, key_version, created_at, seq, deleted) = r?;
            out.push(HistoryEntry {
                id: parse_uuid(&id)?,
                kind: parse_kind(&kind)?,
                data,
                key_version,
                created_at: parse_time(&created_at)?,
                seq,
                deleted,
            });
        }
        Ok(out)
    }

    /// Mark pushed history rows clean.
    pub fn mark_history_pushed(&self, ids: &[Uuid], seq: i64) -> Result<()> {
        let conn = self.conn();
        for id in ids {
            conn.execute(
                "UPDATE history SET dirty = 0, seq = CASE WHEN seq = 0 THEN ?2 ELSE seq END WHERE id = ?1",
                params![id.to_string(), seq],
            )?;
        }
        conn.execute("DELETE FROM history WHERE deleted = 1 AND dirty = 0", [])?;
        Ok(())
    }

    /// Apply entries pulled from the server (personal vault key).
    pub fn apply_remote_history(&self, entries: &[HistoryEntry]) -> Result<()> {
        let Some(personal) = self.personal_vault()? else {
            return Ok(());
        };
        let conn = self.conn();
        for e in entries {
            if e.deleted {
                conn.execute(
                    "DELETE FROM history WHERE id = ?1",
                    params![e.id.to_string()],
                )?;
                continue;
            }
            conn.execute(
                "INSERT INTO history (id, kind, vault_id, data, key_version, created_at, seq, deleted, dirty)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0)
                 ON CONFLICT(id) DO UPDATE SET data = excluded.data, key_version = excluded.key_version,
                   seq = excluded.seq, deleted = 0, dirty = 0",
                params![
                    e.id.to_string(),
                    kind_str(e.kind),
                    personal.id.to_string(),
                    e.data,
                    e.key_version,
                    e.created_at.to_rfc3339(),
                    e.seq
                ],
            )?;
        }
        Ok(())
    }

    /// Move local-vault history into the personal vault after sign-in so it
    /// starts syncing (re-encrypts with the personal key).
    pub fn adopt_local_history(&self) -> Result<usize> {
        let Some(personal) = self.personal_vault()? else {
            return Ok(0);
        };
        let local = self.local_vault()?;
        let (lk, pk) = (self.vault_key(local.id)?, self.vault_key(personal.id)?);
        let rows: Vec<(String, String, String)> = {
            let conn = self.conn();
            let mut st = conn.prepare(
                "SELECT id, kind, data FROM history WHERE vault_id = ?1 AND deleted = 0",
            )?;
            st.query_map(params![local.id.to_string()], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let conn = self.conn();
        for (id, kind, data) in &rows {
            let uid = parse_uuid(id)?;
            let k = parse_kind(kind)?;
            let pt = aead::decrypt_str(&lk, &aad(k, uid), data)?;
            let ct = aead::encrypt_str(&pk, &aad(k, uid), &pt)?;
            conn.execute(
                "UPDATE history SET vault_id = ?2, data = ?3, key_version = ?4, dirty = 1 WHERE id = ?1",
                params![id, personal.id.to_string(), ct, personal.key_version],
            )?;
        }
        Ok(rows.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_crypto::keys::SymmetricKey;

    #[test]
    fn commands_and_autocomplete() {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        for c in ["ls -la", "git status", "git push", "ls"] {
            s.record_command(&CommandHistory {
                host_id: None,
                command: c.into(),
            })
            .unwrap();
        }
        assert!(
            s.record_command(&CommandHistory {
                host_id: None,
                command: "  ".into()
            })
            .is_err()
        );
        let all = s.commands(10).unwrap();
        assert_eq!(all.len(), 4);
        assert_eq!(all[0].data.command, "ls");
        let git = s.complete_command("git", 10).unwrap();
        assert_eq!(git, vec!["git push", "git status"]);
        assert!(
            s.dirty_history().unwrap().is_empty(),
            "local vault never syncs"
        );
        s.clear_history(Some(HistoryKind::Command)).unwrap();
        assert!(s.commands(10).unwrap().is_empty());
    }

    #[test]
    fn connections_recorded() {
        let s = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        s.record_connection(&ConnectionHistory {
            host_id: None,
            label: "quick".into(),
            target: "root@1.2.3.4:22".into(),
            protocol: "ssh".into(),
            duration_secs: Some(3),
            error: None,
        })
        .unwrap();
        assert_eq!(s.connections(5).unwrap()[0].data.target, "root@1.2.3.4:22");
    }
}
