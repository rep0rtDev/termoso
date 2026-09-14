//! SFTP over the shared connection path: one [`SftpSession`] per remote,
//! synchronous directory operations (the caller runs them off the main
//! thread) and background transfers that report through [`SftpListener`].
//!
//! Local paths are plain files Kotlin owns (its cache directory); moving
//! bytes between those and SAF documents is Kotlin's job. Rust never sees
//! content URIs.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use termoso_core::model::ResolvedHost;
use termoso_core::sftp::{self, Progress, ProgressFn, RemoteEntry, Sftp, TransferOptions};
use termoso_core::ssh::{SshClient, SshTarget};
use termoso_core::store::{ConnectionHistory, Store};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::connect::{ConnectUi, Connector, PromptAnswer, PromptRequest, connect_resolved};
use crate::error::{MobileError, Result};
use crate::session::SessionState;
use crate::settings::MobileSettings;

/// Progress callbacks are coalesced to this rate; the final one always goes out.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EntryKind {
    Dir,
    File,
    Symlink,
    Other,
}

impl From<sftp::EntryKind> for EntryKind {
    fn from(k: sftp::EntryKind) -> Self {
        match k {
            sftp::EntryKind::Dir => Self::Dir,
            sftp::EntryKind::File => Self::File,
            sftp::EntryKind::Symlink => Self::Symlink,
            sftp::EntryKind::Other => Self::Other,
        }
    }
}

/// One directory entry, ready to display.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SftpEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    /// What a symlink points at; equals `kind` for everything else.
    pub target_kind: EntryKind,
    /// Navigable: a directory or a symlink to one.
    pub is_dir: bool,
    pub size: Option<u64>,
    pub mode: Option<u32>,
    /// `drwxr-xr-x` style, `-` per unknown bit.
    pub permissions: String,
    /// `user:group` (names when the server sends them, ids otherwise).
    pub owner: Option<String>,
    pub modified_ms: Option<i64>,
    pub link_target: Option<String>,
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum TransferStatus {
    Queued,
    Running,
    Done,
    Failed { message: String },
    Cancelled,
}

/// A queued, running or finished transfer.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TransferCard {
    pub id: u64,
    pub direction: TransferDirection,
    /// Remote file name.
    pub name: String,
    pub remote_path: String,
    pub local_path: String,
    pub done: u64,
    pub total: Option<u64>,
    pub bytes_per_sec: u64,
    pub status: TransferStatus,
}

/// Callbacks into Kotlin, from Rust worker threads.
#[uniffi::export(with_foreign)]
pub trait SftpListener: Send + Sync {
    fn on_state(&self, state: SessionState);
    /// Answer with [`SftpSession::answer`] using the same `prompt_id`.
    fn on_prompt(&self, prompt_id: u64, request: PromptRequest);
    /// A transfer was queued, progressed or finished.
    fn on_transfer(&self, transfer: TransferCard);
}

struct SftpUi {
    listener: Arc<dyn SftpListener>,
    state: Arc<Mutex<SessionState>>,
}

impl ConnectUi for SftpUi {
    fn phase(&self, detail: String) {
        let state = SessionState::Connecting { detail };
        *self.state.lock().expect("state poisoned") = state.clone();
        self.listener.on_state(state);
    }

    fn prompt(&self, prompt_id: u64, request: PromptRequest) {
        self.listener.on_prompt(prompt_id, request);
    }
}

struct Live {
    sftp: Arc<Sftp>,
    client: Arc<SshClient>,
    /// Kept alive for the duration of `client`.
    _jumps: Vec<Arc<SshClient>>,
}

struct Transfer {
    card: TransferCard,
    cancel: CancellationToken,
    started: Instant,
    /// `done` when the transfer began (resumed prefix), for the rate.
    base: u64,
    last_report: Instant,
}

struct Inner {
    store: Arc<Store>,
    conn: Arc<Connector>,
    listener: Arc<dyn SftpListener>,
    state: Arc<Mutex<SessionState>>,
    live: Mutex<Option<Arc<Live>>>,
    transfers: Mutex<BTreeMap<u64, Transfer>>,
    next_transfer: AtomicU64,
    /// Fired by `disconnect`: aborts the connect and every transfer.
    closed: CancellationToken,
}

