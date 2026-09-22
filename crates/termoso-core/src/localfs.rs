//! The device's own filesystem behind [`RemoteFs`], confined to one
//! directory — the home of the local shell on mobile. Callers see virtual
//! absolute paths where `/` is that directory; nothing above it can be
//! named, and a symlink that leaves it is treated as dangling so the
//! browser (and the Android documents provider built on it, which hands
//! files to other apps) never reaches the rest of the app's private data.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::error::{CoreError, Result};
use crate::remote::{self, RemoteCapabilities, RemoteFile, RemoteFs, RemoteProtocol};
use crate::sftp::{EntryKind, OpenMode, RemoteEntry, TransferOptions};
use crate::webdav::normalize_path;

const CHUNK: usize = 256 * 1024;
/// [`RemoteFs::read`] refuses files above this.
const MAX_IN_MEMORY: u64 = 64 * 1024 * 1024;

/// A directory on this device served as a remote filesystem.
#[derive(Debug, Clone)]
pub struct LocalFs {
    root: Arc<PathBuf>,
}

impl LocalFs {
    /// Serve `root`, creating it if needed. The root is canonicalised once
    /// so the confinement check below compares real paths.
    pub async fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        tokio::fs::create_dir_all(root).await?;
        let root = tokio::fs::canonicalize(root).await?;
        Ok(Self {
            root: Arc::new(root),
        })
    }

    /// The directory behind `/`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Virtual path → normalised virtual path and the real path under the root.
    fn real(&self, path: &str) -> Result<(String, PathBuf)> {
        let norm = normalize_path(path)?;
        let mut real = (*self.root).clone();
        for seg in norm.split('/').filter(|s| !s.is_empty()) {
            real.push(seg);
        }
        Ok((norm, real))
    }

    /// Real path → virtual path, `None` when it lies outside the root.
    fn virt(&self, real: &Path) -> Option<String> {
        let rel = real.strip_prefix(&*self.root).ok()?;
        let mut out = String::new();
        for c in rel.components() {
            match c {
                Component::Normal(s) => {
                    out.push('/');
                    out.push_str(&s.to_string_lossy());
                }
                Component::CurDir => {}
                _ => return None,
            }
        }
        Some(if out.is_empty() { "/".into() } else { out })
    }

    /// Fail unless `probe`, symlinks followed, lies under the root.
    async fn confine(&self, norm: &str, probe: &Path) -> Result<()> {
        let canonical = tokio::fs::canonicalize(probe)
            .await
            .map_err(|_| CoreError::NotFound(norm.to_string()))?;
        if canonical.starts_with(&*self.root) {
            Ok(())
        } else {
            Err(CoreError::NotFound(norm.to_string()))
        }
    }

    /// Normalise `path` and check that the directory holding it is inside
    /// the root. The entry itself is not followed: a symlink pointing out
    /// can still be listed, renamed or removed, just not read through.
    async fn locate(&self, path: &str) -> Result<(String, PathBuf)> {
        let (norm, real) = self.real(path)?;
        if let Some(parent) = real.parent()
            && real != *self.root
        {
            self.confine(&norm, parent).await?;
        }
        Ok((norm, real))
    }

    /// [`Self::locate`], plus: when the entry exists, following it must stay
    /// inside the root. For everything that opens, lists or copies content.
    async fn locate_target(&self, path: &str) -> Result<(String, PathBuf)> {
        let (norm, real) = self.locate(path).await?;
        match tokio::fs::symlink_metadata(&real).await {
            Ok(_) => self.confine(&norm, &real).await?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok((norm, real))
    }

    async fn entry(&self, norm: &str, real: &Path) -> Result<RemoteEntry> {
        let meta = tokio::fs::symlink_metadata(real)
            .await
            .map_err(|e| not_found(norm, e))?;
        let kind = kind_of(&meta.file_type());
        let (link_target, target_kind) = if kind == EntryKind::Symlink {
            let target = tokio::fs::read_link(real)
                .await
                .ok()
                .map(|t| t.to_string_lossy().into_owned());
            // Where the link lands: only a kind when that is inside the root.
            let target_kind = match tokio::fs::canonicalize(real).await {
                Ok(c) if c.starts_with(&*self.root) => tokio::fs::metadata(&c)
                    .await
                    .ok()
                    .map(|m| kind_of(&m.file_type())),
                _ => None,
            };
            (target, target_kind)
        } else {
            (None, None)
        };
        let name = if norm == "/" {
            String::new()
        } else {
            norm.rsplit('/').next().unwrap_or_default().to_string()
        };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as u32);
        let atime = meta
            .accessed()
            .ok()
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as u32);
        let (mode, uid, gid) = owner_bits(&meta);
        Ok(RemoteEntry {
            name,
            path: norm.to_string(),
            kind,
            size: (kind == EntryKind::File).then_some(meta.len()),
            mode,
            uid,
            gid,
            user: None,
            group: None,
            mtime,
            atime,
            link_target,
            target_kind,
        })
    }

    async fn copy_tree(&self, from: &Path, to: &Path) -> Result<()> {
        let meta = tokio::fs::symlink_metadata(from).await?;
        if meta.is_dir() {
            tokio::fs::create_dir(to).await?;
            let mut stack = vec![(from.to_path_buf(), to.to_path_buf())];
            while let Some((src, dst)) = stack.pop() {
                let mut rd = tokio::fs::read_dir(&src).await?;
                while let Some(e) = rd.next_entry().await? {
                    let s = e.path();
                    let d = dst.join(e.file_name());
                    let ft = e.file_type().await?;
                    if ft.is_dir() {
                        tokio::fs::create_dir(&d).await?;
                        stack.push((s, d));
                    } else if ft.is_file() {
                        tokio::fs::copy(&s, &d).await?;
                    }
                    // Symlinks and special files are not copied: a copy that
                    // followed a link could pull in something outside the root.
                }
            }
            Ok(())
        } else if meta.is_file() {
            tokio::fs::copy(from, to).await?;
            Ok(())
        } else {
            Err(CoreError::Invalid(format!(
                "not a regular file or directory: {}",
                from.display()
            )))
        }
    }
}

