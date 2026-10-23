//! Watch & upload: a local folder is watched and every file saved under it
//! is uploaded to the same place under a remote folder of an open SFTP
//! connection — the «saved — it's on the server» deploy loop. One-way, local
//! → remote; remote deletions only when asked. Uploads go straight through
//! the connection (no transfer queue entry per save), progress and errors
//! arrive as `sftp_sync` events.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use notify::Watcher;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use termoso_core::remote::RemoteFs;
use termoso_core::sftp::TransferOptions;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

pub const SYNC_EVENT: &str = "sftp_sync";
/// Editors write a file in several steps; wait for the burst to end.
const SETTLE: Duration = Duration::from_millis(400);
/// A save that fails is retried a few times (editor still holding the file, network blip).
const RETRIES: u32 = 2;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncInfo {
    pub id: Uuid,
    pub sftp_id: Uuid,
    pub local: String,
    pub remote: String,
    /// Names / globs skipped at any depth (`.git`, `node_modules`, `*.swp`).
    pub ignore: Vec<String>,
    /// Delete on the server what was deleted locally.
    pub delete_remote: bool,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStats {
    pub uploaded: u64,
    pub bytes: u64,
    pub removed: u64,
    pub failed: u64,
    /// Files waiting for the settle timer or being uploaded right now.
    pub pending: u64,
    pub last_path: Option<String>,
    pub last_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCard {
    pub info: SyncInfo,
    pub stats: SyncStats,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncEvent {
    Started {
        info: SyncInfo,
    },
    /// Stats changed (a file went up, failed, or the pending count moved).
    Progress {
        id: Uuid,
        stats: SyncStats,
    },
    Stopped {
        id: Uuid,
        reason: Option<String>,
    },
}

struct Live {
    info: SyncInfo,
    cancel: CancellationToken,
    stats: Mutex<SyncStats>,
}

#[derive(Default)]
pub struct Syncs {
    live: Mutex<HashMap<Uuid, Arc<Live>>>,
}

impl Syncs {
    pub fn list(&self) -> Vec<SyncCard> {
        let mut v: Vec<SyncCard> = self
            .live
            .lock()
            .expect("syncs poisoned")
            .values()
            .map(|l| SyncCard {
                info: l.info.clone(),
                stats: l.stats.lock().expect("sync stats poisoned").clone(),
            })
            .collect();
        v.sort_by_key(|c| c.info.started_at);
        v
    }

    fn get(&self, id: Uuid) -> Result<Arc<Live>> {
        self.live
            .lock()
            .expect("syncs poisoned")
            .get(&id)
            .cloned()
            .ok_or_else(|| DesktopError::not_found(format!("sync {id}")))
    }

    fn remove(&self, id: Uuid) -> Option<Arc<Live>> {
        self.live.lock().expect("syncs poisoned").remove(&id)
    }

    fn for_sftp(&self, sftp_id: Uuid) -> Vec<Uuid> {
        self.live
            .lock()
            .expect("syncs poisoned")
            .values()
            .filter(|l| l.info.sftp_id == sftp_id)
            .map(|l| l.info.id)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// Created or modified: upload (or mkdir for a directory).
    Upsert,
    /// Gone locally.
    Remove,
}

/// Start watching `local` and mirroring saves into `remote` on `sftp_id`.
#[allow(clippy::too_many_arguments)]
pub async fn start<R: Runtime>(
    app: AppHandle<R>,
    sftp_id: Uuid,
    local: String,
    remote: String,
    ignore: Vec<String>,
    delete_remote: bool,
    upload_existing: bool,
) -> Result<SyncInfo> {
    let state = app.state::<AppState>();
    let fs = state.sftp.sftp(sftp_id)?;
    let root = PathBuf::from(local.trim());
    if !root.is_dir() {
        return Err(DesktopError::invalid(format!(
            "{} is not a folder",
            root.display()
        )));
    }
    let root = root.canonicalize().unwrap_or(root);
    let remote = normalize_remote(&remote);
    if remote.is_empty() {
        return Err(DesktopError::invalid("remote folder is required"));
    }
    fs.mkdir_all(&remote).await?;
    let ignore: Vec<String> = ignore
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<notify::Event>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            let _ = tx.send(ev);
        }
    })
    .map_err(|e| DesktopError::new("sync", e.to_string()))?;
    watcher
        .watch(&root, notify::RecursiveMode::Recursive)
        .map_err(|e| DesktopError::new("sync", e.to_string()))?;

    let info = SyncInfo {
        id: Uuid::new_v4(),
        sftp_id,
        local: root.to_string_lossy().into_owned(),
        remote: remote.clone(),
        ignore: ignore.clone(),
        delete_remote,
        started_at: Utc::now(),
    };
    let live = Arc::new(Live {
        info: info.clone(),
        cancel: CancellationToken::new(),
        stats: Mutex::new(SyncStats::default()),
    });
    state
        .syncs
        .live
        .lock()
        .expect("syncs poisoned")
        .insert(info.id, live.clone());
    let _ = app.emit(SYNC_EVENT, SyncEvent::Started { info: info.clone() });

    let worker = Worker {
        app: app.clone(),
        live,
        fs,
        root,
        remote,
        ignore,
        delete_remote,
    };
    tauri::async_runtime::spawn(async move {
        // The watcher lives as long as the task.
        let _watcher = watcher;
        let reason = worker.run(rx, upload_existing).await;
        let id = worker.live.info.id;
        let app = worker.app.clone();
        if app.state::<AppState>().syncs.remove(id).is_some() {
            let _ = app.emit(SYNC_EVENT, SyncEvent::Stopped { id, reason });
        }
    });
    Ok(info)
}

/// Stop a sync; files still uploading are cancelled.
pub fn stop<R: Runtime>(app: &AppHandle<R>, id: Uuid) -> Result<()> {
    let state = app.state::<AppState>();
    let live = state.syncs.get(id)?;
    live.cancel.cancel();
    if state.syncs.remove(id).is_some() {
        let _ = app.emit(SYNC_EVENT, SyncEvent::Stopped { id, reason: None });
    }
    Ok(())
}

/// Stop every sync that uploads through an SFTP connection being closed.
pub fn close_for_sftp<R: Runtime>(app: &AppHandle<R>, sftp_id: Uuid) {
    let state = app.state::<AppState>();
    for id in state.syncs.for_sftp(sftp_id) {
        if let Some(live) = state.syncs.remove(id) {
            live.cancel.cancel();
            let _ = app.emit(
                SYNC_EVENT,
                SyncEvent::Stopped {
                    id,
                    reason: Some("connection closed".into()),
                },
            );
        }
    }
}

struct Worker<R: Runtime> {
    app: AppHandle<R>,
    live: Arc<Live>,
    fs: Arc<dyn RemoteFs>,
    root: PathBuf,
    remote: String,
    ignore: Vec<String>,
    delete_remote: bool,
}

impl<R: Runtime> Worker<R> {
    async fn run(
        &self,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<notify::Event>,
        upload_existing: bool,
    ) -> Option<String> {
        let cancel = self.live.cancel.clone();
        let mut pending: BTreeMap<PathBuf, Change> = BTreeMap::new();
        let mut known_dirs: HashSet<String> = HashSet::new();
        if upload_existing {
            for p in walk(&self.root, &self.ignore) {
                pending.insert(p, Change::Upsert);
            }
            self.set_pending(pending.len());
        }
        loop {
            let wait = if pending.is_empty() {
                None
            } else {
                Some(tokio::time::sleep(SETTLE))
            };
            tokio::select! {
                _ = cancel.cancelled() => return None,
                ev = rx.recv() => {
                    let Some(ev) = ev else { return Some("watcher stopped".into()) };
                    self.absorb(ev, &mut pending);
                    self.set_pending(pending.len());
                }
                _ = async { match wait { Some(s) => s.await, None => std::future::pending().await } } => {
                    // Settle timer fired: flush everything collected so far,
                    // directories first so files have somewhere to land.
                    let batch = std::mem::take(&mut pending);
                    for (path, change) in batch {
                        if cancel.is_cancelled() { return None; }
                        self.apply(&path, change, &mut known_dirs).await;
                        self.set_pending(pending.len());
                    }
                    self.set_pending(0);
                }
            }
        }
    }

    fn absorb(&self, ev: notify::Event, pending: &mut BTreeMap<PathBuf, Change>) {
        use notify::EventKind::*;
        let change = match ev.kind {
            Create(_) | Modify(_) | Any | Other => Change::Upsert,
            Remove(_) => Change::Remove,
            Access(_) => return,
        };
        // A rename arrives as one event with [from, to]; `from` is gone, `to` is new.
        let rename_pair = matches!(
            ev.kind,
            Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::Both
            ))
        ) && ev.paths.len() == 2;
        for (i, p) in ev.paths.iter().enumerate() {
            if p == &self.root
                || !p.starts_with(&self.root)
                || is_ignored(&self.root, p, &self.ignore)
            {
                continue;
            }
            let c = if (rename_pair && i == 0) || (change == Change::Upsert && !p.exists()) {
                Change::Remove
            } else {
                change
            };
            pending.insert(p.clone(), c);
        }
    }

    async fn apply(&self, path: &Path, change: Change, known_dirs: &mut HashSet<String>) {
        let Some(rel) = relative(&self.root, path) else {
            return;
        };
        let remote_path = format!("{}/{}", self.remote, rel);
        let cancel = self.live.cancel.clone();
        match change {
            Change::Upsert => {
                let Ok(meta) = std::fs::metadata(path) else {
                    // Vanished between the event and now: nothing to upload.
                    return;
                };
                if meta.is_dir() {
                    if known_dirs.insert(remote_path.clone())
                        && let Err(e) = self.fs.mkdir_all(&remote_path).await
                    {
                        self.failed(&rel, e.to_string());
                    }
                    return;
                }
                if !meta.is_file() {
                    return;
                }
                if let Some(parent) = remote_parent(&remote_path)
                    && known_dirs.insert(parent.clone())
                    && let Err(e) = self.fs.mkdir_all(&parent).await
                {
                    tracing::debug!(error = %e, parent, "sync: mkdir parent");
                }
                let opts = TransferOptions {
                    resume: false,
                    preserve_mtime: true,
                    cancel: cancel.clone(),
                    progress: None,
                };
                let mut attempt = 0;
                loop {
                    match self.fs.upload(path, &remote_path, &opts).await {
                        Ok(bytes) => {
                            self.uploaded(&rel, bytes);
                            return;
                        }
                        Err(_) if cancel.is_cancelled() => return,
                        Err(e) if attempt < RETRIES => {
                            attempt += 1;
                            tokio::time::sleep(Duration::from_millis(300 * u64::from(attempt)))
                                .await;
                            tracing::debug!(error = %e, rel, attempt, "sync: retrying upload");
                        }
                        Err(e) => {
                            self.failed(&rel, e.to_string());
                            return;
                        }
                    }
                }
            }
            Change::Remove => {
                if !self.delete_remote {
                    return;
                }
                let res = match self.fs.stat(&remote_path).await {
                    Ok(entry) if entry.kind == termoso_core::sftp::EntryKind::Dir => {
                        known_dirs.remove(&remote_path);
                        self.fs.remove_dir_all(&remote_path, &cancel).await
                    }
                    Ok(_) => self.fs.remove_file(&remote_path).await,
                    // Never there — nothing to mirror.
                    Err(_) => Ok(()),
                };
                match res {
                    Ok(()) => self.removed(&rel),
                    Err(e) => self.failed(&rel, e.to_string()),
                }
            }
        }
    }

    fn set_pending(&self, n: usize) {
        let mut s = self.live.stats.lock().expect("sync stats poisoned");
        if s.pending != n as u64 {
            s.pending = n as u64;
            let snap = s.clone();
            drop(s);
            self.emit(snap);
        }
    }

    fn uploaded(&self, rel: &str, bytes: u64) {
        let mut s = self.live.stats.lock().expect("sync stats poisoned");
        s.uploaded += 1;
        s.bytes += bytes;
        s.last_path = Some(rel.to_string());
        s.last_at = Some(Utc::now());
        s.last_error = None;
        let snap = s.clone();
        drop(s);
        self.emit(snap);
    }

    fn removed(&self, rel: &str) {
        let mut s = self.live.stats.lock().expect("sync stats poisoned");
        s.removed += 1;
        s.last_path = Some(rel.to_string());
        s.last_at = Some(Utc::now());
        let snap = s.clone();
        drop(s);
        self.emit(snap);
    }

    fn failed(&self, rel: &str, error: String) {
        let mut s = self.live.stats.lock().expect("sync stats poisoned");
        s.failed += 1;
        s.last_error = Some(format!("{rel}: {error}"));
        let snap = s.clone();
        drop(s);
        self.emit(snap);
    }

    fn emit(&self, stats: SyncStats) {
        let _ = self.app.emit(
            SYNC_EVENT,
            SyncEvent::Progress {
                id: self.live.info.id,
                stats,
            },
        );
    }
}

