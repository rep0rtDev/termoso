//! Session logs (terminal recordings). Metadata is an encrypted envelope in
//! the database; the body is an encrypted file next to it. Both are encrypted
//! with the vault key, so the server and its object storage only ever see
//! ciphertext. Synced through `/logs/*` when signed in.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use termoso_crypto::aead::{self, Aad};
use termoso_proto::logs::SessionLog;
use uuid::Uuid;

use super::{Store, parse_time, parse_uuid};
use crate::error::{CoreError, Result};

/// Plaintext metadata of a recording.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogMeta {
    /// Host the session was on (`None` for local terminal).
    pub host_id: Option<Uuid>,
    /// Label shown in the list.
    pub label: String,
    /// `user@address:port`.
    pub target: String,
    /// Protocol (`ssh`, `telnet`, `local`, `serial`).
    pub protocol: String,
    /// Session start.
    pub started_at: DateTime<Utc>,
    /// Session end (`None` while recording).
    pub ended_at: Option<DateTime<Utc>>,
    /// Terminal size at start.
    pub cols: u16,
    /// Terminal size at start.
    pub rows: u16,
}

/// A recording as listed locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogItem {
    /// Id.
    pub id: Uuid,
    /// Vault.
    pub vault_id: Uuid,
    /// Decrypted metadata.
    pub meta: LogMeta,
    /// Encrypted body size.
    pub size_bytes: i64,
    /// Body is present on this device.
    pub cached: bool,
    /// Body is on the server.
    pub uploaded: bool,
    /// Recording finished.
    pub completed: bool,
    /// Created.
    pub created_at: DateTime<Utc>,
}

/// A row as the sync engine sees it.
#[derive(Debug, Clone)]
pub struct LogRow {
    /// Id.
    pub id: Uuid,
    /// Vault.
    pub vault_id: Uuid,
    /// Meta envelope.
    pub meta: String,
    /// Key version the envelopes were made with.
    pub key_version: i32,
    /// Encrypted body size.
    pub size_bytes: i64,
    /// Encrypted body file.
    pub local_path: Option<PathBuf>,
    /// Server has the body.
    pub uploaded: bool,
    /// Recording finished.
    pub completed: bool,
    /// Server seq (0 = server never saw it).
    pub seq: i64,
    /// Deleted locally.
    pub deleted: bool,
}

fn meta_aad(id: Uuid) -> Aad {
    Aad::label(&["log", &id.to_string()])
}

fn body_aad(id: Uuid) -> Aad {
    Aad::label(&["log", &id.to_string(), "body"])
}

const SELECT: &str = "SELECT id, vault_id, meta, key_version, size_bytes, local_path, uploaded, completed, created_at, seq, deleted FROM session_logs";

type RawLog = (
    String,
    String,
    String,
    i32,
    i64,
    Option<String>,
    bool,
    bool,
    String,
    i64,
    bool,
);

fn row_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawLog> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
        r.get(10)?,
    ))
}

fn into_row(raw: RawLog) -> Result<LogRow> {
    let (
        id,
        vault_id,
        meta,
        key_version,
        size_bytes,
        local_path,
        uploaded,
        completed,
        _created,
        seq,
        deleted,
    ) = raw;
    Ok(LogRow {
        id: parse_uuid(&id)?,
        vault_id: parse_uuid(&vault_id)?,
        meta,
        key_version,
        size_bytes,
        local_path: local_path.map(PathBuf::from),
        uploaded,
        completed,
        seq,
        deleted,
    })
}

impl Store {
    fn log_rows(&self, where_clause: &str) -> Result<Vec<LogRow>> {
        let conn = self.conn();
        let mut st = conn.prepare(&format!("{SELECT} {where_clause} ORDER BY created_at DESC"))?;
        let rows = st.query_map([], row_from)?;
        rows.map(|r| r.map_err(CoreError::from).and_then(into_row))
            .collect()
    }

    fn log_row(&self, id: Uuid) -> Result<LogRow> {
        self.conn()
            .query_row(
                &format!("{SELECT} WHERE id = ?1"),
                params![id.to_string()],
                row_from,
            )
            .optional()?
            .map(into_row)
            .transpose()?
            .ok_or_else(|| CoreError::NotFound(format!("log {id}")))
    }