fn kind_of(t: &std::fs::FileType) -> EntryKind {
    if t.is_symlink() {
        EntryKind::Symlink
    } else if t.is_dir() {
        EntryKind::Dir
    } else if t.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

#[cfg(unix)]
fn owner_bits(meta: &std::fs::Metadata) -> (Option<u32>, Option<u32>, Option<u32>) {
    use std::os::unix::fs::MetadataExt;
    (Some(meta.mode()), Some(meta.uid()), Some(meta.gid()))
}

#[cfg(not(unix))]
fn owner_bits(_meta: &std::fs::Metadata) -> (Option<u32>, Option<u32>, Option<u32>) {
    (None, None, None)
}

fn not_found(norm: &str, e: std::io::Error) -> CoreError {
    if e.kind() == std::io::ErrorKind::NotFound {
        CoreError::NotFound(norm.to_string())
    } else {
        e.into()
    }
}

#[async_trait::async_trait]
impl RemoteFs for LocalFs {
    fn protocol(&self) -> RemoteProtocol {
        RemoteProtocol::Local
    }

    fn capabilities(&self) -> RemoteCapabilities {
        RemoteCapabilities {
            permissions: cfg!(unix),
            symlinks: true,
            ownership: false,
            server_copy: true,
            resume_upload: true,
        }
    }

    fn home(&self) -> &str {
        "/"
    }

    async fn canonicalize(&self, path: &str) -> Result<String> {
        let (norm, real) = self.locate_target(path).await?;
        match tokio::fs::canonicalize(&real).await {
            Ok(c) => self
                .virt(&c)
                .ok_or_else(|| CoreError::NotFound(norm.clone())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(norm),
            Err(e) => Err(e.into()),
        }
    }

    async fn list(&self, dir: &str) -> Result<Vec<RemoteEntry>> {
        let (norm, real) = self.locate_target(dir).await?;
        let mut rd = tokio::fs::read_dir(&real)
            .await
            .map_err(|e| not_found(&norm, e))?;
        let mut out = Vec::new();
        while let Some(e) = rd.next_entry().await? {
            let name = e.file_name().to_string_lossy().into_owned();
            let child = remote::join(&norm, &name);
            if let Ok(entry) = self.entry(&child, &e.path()).await {
                out.push(entry);
            }
        }
        out.sort_by(|a, b| {
            let da = a.kind == EntryKind::Dir;
            let db = b.kind == EntryKind::Dir;
            db.cmp(&da)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(out)
    }

    async fn stat(&self, path: &str) -> Result<RemoteEntry> {
        let (norm, real) = self.locate(path).await?;
        self.entry(&norm, &real).await
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let (_, real) = match self.locate(path).await {
            Ok(v) => v,
            Err(CoreError::NotFound(_)) => return Ok(false),
            Err(e) => return Err(e),
        };
        Ok(tokio::fs::symlink_metadata(&real).await.is_ok())
    }

    async fn mkdir(&self, path: &str) -> Result<()> {
        let (_, real) = self.locate(path).await?;
        tokio::fs::create_dir(&real).await?;
        Ok(())
    }

    async fn mkdir_all(&self, path: &str) -> Result<()> {
        let (norm, real) = self.real(path)?;
        // Confine against the deepest ancestor that exists.
        let mut probe = real.as_path();
        while tokio::fs::symlink_metadata(probe).await.is_err() {
            match probe.parent() {
                Some(p) => probe = p,
                None => break,
            }
        }
        self.confine(&norm, probe).await?;
        tokio::fs::create_dir_all(&real).await?;
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let (from_norm, from_real) = self.locate(from).await?;
        let (to_norm, to_real) = self.locate(to).await?;
        if from_norm == "/" {
            return Err(CoreError::Invalid("cannot rename the root".into()));
        }
        if tokio::fs::symlink_metadata(&to_real).await.is_ok() {
            return Err(CoreError::Invalid(format!("already exists: {to_norm}")));
        }
        tokio::fs::rename(&from_real, &to_real)
            .await
            .map_err(|e| not_found(&from_norm, e))?;
        Ok(())
    }

    async fn copy(&self, from: &str, to: &str) -> Result<()> {
        let (from_norm, from_real) = self.locate_target(from).await?;
        let (to_norm, to_real) = self.locate(to).await?;
        if from_norm == "/" || to_norm == "/" {
            return Err(CoreError::Invalid("cannot copy the root".into()));
        }
        if to_norm.starts_with(&format!("{from_norm}/")) {
            return Err(CoreError::Invalid(
                "cannot copy a directory into itself".into(),
            ));
        }
        if tokio::fs::symlink_metadata(&to_real).await.is_ok() {
            return Err(CoreError::Invalid(format!("already exists: {to_norm}")));
        }
        if tokio::fs::symlink_metadata(&from_real).await.is_err() {
            return Err(CoreError::NotFound(from_norm));
        }
        self.copy_tree(&from_real, &to_real).await
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        let (norm, real) = self.locate(path).await?;
        tokio::fs::remove_file(&real)
            .await
            .map_err(|e| not_found(&norm, e))?;
        Ok(())
    }

    async fn remove_dir(&self, path: &str) -> Result<()> {
        let (norm, real) = self.locate(path).await?;
        if norm == "/" {
            return Err(CoreError::Invalid("cannot remove the root".into()));
        }
        tokio::fs::remove_dir(&real)
            .await
            .map_err(|e| not_found(&norm, e))?;
        Ok(())
    }

    async fn remove_dir_all(&self, path: &str, cancel: &CancellationToken) -> Result<()> {
        let (norm, real) = self.locate(path).await?;
        if norm == "/" {
            return Err(CoreError::Invalid("cannot remove the root".into()));
        }
        let meta = tokio::fs::symlink_metadata(&real)
            .await
            .map_err(|e| not_found(&norm, e))?;
        if !meta.is_dir() {
            // A symlink to a directory is unlinked, never followed.
            tokio::fs::remove_file(&real).await?;
            return Ok(());
        }
        tokio::select! {
            r = tokio::fs::remove_dir_all(&real) => r?,
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        }
        Ok(())
    }

    #[cfg(unix)]
    async fn chmod(&self, path: &str, mode: u32) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let (norm, real) = self.locate_target(path).await?;
        let meta = tokio::fs::metadata(&real)
            .await
            .map_err(|e| not_found(&norm, e))?;
        let mut perm = meta.permissions();
        perm.set_mode(mode & 0o7777);
        tokio::fs::set_permissions(&real, perm).await?;
        Ok(())
    }

    #[cfg(not(unix))]
    async fn chmod(&self, _path: &str, _mode: u32) -> Result<()> {
        Err(CoreError::Invalid(
            "permission bits are not supported here".into(),
        ))
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>> {
        let (norm, real) = self.locate_target(path).await?;
        let meta = tokio::fs::metadata(&real)
            .await
            .map_err(|e| not_found(&norm, e))?;
        if meta.is_dir() {
            return Err(CoreError::Invalid(format!("{norm} is a directory")));
        }
        if meta.len() > MAX_IN_MEMORY {
            return Err(CoreError::Invalid(
                "file too large to read into memory".into(),
            ));
        }
        Ok(tokio::fs::read(&real).await?)
    }

    async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let (_, real) = self.locate_target(path).await?;
        tokio::fs::write(&real, data).await?;
        Ok(())
    }

    async fn open_file(&self, path: &str, mode: OpenMode) -> Result<Box<dyn RemoteFile>> {
        let (norm, real) = self.locate_target(path).await?;
        if tokio::fs::metadata(&real).await.is_ok_and(|m| m.is_dir()) {
            return Err(CoreError::Invalid(format!("{norm} is a directory")));
        }
        let mut opts = tokio::fs::OpenOptions::new();
        match mode {
            OpenMode::Read => opts.read(true),
            OpenMode::Write => opts.write(true).create(true).truncate(true),
            OpenMode::ReadWrite => opts.read(true).write(true).create(true),
        };
        let file = opts.open(&real).await.map_err(|e| not_found(&norm, e))?;
        Ok(Box::new(LocalFile {
            file: Mutex::new(file),
        }))
    }

    async fn download(&self, remote: &str, local: &Path, opts: &TransferOptions) -> Result<u64> {
        let (norm, real) = self.locate_target(remote).await?;
        let meta = tokio::fs::metadata(&real)
            .await
            .map_err(|e| not_found(&norm, e))?;
        if meta.is_dir() {
            return Err(CoreError::Invalid(format!("{norm} is a directory")));
        }
        let src = tokio::fs::File::open(&real).await?;
        let moved = stream_copy(src, Some(meta.len()), local, opts).await?;
        if opts.preserve_mtime
            && let Ok(m) = meta.modified()
        {
            let f = std::fs::File::open(local)?;
            let _ = f.set_modified(m);
        }
        Ok(moved)
    }

    async fn upload(&self, local: &Path, remote: &str, opts: &TransferOptions) -> Result<u64> {
        let (norm, real) = self.locate_target(remote).await?;
        if tokio::fs::metadata(&real).await.is_ok_and(|m| m.is_dir()) {
            return Err(CoreError::Invalid(format!("{norm} is a directory")));
        }
        let meta = tokio::fs::metadata(local).await?;
        if meta.is_dir() {
            return Err(CoreError::Invalid(format!(
                "{} is a directory",
                local.display()
            )));
        }
        let src = tokio::fs::File::open(local).await?;
        let moved = stream_copy(src, Some(meta.len()), &real, opts).await?;
        if opts.preserve_mtime
            && let Ok(m) = meta.modified()
        {
            let f = std::fs::File::open(&real)?;
            let _ = f.set_modified(m);
        }
        Ok(moved)
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }
}

/// Copy `src` (of `total` bytes) to `dst`, appending to what `dst` already
/// holds when `opts.resume` asks for it. Returns the bytes moved by this call.
async fn stream_copy(
    mut src: tokio::fs::File,
    total: Option<u64>,
    dst: &Path,
    opts: &TransferOptions,
) -> Result<u64> {
    let mut offset = 0u64;
    let mut file = if opts.resume && dst.exists() {
        let mut f = tokio::fs::OpenOptions::new().append(true).open(dst).await?;
        offset = f.metadata().await?.len();
        if let Some(t) = total
            && offset > t
        {
            f = tokio::fs::File::create(dst).await?;
            offset = 0;
        }
        f
    } else {
        if let Some(p) = dst.parent() {
            tokio::fs::create_dir_all(p).await?;
        }
        tokio::fs::File::create(dst).await?
    };
    if offset > 0 {
        src.seek(std::io::SeekFrom::Start(offset)).await?;
    }
    opts.report(offset, total);

    let mut done = offset;
    let mut buf = vec![0u8; CHUNK];
    loop {
        if opts.cancel.is_cancelled() {
            file.flush().await?;
            return Err(CoreError::Cancelled);
        }
        let n = src.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).await?;
        done += n as u64;
        opts.report(done, total);
    }
    file.flush().await?;
    Ok(done - offset)
}

/// Positional access to one file under the root.
pub struct LocalFile {
    file: Mutex<tokio::fs::File>,
}

#[async_trait::async_trait]
impl RemoteFile for LocalFile {
    async fn size(&self) -> Result<u64> {
        let mut f = self.file.lock().await;
        f.flush().await?;
        Ok(f.metadata().await?.len())
    }

    async fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let mut f = self.file.lock().await;
        f.seek(std::io::SeekFrom::Start(offset)).await?;
        let mut out = vec![0u8; len];
        let mut filled = 0;
        while filled < len {
            let n = f.read(&mut out[filled..]).await?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        out.truncate(filled);
        Ok(out)
    }

    async fn write_at(&self, offset: u64, data: &[u8]) -> Result<()> {
        let mut f = self.file.lock().await;
        f.seek(std::io::SeekFrom::Start(offset)).await?;
        f.write_all(data).await?;
        Ok(())
    }

    async fn truncate(&self, size: u64) -> Result<()> {
        let mut f = self.file.lock().await;
        f.flush().await?;
        f.set_len(size).await?;
        Ok(())
    }

    async fn sync(&self) -> Result<()> {
        let mut f = self.file.lock().await;
        f.flush().await?;
        f.sync_all().await?;
        Ok(())
    }

    async fn close(self: Box<Self>) -> Result<()> {
        let mut f = self.file.lock().await;
        f.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fixture() -> (tempfile::TempDir, LocalFs) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("home");
        let fs = LocalFs::open(&root).await.unwrap();
        (dir, fs)
    }

    #[tokio::test]
    async fn paths_are_virtual_and_confined() {
        let (dir, fs) = fixture().await;
        assert_eq!(fs.protocol(), RemoteProtocol::Local);
        assert_eq!(fs.home(), "/");
        fs.mkdir("/a").await.unwrap();
        fs.write("/a/f.txt", b"hello").await.unwrap();
        assert_eq!(fs.read("a/f.txt").await.unwrap(), b"hello");
        assert_eq!(
            fs.canonicalize("/a/../a/./f.txt").await.unwrap(),
            "/a/f.txt"
        );
        assert!(fs.canonicalize("/..").await.is_err());
        assert!(fs.exists("/../home").await.is_err());
        let listed = fs.list("/").await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].path, "/a");
        assert_eq!(listed[0].kind, EntryKind::Dir);
        let f = fs.stat("/a/f.txt").await.unwrap();
        assert_eq!(f.name, "f.txt");
        assert_eq!(f.size, Some(5));
        assert!(dir.path().join("home/a/f.txt").is_file());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlinks_out_of_the_root_are_dangling() {
        let (dir, fs) = fixture().await;
        let secret = dir.path().join("vault.db");
        std::fs::write(&secret, b"top secret").unwrap();
        std::os::unix::fs::symlink(&secret, dir.path().join("home/leak")).unwrap();
        std::os::unix::fs::symlink(dir.path(), dir.path().join("home/updir")).unwrap();
        fs.write("/inside.txt", b"ok").await.unwrap();
        std::os::unix::fs::symlink(
            dir.path().join("home/inside.txt"),
            dir.path().join("home/fine"),
        )
        .unwrap();

        let leak = fs.stat("/leak").await.unwrap();
        assert_eq!(leak.kind, EntryKind::Symlink);
        assert_eq!(leak.target_kind, None);
        assert!(matches!(
            fs.read("/leak").await,
            Err(CoreError::NotFound(_))
        ));
        assert!(matches!(
            fs.open_file("/leak", OpenMode::Read).await.map(|_| ()),
            Err(CoreError::NotFound(_))
        ));
        assert!(matches!(
            fs.list("/updir").await,
            Err(CoreError::NotFound(_))
        ));
        assert!(matches!(
            fs.read("/updir/vault.db").await,
            Err(CoreError::NotFound(_))
        ));
        assert!(matches!(
            fs.write("/updir/evil", b"x").await,
            Err(CoreError::NotFound(_))
        ));
        assert!(!dir.path().join("evil").exists());

        let fine = fs.stat("/fine").await.unwrap();
        assert_eq!(fine.target_kind, Some(EntryKind::File));
        assert_eq!(fs.read("/fine").await.unwrap(), b"ok");
        assert_eq!(fs.canonicalize("/fine").await.unwrap(), "/inside.txt");

        // Removing the link never touches what it points at.
        fs.remove_dir_all("/updir", &CancellationToken::new())
            .await
            .unwrap();
        assert!(dir.path().join("vault.db").is_file());
        assert!(!dir.path().join("home/updir").exists());
    }

    #[tokio::test]
    async fn rename_copy_remove() {
        let (_dir, fs) = fixture().await;
        fs.mkdir_all("/d/e").await.unwrap();
        fs.write("/d/e/x", b"1").await.unwrap();
        fs.rename("/d/e/x", "/d/y").await.unwrap();
        assert!(!fs.exists("/d/e/x").await.unwrap());
        assert!(matches!(
            fs.rename("/d/y", "/d/e").await,
            Err(CoreError::Invalid(_))
        ));
        fs.copy("/d", "/d2").await.unwrap();
        assert_eq!(fs.read("/d2/y").await.unwrap(), b"1");
        assert!(fs.stat("/d2/e").await.unwrap().kind == EntryKind::Dir);
        assert!(fs.copy("/d", "/d/inner").await.is_err());
        assert!(matches!(fs.remove_dir("/d").await, Err(CoreError::Io(_))));
        fs.remove_dir_all("/d", &CancellationToken::new())
            .await
            .unwrap();
        assert!(!fs.exists("/d").await.unwrap());
        assert!(matches!(
            fs.remove_dir("/").await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            fs.remove_dir_all("/", &CancellationToken::new()).await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            fs.remove_file("/missing").await,
            Err(CoreError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn positional_file_and_transfers() {
        let (dir, fs) = fixture().await;
        let f = fs.open_file("/pos", OpenMode::ReadWrite).await.unwrap();
        f.write_at(0, b"hello world").await.unwrap();
        f.write_at(6, b"there").await.unwrap();
        assert_eq!(f.read_at(0, 32).await.unwrap(), b"hello there");
        f.truncate(5).await.unwrap();
        assert_eq!(f.size().await.unwrap(), 5);
        f.sync().await.unwrap();
        f.close().await.unwrap();
        assert_eq!(fs.read("/pos").await.unwrap(), b"hello");

        let outside = dir.path().join("out.bin");
        let moved = fs
            .download("/pos", &outside, &TransferOptions::default())
            .await
            .unwrap();
        assert_eq!(moved, 5);
        assert_eq!(std::fs::read(&outside).unwrap(), b"hello");

        std::fs::write(&outside, b"hello again").unwrap();
        let resumed = fs
            .upload(
                &outside,
                "/pos",
                &TransferOptions {
                    resume: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(resumed, 6);
        assert_eq!(fs.read("/pos").await.unwrap(), b"hello again");
        assert!(
            fs.upload(&outside, "/", &TransferOptions::default())
                .await
                .is_err()
        );
    }
}
