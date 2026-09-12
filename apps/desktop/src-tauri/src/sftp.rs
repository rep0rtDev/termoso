//! SFTP sessions and the local file system for the two-pane file browser.
//! Rust owns the transport, walks directories and moves bytes; the webview
//! only renders listings and transfer progress.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use termoso_core::error::CoreError;
use termoso_core::sftp::{EntryKind, Progress, RemoteEntry, Sftp, TransferOptions};
use termoso_core::ssh::SshClient;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::{DesktopError, Result};
use crate::sessions;
use crate::state::AppState;

pub const SFTP_EVENT: &str = "sftp";
pub const TRANSFER_EVENT: &str = "transfer";
const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);

/// What to browse.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SftpTarget {
    /// Open a fresh SSH connection to a saved host.
    Host { host_id: Uuid },
    /// Reuse the transport of a live terminal session.
    Session { session_id: Uuid },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpInfo {
    pub id: Uuid,
    pub title: String,
    pub target: String,
    pub host_id: Option<Uuid>,
    pub home: String,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SftpEvent {
    Opened { id: Uuid, info: SftpInfo },
    Closed { id: Uuid },
}

/// A directory listing for either pane.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<RemoteEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Upload,
    Download,
}

/// What to do when a file already exists at the destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conflict {
    /// Overwrite the existing file.
    #[default]
    Replace,
    /// Leave the existing file alone.
    Skip,
    /// Keep both: the incoming item lands next to it as `name (1).ext`.
    Rename,
    /// Continue an interrupted copy of the same file.
    Resume,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferInfo {
    pub id: Uuid,
    pub sftp_id: Uuid,
    pub direction: Direction,
    pub local: String,
    pub remote: String,
    pub conflict: Conflict,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransferEvent {
    Started {
        id: Uuid,
        info: TransferInfo,
    },
    Progress {
        id: Uuid,
        done: u64,
        total: Option<u64>,
        files_done: usize,
        files_total: usize,
        files_skipped: usize,
        current: String,
    },
    Finished {
        id: Uuid,
        bytes: u64,
        files_skipped: usize,
    },
    Failed {
        id: Uuid,
        message: String,
    },
    Cancelled {
        id: Uuid,
    },
}

struct Live {
    info: SftpInfo,
    sftp: Arc<Sftp>,
    /// Keeps the transport (and any jump hosts) alive while we browse.
    #[allow(dead_code)]
    client: Arc<SshClient>,
    #[allow(dead_code)]
    jumps: Vec<Arc<SshClient>>,
}

#[derive(Default)]
pub struct SftpSessions {
    live: Mutex<HashMap<Uuid, Arc<Live>>>,
    pending: Mutex<HashMap<Uuid, CancellationToken>>,
    transfers: Mutex<HashMap<Uuid, CancellationToken>>,
}

impl SftpSessions {
    pub fn list(&self) -> Vec<SftpInfo> {
        let mut v: Vec<SftpInfo> = self
            .live
            .lock()
            .expect("sftp poisoned")
            .values()
            .map(|l| l.info.clone())
            .collect();
        v.sort_by_key(|s| s.started_at);
        v
    }

    fn get(&self, id: Uuid) -> Result<Arc<Live>> {
        self.live
            .lock()
            .expect("sftp poisoned")
            .get(&id)
            .cloned()
            .ok_or_else(|| DesktopError::not_found(format!("sftp session {id}")))
    }

    pub(crate) fn sftp(&self, id: Uuid) -> Result<Arc<Sftp>> {
        Ok(self.get(id)?.sftp.clone())
    }

    fn begin(&self, id: Uuid) -> Result<CancellationToken> {
        if self.live.lock().expect("sftp poisoned").contains_key(&id) {
            return Err(DesktopError::invalid(format!(
                "sftp session {id} already open"
            )));
        }
        let mut pending = self.pending.lock().expect("sftp poisoned");
        if pending.contains_key(&id) {
            return Err(DesktopError::invalid(format!(
                "sftp session {id} already opening"
            )));
        }
        let tok = CancellationToken::new();
        pending.insert(id, tok.clone());
        Ok(tok)
    }

    fn finish_pending(&self, id: Uuid) -> bool {
        self.pending
            .lock()
            .expect("sftp poisoned")
            .remove(&id)
            .is_some()
    }

    fn remove(&self, id: Uuid) -> Option<Arc<Live>> {
        if let Some(tok) = self.pending.lock().expect("sftp poisoned").remove(&id) {
            tok.cancel();
        }
        self.live.lock().expect("sftp poisoned").remove(&id)
    }

    pub fn cancel_transfer(&self, id: Uuid) -> bool {
        match self.transfers.lock().expect("sftp poisoned").get(&id) {
            Some(tok) => {
                tok.cancel();
                true
            }
            None => false,
        }
    }
}

// ───────────────────────────── sessions ─────────────────────────────

pub async fn open<R: Runtime>(
    app: AppHandle<R>,
    id: Option<Uuid>,
    target: SftpTarget,
) -> Result<SftpInfo> {
    let state = app.state::<AppState>();
    let id = id.unwrap_or_else(Uuid::new_v4);
    let cancel = state.sftp.begin(id)?;

    let result = tokio::select! {
        r = connect(&app, id, &target) => r,
        _ = cancel.cancelled() => Err(CoreError::Cancelled.into()),
    };
    state.prompts.cancel_session(id);
    let still_wanted = state.sftp.finish_pending(id);
    let (title, display, host_id, client, jumps) = result?;
    if !still_wanted {
        return Err(CoreError::Cancelled.into());
    }

    let sftp = Sftp::open(&client).await?;
    let info = SftpInfo {
        id,
        title,
        target: display,
        host_id,
        home: sftp.home().to_string(),
        started_at: Utc::now(),
    };
    state.sftp.live.lock().expect("sftp poisoned").insert(
        id,
        Arc::new(Live {
            info: info.clone(),
            sftp: Arc::new(sftp),
            client,
            jumps,
        }),
    );
    let _ = app.emit(
        SFTP_EVENT,
        SftpEvent::Opened {
            id,
            info: info.clone(),
        },
    );
    Ok(info)
}

type Connected = (
    String,
    String,
    Option<Uuid>,
    Arc<SshClient>,
    Vec<Arc<SshClient>>,
);

async fn connect<R: Runtime>(
    app: &AppHandle<R>,
    id: Uuid,
    target: &SftpTarget,
) -> Result<Connected> {
    let state = app.state::<AppState>();
    match target {
        SftpTarget::Host { host_id } => {
            let conn = sessions::connect_host(app, id, *host_id).await?;
            Ok((
                conn.label,
                conn.display,
                Some(*host_id),
                conn.client,
                conn.jumps,
            ))
        }
        SftpTarget::Session { session_id } => {
            let client = state.sessions.client(*session_id)?;
            let info = state
                .sessions
                .list()
                .into_iter()
                .find(|s| s.id == *session_id)
                .ok_or_else(|| DesktopError::not_found(format!("session {session_id}")))?;
            Ok((info.title, info.target, info.host_id, client, Vec::new()))
        }
    }
}

pub async fn close<R: Runtime>(app: &AppHandle<R>, id: Uuid) -> Result<()> {
    let state = app.state::<AppState>();
    state.prompts.cancel_session(id);
    if let Some(live) = state.sftp.remove(id) {
        crate::edits::close_for_sftp(app, id).await;
        let _ = live.sftp.close().await;
        let _ = app.emit(SFTP_EVENT, SftpEvent::Closed { id });
    }
    Ok(())
}

// ───────────────────────────── remote fs ─────────────────────────────

fn remote_parent(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.rfind('/') {
        Some(0) => Some("/".into()),
        Some(i) => Some(trimmed[..i].to_string()),
        None => None,
    }
}

fn sort_entries(entries: &mut [RemoteEntry]) {
    entries.sort_by(|a, b| {
        let da = a.kind == EntryKind::Dir;
        let db = b.kind == EntryKind::Dir;
        db.cmp(&da)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

pub async fn remote_list(state: &AppState, id: Uuid, path: Option<String>) -> Result<Listing> {
    let live = state.sftp.get(id)?;
    let path = match path {
        Some(p) if !p.trim().is_empty() => live.sftp.canonicalize(p.trim()).await?,
        _ => live.info.home.clone(),
    };
    let mut entries = live.sftp.list(&path).await?;
    sort_entries(&mut entries);
    Ok(Listing {
        parent: remote_parent(&path),
        path,
        entries,
    })
}

pub async fn remote_stat(state: &AppState, id: Uuid, path: String) -> Result<RemoteEntry> {
    Ok(state.sftp.sftp(id)?.stat(&path).await?)
}

pub async fn remote_mkdir(state: &AppState, id: Uuid, path: String) -> Result<()> {
    Ok(state.sftp.sftp(id)?.mkdir(&path).await?)
}

pub async fn remote_rename(state: &AppState, id: Uuid, from: String, to: String) -> Result<()> {
    Ok(state.sftp.sftp(id)?.rename(&from, &to).await?)
}

pub async fn remote_remove(
    state: &AppState,
    id: Uuid,
    path: String,
    recursive: bool,
) -> Result<()> {
    let sftp = state.sftp.sftp(id)?;
    let entry = sftp.stat(&path).await?;
    match entry.kind {
        EntryKind::Dir if recursive => {
            sftp.remove_dir_all(&path, &CancellationToken::new())
                .await?
        }
        EntryKind::Dir => sftp.remove_dir(&path).await?,
        _ => sftp.remove_file(&path).await?,
    }
    Ok(())
}

pub async fn remote_chmod(state: &AppState, id: Uuid, path: String, mode: u32) -> Result<()> {
    Ok(state.sftp.sftp(id)?.chmod(&path, mode & 0o7777).await?)
}

// ───────────────────────────── local fs ─────────────────────────────

pub fn local_home() -> String {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".into())
}

fn local_entry(path: &Path, name: String) -> Option<RemoteEntry> {
    let link_meta = std::fs::symlink_metadata(path).ok()?;
    let is_link = link_meta.file_type().is_symlink();
    let meta = if is_link {
        std::fs::metadata(path).ok()
    } else {
        Some(link_meta.clone())
    };
    let kind = if is_link {
        EntryKind::Symlink
    } else if link_meta.is_dir() {
        EntryKind::Dir
    } else if link_meta.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    };
    let target_meta = meta.as_ref().unwrap_or(&link_meta);
    let mtime = target_meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32);
    let atime = target_meta
        .accessed()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32);
    #[cfg(unix)]
    let (mode, uid, gid) = {
        use std::os::unix::fs::MetadataExt;
        (
            Some(target_meta.mode() & 0o7777),
            Some(target_meta.uid()),
            Some(target_meta.gid()),
        )
    };
    #[cfg(not(unix))]
    let (mode, uid, gid) = (None, None, None);
    Some(RemoteEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        size: target_meta.is_file().then_some(target_meta.len()),
        mode,
        uid,
        gid,
        user: None,
        group: None,
        mtime,
        atime,
        link_target: is_link
            .then(|| std::fs::read_link(path).ok())
            .flatten()
            .map(|p| p.to_string_lossy().into_owned()),
    })
}

