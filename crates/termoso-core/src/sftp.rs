//! SFTP over an [`SshClient`](crate::ssh::SshClient), with typed listing and
//! resumable, cancellable transfers that report progress.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use russh_sftp::client::{Config as SftpConfig, SftpSession};
use russh_sftp::protocol::{FileAttributes, FileType, OpenFlags, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use crate::error::{CoreError, Result};
use crate::ssh::SshClient;

/// Chunk size for transfers.
const CHUNK: usize = 256 * 1024;
/// Requests kept in flight per open file; with 256 KiB packets this allows
/// 8 MiB outstanding, enough to fill a 100+ ms RTT link.
const IN_FLIGHT: usize = 32;
/// Per-request response deadline. The last of the in-flight requests waits
/// behind everything queued before it (8 MiB per file, several files at
/// once), which on a slow mobile link takes minutes; a dead connection is
/// caught by SSH keepalives, so this only has to be a safety net.
const REQUEST_TIMEOUT_SECS: u64 = 600;

/// Directory entry kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Directory.
    Dir,
    /// Regular file.
    File,
    /// Symbolic link.
    Symlink,
    /// Device, socket, fifo…
    Other,
}

impl From<FileType> for EntryKind {
    fn from(t: FileType) -> Self {
        match t {
            FileType::Dir => Self::Dir,
            FileType::File => Self::File,
            FileType::Symlink => Self::Symlink,
            FileType::Other => Self::Other,
        }
    }
}

/// A remote file or directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEntry {
    /// Name without directory.
    pub name: String,
    /// Absolute path.
    pub path: String,
    /// Kind.
    pub kind: EntryKind,
    /// Size in bytes.
    pub size: Option<u64>,
    /// Unix mode bits (permissions only).
    pub mode: Option<u32>,
    /// Owner uid.
    pub uid: Option<u32>,
    /// Owner gid.
    pub gid: Option<u32>,
    /// Owner name if the server sent one.
    pub user: Option<String>,
    /// Group name if the server sent one.
    pub group: Option<String>,
    /// Modification time (unix seconds).
    pub mtime: Option<u32>,
    /// Access time (unix seconds).
    pub atime: Option<u32>,
    /// Link target for symlinks (filled by [`Sftp::list`]).
    pub link_target: Option<String>,
    /// What a symlink points at; `None` for non-links and dangling links.
    pub target_kind: Option<EntryKind>,
}

impl RemoteEntry {
    fn from_attrs(name: String, path: String, a: &FileAttributes) -> Self {
        Self {
            name,
            path,
            kind: a.file_type().into(),
            size: a.size,
            mode: a.permissions.map(|p| p & 0o7777),
            uid: a.uid,
            gid: a.gid,
            user: a.user.clone(),
            group: a.group.clone(),
            mtime: a.mtime,
            atime: a.atime,
            link_target: None,
            target_kind: None,
        }
    }
}

/// Transfer progress snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// Bytes moved so far (including any resumed offset).
    pub done: u64,
    /// Total bytes if known.
    pub total: Option<u64>,
}

/// Progress sink.
pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;

/// Options for one transfer.
#[derive(Clone)]
pub struct TransferOptions {
    /// Continue an interrupted transfer if the destination already has a
    /// prefix of the file.
    pub resume: bool,
    /// Copy mtime to the destination.
    pub preserve_mtime: bool,
    /// Cancellation.
    pub cancel: CancellationToken,
    /// Called after every chunk.
    pub progress: Option<ProgressFn>,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            resume: false,
            preserve_mtime: true,
            cancel: CancellationToken::new(),
            progress: None,
        }
    }
}

impl TransferOptions {
    fn report(&self, done: u64, total: Option<u64>) {
        if let Some(p) = &self.progress {
            p(Progress { done, total });
        }
    }
}

/// SFTP session on top of an SSH connection.
pub struct Sftp {
    session: SftpSession,
    home: String,
}

impl std::fmt::Debug for Sftp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sftp").field("home", &self.home).finish()
    }
}