/// `/srv/app/` → `/srv/app`; a bare `/` stays.
fn normalize_remote(remote: &str) -> String {
    let r = remote.trim();
    if r == "/" {
        return r.to_string();
    }
    r.trim_end_matches('/').to_string()
}

fn remote_parent(path: &str) -> Option<String> {
    let i = path.rfind('/')?;
    let p = &path[..i];
    Some(if p.is_empty() {
        "/".to_string()
    } else {
        p.to_string()
    })
}

/// Path below the root with forward slashes; none when it escapes the root.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Does any component of the path (relative to the root) match an ignore
/// pattern? Patterns are names or shell-style globs (`*`, `?`); a trailing `/`
/// is accepted and ignored.
fn is_ignored(root: &Path, path: &Path, patterns: &[String]) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    rel.components().any(|c| match c {
        Component::Normal(s) => {
            let name = s.to_string_lossy();
            patterns
                .iter()
                .any(|p| glob_match(p.trim_end_matches('/'), &name))
        }
        _ => false,
    })
}

/// Minimal glob: `*` any run, `?` one char, everything else literal (case-sensitive).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    fn go(p: &[char], n: &[char]) -> bool {
        match (p.first(), n.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], n) || (!n.is_empty() && go(p, &n[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &n[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &n[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    go(&p, &n)
}

/// Every file and directory under `root` that is not ignored (directories first).
fn walk(root: &Path, ignore: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if is_ignored(root, &p, ignore) {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                out.push(p.clone());
                stack.push(p);
            } else if ft.is_file() {
                out.push(p);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(glob_match("*.swp", ".main.rs.swp"));
        assert!(glob_match("*~", "notes.txt~"));
        assert!(glob_match(".#*", ".#lock"));
        assert!(glob_match("node_modules", "node_modules"));
        assert!(!glob_match("node_modules", "node_modules2"));
        assert!(glob_match("?.log", "a.log"));
        assert!(!glob_match("?.log", "ab.log"));
    }

    #[test]
    fn ignore_matches_any_component() {
        let root = Path::new("/w");
        let pats = vec![".git".to_string(), "*.swp".to_string(), "dist/".to_string()];
        assert!(is_ignored(root, Path::new("/w/.git/HEAD"), &pats));
        assert!(is_ignored(root, Path::new("/w/src/.x.swp"), &pats));
        assert!(is_ignored(root, Path::new("/w/dist/app.js"), &pats));
        assert!(!is_ignored(root, Path::new("/w/src/main.rs"), &pats));
        assert!(!is_ignored(
            Path::new("/other"),
            Path::new("/w/.git"),
            &pats
        ));
    }

    #[test]
    fn relative_paths_use_slashes_and_stay_inside() {
        let root = Path::new("/w");
        assert_eq!(
            relative(root, Path::new("/w/a/b.txt")).as_deref(),
            Some("a/b.txt")
        );
        assert_eq!(relative(root, Path::new("/w")), None);
        assert_eq!(relative(root, Path::new("/elsewhere/x")), None);
        assert_eq!(normalize_remote("/srv/app/"), "/srv/app");
        assert_eq!(normalize_remote("/"), "/");
        assert_eq!(remote_parent("/srv/app/x").as_deref(), Some("/srv/app"));
        assert_eq!(remote_parent("/x").as_deref(), Some("/"));
    }

    #[test]
    fn walk_skips_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        std::fs::create_dir_all(r.join("src")).unwrap();
        std::fs::create_dir_all(r.join(".git")).unwrap();
        std::fs::write(r.join("src/main.rs"), "x").unwrap();
        std::fs::write(r.join(".git/HEAD"), "x").unwrap();
        std::fs::write(r.join("a.swp"), "x").unwrap();
        let got: Vec<String> = walk(r, &[".git".into(), "*.swp".into()])
            .into_iter()
            .filter_map(|p| relative(r, &p))
            .collect();
        assert_eq!(got.len(), 2);
        assert!(got.contains(&"src".to_string()));
        assert!(got.contains(&"src/main.rs".to_string()));
    }
}
