//! Session logs (terminal recordings). Metadata is an encrypted envelope in
//! the database; the body is an encrypted file next to it. Both are encrypted
//! with the vault key, so the server and its object storage only ever see
//! ciphertext. Synced through `/logs/*` when signed in.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use termoso_crypto::aead::{self, Aad};
use termoso_proto::logs::{LogAuthor, SessionLog};
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

/// Capture stops growing after this much output; the tail is dropped and
/// the recording is flagged truncated.
pub const MAX_CAPTURE_BYTES: usize = 64 * 1024 * 1024;

/// In-flight recording of one terminal session.
///
/// Only bytes received from the remote side are captured, never keystrokes:
/// anything typed while the remote has echo turned off (passwords, sudo
/// prompts) therefore never reaches the log, and auth answers given through
/// the app's own prompt dialogs are not part of the stream at all.
pub struct Recorder {
    id: Uuid,
    meta: LogMeta,
    buf: Mutex<Vec<u8>>,
    truncated: AtomicBool,
}

impl Recorder {
    /// Register the recording in `vault_id` and start buffering.
    pub fn begin(store: &Store, vault_id: Uuid, meta: LogMeta) -> Result<Self> {
        let id = store.begin_log(vault_id, &meta)?;
        Ok(Self {
            id,
            meta,
            buf: Mutex::new(Vec::new()),
            truncated: AtomicBool::new(false),
        })
    }

    /// Row id of the recording being written.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Buffer remote output; silently stops at [`MAX_CAPTURE_BYTES`].
    pub fn append(&self, bytes: &[u8]) {
        let mut buf = self.buf.lock().unwrap_or_else(|p| p.into_inner());
        let room = MAX_CAPTURE_BYTES.saturating_sub(buf.len());
        if bytes.len() > room {
            buf.extend_from_slice(&bytes[..room]);
            self.truncated.store(true, Ordering::Relaxed);
        } else {
            buf.extend_from_slice(bytes);
        }
    }

    /// Persist the capture under `dir`; called once when the session ends.
    pub fn finish(&self, store: &Store, dir: &Path) -> Result<()> {
        let body = std::mem::take(&mut *self.buf.lock().unwrap_or_else(|p| p.into_inner()));
        let mut meta = self.meta.clone();
        meta.ended_at = Some(Utc::now());
        if self.truncated.load(Ordering::Relaxed) {
            meta.label = format!("{} (truncated)", meta.label);
        }
        store.finish_log(self.id, &meta, &body, dir)?;
        Ok(())
    }
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
    /// Recorded by this account (or on this device, for the local vault).
    pub mine: bool,
    /// Who recorded it, when known from the server.
    pub author: Option<LogAuthor>,
    /// Pinned by a teammate.
    pub pinned: bool,
    /// Team note.
    pub note: String,
    /// Who wrote the note last.
    pub note_by: Option<Uuid>,
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

const SELECT: &str = "SELECT id, vault_id, meta, key_version, size_bytes, local_path, uploaded, completed, created_at, seq, deleted,
                             author_id, author, pinned, note, note_by FROM session_logs";

/// Everything in a row; `LogRow` is the sync-relevant subset.
struct RawLog {
    row: LogRow,
    created_at: String,
    author_id: Option<String>,
    author: Option<String>,
    pinned: bool,
    note: String,
    note_by: Option<String>,
}

fn row_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawLog> {
    let id: String = r.get(0)?;
    let vault_id: String = r.get(1)?;
    let local_path: Option<String> = r.get(5)?;
    let row = LogRow {
        id: parse_uuid(&id).map_err(|e| invalid(0, e))?,
        vault_id: parse_uuid(&vault_id).map_err(|e| invalid(1, e))?,
        meta: r.get(2)?,
        key_version: r.get(3)?,
        size_bytes: r.get(4)?,
        local_path: local_path.map(PathBuf::from),
        uploaded: r.get(6)?,
        completed: r.get(7)?,
        seq: r.get(9)?,
        deleted: r.get(10)?,
    };
    Ok(RawLog {
        row,
        created_at: r.get(8)?,
        author_id: r.get(11)?,
        author: r.get(12)?,
        pinned: r.get(13)?,
        note: r.get(14)?,
        note_by: r.get(15)?,
    })
}

fn invalid(col: usize, e: CoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e))
}

impl Store {
    fn raw_rows(&self, where_clause: &str) -> Result<Vec<RawLog>> {
        let conn = self.conn();
        let mut st = conn.prepare(&format!(
            "{SELECT} {where_clause} ORDER BY pinned DESC, created_at DESC"
        ))?;
        let rows = st.query_map([], row_from)?;
        rows.map(|r| r.map_err(CoreError::from)).collect()
    }

    fn log_rows(&self, where_clause: &str) -> Result<Vec<LogRow>> {
        Ok(self
            .raw_rows(where_clause)?
            .into_iter()
            .map(|r| r.row)
            .collect())
    }

