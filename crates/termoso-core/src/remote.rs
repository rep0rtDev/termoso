//! Protocol-neutral view of a remote filesystem, implemented by
//! [`Sftp`](crate::sftp::Sftp), [`WebDav`](crate::webdav::WebDav) and
//! [`LocalFs`](crate::localfs::LocalFs) so the file panels, transfer queues
//! and the Android documents provider drive any of them through one object.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::sftp::{OpenMode, RemoteEntry, TransferOptions};

/// Which wire protocol a [`RemoteFs`] speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProtocol {
    /// SFTP over SSH.
    Sftp,
    /// WebDAV over HTTP(S).
    WebDav,
    /// A directory on this device.
    Local,
}

impl RemoteProtocol {
    /// Lowercase wire name (`"sftp"`, `"webdav"`, `"local"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sftp => "sftp",
            Self::WebDav => "webdav",
            Self::Local => "local",
        }
    }
}

/// What a backend can do beyond the common listing/transfer set, so UIs
/// hide the controls a protocol has no answer for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCapabilities {
    /// Unix permission bits can be changed (`chmod`).
    pub permissions: bool,
    /// Symlinks are reported and can be created.
    pub symlinks: bool,
    /// Owner / group are reported.
    pub ownership: bool,
    /// Server-side copy without downloading (`copy`).
    pub server_copy: bool,
    /// Interrupted uploads can continue from the bytes already there.
    pub resume_upload: bool,
}

/// An open remote file with positional access.
#[async_trait]
pub trait RemoteFile: Send + Sync {
    /// Current size in bytes.
    async fn size(&self) -> Result<u64>;
    /// Up to `len` bytes starting at `offset`; shorter only at end of file.
    async fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>>;
    /// Write all of `data` at `offset`, growing the file as needed.
    async fn write_at(&self, offset: u64, data: &[u8]) -> Result<()>;
    /// Set the file's length.
    async fn truncate(&self, size: u64) -> Result<()>;
    /// Push pending writes to the server.
    async fn sync(&self) -> Result<()>;
    /// Flush and release the handle.
    async fn close(self: Box<Self>) -> Result<()>;
}

/// A connected remote filesystem. Paths are absolute, `/`-separated.
#[async_trait]
pub trait RemoteFs: Send + Sync {
    /// Wire protocol.
    fn protocol(&self) -> RemoteProtocol;
    /// Feature set.
    fn capabilities(&self) -> RemoteCapabilities;
    /// Directory a fresh panel opens on.
    fn home(&self) -> &str;
    /// Make a path absolute and normalised per the server.
    async fn canonicalize(&self, path: &str) -> Result<String>;
    /// List a directory (directories first, then by name).
    async fn list(&self, dir: &str) -> Result<Vec<RemoteEntry>>;
    /// Stat a path.
    async fn stat(&self, path: &str) -> Result<RemoteEntry>;
    /// Does `path` exist?
    async fn exists(&self, path: &str) -> Result<bool>;
    /// Create a single directory.
    async fn mkdir(&self, path: &str) -> Result<()>;
    /// Create a directory and all missing parents.
    async fn mkdir_all(&self, path: &str) -> Result<()>;
    /// Rename / move; fails when `to` already exists.
    async fn rename(&self, from: &str, to: &str) -> Result<()>;
    /// Server-side copy; only when `capabilities().server_copy`.
    async fn copy(&self, from: &str, to: &str) -> Result<()>;
    /// Remove a file.
    async fn remove_file(&self, path: &str) -> Result<()>;
    /// Remove an empty directory.
    async fn remove_dir(&self, path: &str) -> Result<()>;
    /// Remove a directory tree.
    async fn remove_dir_all(&self, path: &str, cancel: &CancellationToken) -> Result<()>;
    /// Set permission bits; only when `capabilities().permissions`.
    async fn chmod(&self, path: &str, mode: u32) -> Result<()>;
    /// Read a whole file into memory (refuses very large files).
    async fn read(&self, path: &str) -> Result<Vec<u8>>;
    /// Write a whole file (truncating).
    async fn write(&self, path: &str, data: &[u8]) -> Result<()>;
    /// Open `path` for positional access.
    async fn open_file(&self, path: &str, mode: OpenMode) -> Result<Box<dyn RemoteFile>>;
    /// Download `remote` to `local`; returns bytes moved by this call.
    async fn download(&self, remote: &str, local: &Path, opts: &TransferOptions) -> Result<u64>;
    /// Upload `local` to `remote`; returns bytes moved by this call.
    async fn upload(&self, local: &Path, remote: &str, opts: &TransferOptions) -> Result<u64>;
    /// Release the connection.
    async fn close(&self) -> Result<()>;
}

/// Shared handle to any backend.
pub type SharedRemoteFs = Arc<dyn RemoteFs>;

/// Parent of an absolute `/`-separated path; `None` at the root.
pub fn parent(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.rfind('/') {
        Some(0) => Some("/".to_string()),
        Some(i) => Some(trimmed[..i].to_string()),
        None => None,
    }
}

/// Join `name` onto directory `dir`.
pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_of_paths() {
        assert_eq!(parent("/"), None);
        assert_eq!(parent(""), None);
        assert_eq!(parent("/a"), Some("/".into()));
        assert_eq!(parent("/a/"), Some("/".into()));
        assert_eq!(parent("/a/b/c"), Some("/a/b".into()));
        assert_eq!(parent("rel"), None);
    }

    #[test]
    fn join_paths() {
        assert_eq!(join("/", "a"), "/a");
        assert_eq!(join("/a", "b"), "/a/b");
        assert_eq!(join("/a/", "b"), "/a/b");
    }
}