fn sftp_err(e: russh_sftp::client::error::Error) -> CoreError {
    match &e {
        russh_sftp::client::error::Error::Status(s) if s.status_code == StatusCode::NoSuchFile => {
            CoreError::NotFound(s.error_message.clone())
        }
        _ => CoreError::Sftp(e.to_string()),
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

impl Sftp {
    /// Request the `sftp` subsystem on `client`.
    pub async fn open(client: &SshClient) -> Result<Self> {
        let channel = client.open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        let cfg = SftpConfig {
            max_packet_len: CHUNK as u32,
            max_write_packet_len: CHUNK as u32,
            max_concurrent_reads: IN_FLIGHT,
            max_concurrent_writes: IN_FLIGHT,
            request_timeout_secs: REQUEST_TIMEOUT_SECS,
        };
        let session = SftpSession::new_with_config(channel.into_stream(), cfg)
            .await
            .map_err(sftp_err)?;
        let home = session.canonicalize(".").await.map_err(sftp_err)?;
        Ok(Self { session, home })
    }

    /// The user's home directory (server's idea of `.`).
    pub fn home(&self) -> &str {
        &self.home
    }

    /// Make a path absolute per the server.
    pub async fn canonicalize(&self, path: &str) -> Result<String> {
        self.session.canonicalize(path).await.map_err(sftp_err)
    }

    /// List a directory, sorted directories-first then by name. Symlink
    /// targets are resolved so the UI can show where they point and whether
    /// they can be followed.
    pub async fn list(&self, dir: &str) -> Result<Vec<RemoteEntry>> {
        let dir = self.canonicalize(dir).await?;
        let rd = self.session.read_dir(&dir).await.map_err(sftp_err)?;
        let mut out = Vec::new();
        for e in rd {
            let name = e.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let path = join(&dir, &name);
            let mut entry = RemoteEntry::from_attrs(name, path.clone(), &e.metadata());
            if entry.kind == EntryKind::Symlink {
                if let Ok(target) = self.session.read_link(&path).await {
                    entry.link_target = Some(target);
                }
                entry.target_kind = self
                    .session
                    .metadata(&path)
                    .await
                    .ok()
                    .map(|a| a.file_type().into());
            }
            out.push(entry);
        }
        out.sort_by(|a, b| {
            let da = a.kind == EntryKind::Dir;
            let db = b.kind == EntryKind::Dir;
            db.cmp(&da)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(out)
    }

    /// Stat a path (follows symlinks).
    pub async fn stat(&self, path: &str) -> Result<RemoteEntry> {
        let a = self.session.metadata(path).await.map_err(sftp_err)?;
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        Ok(RemoteEntry::from_attrs(name, path.to_string(), &a))
    }

    /// Does `path` exist?
    pub async fn exists(&self, path: &str) -> Result<bool> {
        self.session.try_exists(path).await.map_err(sftp_err)
    }

    /// Create a single directory.
    pub async fn mkdir(&self, path: &str) -> Result<()> {
        self.session.create_dir(path).await.map_err(sftp_err)
    }

    /// Create a directory and all missing parents.
    pub async fn mkdir_all(&self, path: &str) -> Result<()> {
        let mut cur = if path.starts_with('/') {
            String::from("/")
        } else {
            String::new()
        };
        for seg in path.split('/').filter(|s| !s.is_empty()) {
            if !cur.is_empty() && !cur.ends_with('/') {
                cur.push('/');
            }
            cur.push_str(seg);
            match self.session.metadata(&cur).await {
                Ok(a) if a.is_dir() => continue,
                Ok(_) => {
                    return Err(CoreError::Sftp(format!(
                        "{cur} exists and is not a directory"
                    )));
                }
                Err(_) => self.session.create_dir(&cur).await.map_err(sftp_err)?,
            }
        }
        Ok(())
    }

    /// Rename / move.
    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.session.rename(from, to).await.map_err(sftp_err)
    }

    /// Remove a file.
    pub async fn remove_file(&self, path: &str) -> Result<()> {
        self.session.remove_file(path).await.map_err(sftp_err)
    }

    /// Remove an empty directory.
    pub async fn remove_dir(&self, path: &str) -> Result<()> {
        self.session.remove_dir(path).await.map_err(sftp_err)
    }

    /// Remove a directory tree. Symlinks are unlinked, not followed.
    pub async fn remove_dir_all(&self, path: &str, cancel: &CancellationToken) -> Result<()> {
        let mut stack = vec![path.to_string()];
        let mut dirs = Vec::new();
        while let Some(d) = stack.pop() {
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            let rd = self.session.read_dir(&d).await.map_err(sftp_err)?;
            for e in rd {
                let name = e.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let p = join(&d, &name);
                if e.file_type().is_dir() {
                    stack.push(p);
                } else {
                    self.session.remove_file(&p).await.map_err(sftp_err)?;
                }
            }
            dirs.push(d);
        }
        for d in dirs.into_iter().rev() {
            self.session.remove_dir(&d).await.map_err(sftp_err)?;
        }
        Ok(())
    }

    /// Set permission bits.
    pub async fn chmod(&self, path: &str, mode: u32) -> Result<()> {
        let mut a = self.session.metadata(path).await.map_err(sftp_err)?;
        let ftype = a.permissions.unwrap_or(0) & !0o7777;
        a.permissions = Some(ftype | (mode & 0o7777));
        a.size = None;
        a.uid = None;
        a.gid = None;
        a.user = None;
        a.group = None;
        a.atime = None;
        a.mtime = None;
        self.session.set_metadata(path, a).await.map_err(sftp_err)
    }

    /// Create a symlink at `link` pointing to `target`.
    pub async fn symlink(&self, link: &str, target: &str) -> Result<()> {
        self.session.symlink(link, target).await.map_err(sftp_err)
    }

    /// Read a whole file into memory (for editors/preview; refuses > 64 MiB).
    pub async fn read(&self, path: &str) -> Result<Vec<u8>> {
        let a = self.session.metadata(path).await.map_err(sftp_err)?;
        if a.size.unwrap_or(0) > 64 * 1024 * 1024 {
            return Err(CoreError::Sftp("file too large to read into memory".into()));
        }
        self.session.read(path).await.map_err(sftp_err)
    }

    /// Write a whole file (truncating).
    pub async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let mut f = self.session.create(path).await.map_err(sftp_err)?;
        f.write_all(data).await?;
        f.close().await?;
        Ok(())
    }

    /// Download `remote` to `local`. Returns the number of bytes transferred by
    /// this call (excluding any resumed prefix).
    pub async fn download(
        &self,
        remote: &str,
        local: &Path,
        opts: &TransferOptions,
    ) -> Result<u64> {
        let attrs = self.session.metadata(remote).await.map_err(sftp_err)?;
        if attrs.is_dir() {
            return Err(CoreError::Sftp(format!("{remote} is a directory")));
        }
        let total = attrs.size;

        let mut offset = 0u64;
        let mut file = if opts.resume && local.exists() {
            let mut f = tokio::fs::OpenOptions::new()
                .append(true)
                .open(local)
                .await?;
            offset = f.metadata().await?.len();
            if let Some(t) = total
                && offset > t
            {
                // Local is longer than remote: start over.
                f = tokio::fs::File::create(local).await?;
                offset = 0;
            }
            f
        } else {
            if let Some(parent) = local.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::File::create(local).await?
        };

        let mut src = self
            .session
            .open_with_flags(remote, OpenFlags::READ)
            .await
            .map_err(sftp_err)?;
        if offset > 0 {
            src.seek(std::io::SeekFrom::Start(offset)).await?;
        }
        opts.report(offset, total);

        let done = AtomicU64::new(offset);
        let mut buf = vec![0u8; CHUNK];
        loop {
            if opts.cancel.is_cancelled() {
                file.flush().await?;
                return Err(CoreError::Cancelled);
            }
            let n = tokio::select! {
                r = src.read(&mut buf) => r?,
                _ = opts.cancel.cancelled() => { file.flush().await?; return Err(CoreError::Cancelled) }
            };
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).await?;
            let d = done.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
            opts.report(d, total);
        }
        file.flush().await?;
        drop(file);
        let _ = src.close().await;

        if opts.preserve_mtime
            && let Some(mtime) = attrs.mtime
        {
            let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime as u64);
            let f = std::fs::File::open(local)?;
            let _ = f.set_modified(t);
        }
        Ok(done.load(Ordering::Relaxed) - offset)
    }

    /// Upload `local` to `remote`. Returns the number of bytes transferred by
    /// this call (excluding any resumed prefix).
    pub async fn upload(&self, local: &Path, remote: &str, opts: &TransferOptions) -> Result<u64> {
        let meta = tokio::fs::metadata(local).await?;
        if meta.is_dir() {
            return Err(CoreError::Sftp(format!(
                "{} is a directory",
                local.display()
            )));
        }
        let total = Some(meta.len());

        let mut offset = 0u64;
        let mut flags = OpenFlags::WRITE | OpenFlags::CREATE;
        if opts.resume
            && let Ok(existing) = self.session.metadata(remote).await
        {
            let have = existing.size.unwrap_or(0);
            if have <= meta.len() {
                offset = have;
                flags |= OpenFlags::APPEND;
            } else {
                flags |= OpenFlags::TRUNCATE;
            }
        } else {
            flags |= OpenFlags::TRUNCATE;
        }

        let mut src = tokio::fs::File::open(local).await?;
        if offset > 0 {
            src.seek(std::io::SeekFrom::Start(offset)).await?;
        }
        let mut dst = self
            .session
            .open_with_flags(remote, flags)
            .await
            .map_err(sftp_err)?;
        // O_APPEND servers ignore the offset; the rest need it.
        if offset > 0 {
            dst.seek(std::io::SeekFrom::Start(offset)).await?;
        }
        opts.report(offset, total);

        let mut done = offset;
        let mut buf = vec![0u8; CHUNK];
        loop {
            if opts.cancel.is_cancelled() {
                let _ = dst.close().await;
                return Err(CoreError::Cancelled);
            }
            let n = src.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            tokio::select! {
                r = dst.write_all(&buf[..n]) => r?,
                _ = opts.cancel.cancelled() => { let _ = dst.close().await; return Err(CoreError::Cancelled) }
            }
            done += n as u64;
            opts.report(done, total);
        }
        dst.flush().await?;
        dst.close().await?;

        if opts.preserve_mtime
            && let Ok(modified) = meta.modified()
        {
            let secs = modified
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as u32)
                .unwrap_or(0);
            let mut a = FileAttributes::empty();
            a.mtime = Some(secs);
            a.atime = Some(secs);
            let _ = self.session.set_metadata(remote, a).await;
        }
        Ok(done - offset)
    }

    /// Close the subsystem channel.
    pub async fn close(&self) -> Result<()> {
        self.session.close().await.map_err(sftp_err)
    }
}