fn local_list_sync(path: Option<String>) -> Result<Listing> {
    let raw = match path {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p.trim()),
        _ => PathBuf::from(local_home()),
    };
    let dir = std::fs::canonicalize(&raw)?;
    if !dir.is_dir() {
        return Err(DesktopError::invalid(format!(
            "{} is not a directory",
            dir.display()
        )));
    }
    let mut entries = Vec::new();
    for e in std::fs::read_dir(&dir)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(entry) = local_entry(&e.path(), name) {
            entries.push(entry);
        }
    }
    sort_entries(&mut entries);
    Ok(Listing {
        parent: dir.parent().map(|p| p.to_string_lossy().into_owned()),
        path: dir.to_string_lossy().into_owned(),
        entries,
    })
}

pub async fn local_list(path: Option<String>) -> Result<Listing> {
    tokio::task::spawn_blocking(move || local_list_sync(path))
        .await
        .map_err(|e| DesktopError::new("io", e.to_string()))?
}

pub async fn local_stat(path: String) -> Result<RemoteEntry> {
    let p = PathBuf::from(&path);
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    tokio::task::spawn_blocking(move || {
        local_entry(&p, name).ok_or_else(|| DesktopError::not_found(format!("{path} not found")))
    })
    .await
    .map_err(|e| DesktopError::new("io", e.to_string()))?
}

