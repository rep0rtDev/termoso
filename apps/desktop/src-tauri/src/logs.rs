//! Session recordings: live capture into the encrypted log store, listing,
//! playback text, export and bookmarks. Bodies never leave Rust unencrypted
//! except for the explicit `read`/`export` operations.

use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use termoso_core::model::LogBookmark;
use termoso_core::store::{LogItem, LogMeta, Store};
use uuid::Uuid;

use crate::error::{DesktopError, Result};

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
    truncated: std::sync::atomic::AtomicBool,
}

impl Recorder {
    pub fn begin(store: &Store, vault_id: Uuid, meta: LogMeta) -> Result<Self> {
        let id = store.begin_log(vault_id, &meta)?;
        Ok(Self {
            id,
            meta,
            buf: Mutex::new(Vec::new()),
            truncated: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn append(&self, bytes: &[u8]) {
        let mut buf = self.buf.lock().expect("recorder poisoned");
        let room = MAX_CAPTURE_BYTES.saturating_sub(buf.len());
        if bytes.len() > room {
            buf.extend_from_slice(&bytes[..room]);
            self.truncated
                .store(true, std::sync::atomic::Ordering::Relaxed);
        } else {
            buf.extend_from_slice(bytes);
        }
    }

    /// Persist the capture; called once when the session ends.
    pub fn finish(&self, store: &Store, dir: &Path) -> Result<()> {
        let body = std::mem::take(&mut *self.buf.lock().expect("recorder poisoned"));
        let mut meta = self.meta.clone();
        meta.ended_at = Some(Utc::now());
        if self.truncated.load(std::sync::atomic::Ordering::Relaxed) {
            meta.label = format!("{} (truncated)", meta.label);
        }
        store.finish_log(self.id, &meta, &body, dir)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogCard {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub host_id: Option<Uuid>,
    pub label: String,
    pub target: String,
    pub protocol: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub cols: u16,
    pub rows: u16,
    pub size_bytes: i64,
    pub cached: bool,
    pub uploaded: bool,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub bookmarks: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkCard {
    pub id: Uuid,
    pub log_id: Uuid,
    pub offset: u64,
    pub note: String,
    pub updated_at: DateTime<Utc>,
}

/// Decrypted recording as text (lossy UTF-8) plus its size in bytes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogBody {
    pub id: Uuid,
    pub text: String,
    pub bytes: usize,
}

fn card(item: LogItem, bookmarks: usize) -> LogCard {
    let duration_secs = item
        .meta
        .ended_at
        .map(|e| (e - item.meta.started_at).num_seconds().max(0));
    LogCard {
        id: item.id,
        vault_id: item.vault_id,
        host_id: item.meta.host_id,
        label: item.meta.label,
        target: item.meta.target,
        protocol: item.meta.protocol,
        started_at: item.meta.started_at,
        ended_at: item.meta.ended_at,
        duration_secs,
        cols: item.meta.cols,
        rows: item.meta.rows,
        size_bytes: item.size_bytes,
        cached: item.cached,
        uploaded: item.uploaded,
        completed: item.completed,
        created_at: item.created_at,
        bookmarks,
    }
}

pub fn list(store: &Store) -> Result<Vec<LogCard>> {
    let bookmarks = store.list::<LogBookmark>(None)?;
    let mut out: Vec<LogCard> = store
        .logs()?
        .into_iter()
        .map(|item| {
            let n = bookmarks
                .iter()
                .filter(|b| b.data.log_id == item.id)
                .count();
            card(item, n)
        })
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.started_at));
    Ok(out)
}

pub fn read(store: &Store, id: Uuid) -> Result<LogBody> {
    let body = store.read_log(id)?;
    Ok(LogBody {
        id,
        bytes: body.len(),
        text: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// Write the decrypted recording to `path`.
pub fn export(store: &Store, id: Uuid, path: &str) -> Result<usize> {
    let body = store.read_log(id)?;
    std::fs::write(path, &body)?;
    Ok(body.len())
}

pub fn delete(store: &Store, id: Uuid) -> Result<()> {
    for b in store.list::<LogBookmark>(None)? {
        if b.data.log_id == id {
            store.delete(b.id)?;
        }
    }
    store.delete_log(id)?;
    Ok(())
}

/// Delete completed recordings older than `days` (0 = keep everything).
pub fn prune(store: &Store, days: u32) -> Result<usize> {
    if days == 0 {
        return Ok(0);
    }
    let cutoff = Utc::now() - Duration::days(i64::from(days));
    let mut n = 0;
    for item in store.logs()? {
        if item.completed && item.meta.started_at < cutoff {
            delete(store, item.id)?;
            n += 1;
        }
    }
    Ok(n)
}

// ───────────────────────────── bookmarks ─────────────────────────────

fn bookmark_card(e: termoso_core::model::Entity<LogBookmark>) -> BookmarkCard {
    BookmarkCard {
        id: e.id,
        log_id: e.data.log_id,
        offset: e.data.offset,
        note: e.data.note,
        updated_at: e.updated_at,
    }
}

pub fn bookmarks(store: &Store, log_id: Uuid) -> Result<Vec<BookmarkCard>> {
    let mut out: Vec<BookmarkCard> = store
        .list::<LogBookmark>(None)?
        .into_iter()
        .filter(|b| b.data.log_id == log_id)
        .map(bookmark_card)
        .collect();
    out.sort_by_key(|b| b.offset);
    Ok(out)
}

pub fn add_bookmark(store: &Store, log_id: Uuid, offset: u64, note: &str) -> Result<BookmarkCard> {
    let item = store
        .logs()?
        .into_iter()
        .find(|l| l.id == log_id)
        .ok_or_else(|| DesktopError::not_found(format!("log {log_id}")))?;
    let note = note.trim();
    if note.len() > 2000 {
        return Err(DesktopError::invalid("bookmark note is too long"));
    }
    let id = store.insert(
        item.vault_id,
        &LogBookmark {
            log_id,
            offset,
            note: note.to_string(),
        },
    )?;
    Ok(bookmark_card(store.require::<LogBookmark>(id)?))
}

pub fn delete_bookmark(store: &Store, id: Uuid) -> Result<()> {
    store.require::<LogBookmark>(id)?;
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

    fn meta(label: &str, started: DateTime<Utc>) -> LogMeta {
        LogMeta {
            host_id: None,
            label: label.into(),
            target: "local".into(),
            protocol: "local".into(),
            started_at: started,
            ended_at: None,
            cols: 80,
            rows: 24,
        }
    }

    #[test]
    fn record_list_read_bookmark_delete() {
        let store = store();
        let vault = store.local_vault().unwrap().id;
        let dir = tempfile::tempdir().unwrap();

        let rec = Recorder::begin(&store, vault, meta("shell", Utc::now())).unwrap();
        rec.append(b"$ echo hi\r\n");
        rec.append(b"hi\r\n");
        rec.finish(&store, dir.path()).unwrap();

        let cards = list(&store).unwrap();
        assert_eq!(cards.len(), 1);
        assert!(cards[0].completed && cards[0].cached);
        assert!(cards[0].ended_at.is_some());
        assert_eq!(cards[0].bookmarks, 0);

        let body = read(&store, rec.id()).unwrap();
        assert_eq!(body.text, "$ echo hi\r\nhi\r\n");
        assert_eq!(body.bytes, 15);
        // The on-disk body is encrypted, not the plaintext.
        let raw = std::fs::read(dir.path().join(format!("{}.tlog", rec.id()))).unwrap();
        assert!(!raw.windows(7).any(|w| w == b"echo hi"));

        let b = add_bookmark(&store, rec.id(), 11, "output").unwrap();
        assert_eq!(bookmarks(&store, rec.id()).unwrap().len(), 1);
        assert_eq!(list(&store).unwrap()[0].bookmarks, 1);
        assert!(add_bookmark(&store, Uuid::new_v4(), 0, "").is_err());

        let out = dir.path().join("export.log");
        assert_eq!(export(&store, rec.id(), out.to_str().unwrap()).unwrap(), 15);
        assert_eq!(std::fs::read(&out).unwrap(), b"$ echo hi\r\nhi\r\n");

        delete(&store, rec.id()).unwrap();
        assert!(list(&store).unwrap().is_empty());
        assert!(delete_bookmark(&store, b.id).is_err());
    }

    #[test]
    fn prune_by_age_and_capture_cap() {
        let store = store();
        let vault = store.local_vault().unwrap().id;
        let dir = tempfile::tempdir().unwrap();
        let old =
            Recorder::begin(&store, vault, meta("old", Utc::now() - Duration::days(40))).unwrap();
        old.finish(&store, dir.path()).unwrap();
        let fresh = Recorder::begin(&store, vault, meta("fresh", Utc::now())).unwrap();
        fresh.finish(&store, dir.path()).unwrap();
        assert_eq!(prune(&store, 0).unwrap(), 0);
        assert_eq!(prune(&store, 30).unwrap(), 1);
        let left = list(&store).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].label, "fresh");

        let rec = Recorder::begin(&store, vault, meta("big", Utc::now())).unwrap();
        rec.append(&vec![b'x'; MAX_CAPTURE_BYTES]);
        rec.append(b"tail");
        rec.finish(&store, dir.path()).unwrap();
        let big = list(&store)
            .unwrap()
            .into_iter()
            .find(|c| c.label.starts_with("big"))
            .unwrap();
        assert!(big.label.ends_with("(truncated)"));
        assert_eq!(read(&store, big.id).unwrap().bytes, MAX_CAPTURE_BYTES);
    }
}