    /// Start a recording: registers the metadata; the body arrives with
    /// [`Store::finish_log`].
    pub fn begin_log(&self, vault_id: Uuid, meta: &LogMeta) -> Result<Uuid> {
        let key = self.vault_key(vault_id)?;
        let key_version = self.vault(vault_id)?.key_version;
        let id = Uuid::new_v4();
        let ct = aead::encrypt_str(&key, &meta_aad(id), &serde_json::to_string(meta)?)?;
        self.conn().execute(
            "INSERT INTO session_logs (id, vault_id, meta, key_version, size_bytes, local_path, uploaded, completed, created_at, seq, deleted)
             VALUES (?1, ?2, ?3, ?4, 0, NULL, 0, 0, ?5, 0, 0)",
            params![
                id.to_string(),
                vault_id.to_string(),
                ct,
                key_version,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(id)
    }

    /// Store the finished recording: encrypts `body` into `dir/<id>.tlog`,
    /// updates the metadata and marks the log complete (and pending upload).
    pub fn finish_log(&self, id: Uuid, meta: &LogMeta, body: &[u8], dir: &Path) -> Result<PathBuf> {
        let row = self.log_row(id)?;
        let key = self.vault_key(row.vault_id)?;
        let ct = aead::encrypt(&key, &body_aad(id), body)?;
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{id}.tlog"));
        std::fs::write(&path, &ct)?;
        let meta_ct = aead::encrypt_str(&key, &meta_aad(id), &serde_json::to_string(meta)?)?;
        self.conn().execute(
            "UPDATE session_logs SET meta = ?2, size_bytes = ?3, local_path = ?4, completed = 1, uploaded = 0 WHERE id = ?1",
            params![
                id.to_string(),
                meta_ct,
                ct.len() as i64,
                path.to_string_lossy().into_owned()
            ],
        )?;
        Ok(path)
    }

    /// Decrypted recording body. Fails with `NotFound` when the body has not
    /// been downloaded to this device.
    pub fn read_log(&self, id: Uuid) -> Result<Vec<u8>> {
        let row = self.log_row(id)?;
        let path = row
            .local_path
            .ok_or_else(|| CoreError::NotFound(format!("log {id} body not cached")))?;
        let key = self.vault_key(row.vault_id)?;
        let ct = std::fs::read(path)?;
        Ok(aead::decrypt(&key, &body_aad(id), &ct)?)
    }

    /// Recordings visible on this device (deleted ones excluded).
    pub fn logs(&self) -> Result<Vec<LogItem>> {
        let rows = self.log_rows("WHERE deleted = 0")?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let Ok(key) = self.vault_key(r.vault_id) else {
                continue;
            };
            let pt = aead::decrypt_str(&key, &meta_aad(r.id), &r.meta)?;
            let created_at: String = self.conn().query_row(
                "SELECT created_at FROM session_logs WHERE id = ?1",
                params![r.id.to_string()],
                |x| x.get(0),
            )?;
            out.push(LogItem {
                id: r.id,
                vault_id: r.vault_id,
                meta: serde_json::from_str(&pt)?,
                size_bytes: r.size_bytes,
                cached: r.local_path.is_some(),
                uploaded: r.uploaded,
                completed: r.completed,
                created_at: parse_time(&created_at)?,
            });
        }
        Ok(out)
    }

    /// Delete a recording. Removes the body file; the row becomes a tombstone
    /// until the deletion is pushed (or disappears at once when the server
    /// never had it).
    pub fn delete_log(&self, id: Uuid) -> Result<()> {
        let row = self.log_row(id)?;
        if let Some(p) = &row.local_path {
            let _ = std::fs::remove_file(p);
        }
        let conn = self.conn();
        if row.seq == 0 {
            conn.execute(
                "DELETE FROM session_logs WHERE id = ?1",
                params![id.to_string()],
            )?;
        } else {
            conn.execute(
                "UPDATE session_logs SET deleted = 1, local_path = NULL WHERE id = ?1",
                params![id.to_string()],
            )?;
        }
        Ok(())
    }

    // ───────────────────────────── sync helpers ─────────────────────────────

    /// Completed recordings in synced vaults the server does not have yet.
    pub fn logs_to_upload(&self) -> Result<Vec<LogRow>> {
        self.log_rows(
            "WHERE completed = 1 AND uploaded = 0 AND deleted = 0 AND local_path IS NOT NULL
               AND vault_id IN (SELECT id FROM vaults WHERE kind <> 'local')",
        )
    }

    /// Tombstones the server still has to hear about.
    pub fn logs_to_delete_remote(&self) -> Result<Vec<LogRow>> {
        self.log_rows("WHERE deleted = 1 AND seq > 0")
    }

    /// Encrypted body bytes for upload.
    pub fn log_body_ciphertext(&self, id: Uuid) -> Result<Vec<u8>> {
        let row = self.log_row(id)?;
        let path = row
            .local_path
            .ok_or_else(|| CoreError::NotFound(format!("log {id} body not cached")))?;
        Ok(std::fs::read(path)?)
    }

    /// Server accepted the upload.
    pub fn mark_log_uploaded(&self, id: Uuid, seq: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE session_logs SET uploaded = 1, seq = ?2 WHERE id = ?1",
            params![id.to_string(), seq],
        )?;
        Ok(())
    }