/// A remote file system. Drop-safe: dropping the last reference closes the
/// connection and cancels its transfers.
#[derive(uniffi::Object)]
pub struct SftpSession {
    id: Uuid,
    inner: Arc<Inner>,
    runtime: tokio::runtime::Handle,
}

pub(crate) struct SftpLaunch {
    pub store: Arc<Store>,
    pub target: SshTarget,
    pub resolved: Option<ResolvedHost>,
    pub settings: MobileSettings,
    pub listener: Arc<dyn SftpListener>,
}

impl SftpSession {
    pub(crate) fn launch(runtime: tokio::runtime::Handle, launch: SftpLaunch) -> Arc<Self> {
        let SftpLaunch {
            store,
            target,
            resolved,
            settings,
            listener,
        } = launch;
        let state = Arc::new(Mutex::new(SessionState::Connecting {
            detail: "Connecting…".into(),
        }));
        let conn = Arc::new(Connector::new(
            store.clone(),
            Arc::new(SftpUi {
                listener: listener.clone(),
                state: state.clone(),
            }),
        ));
        let inner = Arc::new(Inner {
            store,
            conn,
            listener,
            state,
            live: Mutex::new(None),
            transfers: Mutex::new(BTreeMap::new()),
            next_transfer: AtomicU64::new(1),
            closed: CancellationToken::new(),
        });
        let session = Arc::new(Self {
            id: Uuid::new_v4(),
            inner: inner.clone(),
            runtime: runtime.clone(),
        });
        runtime.spawn(run(inner, target, resolved, settings));
        session
    }

    fn live(&self) -> Result<Arc<Live>> {
        match self.inner.live.lock().expect("live poisoned").clone() {
            Some(l) => Ok(l),
            None => match self.state() {
                SessionState::Connecting { .. } => Err(MobileError::invalid("still connecting")),
                _ => Err(MobileError::Closed),
            },
        }
    }

    fn queue(&self, direction: TransferDirection, remote: String, local: String) -> u64 {
        let id = self.inner.next_transfer.fetch_add(1, Ordering::Relaxed);
        let name = remote.rsplit('/').next().unwrap_or(&remote).to_string();
        let card = TransferCard {
            id,
            direction,
            name,
            remote_path: remote,
            local_path: local,
            done: 0,
            total: None,
            bytes_per_sec: 0,
            status: TransferStatus::Queued,
        };
        let cancel = self.inner.closed.child_token();
        let now = Instant::now();
        self.inner
            .transfers
            .lock()
            .expect("transfers poisoned")
            .insert(
                id,
                Transfer {
                    card: card.clone(),
                    cancel: cancel.clone(),
                    started: now,
                    base: 0,
                    last_report: now,
                },
            );
        self.inner.listener.on_transfer(card);
        let inner = self.inner.clone();
        self.runtime.spawn(async move {
            let live = match inner.live.lock().expect("live poisoned").clone() {
                Some(l) => l,
                None => {
                    inner.finish(id, Err(MobileError::Closed));
                    return;
                }
            };
            let (remote, local) = {
                let mut map = inner.transfers.lock().expect("transfers poisoned");
                let Some(t) = map.get_mut(&id) else { return };
                t.card.status = TransferStatus::Running;
                t.started = Instant::now();
                (t.card.remote_path.clone(), t.card.local_path.clone())
            };
            let reporter = inner.clone();
            let progress: ProgressFn = Arc::new(move |p: Progress| reporter.progress(id, p));
            let opts = TransferOptions {
                resume: false,
                preserve_mtime: true,
                cancel,
                progress: Some(progress),
            };
            let result = match direction {
                TransferDirection::Download => {
                    live.sftp.download(&remote, Path::new(&local), &opts).await
                }
                TransferDirection::Upload => {
                    live.sftp.upload(Path::new(&local), &remote, &opts).await
                }
            };
            inner.finish(id, result.map(|_| ()).map_err(MobileError::from));
        });
        id
    }
}

#[uniffi::export]
impl SftpSession {
    pub fn id(&self) -> String {
        self.id.to_string()
    }

    pub fn state(&self) -> SessionState {
        self.inner.state.lock().expect("state poisoned").clone()
    }

    /// The remote home directory; empty until connected.
    pub fn home(&self) -> String {
        self.inner
            .live
            .lock()
            .expect("live poisoned")
            .as_ref()
            .map(|l| l.sftp.home().to_string())
            .unwrap_or_default()
    }

