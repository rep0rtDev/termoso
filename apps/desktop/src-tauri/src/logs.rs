//! Session recordings: live capture into the encrypted log store, listing,
//! playback text, export and bookmarks. Bodies never leave Rust unencrypted
//! except for the explicit `read`/`export` operations.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use termoso_core::model::LogBookmark;
use termoso_core::store::{LocalVault, LocalVaultKind, LogItem, Store};
use termoso_core::termoso_proto::logs::MAX_NOTE_CHARS;
use termoso_core::termoso_proto::vault::VaultRole;
use uuid::Uuid;

use crate::error::{DesktopError, Result};

pub use termoso_core::store::Recorder;

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
    /// Recorded by this account / device.
    pub mine: bool,
    /// Lives in a team vault (teammates see it too).
    pub team: bool,
    /// Who recorded it, when it came from the server.
    pub author: Option<LogAuthorCard>,
    pub pinned: bool,
    pub note: String,
    pub note_by: Option<Uuid>,
    /// May pin / annotate (write role in the vault).
    pub can_annotate: bool,
    /// May delete (author, or manager of the vault).
    pub can_delete: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogAuthorCard {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    /// Picture tag for `user_avatar`.
    pub avatar: Option<String>,
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

fn card(item: LogItem, vault: Option<&LocalVault>, bookmarks: usize) -> LogCard {
    let duration_secs = item
        .meta
        .ended_at
        .map(|e| (e - item.meta.started_at).num_seconds().max(0));
    let team = vault.is_some_and(|v| v.kind == LocalVaultKind::Team);
    let role = vault.map_or(VaultRole::Manager, |v| v.role);
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
        mine: item.mine,
        team,
        author: item.author.map(|a| LogAuthorCard {
            user_id: a.user_id,
            email: a.email,
            display_name: a.display_name,
            avatar: a.avatar_tag,
        }),
        pinned: item.pinned,
        note: item.note,
        note_by: item.note_by,
        can_annotate: role.can_write(),
        can_delete: item.mine || role.can_manage(),
    }
}

pub fn list(store: &Store) -> Result<Vec<LogCard>> {
    let bookmarks = store.list::<LogBookmark>(None)?;
    let vaults = store.vaults()?;
    let mut out: Vec<LogCard> = store
        .logs()?
        .into_iter()
        .map(|item| {
            let n = bookmarks
                .iter()
                .filter(|b| b.data.log_id == item.id)
                .count();
            let vault = vaults.iter().find(|v| v.id == item.vault_id);
            card(item, vault, n)
        })
        .collect();
    // Pinned recordings stay on top; the rest newest first.
    out.sort_by_key(|a| (std::cmp::Reverse(a.pinned), std::cmp::Reverse(a.started_at)));
    Ok(out)
}

fn find(store: &Store, id: Uuid) -> Result<LogCard> {
    list(store)?
        .into_iter()
        .find(|l| l.id == id)
        .ok_or_else(|| DesktopError::not_found(format!("log {id}")))
}

/// Validate a team note the way the server does, so the user hears about a
/// problem before the request leaves.
pub fn check_note(note: &str) -> Result<()> {
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(DesktopError::invalid(format!(
            "note is longer than {MAX_NOTE_CHARS} characters"
        )));
    }
    if note
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(DesktopError::invalid("note contains control characters"));
    }
    Ok(())
}

/// Pin / annotate a recording that the server has not seen (local vault, or
/// not uploaded yet): the annotation stays on this device.
pub fn annotate_local(
    store: &Store,
    id: Uuid,
    pinned: Option<bool>,
    note: Option<&str>,
) -> Result<LogCard> {
    let cur = find(store, id)?;
    if !cur.can_annotate {
        return Err(DesktopError::forbidden(
            "Editor role required to pin or annotate",
        ));
    }
    let note = note.map(str::trim);
    if let Some(n) = note {
        check_note(n)?;
    }
    let pinned = pinned.unwrap_or(cur.pinned);
    let (note, note_by) = match note {
        Some(n) if n != cur.note => (n.to_string(), None),
        _ => (cur.note.clone(), cur.note_by),
    };
    store.annotate_log(id, pinned, &note, note_by)?;
    find(store, id)
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
    if !find(store, id)?.can_delete {
        return Err(DesktopError::forbidden(
            "Only the author or a vault manager can delete this recording",
        ));
    }
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
    // Teammates' recordings are the team's to keep; retention is about
    // what this account recorded.
    for item in store.logs()? {
        if item.mine && item.completed && item.meta.started_at < cutoff {
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
    use termoso_core::store::{LogMeta, MAX_CAPTURE_BYTES};
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