    /// Forget a row entirely (deletion acknowledged by the server, or the
    /// server never had the object).
    pub fn forget_log(&self, id: Uuid) -> Result<()> {
        if let Ok(row) = self.log_row(id)
            && let Some(p) = row.local_path
        {
            let _ = std::fs::remove_file(p);
        }
        self.conn().execute(
            "DELETE FROM session_logs WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    /// Mirror server metadata. Bodies are fetched lazily with
    /// [`Store::cache_log_body`]; local tombstones win over remote rows.
    pub fn apply_remote_logs(&self, logs: &[SessionLog]) -> Result<()> {
        let conn = self.conn();
        for l in logs {
            if l.deleted {
                let path: Option<String> = conn
                    .query_row(
                        "SELECT local_path FROM session_logs WHERE id = ?1",
                        params![l.id.to_string()],
                        |r| r.get(0),
                    )
                    .optional()?
                    .flatten();
                if let Some(p) = path {
                    let _ = std::fs::remove_file(p);
                }
                conn.execute(
                    "DELETE FROM session_logs WHERE id = ?1",
                    params![l.id.to_string()],
                )?;
                continue;
            }
            conn.execute(
                "INSERT INTO session_logs (id, vault_id, meta, key_version, size_bytes, local_path, uploaded, completed, created_at, seq, deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, 1, ?6, ?7, ?8, 0)
                 ON CONFLICT(id) DO UPDATE SET
                   meta = CASE WHEN session_logs.deleted = 1 THEN session_logs.meta ELSE excluded.meta END,
                   key_version = excluded.key_version, size_bytes = excluded.size_bytes,
                   uploaded = 1, completed = excluded.completed, seq = excluded.seq",
                params![
                    l.id.to_string(),
                    l.vault_id.to_string(),
                    l.meta,
                    l.key_version,
                    l.size_bytes,
                    l.completed,
                    l.created_at.to_rfc3339(),
                    l.seq,
                ],
            )?;
        }
        Ok(())
    }

    /// Store a downloaded (still encrypted) body in `dir`.
    pub fn cache_log_body(&self, id: Uuid, ciphertext: &[u8], dir: &Path) -> Result<PathBuf> {
        let row = self.log_row(id)?;
        let key = self.vault_key(row.vault_id)?;
        // Verify before trusting the bytes we got from object storage.
        aead::decrypt(&key, &body_aad(id), ciphertext)?;
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{id}.tlog"));
        std::fs::write(&path, ciphertext)?;
        self.conn().execute(
            "UPDATE session_logs SET local_path = ?2, size_bytes = ?3 WHERE id = ?1",
            params![
                id.to_string(),
                path.to_string_lossy().into_owned(),
                ciphertext.len() as i64
            ],
        )?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::LocalVaultKind;
    use termoso_crypto::keys::SymmetricKey;
    use termoso_proto::vault::VaultRole;

    fn meta() -> LogMeta {
        LogMeta {
            host_id: None,
            label: "box".into(),
            target: "root@box:22".into(),
            protocol: "ssh".into(),
            started_at: Utc::now(),
            ended_at: None,
            cols: 80,
            rows: 24,
        }
    }

    #[test]
    fn record_read_delete_roundtrip() {
        let store = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let local = store.local_vault().unwrap().id;
        let id = store.begin_log(local, &meta()).unwrap();
        assert!(!store.logs().unwrap()[0].completed);
        let body = b"$ ls\r\nfoo bar\r\n".repeat(100);
        let mut m = meta();
        m.ended_at = Some(Utc::now());
        let path = store.finish_log(id, &m, &body, dir.path()).unwrap();
        assert!(path.exists());
        assert_ne!(std::fs::read(&path).unwrap(), body);
        assert_eq!(store.read_log(id).unwrap(), body);
        let items = store.logs().unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].completed && items[0].cached && !items[0].uploaded);
        assert_eq!(items[0].meta, m);
        // Local vault never uploads.
        assert!(store.logs_to_upload().unwrap().is_empty());
        store.delete_log(id).unwrap();
        assert!(!path.exists());
        assert!(store.logs().unwrap().is_empty());
        assert!(store.logs_to_delete_remote().unwrap().is_empty());
    }

    fn synced_store() -> (Store, Uuid) {
        let store = Store::open_in_memory(SymmetricKey::generate()).unwrap();
        let vid = Uuid::new_v4();
        store
            .upsert_vault(
                vid,
                LocalVaultKind::Personal,
                "Personal",
                None,
                VaultRole::Manager,
                Some(&SymmetricKey::generate()),
                1,
            )
            .unwrap();
        (store, vid)
    }

    #[test]
    fn synced_logs_queue_uploads_and_tombstones() {
        let (store, vid) = synced_store();
        let dir = tempfile::tempdir().unwrap();
        let id = store.begin_log(vid, &meta()).unwrap();
        assert!(store.logs_to_upload().unwrap().is_empty(), "unfinished");

        let path = store.finish_log(id, &meta(), b"body", dir.path()).unwrap();
        let queued = store.logs_to_upload().unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(
            queued[0].size_bytes as usize,
            std::fs::read(&path).unwrap().len()
        );
        assert_eq!(
            store.log_body_ciphertext(id).unwrap(),
            std::fs::read(&path).unwrap()
        );

        store.mark_log_uploaded(id, 7).unwrap();
        assert!(store.logs_to_upload().unwrap().is_empty());
        assert!(store.logs().unwrap()[0].uploaded);

        // Deleting a synced log leaves a tombstone until the server hears.
        store.delete_log(id).unwrap();
        assert!(!path.exists());
        assert!(store.logs().unwrap().is_empty());
        let tomb = store.logs_to_delete_remote().unwrap();
        assert_eq!(tomb.len(), 1);
        assert_eq!(tomb[0].seq, 7);

        // A remote echo of the row must not resurrect it.
        store
            .apply_remote_logs(&[SessionLog {
                id,
                vault_id: vid,
                meta: tomb[0].meta.clone(),
                key_version: 1,
                size_bytes: 4,
                completed: true,
                created_at: Utc::now(),
                seq: 7,
                deleted: false,
            }])
            .unwrap();
        assert!(store.logs().unwrap().is_empty());
        assert_eq!(store.logs_to_delete_remote().unwrap().len(), 1);

        store.forget_log(id).unwrap();
        assert!(store.logs_to_delete_remote().unwrap().is_empty());
        assert!(store.log_row(id).is_err());
    }

    #[test]
    fn cached_body_is_verified_against_vault_key() {
        let (store, vid) = synced_store();
        let dir = tempfile::tempdir().unwrap();
        let id = store.begin_log(vid, &meta()).unwrap();
        let mut ct =
            aead::encrypt(&store.vault_key(vid).unwrap(), &body_aad(id), b"hello").unwrap();
        assert_eq!(
            store.cache_log_body(id, &ct, dir.path()).unwrap(),
            dir.path().join(format!("{id}.tlog"))
        );
        assert_eq!(store.read_log(id).unwrap(), b"hello");

        *ct.last_mut().unwrap() ^= 1;
        assert!(store.cache_log_body(id, &ct, dir.path()).is_err());
        assert_eq!(
            store.read_log(id).unwrap(),
            b"hello",
            "tampered download not persisted"
        );
    }
}