    /// Reply to a prompt. Returns `false` if the prompt is no longer waiting.
    pub fn answer(&self, prompt_id: u64, answer: PromptAnswer) -> bool {
        self.inner.conn.answer(prompt_id, answer)
    }

    /// Directory listing, directories first, then by name; `.`/`..` excluded.
    pub fn list(&self, dir: String) -> Result<Vec<SftpEntry>> {
        let live = self.live()?;
        self.runtime.block_on(async move {
            let mut entries: Vec<SftpEntry> = live
                .sftp
                .list(&dir)
                .await?
                .into_iter()
                .map(entry_from)
                .collect();
            entries.sort_by(|a, b| {
                b.is_dir
                    .cmp(&a.is_dir)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            Ok(entries)
        })
    }

    pub fn stat(&self, path: String) -> Result<SftpEntry> {
        let live = self.live()?;
        self.runtime
            .block_on(async move { Ok(entry_from(live.sftp.stat(&path).await?)) })
    }

    /// Absolute, symlink-free form of `path` (`~` and relative paths resolve
    /// against the home directory).
    pub fn canonicalize(&self, path: String) -> Result<String> {
        let live = self.live()?;
        self.runtime.block_on(async move {
            let p = path.trim();
            let p = if p.is_empty() || p == "~" {
                live.sftp.home().to_string()
            } else if let Some(rest) = p.strip_prefix("~/") {
                format!("{}/{rest}", live.sftp.home().trim_end_matches('/'))
            } else {
                p.to_string()
            };
            Ok(live.sftp.canonicalize(&p).await?)
        })
    }

    pub fn exists(&self, path: String) -> Result<bool> {
        let live = self.live()?;
        self.runtime
            .block_on(async move { Ok(live.sftp.exists(&path).await?) })
    }

    pub fn mkdir(&self, path: String) -> Result<()> {
        let live = self.live()?;
        self.runtime
            .block_on(async move { Ok(live.sftp.mkdir(&path).await?) })
    }

    pub fn rename(&self, from: String, to: String) -> Result<()> {
        let live = self.live()?;
        self.runtime
            .block_on(async move { Ok(live.sftp.rename(&from, &to).await?) })
    }

    /// Remove a file, symlink or (recursively) a directory.
    pub fn remove(&self, path: String) -> Result<()> {
        let live = self.live()?;
        let cancel = self.inner.closed.child_token();
        self.runtime.block_on(async move {
            let entry = live.sftp.stat(&path).await?;
            if entry.kind == sftp::EntryKind::Dir {
                live.sftp.remove_dir_all(&path, &cancel).await?;
            } else {
                live.sftp.remove_file(&path).await?;
            }
            Ok(())
        })
    }

    /// `mode` is the permission bits only (e.g. `0o644`).
    pub fn chmod(&self, path: String, mode: u32) -> Result<()> {
        let live = self.live()?;
        self.runtime
            .block_on(async move { Ok(live.sftp.chmod(&path, mode & 0o7777).await?) })
    }

    /// Small text files for previews; refuses anything over 64 MiB.
    pub fn read(&self, path: String) -> Result<Vec<u8>> {
        let live = self.live()?;
        self.runtime
            .block_on(async move { Ok(live.sftp.read(&path).await?) })
    }

    /// Queue a download of `remote` into the local file `local_path`
    /// (parents are created, an existing file is replaced). Returns the
    /// transfer id; progress arrives on the listener.
    pub fn download(&self, remote: String, local_path: String) -> u64 {
        self.queue(TransferDirection::Download, remote, local_path)
    }

    /// Queue an upload of the local file `local_path` to `remote`
    /// (replaced if it exists).
    pub fn upload(&self, local_path: String, remote: String) -> u64 {
        self.queue(TransferDirection::Upload, remote, local_path)
    }

    pub fn transfers(&self) -> Vec<TransferCard> {
        self.inner
            .transfers
            .lock()
            .expect("transfers poisoned")
            .values()
            .map(|t| t.card.clone())
            .collect()
    }

    pub fn cancel_transfer(&self, id: u64) {
        if let Some(t) = self
            .inner
            .transfers
            .lock()
            .expect("transfers poisoned")
            .get(&id)
        {
            t.cancel.cancel();
        }
    }

    /// Forget a finished transfer; running ones are left alone.
    pub fn dismiss_transfer(&self, id: u64) {
        let mut map = self.inner.transfers.lock().expect("transfers poisoned");
        if map.get(&id).is_some_and(|t| {
            !matches!(
                t.card.status,
                TransferStatus::Queued | TransferStatus::Running
            )
        }) {
            map.remove(&id);
        }
    }

    /// Tear the connection down; the object stays usable for `state()`.
    pub fn disconnect(&self) {
        self.inner.conn.cancel_prompts();
        self.inner.closed.cancel();
        let live = self.inner.live.lock().expect("live poisoned").take();
        if let Some(live) = live {
            self.runtime.spawn(async move {
                let _ = live.sftp.close().await;
                let _ = live.client.disconnect().await;
            });
        }
    }
}

impl Drop for SftpSession {
    fn drop(&mut self) {
        self.disconnect();
    }
}

impl Inner {
    fn set_state(&self, state: SessionState) {
        *self.state.lock().expect("state poisoned") = state.clone();
        self.listener.on_state(state);
    }

    fn progress(&self, id: u64, p: Progress) {
        let card = {
            let mut map = self.transfers.lock().expect("transfers poisoned");
            let Some(t) = map.get_mut(&id) else { return };
            if t.card.total.is_none() && p.total.is_some() && t.card.done == 0 {
                t.base = p.done;
            }
            t.card.done = p.done;
            t.card.total = p.total;
            let elapsed = t.started.elapsed().as_secs_f64();
            if elapsed > 0.2 {
                t.card.bytes_per_sec = ((p.done.saturating_sub(t.base)) as f64 / elapsed) as u64;
            }
            let now = Instant::now();
            let finished = p.total.is_some_and(|total| p.done >= total);
            if !finished && now.duration_since(t.last_report) < PROGRESS_INTERVAL {
                return;
            }
            t.last_report = now;
            t.card.clone()
        };
        self.listener.on_transfer(card);
    }

    fn finish(&self, id: u64, result: Result<()>) {
        let card = {
            let mut map = self.transfers.lock().expect("transfers poisoned");
            let Some(t) = map.get_mut(&id) else { return };
            t.card.status = match result {
                Ok(()) => {
                    if let Some(total) = t.card.total {
                        t.card.done = total;
                    }
                    TransferStatus::Done
                }
                Err(MobileError::Cancelled) => TransferStatus::Cancelled,
                Err(_) if t.cancel.is_cancelled() => TransferStatus::Cancelled,
                Err(e) => TransferStatus::Failed {
                    message: e.to_string(),
                },
            };
            t.card.clone()
        };
        self.listener.on_transfer(card);
    }
}

fn permissions(mode: Option<u32>, kind: EntryKind) -> String {
    let type_char = match kind {
        EntryKind::Dir => 'd',
        EntryKind::Symlink => 'l',
        EntryKind::File | EntryKind::Other => '-',
    };
    let Some(mode) = mode else {
        return format!("{type_char}---------");
    };
    let mut out = String::with_capacity(10);
    out.push(type_char);
    let bits = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    for (i, (bit, ch)) in bits.iter().enumerate() {
        let set = mode & bit != 0;
        let special = match i {
            2 => mode & 0o4000 != 0,
            5 => mode & 0o2000 != 0,
            8 => mode & 0o1000 != 0,
            _ => false,
        };
        out.push(match (set, special, i) {
            (true, true, 8) => 't',
            (false, true, 8) => 'T',
            (true, true, _) => 's',
            (false, true, _) => 'S',
            (true, false, _) => *ch,
            (false, false, _) => '-',
        });
    }
    out
}

fn entry_from(e: RemoteEntry) -> SftpEntry {
    let kind = EntryKind::from(e.kind);
    let target_kind = e.target_kind.map(EntryKind::from).unwrap_or(kind);
    let owner = match (&e.user, &e.group, e.uid, e.gid) {
        (Some(u), Some(g), _, _) => Some(format!("{u}:{g}")),
        (Some(u), None, _, Some(g)) => Some(format!("{u}:{g}")),
        (None, Some(g), Some(u), _) => Some(format!("{u}:{g}")),
        (None, None, Some(u), Some(g)) => Some(format!("{u}:{g}")),
        _ => None,
    };
    SftpEntry {
        hidden: e.name.starts_with('.'),
        permissions: permissions(e.mode, kind),
        is_dir: target_kind == EntryKind::Dir,
        owner,
        modified_ms: e.mtime.map(|t| t as i64 * 1000),
        name: e.name,
        path: e.path,
        kind,
        target_kind,
        size: e.size,
        mode: e.mode.map(|m| m & 0o7777),
        link_target: e.link_target,
    }
}

async fn run(
    inner: Arc<Inner>,
    target: SshTarget,
    resolved: Option<ResolvedHost>,
    settings: MobileSettings,
) {
    let started = Instant::now();
    let label = resolved
        .as_ref()
        .map(|r| r.host.data.label.clone())
        .unwrap_or_else(|| target.host.clone());
    let history = |duration: Option<u64>, error: Option<String>| ConnectionHistory {
        host_id: resolved.as_ref().map(|r| r.host.id),
        label: label.clone(),
        target: target.display(),
        protocol: "sftp".into(),
        duration_secs: duration,
        error,
    };
    let history_id = inner.store.record_connection(&history(None, None)).ok();
    let finish = |error: Option<String>| {
        if let Some(id) = history_id {
            let _ = inner
                .store
                .update_connection(id, &history(Some(started.elapsed().as_secs()), error));
        }
    };

    let connect = async {
        let (client, jumps) =
            connect_resolved(&inner.conn, &settings, target.clone(), resolved.as_ref()).await?;
        inner.set_state(SessionState::Connecting {
            detail: "Opening SFTP…".into(),
        });
        let sftp = Sftp::open(&client).await?;
        Ok::<_, MobileError>(Live {
            sftp: Arc::new(sftp),
            client,
            _jumps: jumps,
        })
    };
    let live = tokio::select! {
        r = connect => match r {
            Ok(l) => l,
            Err(e) => {
                let e = inner.conn.map_error(e);
                finish(Some(e.to_string()));
                inner.set_state(SessionState::Failed { kind: e.kind(), message: e.to_string() });
                return;
            }
        },
        _ = inner.closed.cancelled() => {
            finish(Some("cancelled".into()));
            inner.set_state(SessionState::Closed { reason: None });
            return;
        }
    };
    if inner.closed.is_cancelled() {
        let _ = live.sftp.close().await;
        let _ = live.client.disconnect().await;
        finish(Some("cancelled".into()));
        inner.set_state(SessionState::Closed { reason: None });
        return;
    }
    *inner.live.lock().expect("live poisoned") = Some(Arc::new(live));
    inner.set_state(SessionState::Connected);

    inner.closed.cancelled().await;
    finish(None);
    inner.set_state(SessionState::Closed { reason: None });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_strings() {
        assert_eq!(permissions(Some(0o755), EntryKind::Dir), "drwxr-xr-x");
        assert_eq!(permissions(Some(0o644), EntryKind::File), "-rw-r--r--");
        assert_eq!(permissions(Some(0o4755), EntryKind::File), "-rwsr-xr-x");
        assert_eq!(permissions(Some(0o1777), EntryKind::Dir), "drwxrwxrwt");
        assert_eq!(permissions(None, EntryKind::Symlink), "l---------");
    }

    #[test]
    fn entry_mapping() {
        let e = entry_from(RemoteEntry {
            name: ".bashrc".into(),
            path: "/home/u/.bashrc".into(),
            kind: sftp::EntryKind::Symlink,
            size: Some(12),
            mode: Some(0o100777),
            uid: Some(1000),
            gid: Some(1000),
            user: Some("u".into()),
            group: None,
            mtime: Some(1_700_000_000),
            atime: None,
            link_target: Some("/etc/skel/.bashrc".into()),
            target_kind: Some(sftp::EntryKind::File),
        });
        assert!(e.hidden);
        assert!(!e.is_dir);
        assert_eq!(e.kind, EntryKind::Symlink);
        assert_eq!(e.target_kind, EntryKind::File);
        assert_eq!(e.mode, Some(0o777));
        assert_eq!(e.permissions, "lrwxrwxrwx");
        assert_eq!(e.owner.as_deref(), Some("u:1000"));
        assert_eq!(e.modified_ms, Some(1_700_000_000_000));
    }
}