pub async fn local_mkdir(path: String) -> Result<()> {
    Ok(tokio::fs::create_dir(path).await?)
}

pub async fn local_rename(from: String, to: String) -> Result<()> {
    Ok(tokio::fs::rename(from, to).await?)
}

pub async fn local_remove(path: String, recursive: bool) -> Result<()> {
    let meta = tokio::fs::symlink_metadata(&path).await?;
    if meta.is_dir() {
        if recursive {
            tokio::fs::remove_dir_all(&path).await?;
        } else {
            tokio::fs::remove_dir(&path).await?;
        }
    } else {
        tokio::fs::remove_file(&path).await?;
    }
    Ok(())
}

// ───────────────────────────── transfers ─────────────────────────────

/// One file inside a (possibly recursive) transfer.
struct Item {
    local: PathBuf,
    remote: String,
    size: Option<u64>,
}

struct Plan {
    /// Directories to create on the destination, parents first.
    dirs: Vec<String>,
    files: Vec<Item>,
    total: Option<u64>,
}

fn remote_join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn plan_upload_sync(local: PathBuf, remote: String) -> Result<Plan> {
    let meta = std::fs::symlink_metadata(&local)?;
    if !meta.is_dir() {
        return Ok(Plan {
            dirs: Vec::new(),
            files: vec![Item {
                local,
                remote,
                size: Some(meta.len()),
            }],
            total: Some(meta.len()),
        });
    }
    let mut dirs = vec![remote.clone()];
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut stack = vec![(local, remote)];
    while let Some((dir, rdir)) = stack.pop() {
        for e in std::fs::read_dir(&dir)? {
            let e = e?;
            let ft = e.file_type()?;
            let name = e.file_name().to_string_lossy().into_owned();
            let rpath = remote_join(&rdir, &name);
            if ft.is_dir() {
                dirs.push(rpath.clone());
                stack.push((e.path(), rpath));
            } else if ft.is_file() {
                let size = e.metadata()?.len();
                total += size;
                files.push(Item {
                    local: e.path(),
                    remote: rpath,
                    size: Some(size),
                });
            }
        }
    }
    Ok(Plan {
        dirs,
        files,
        total: Some(total),
    })
}