    fn log_row(&self, id: Uuid) -> Result<LogRow> {
        self.conn()
            .query_row(
                &format!("{SELECT} WHERE id = ?1"),
                params![id.to_string()],
                row_from,
            )
            .optional()?
            .map(|r| r.row)
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

    /// Recordings visible on this device (deleted ones excluded), pinned
    /// first, then newest first. Teammates' recordings are included once
    /// their metadata has been pulled; rows whose vault key we do not hold
    /// are skipped.
    pub fn logs(&self) -> Result<Vec<LogItem>> {
        let account = self.account()?;
        let me = account.as_ref().map(|a| a.user_id);
        let self_author = account.map(|a| LogAuthor {
            user_id: a.user_id,
            email: a.email,
            display_name: a.display_name,
            avatar_tag: a.avatar,
        });
        let rows = self.raw_rows("WHERE deleted = 0")?;
        let mut out = Vec::with_capacity(rows.len());
        for raw in rows {
            let r = raw.row;
            let Ok(key) = self.vault_key(r.vault_id) else {
                continue;
            };
            let pt = match aead::decrypt_str(&key, &meta_aad(r.id), &r.meta) {
                Ok(pt) => pt,
                // A teammate's row encrypted with a key version we have not
                // caught up with yet; it will decrypt after the next pull.
                Err(_) if raw.author_id.is_some() => continue,
                Err(e) => return Err(e.into()),
            };
            let author_id = raw.author_id.as_deref().map(parse_uuid).transpose()?;
            let mine = author_id.is_none_or(|a| Some(a) == me);
            // Own recordings the server has not echoed back yet are ours.
            let author = match raw.author.as_deref() {
                Some(json) => Some(serde_json::from_str::<LogAuthor>(json)?),
                None if mine => self_author.clone(),
                None => None,
            };
            out.push(LogItem {
                id: r.id,
                vault_id: r.vault_id,
                meta: serde_json::from_str(&pt)?,
                size_bytes: r.size_bytes,
                cached: r.local_path.is_some(),
                uploaded: r.uploaded,
                completed: r.completed,
                created_at: parse_time(&raw.created_at)?,
                mine,
                author,
                pinned: raw.pinned,
                note: raw.note,
                note_by: raw.note_by.as_deref().map(parse_uuid).transpose()?,
            });
        }
        Ok(out)
    }

    /// Apply a pin/note change the server confirmed (or, for local-only
    /// recordings, the user's own annotation).
    pub fn annotate_log(
        &self,
        id: Uuid,
        pinned: bool,
        note: &str,
        note_by: Option<Uuid>,
    ) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE session_logs SET pinned = ?2, note = ?3, note_by = ?4 WHERE id = ?1 AND deleted = 0",
            params![
                id.to_string(),
                pinned,
                note,
                note_by.map(|u| u.to_string())
            ],
        )?;
        if n == 0 {
            return Err(CoreError::NotFound(format!("log {id}")));
        }
        Ok(())
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