async fn plan_download(sftp: &Sftp, remote: String, local: PathBuf) -> Result<Plan> {
    let root = sftp.stat(&remote).await?;
    if root.kind != EntryKind::Dir {
        return Ok(Plan {
            dirs: Vec::new(),
            files: vec![Item {
                local,
                remote,
                size: root.size,
            }],
            total: root.size,
        });
    }
    let mut dirs = vec![local.to_string_lossy().into_owned()];
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut stack = vec![(remote, local)];
    while let Some((rdir, ldir)) = stack.pop() {
        for e in sftp.list(&rdir).await? {
            let lpath = ldir.join(&e.name);
            match e.kind {
                EntryKind::Dir => {
                    dirs.push(lpath.to_string_lossy().into_owned());
                    stack.push((e.path, lpath));
                }
                EntryKind::File => {
                    total += e.size.unwrap_or(0);
                    files.push(Item {
                        local: lpath,
                        remote: e.path,
                        size: e.size,
                    });
                }
                _ => {}
            }
        }
    }
    Ok(Plan {
        dirs,
        files,
        total: Some(total),
    })
}

/// Throttled progress emitter shared by every file of one transfer.
struct Reporter<R: Runtime> {
    app: AppHandle<R>,
    id: Uuid,
    total: Option<u64>,
    files_total: usize,
    state: Mutex<(Instant, u64, usize, String)>,
}

impl<R: Runtime> Reporter<R> {
    fn report(&self, done: u64, files_done: usize, skipped: usize, current: &str, force: bool) {
        let mut s = self.state.lock().expect("reporter poisoned");
        let now = Instant::now();
        if !force && now.duration_since(s.0) < PROGRESS_INTERVAL {
            return;
        }
        s.0 = now;
        s.1 = done;
        s.2 = files_done;
        if s.3 != current {
            s.3 = current.to_string();
        }
        let _ = self.app.emit(
            TRANSFER_EVENT,
            TransferEvent::Progress {
                id: self.id,
                done,
                total: self.total,
                files_done,
                files_total: self.files_total,
                files_skipped: skipped,
                current: current.to_string(),
            },
        );
    }
}

/// The entry currently occupying the destination of a would-be transfer, if
/// any — the UI asks before starting so it can offer Replace / Skip / Rename.
pub async fn transfer_probe(
    state: &AppState,
    sftp_id: Uuid,
    direction: Direction,
    local: String,
    remote: String,
) -> Result<Option<RemoteEntry>> {
    match direction {
        Direction::Upload => Ok(state.sftp.sftp(sftp_id)?.stat(&remote).await.ok()),
        Direction::Download => Ok(local_stat(local).await.ok()),
    }
}

pub fn transfer_start<R: Runtime>(
    app: AppHandle<R>,
    sftp_id: Uuid,
    direction: Direction,
    local: String,
    remote: String,
    conflict: Conflict,
    temp: bool,
) -> Result<TransferInfo> {
    let state = app.state::<AppState>();
    let sftp = state.sftp.sftp(sftp_id)?;
    let local_path = PathBuf::from(&local);
    if temp && !local_path.starts_with(drop_root()) {
        return Err(DesktopError::invalid(
            "temp transfers must come from the drop staging area",
        ));
    }
    let id = Uuid::new_v4();
    let cancel = CancellationToken::new();
    state
        .sftp
        .transfers
        .lock()
        .expect("sftp poisoned")
        .insert(id, cancel.clone());
    let info = TransferInfo {
        id,
        sftp_id,
        direction,
        local: local.clone(),
        remote: remote.clone(),
        conflict,
        started_at: Utc::now(),
    };
    let _ = app.emit(
        TRANSFER_EVENT,
        TransferEvent::Started {
            id,
            info: info.clone(),
        },
    );
    tauri::async_runtime::spawn(async move {
        let result = tokio::select! {
            r = run_transfer(&app, id, sftp, direction, local_path.clone(), remote, conflict, cancel.clone()) => r,
            _ = cancel.cancelled() => Err(CoreError::Cancelled.into()),
        };
        app.state::<AppState>()
            .sftp
            .transfers
            .lock()
            .expect("sftp poisoned")
            .remove(&id);
        if temp {
            drop_discard(&local_path).await;
        }
        let ev = match result {
            Ok((bytes, files_skipped)) => TransferEvent::Finished {
                id,
                bytes,
                files_skipped,
            },
            Err(e) if e.kind == "cancelled" => TransferEvent::Cancelled { id },
            Err(e) => TransferEvent::Failed {
                id,
                message: e.message,
            },
        };
        let _ = app.emit(TRANSFER_EVENT, ev);
    });
    Ok(info)
}

/// `name (1).ext`, `name (2).ext`, … for the first `n` that keeps `taken` false.
async fn unique_name<F, Fut>(name: &str, taken: F) -> Result<String>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    for n in 1..1000 {
        let candidate = format!("{stem} ({n}){ext}");
        if !taken(candidate.clone()).await {
            return Ok(candidate);
        }
    }
    Err(DesktopError::invalid(format!("no free name for {name}")))
}

fn remote_split(path: &str) -> (String, String) {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(i) => (
            trimmed[..i.max(1)].to_string(),
            trimmed[i + 1..].to_string(),
        ),
        None => (String::new(), trimmed.to_string()),
    }
}

async fn unique_remote(sftp: &Sftp, path: &str) -> Result<String> {
    let (dir, name) = remote_split(path);
    let fresh = unique_name(&name, |c| {
        let candidate = remote_join(&dir, &c);
        async move { sftp.exists(&candidate).await.unwrap_or(true) }
    })
    .await?;
    Ok(remote_join(&dir, &fresh))
}

async fn unique_local(path: &Path) -> Result<PathBuf> {
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let fresh = unique_name(&name, |c| {
        let p = dir.join(c);
        async move { tokio::fs::symlink_metadata(p).await.is_ok() }
    })
    .await?;
    Ok(dir.join(fresh))
}