    /// Completed recordings that are not on this device yet (e.g. teammates'
    /// uploads), for prefetching.
    pub fn logs_without_body(&self) -> Result<Vec<LogRow>> {
        self.log_rows("WHERE deleted = 0 AND completed = 1 AND uploaded = 1 AND local_path IS NULL")
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

    /// Undo a local tombstone the server refused (the body stays gone until
    /// downloaded again).
    pub fn restore_log(&self, id: Uuid) -> Result<()> {
        self.conn().execute(
            "UPDATE session_logs SET deleted = 0 WHERE id = ?1",
            params![id.to_string()],
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
            let author = l.author.as_ref().map(serde_json::to_string).transpose()?;
            conn.execute(
                "INSERT INTO session_logs (id, vault_id, meta, key_version, size_bytes, local_path, uploaded, completed, created_at, seq, deleted,
                                           author_id, author, pinned, note, note_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, 1, ?6, ?7, ?8, 0, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(id) DO UPDATE SET
                   meta = CASE WHEN session_logs.deleted = 1 THEN session_logs.meta ELSE excluded.meta END,
                   key_version = excluded.key_version, size_bytes = excluded.size_bytes,
                   uploaded = 1, completed = excluded.completed, seq = excluded.seq,
                   author_id = excluded.author_id, author = excluded.author,
                   pinned = excluded.pinned, note = excluded.note, note_by = excluded.note_by",
                params![
                    l.id.to_string(),
                    l.vault_id.to_string(),
                    l.meta,
                    l.key_version,
                    l.size_bytes,
                    l.completed,
                    l.created_at.to_rfc3339(),
                    l.seq,
                    (!l.user_id.is_nil()).then(|| l.user_id.to_string()),
                    author,
                    l.pinned,
                    l.note,
                    l.note_by.map(|u| u.to_string()),
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
                user_id: Uuid::nil(),
                author: None,
                meta: tomb[0].meta.clone(),
                key_version: 1,
                size_bytes: 4,
                completed: true,
                created_at: Utc::now(),
                seq: 7,
                deleted: false,
                pinned: false,
                note: String::new(),
                note_by: None,
            }])
            .unwrap();
        assert!(store.logs().unwrap().is_empty());
        assert_eq!(store.logs_to_delete_remote().unwrap().len(), 1);

        store.forget_log(id).unwrap();
        assert!(store.logs_to_delete_remote().unwrap().is_empty());
        assert!(store.log_row(id).is_err());
    }

    #[test]
    fn teammates_recordings_keep_author_pin_and_note() {
        let (store, vid) = synced_store();
        let dir = tempfile::tempdir().unwrap();
        let key = store.vault_key(vid).unwrap();
        let me = Uuid::new_v4();
        let mate = Uuid::new_v4();
        let kp = termoso_crypto::keys::KeyPair::generate();
        store
            .save_account(
                &crate::store::StoredAccount {
                    server_url: "https://t.example".into(),
                    user_id: me,
                    email: "me@example.com".into(),
                    display_name: None,
                    avatar: None,
                    is_admin: false,
                    device_id: Uuid::new_v4(),
                    public_key: kp.public_b64(),
                    key_version: 1,
                    history_cursor: 0,
                    logs_cursor: 0,
                    signed_in_at: Utc::now(),
                },
                "tok",
                &kp,
            )
            .unwrap();
        store
            .upsert_vault(
                vid,
                LocalVaultKind::Team,
                "Ops",
                Some(Uuid::new_v4()),
                VaultRole::Editor,
                Some(&key),
                1,
            )
            .unwrap();

        let theirs = Uuid::new_v4();
        let m = meta();
        let meta_ct =
            aead::encrypt_str(&key, &meta_aad(theirs), &serde_json::to_string(&m).unwrap())
                .unwrap();
        let remote = SessionLog {
            id: theirs,
            vault_id: vid,
            user_id: mate,
            author: Some(LogAuthor {
                user_id: mate,
                email: "mate@example.com".into(),
                display_name: Some("Mate".into()),
                avatar_tag: Some("abc".into()),
            }),
            meta: meta_ct,
            key_version: 1,
            size_bytes: 12,
            completed: true,
            created_at: Utc::now(),
            seq: 3,
            deleted: false,
            pinned: true,
            note: "deploy went sideways".into(),
            note_by: Some(me),
        };
        store
            .apply_remote_logs(std::slice::from_ref(&remote))
            .unwrap();

        // My own recording, with nothing from the server yet.
        let mine = store.begin_log(vid, &meta()).unwrap();
        store
            .finish_log(mine, &meta(), b"$ ok", dir.path())
            .unwrap();

        let items = store.logs().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, theirs, "pinned first");
        assert!(!items[0].mine && !items[0].cached && items[0].uploaded);
        assert_eq!(items[0].meta, m, "metadata decrypted with the vault key");
        let a = items[0].author.as_ref().unwrap();
        assert_eq!((a.user_id, a.email.as_str()), (mate, "mate@example.com"));
        assert_eq!(a.avatar_tag.as_deref(), Some("abc"));
        assert!(items[0].pinned);
        assert_eq!(items[0].note, "deploy went sideways");
        assert_eq!(items[0].note_by, Some(me));
        assert!(items[1].mine && !items[1].pinned);
        assert_eq!(
            items[1]
                .author
                .as_ref()
                .map(|a| (a.user_id, a.email.as_str())),
            Some((me, "me@example.com")),
            "own recording is attributed to me before the server echoes it"
        );
        assert_eq!(
            store.logs_without_body().unwrap().len(),
            1,
            "teammate's body is fetched lazily"
        );
        assert!(store.read_log(theirs).is_err());

        // A row encrypted with a key version we do not hold yet is skipped
        // rather than breaking the whole listing.
        let other_key = SymmetricKey::generate();
        let stale = Uuid::new_v4();
        store
            .apply_remote_logs(&[SessionLog {
                id: stale,
                user_id: mate,
                seq: 4,
                pinned: false,
                note: String::new(),
                note_by: None,
                meta: aead::encrypt_str(
                    &other_key,
                    &meta_aad(stale),
                    &serde_json::to_string(&m).unwrap(),
                )
                .unwrap(),
                key_version: 2,
                ..remote.clone()
            }])
            .unwrap();
        assert_eq!(store.logs().unwrap().len(), 2);

        // Annotation echo from the server, then a refused delete restores.
        store.annotate_log(theirs, false, "", None).unwrap();
        let t = store
            .logs()
            .unwrap()
            .into_iter()
            .find(|l| l.id == theirs)
            .unwrap();
        assert!(!t.pinned && t.note.is_empty() && t.note_by.is_none());
        store.delete_log(theirs).unwrap();
        assert_eq!(store.logs_to_delete_remote().unwrap().len(), 1);
        store.restore_log(theirs).unwrap();
        assert!(store.logs_to_delete_remote().unwrap().is_empty());
        assert!(store.logs().unwrap().iter().any(|l| l.id == theirs));

        // The remote tombstone wins over everything.
        store
            .apply_remote_logs(&[SessionLog {
                deleted: true,
                author: None,
                ..remote
            }])
            .unwrap();
        assert!(store.logs().unwrap().iter().all(|l| l.id != theirs));
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