#[allow(clippy::too_many_arguments)]
async fn run_transfer<R: Runtime>(
    app: &AppHandle<R>,
    id: Uuid,
    sftp: Arc<Sftp>,
    direction: Direction,
    mut local: PathBuf,
    mut remote: String,
    conflict: Conflict,
    cancel: CancellationToken,
) -> Result<(u64, usize)> {
    if conflict == Conflict::Rename {
        match direction {
            Direction::Upload if sftp.exists(&remote).await? => {
                remote = unique_remote(&sftp, &remote).await?;
            }
            Direction::Download if tokio::fs::symlink_metadata(&local).await.is_ok() => {
                local = unique_local(&local).await?;
            }
            _ => {}
        }
    }
    let plan = match direction {
        Direction::Upload => {
            let (l, r) = (local.clone(), remote.clone());
            tokio::task::spawn_blocking(move || plan_upload_sync(l, r))
                .await
                .map_err(|e| DesktopError::new("io", e.to_string()))??
        }
        Direction::Download => plan_download(&sftp, remote.clone(), local.clone()).await?,
    };
    let reporter = Arc::new(Reporter {
        app: app.clone(),
        id,
        total: plan.total,
        files_total: plan.files.len(),
        state: Mutex::new((Instant::now() - PROGRESS_INTERVAL, 0, 0, String::new())),
    });

    for d in &plan.dirs {
        match direction {
            Direction::Upload => sftp.mkdir_all(d).await?,
            Direction::Download => tokio::fs::create_dir_all(d).await?,
        }
    }

    let mut base = 0u64;
    let mut moved = 0u64;
    let mut skipped = 0usize;
    for (i, item) in plan.files.iter().enumerate() {
        let current = match direction {
            Direction::Upload => item.remote.clone(),
            Direction::Download => item.local.to_string_lossy().into_owned(),
        };
        reporter.report(base, i, skipped, &current, true);
        if conflict == Conflict::Skip {
            let exists = match direction {
                Direction::Upload => sftp.exists(&item.remote).await?,
                Direction::Download => tokio::fs::symlink_metadata(&item.local).await.is_ok(),
            };
            if exists {
                skipped += 1;
                base += item.size.unwrap_or(0);
                continue;
            }
        }
        let rep = reporter.clone();
        let cur = current.clone();
        let opts = TransferOptions {
            resume: conflict == Conflict::Resume,
            preserve_mtime: true,
            cancel: cancel.clone(),
            progress: Some(Arc::new(move |p: Progress| {
                rep.report(base + p.done, i, skipped, &cur, false);
            })),
        };
        moved += match direction {
            Direction::Upload => sftp.upload(&item.local, &item.remote, &opts).await?,
            Direction::Download => sftp.download(&item.remote, &item.local, &opts).await?,
        };
        base += item.size.unwrap_or(0);
    }
    reporter.report(base, plan.files.len(), skipped, "", true);
    Ok((moved, skipped))
}

// ───────────────────────────── drops from the OS ─────────────────────────────
//
// The webview receives files dragged in from the desktop as `File` blobs
// without paths, so their bytes are streamed into a private staging directory
// and uploaded from there; the staged copy is removed once the transfer ends.

fn drop_root() -> PathBuf {
    std::env::temp_dir().join("termoso-drop")
}

#[cfg(unix)]
fn restrict(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn restrict(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Create a fresh staging directory and return its path.
pub async fn drop_begin() -> Result<String> {
    let dir = drop_root().join(Uuid::new_v4().to_string());
    tokio::task::spawn_blocking({
        let dir = dir.clone();
        move || -> std::io::Result<()> {
            std::fs::create_dir_all(&dir)?;
            restrict(&dir)
        }
    })
    .await
    .map_err(|e| DesktopError::new("io", e.to_string()))??;
    Ok(dir.to_string_lossy().into_owned())
}

fn staged_path(dir: &str, rel: &str) -> Result<PathBuf> {
    let root = PathBuf::from(dir);
    if !root.starts_with(drop_root()) || rel.is_empty() {
        return Err(DesktopError::invalid("invalid drop target"));
    }
    let mut out = root;
    for seg in rel.split(['/', '\\']) {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(DesktopError::invalid("invalid drop path"));
        }
        out.push(seg);
    }
    Ok(out)
}

/// Append a chunk of a dropped file (creating it and its parents on the first
/// chunk).
pub async fn drop_write(dir: String, rel: String, bytes: Vec<u8>, append: bool) -> Result<()> {
    let path = staged_path(&dir, &rel)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(&path)
        .await?;
    tokio::io::AsyncWriteExt::write_all(&mut file, &bytes).await?;
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    Ok(())
}

/// Materialise an (empty) dropped directory.
pub async fn drop_mkdir(dir: String, rel: String) -> Result<()> {
    let path = staged_path(&dir, &rel)?;
    tokio::fs::create_dir_all(path).await?;
    Ok(())
}

/// Remove a staged item and its staging directory once nothing is left there.
async fn drop_discard(path: &Path) {
    if !path.starts_with(drop_root()) {
        return;
    }
    match tokio::fs::symlink_metadata(path).await {
        Ok(m) if m.is_dir() => {
            let _ = tokio::fs::remove_dir_all(path).await;
        }
        Ok(_) => {
            let _ = tokio::fs::remove_file(path).await;
        }
        Err(_) => {}
    }
    if let Some(parent) = path.parent()
        && parent != drop_root()
    {
        let _ = tokio::fs::remove_dir(parent).await;
    }
}

/// Drop a staging directory the UI gave up on before any transfer started.
pub async fn drop_abort(dir: String) -> Result<()> {
    let path = PathBuf::from(dir);
    if path.parent() != Some(drop_root().as_path()) {
        return Err(DesktopError::invalid("invalid drop target"));
    }
    let _ = tokio::fs::remove_dir_all(path).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unique_name_keeps_extension_and_counts_up() {
        let taken = |c: String| async move { c == "a (1).txt" || c == "a (2).txt" };
        assert_eq!(unique_name("a.txt", taken).await.unwrap(), "a (3).txt");
        let free = |_c: String| async move { false };
        assert_eq!(unique_name("Makefile", free).await.unwrap(), "Makefile (1)");
        assert_eq!(unique_name(".env", free).await.unwrap(), ".env (1)");
    }

    #[test]
    fn remote_paths_split_and_join() {
        assert_eq!(
            remote_split("/srv/www/index.html"),
            ("/srv/www".into(), "index.html".into())
        );
        assert_eq!(remote_split("/top"), ("/".into(), "top".into()));
        assert_eq!(remote_split("/srv/dir/"), ("/srv".into(), "dir".into()));
        assert_eq!(remote_join("/", "x"), "/x");
        assert_eq!(remote_join("/a", "b"), "/a/b");
    }

    #[test]
    fn staged_paths_stay_inside_their_directory() {
        let dir = drop_root().join("test-stage");
        let dir_s = dir.to_string_lossy().into_owned();
        assert_eq!(
            staged_path(&dir_s, "a/b.txt").unwrap(),
            dir.join("a").join("b.txt")
        );
        assert!(staged_path(&dir_s, "../escape").is_err());
        assert!(staged_path(&dir_s, "a/../../escape").is_err());
        assert!(staged_path(&dir_s, "").is_err());
        assert!(staged_path("/tmp/elsewhere", "a").is_err());
    }

    #[tokio::test]
    async fn drop_staging_roundtrip() {
        let dir = drop_begin().await.unwrap();
        drop_mkdir(dir.clone(), "sub".into()).await.unwrap();
        drop_write(dir.clone(), "sub/f.bin".into(), b"hel".to_vec(), false)
            .await
            .unwrap();
        drop_write(dir.clone(), "sub/f.bin".into(), b"lo".to_vec(), true)
            .await
            .unwrap();
        let path = PathBuf::from(&dir).join("sub").join("f.bin");
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"hello");
        drop_discard(&PathBuf::from(&dir).join("sub")).await;
        assert!(!path.exists());
        assert!(!PathBuf::from(&dir).exists());
        assert!(drop_abort("/tmp/not-a-drop".into()).await.is_err());
    }
}
