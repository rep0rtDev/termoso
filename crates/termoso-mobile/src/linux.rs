//! A real Linux userland for the local shell on Android: an official distro
//! rootfs (Alpine / Ubuntu) unpacked into the app's data directory and run
//! under `proot` (bundled in the APK as `libproot.so`, exec'd from the
//! native-library directory so Android's no-exec rule for app data does not
//! apply; guest programs are loaded by proot's own loader, never `execve`d).
//!
//! Nothing here talks to a server of ours: tarballs come from the distros'
//! official mirrors and are checked against the SHA-256 pinned below.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::app::{LocalShell, RUNTIME};
use crate::error::{MobileError, Result};

/// One installable distribution.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LinuxDistro {
    pub id: String,
    pub name: String,
    pub version: String,
    /// Download size for this device's architecture, bytes.
    pub download_bytes: u64,
    /// One line for the picker ("apk, 4 MB" …).
    pub note: String,
}

struct Source {
    id: &'static str,
    name: &'static str,
    version: &'static str,
    note: &'static str,
    /// (url, sha256, bytes) for aarch64 then x86_64.
    aarch64: (&'static str, &'static str, u64),
    x86_64: (&'static str, &'static str, u64),
}

const SOURCES: &[Source] = &[
    Source {
        id: "alpine",
        name: "Alpine Linux",
        version: "3.21.8",
        note: "apk · small and fast",
        aarch64: (
            "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/aarch64/alpine-minirootfs-3.21.8-aarch64.tar.gz",
            "f25a96d2846a4bc439093107c1b48a8b0c93dcb411e2cb9cfded6f790b2bc001",
            3_850_365,
        ),
        x86_64: (
            "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/alpine-minirootfs-3.21.8-x86_64.tar.gz",
            "6ea461b0225faad280b7e13df878b30fdd6d969dde25ef05f85a1bcec9033348",
            3_900_000,
        ),
    },
    Source {
        id: "ubuntu",
        name: "Ubuntu",
        version: "24.04 LTS",
        note: "apt · the full Debian family",
        aarch64: (
            "https://cdimage.ubuntu.com/ubuntu-base/releases/24.04/release/ubuntu-base-24.04.4-base-arm64.tar.gz",
            "04207713ece899c3740823d33690441ad3a7f0ded1101aca744e2b0f37ac7ff2",
            29_870_567,
        ),
        x86_64: (
            "https://cdimage.ubuntu.com/ubuntu-base/releases/24.04/release/ubuntu-base-24.04.4-base-amd64.tar.gz",
            "c1e67ef7b17a6300e136118bd1dc04725009cb376c1aad10abcf8cd453628d58",
            29_989_394,
        ),
    },
];

fn arch_source(s: &Source) -> Option<(&'static str, &'static str, u64)> {
    if cfg!(target_arch = "aarch64") {
        Some(s.aarch64)
    } else if cfg!(target_arch = "x86_64") {
        Some(s.x86_64)
    } else {
        None
    }
}

fn source(id: &str) -> Result<&'static Source> {
    SOURCES
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| MobileError::not_found("distribution"))
}

/// Distributions this device can install (none on unsupported CPUs).
///
/// Alpine is left out on x86_64: musl's `fork()` there uses the legacy
/// `fork` syscall, which Android's app seccomp filter rejects with ENOSYS
/// (`/bin/sh: can't fork`); arm64 has only `clone`, and glibc uses `clone`
/// everywhere, so Ubuntu works on both.
#[uniffi::export]
pub fn linux_distros() -> Vec<LinuxDistro> {
    SOURCES
        .iter()
        .filter(|s| !(s.id == "alpine" && cfg!(target_arch = "x86_64")))
        .filter_map(|s| {
            let (_, _, bytes) = arch_source(s)?;
            Some(LinuxDistro {
                id: s.id.into(),
                name: s.name.into(),
                version: s.version.into(),
                download_bytes: bytes,
                note: s.note.into(),
            })
        })
        .collect()
}

/// What is installed under `root_dir`, if anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct LinuxInstalled {
    pub distro_id: String,
    pub name: String,
    pub version: String,
    pub installed_at: i64,
    /// SHA-256 of the tarball the system was unpacked from; lets a repeat
    /// install of the same image become a no-op.
    #[serde(default)]
    pub sha256: String,
    /// `official`, `file` or `url`.
    #[serde(default)]
    pub source: String,
}

const MARKER: &str = "installed.json";
const ROOTFS: &str = "rootfs";

#[uniffi::export]
pub fn linux_installed(root_dir: String) -> Option<LinuxInstalled> {
    let root = Path::new(&root_dir);
    let text = fs::read_to_string(root.join(MARKER)).ok()?;
    let info: LinuxInstalled = serde_json::from_str(&text).ok()?;
    root.join(ROOTFS).join("bin").is_dir().then_some(info)
}

/// Install progress, in order; `done`/`total` are bytes for the download and
/// entries for the unpack (`total` 0 = unknown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LinuxStage {
    Downloading,
    Verifying,
    Unpacking,
    Configuring,
}

/// Callbacks from the install thread; keep them quick.
#[uniffi::export(with_foreign)]
pub trait LinuxInstallListener: Send + Sync {
    fn on_progress(&self, stage: LinuxStage, done: u64, total: u64);
    fn on_finished(&self, error: Option<String>);
}

/// Download, verify, unpack and configure `distro_id` under `root_dir`,
/// replacing anything there. Runs in the background; the listener hears the
/// end. Not re-entrant: the caller keeps one install at a time.
#[uniffi::export]
pub fn linux_install(
    root_dir: String,
    distro_id: String,
    dns: Vec<String>,
    listener: Arc<dyn LinuxInstallListener>,
) {
    RUNTIME.spawn(async move {
        let result = install(Path::new(&root_dir), &distro_id, &dns, listener.as_ref()).await;
        listener.on_finished(result.err().map(|e| e.to_string()));
    });
}

/// Install a root filesystem the user picked themselves (tar, tar.gz or
/// tar.xz). The archive must contain `/bin/sh` built for this device's CPU;
/// anything else is removed again and reported. `name` is what the picker
/// shows afterwards. The file is deleted once unpacked.
#[uniffi::export]
pub fn linux_install_file(
    root_dir: String,
    path: String,
    name: String,
    dns: Vec<String>,
    listener: Arc<dyn LinuxInstallListener>,
) {
    RUNTIME.spawn(async move {
        let result = install_file(
            Path::new(&root_dir),
            Path::new(&path),
            &name,
            &dns,
            listener.as_ref(),
        )
        .await;
        listener.on_finished(result.err().map(|e| e.to_string()));
    });
}

/// Download a root filesystem from `url` and install it like
/// [`linux_install_file`]. With `sha256` set the download is verified
/// against it and a system already unpacked from the same digest is kept.
#[uniffi::export]
pub fn linux_install_url(
    root_dir: String,
    url: String,
    sha256: Option<String>,
    name: String,
    dns: Vec<String>,
    listener: Arc<dyn LinuxInstallListener>,
) {
    RUNTIME.spawn(async move {
        let result = install_url(
            Path::new(&root_dir),
            &url,
            sha256.as_deref(),
            &name,
            &dns,
            listener.as_ref(),
        )
        .await;
        listener.on_finished(result.err().map(|e| e.to_string()));
    });
}

/// Delete the installed system (rootfs and the user's files in it).
#[uniffi::export]
pub fn linux_remove(root_dir: String) -> Result<()> {
    let root = Path::new(&root_dir);
    let _ = fs::remove_file(root.join(MARKER));
    for sub in [ROOTFS, "tmp", "download.tar.gz"] {
        let p = root.join(sub);
        if p.is_dir() {
            remove_tree(&p)?;
        } else {
            let _ = fs::remove_file(&p);
        }
    }
    Ok(())
}

/// Shell command line for the installed system, ready for
/// [`crate::app::TermosoApp::connect_local`]. `lib_dir` is the app's native
/// library directory (where `libproot.so` lives); `shared_dir` (external
/// storage) appears inside as `/mnt/sdcard`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LinuxShell {
    pub shell: LocalShell,
    /// "Ubuntu 24.04 LTS" for the tab subtitle.
    pub title: String,
}

#[uniffi::export]
pub fn linux_shell(
    root_dir: String,
    lib_dir: String,
    dns: Vec<String>,
    shared_dir: Option<String>,
) -> Result<LinuxShell> {
    let root = Path::new(&root_dir);
    let info = linux_installed(root_dir.clone())
        .ok_or_else(|| MobileError::not_found("Linux is not installed"))?;
    let rootfs = root.join(ROOTFS);
    let tmp = root.join("tmp");
    fs::create_dir_all(&tmp)?;
    // DNS can change between sessions; the file is ours to overwrite.
    write_resolv(&rootfs, &dns)?;
    // The app's supplementary groups change with (re)installs; keep them named.
    name_host_groups(&rootfs, &host_groups())?;
    let lib = Path::new(&lib_dir);
    let proot = lib.join("libproot.so");
    if !proot.is_file() {
        return Err(MobileError::not_found("libproot.so"));
    }
    let shell = if rootfs.join("bin/bash").exists() {
        "/bin/bash"
    } else {
        "/bin/sh"
    };
    let mut argv: Vec<String> = vec![
        proot.to_string_lossy().into_owned(),
        "--kill-on-exit".into(),
        "--link2symlink".into(),
        "-0".into(),
        "-r".into(),
        rootfs.to_string_lossy().into_owned(),
        "-w".into(),
        "/root".into(),
    ];
    for bind in [
        "/dev",
        "/proc",
        "/sys",
        "/proc/self/fd:/dev/fd",
        "/proc/self/fd/0:/dev/stdin",
        "/proc/self/fd/1:/dev/stdout",
        "/proc/self/fd/2:/dev/stderr",
    ] {
        argv.push("-b".into());
        argv.push(bind.into());
    }
    argv.push("-b".into());
    argv.push(format!("{}:/dev/shm", tmp.to_string_lossy()));
    argv.push("-b".into());
    argv.push(format!("{}:/tmp", tmp.to_string_lossy()));
    if let Some(shared) = shared_dir.filter(|s| Path::new(s).is_dir()) {
        argv.push("-b".into());
        argv.push(format!("{shared}:/mnt/sdcard"));
    }
    // Drop the host-side variables proot needed; the guest sees a clean login.
    argv.extend(
        [
            "/usr/bin/env",
            "-u",
            "LD_LIBRARY_PATH",
            "-u",
            "PROOT_LOADER",
            "-u",
            "PROOT_TMP_DIR",
            "-u",
            "ANDROID_DATA",
            "-u",
            "ANDROID_ROOT",
            "HOME=/root",
            "USER=root",
            "LOGNAME=root",
            "LANG=C.UTF-8",
            "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            shell,
            "-l",
        ]
        .map(String::from),
    );
    let env = vec![
        format!("LD_LIBRARY_PATH={}", lib.to_string_lossy()),
        format!(
            "PROOT_LOADER={}",
            lib.join("libproot-loader.so").to_string_lossy()
        ),
        format!("PROOT_TMP_DIR={}", tmp.to_string_lossy()),
    ];
    Ok(LinuxShell {
        shell: LocalShell {
            argv,
            home: rootfs.join("root").to_string_lossy().into_owned(),
            env,
        },
        title: format!("{} {}", info.name, info.version),
    })
}

const CACHE: &str = "cache";

async fn install(
    root: &Path,
    distro_id: &str,
    dns: &[String],
    listener: &dyn LinuxInstallListener,
) -> Result<()> {
    let src = source(distro_id)?;
    let (url, sha, bytes) =
        arch_source(src).ok_or_else(|| MobileError::invalid("unsupported CPU architecture"))?;
    fs::create_dir_all(root)?;
    if already_installed(root, sha) {
        return Ok(());
    }
    let tarball = cached_or_download(root, url, Some(sha), bytes, listener).await?;
    let info = LinuxInstalled {
        distro_id: src.id.into(),
        name: src.name.into(),
        version: src.version.into(),
        installed_at: now(),
        sha256: sha.into(),
        source: "official".into(),
    };
    // Official tarballs stay in the cache so a reinstall skips the download.
    finish(root, &tarball, false, info, dns, listener).await
}

async fn install_file(
    root: &Path,
    path: &Path,
    name: &str,
    dns: &[String],
    listener: &dyn LinuxInstallListener,
) -> Result<()> {
    fs::create_dir_all(root)?;
    listener.on_progress(LinuxStage::Verifying, 0, 0);
    let digest = sha256_file(path)?;
    if already_installed(root, &digest) {
        let _ = fs::remove_file(path);
        return Ok(());
    }
    let info = LinuxInstalled {
        distro_id: format!("file:{}", &digest[..12]),
        name: name.trim().to_string(),
        version: String::new(),
        installed_at: now(),
        sha256: digest,
        source: "file".into(),
    };
    finish(root, path, true, info, dns, listener).await
}

async fn install_url(
    root: &Path,
    url: &str,
    sha256: Option<&str>,
    name: &str,
    dns: &[String],
    listener: &dyn LinuxInstallListener,
) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|_| MobileError::invalid("invalid URL"))?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err(MobileError::invalid("only http(s) URLs are supported"));
    }
    let expected = sha256
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    if expected
        .as_deref()
        .is_some_and(|sha| sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(MobileError::invalid("SHA-256 must be 64 hex characters"));
    }
    fs::create_dir_all(root)?;
    if expected
        .as_deref()
        .is_some_and(|sha| already_installed(root, sha))
    {
        return Ok(());
    }
    let tarball = cached_or_download(root, url, expected.as_deref(), 0, listener).await?;
    let digest = match &expected {
        Some(sha) => sha.clone(),
        None => sha256_file(&tarball)?,
    };
    let label = if name.trim().is_empty() {
        parsed
            .path_segments()
            .and_then(|mut p| p.next_back())
            .filter(|s| !s.is_empty())
            .unwrap_or("Custom Linux")
            .to_string()
    } else {
        name.trim().to_string()
    };
    let info = LinuxInstalled {
        distro_id: format!("url:{}", &digest[..12]),
        name: label,
        version: String::new(),
        installed_at: now(),
        sha256: digest,
        source: "url".into(),
    };
    finish(root, &tarball, true, info, dns, listener).await
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The same image is already unpacked and usable.
fn already_installed(root: &Path, sha: &str) -> bool {
    linux_installed(root.to_string_lossy().into_owned())
        .map(|i| i.sha256 == sha && !sha.is_empty())
        .unwrap_or(false)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Returns a verified tarball: from `root/cache/<sha>` when it is there and
/// still matches, otherwise freshly downloaded (into the cache when a digest
/// is known, else into `root/download.img`).
async fn cached_or_download(
    root: &Path,
    url: &str,
    sha: Option<&str>,
    bytes: u64,
    listener: &dyn LinuxInstallListener,
) -> Result<PathBuf> {
    let dest = match sha {
        Some(sha) => {
            let cache = root.join(CACHE);
            fs::create_dir_all(&cache)?;
            let p = cache.join(sha);
            if p.is_file() {
                listener.on_progress(LinuxStage::Verifying, 0, 0);
                if sha256_file(&p).ok().as_deref() == Some(sha) {
                    return Ok(p);
                }
                let _ = fs::remove_file(&p);
            }
            p
        }
        None => root.join("download.img"),
    };

    listener.on_progress(LinuxStage::Downloading, 0, bytes);
    let client = reqwest::Client::builder()
        .user_agent(concat!("termoso/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| MobileError::invalid(e.to_string()))?;
    let resp = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| MobileError::invalid(format!("download failed: {e}")))?;
    let total = resp.content_length().unwrap_or(bytes);
    let mut file = fs::File::create(&dest)?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| MobileError::invalid(format!("download failed: {e}")))?;
        file.write_all(&chunk)?;
        hasher.update(&chunk);
        done += chunk.len() as u64;
        listener.on_progress(LinuxStage::Downloading, done, total);
    }
    file.flush()?;
    drop(file);

    if let Some(sha) = sha {
        listener.on_progress(LinuxStage::Verifying, 0, 0);
        let digest = format!("{:x}", hasher.finalize());
        if digest != sha {
            let _ = fs::remove_file(&dest);
            return Err(MobileError::invalid(
                "checksum mismatch — the download was corrupted or tampered with",
            ));
        }
    }
    Ok(dest)
}

/// Unpack, check the image runs on this CPU, configure and write the marker.
/// `consume` deletes the tarball afterwards.
async fn finish(
    root: &Path,
    tarball: &Path,
    consume: bool,
    info: LinuxInstalled,
    dns: &[String],
    listener: &dyn LinuxInstallListener,
) -> Result<()> {
    let _ = fs::remove_file(root.join(MARKER));
    let rootfs = root.join(ROOTFS);
    if rootfs.exists() {
        remove_tree(&rootfs)?;
    }
    listener.on_progress(LinuxStage::Unpacking, 0, 0);
    let rootfs2 = rootfs.clone();
    let tarball2 = tarball.to_path_buf();
    let unpack = tokio::task::spawn_blocking(move || unpack(&tarball2, &rootfs2))
        .await
        .map_err(|e| MobileError::invalid(e.to_string()))?;
    if consume {
        let _ = fs::remove_file(tarball);
    }
    let entries = match unpack.and_then(|n| validate(&rootfs).map(|_| n)) {
        Ok(n) => n,
        Err(e) => {
            let _ = remove_tree(&rootfs);
            return Err(e);
        }
    };
    listener.on_progress(LinuxStage::Unpacking, entries, entries);

    listener.on_progress(LinuxStage::Configuring, 0, 0);
    configure(&rootfs, dns)?;
    fs::write(root.join(MARKER), serde_json::to_vec(&info)?)?;
    Ok(())
}

/// ELF `e_machine` of this build's CPU.
const fn this_machine() -> Option<u16> {
    if cfg!(target_arch = "aarch64") {
        Some(0xB7)
    } else if cfg!(target_arch = "x86_64") {
        Some(0x3E)
    } else {
        None
    }
}

/// A usable image has a shell binary built for this CPU. Symlinks are
/// followed inside the rootfs only.
fn validate(rootfs: &Path) -> Result<()> {
    let candidates = [
        "bin/sh",
        "usr/bin/sh",
        "bin/busybox",
        "bin/bash",
        "usr/bin/bash",
    ];
    let mut found = None;
    for c in candidates {
        let mut p = rootfs.join(c);
        for _ in 0..8 {
            match fs::symlink_metadata(&p) {
                Ok(m) if m.file_type().is_symlink() => {
                    let target = fs::read_link(&p)?;
                    p = if target.is_absolute() {
                        rootfs.join(strip(&target))
                    } else {
                        p.parent().unwrap_or(rootfs).join(target)
                    };
                }
                Ok(m) if m.is_file() => {
                    found = Some(p.clone());
                    break;
                }
                _ => break,
            }
        }
        if found.is_some() {
            break;
        }
    }
    let sh = found.ok_or_else(|| {
        MobileError::invalid("not a Linux root filesystem: no /bin/sh inside the archive")
    })?;
    let mut head = [0u8; 20];
    let n = fs::File::open(&sh)?.read(&mut head)?;
    if n < 20 || &head[..4] != b"\x7fELF" {
        return Err(MobileError::invalid(
            "/bin/sh in the archive is not an ELF binary",
        ));
    }
    let machine = u16::from_le_bytes([head[18], head[19]]);
    match this_machine() {
        Some(m) if m == machine => Ok(()),
        Some(_) => Err(MobileError::invalid(
            "this image was built for another CPU architecture",
        )),
        None => Ok(()),
    }
}

/// Picks the decoder by magic bytes: gzip, xz or a plain tar.
fn open_tar(path: &Path) -> Result<Box<dyn Read + Send>> {
    let mut file = fs::File::open(path)?;
    let mut magic = [0u8; 6];
    let n = file.read(&mut magic)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(if n >= 2 && magic[..2] == [0x1f, 0x8b] {
        Box::new(flate2::read::GzDecoder::new(file))
    } else if n >= 6 && magic == [0xfd, b'7', b'z', b'X', b'Z', 0x00] {
        Box::new(xz2::read::XzDecoder::new(file))
    } else {
        Box::new(file)
    })
}

fn unpack(tarball: &Path, rootfs: &Path) -> Result<u64> {
    fs::create_dir_all(rootfs)?;
    let mut archive = tar::Archive::new(open_tar(tarball)?);
    archive.set_preserve_permissions(true);
    archive.set_preserve_ownerships(false);
    archive.set_unpack_xattrs(false);
    let mut count = 0u64;
    let mut deferred: Vec<(PathBuf, PathBuf)> = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let kind = entry.header().entry_type();
        let path = entry.path()?.into_owned();
        match kind {
            tar::EntryType::Char | tar::EntryType::Block | tar::EntryType::Fifo => continue,
            tar::EntryType::Link => {
                let target = entry
                    .link_name()?
                    .ok_or_else(|| MobileError::invalid("bad hard link"))?
                    .into_owned();
                deferred.push((path, target));
            }
            _ => {
                entry.unpack_in(rootfs)?;
            }
        }
        count += 1;
    }
    for (path, target) in deferred {
        let dst = rootfs.join(strip(&path));
        let src = rootfs.join(strip(&target));
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        if fs::hard_link(&src, &dst).is_err() {
            let _ = fs::remove_file(&dst);
            fs::copy(&src, &dst)?;
        }
    }
    Ok(count)
}

fn strip(p: &Path) -> PathBuf {
    p.components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .collect()
}

fn configure(rootfs: &Path, dns: &[String]) -> Result<()> {
    write_resolv(rootfs, dns)?;
    let etc = rootfs.join("etc");
    fs::create_dir_all(&etc)?;
    let hosts = etc.join("hosts");
    if !hosts.is_file()
        || fs::read_to_string(&hosts)
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
    {
        fs::write(
            &hosts,
            "127.0.0.1\tlocalhost\n::1\tlocalhost ip6-localhost ip6-loopback\n",
        )?;
    }
    fs::create_dir_all(rootfs.join("root"))?;
    fs::create_dir_all(rootfs.join("tmp"))?;
    let _ = fs::create_dir_all(rootfs.join("mnt/sdcard"));
    name_host_groups(rootfs, &host_groups())?;
    // Ubuntu base ships no /etc/hostname; proot fakes uname, a name keeps tools quiet.
    let hostname = etc.join("hostname");
    if !hostname.exists() {
        fs::write(&hostname, "termoso\n")?;
    }
    Ok(())
}

/// The process keeps its Android supplementary groups under proot (`-0` only
/// fakes uid/gid 0), and a distro's `/etc/group` knows none of them, so every
/// login shell prints `groups: cannot find name for group ID 3003` and
/// friends. Give them the names Android uses.
fn host_groups() -> Vec<u32> {
    let mut gids: Vec<u32> = fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("Groups:"))
        .map(|l| {
            l.split_whitespace()
                .filter_map(|g| g.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    gids.sort_unstable();
    gids.dedup();
    gids
}

fn android_group_name(gid: u32) -> String {
    match gid {
        1015 => "sdcard_rw".into(),
        1023 => "media_rw".into(),
        1077 => "external_storage".into(),
        1078 => "ext_data_rw".into(),
        1079 => "ext_obb_rw".into(),
        3001 => "net_bt_admin".into(),
        3002 => "net_bt".into(),
        3003 => "inet".into(),
        3004 => "net_raw".into(),
        3005 => "net_admin".into(),
        9997 => "everybody".into(),
        10000..=19999 => format!("u0_a{}", gid - 10000),
        20000..=29999 => format!("cache_a{}", gid - 20000),
        30000..=39999 => format!("ext_a{}", gid - 30000),
        40000..=49999 => format!("ext_cache_a{}", gid - 40000),
        50000..=59999 => format!("all_a{}", gid - 50000),
        _ => format!("aid_{gid}"),
    }
}

/// Append the host's groups that `/etc/group` lacks (by id); existing lines
/// are left alone, including a distro's own use of a colliding id.
fn name_host_groups(rootfs: &Path, gids: &[u32]) -> Result<()> {
    if gids.is_empty() {
        return Ok(());
    }
    let path = rootfs.join("etc/group");
    let mut text = fs::read_to_string(&path).unwrap_or_default();
    let known: Vec<u32> = text
        .lines()
        .filter_map(|l| l.split(':').nth(2))
        .filter_map(|g| g.parse().ok())
        .collect();
    let names: Vec<&str> = text.lines().filter_map(|l| l.split(':').next()).collect();
    let mut added = String::new();
    for &gid in gids {
        if known.contains(&gid) {
            continue;
        }
        let mut name = android_group_name(gid);
        if names.contains(&name.as_str()) {
            name = format!("aid_{gid}");
        }
        added.push_str(&format!("{name}:x:{gid}:\n"));
    }
    if added.is_empty() {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&added);
    fs::create_dir_all(path.parent().unwrap_or(rootfs))?;
    fs::write(&path, text)?;
    Ok(())
}

fn write_resolv(rootfs: &Path, dns: &[String]) -> Result<()> {
    let etc = rootfs.join("etc");
    fs::create_dir_all(&etc)?;
    let resolv = etc.join("resolv.conf");
    // Ubuntu points this at systemd-resolved's stub, which does not exist here.
    if fs::symlink_metadata(&resolv)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        fs::remove_file(&resolv)?;
    }
    let servers: Vec<&str> = if dns.is_empty() {
        vec!["1.1.1.1", "8.8.8.8"]
    } else {
        dns.iter().map(String::as_str).collect()
    };
    let mut text = String::new();
    for s in servers {
        text.push_str("nameserver ");
        text.push_str(s);
        text.push('\n');
    }
    fs::write(&resolv, text)?;
    Ok(())
}

/// `remove_dir_all` that first makes everything writable (rootfs dirs come
/// with 0555 modes).
fn remove_tree(path: &Path) -> Result<()> {
    fn relax(p: &Path) {
        let Ok(meta) = fs::symlink_metadata(p) else {
            return;
        };
        if !meta.is_dir() {
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o755));
        }
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                relax(&e.path());
            }
        }
    }
    relax(path);
    fs::remove_dir_all(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distros_listed_for_this_arch() {
        let d = linux_distros();
        assert!(d.iter().any(|x| x.id == "ubuntu"));
        assert_eq!(
            d.iter().any(|x| x.id == "alpine"),
            cfg!(target_arch = "aarch64")
        );
    }

    #[test]
    fn shell_requires_install() {
        let dir = tempfile::tempdir().unwrap();
        assert!(linux_installed(dir.path().to_string_lossy().into_owned()).is_none());
        assert!(
            linux_shell(
                dir.path().to_string_lossy().into_owned(),
                "/nonexistent".into(),
                vec![],
                None
            )
            .is_err()
        );
    }

    #[test]
    fn unpack_handles_symlinks_hardlinks_and_devices() {
        let dir = tempfile::tempdir().unwrap();
        let tarball = dir.path().join("t.tar.gz");
        {
            let f = fs::File::create(&tarball).unwrap();
            let gz = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
            let mut b = tar::Builder::new(gz);
            let mut h = tar::Header::new_gnu();
            h.set_size(5);
            h.set_mode(0o755);
            h.set_entry_type(tar::EntryType::Regular);
            h.set_cksum();
            b.append_data(&mut h, "bin/sh", &b"hello"[..]).unwrap();
            let mut h = tar::Header::new_gnu();
            h.set_size(0);
            h.set_entry_type(tar::EntryType::Link);
            h.set_cksum();
            b.append_link(&mut h, "bin/ash", "bin/sh").unwrap();
            let mut h = tar::Header::new_gnu();
            h.set_size(0);
            h.set_entry_type(tar::EntryType::Symlink);
            h.set_cksum();
            b.append_link(&mut h, "bin/bash", "sh").unwrap();
            let mut h = tar::Header::new_gnu();
            h.set_size(0);
            h.set_entry_type(tar::EntryType::Char);
            h.set_device_major(1).unwrap();
            h.set_device_minor(3).unwrap();
            h.set_cksum();
            b.append_data(&mut h, "dev/null", &b""[..]).unwrap();
            b.into_inner().unwrap().finish().unwrap();
        }
        let rootfs = dir.path().join("rootfs");
        let n = unpack(&tarball, &rootfs).unwrap();
        assert_eq!(n, 3);
        assert_eq!(fs::read(rootfs.join("bin/sh")).unwrap(), b"hello");
        assert_eq!(fs::read(rootfs.join("bin/ash")).unwrap(), b"hello");
        assert!(
            fs::symlink_metadata(rootfs.join("bin/bash"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(!rootfs.join("dev/null").exists());
        configure(&rootfs, &["9.9.9.9".into()]).unwrap();
        assert_eq!(
            fs::read_to_string(rootfs.join("etc/resolv.conf")).unwrap(),
            "nameserver 9.9.9.9\n"
        );
        linux_remove(dir.path().to_string_lossy().into_owned()).unwrap();
        assert!(!rootfs.exists());
    }

    #[test]
    fn host_groups_get_android_names_without_touching_existing_lines() {
        let dir = tempfile::tempdir().unwrap();
        let rootfs = dir.path();
        fs::create_dir_all(rootfs.join("etc")).unwrap();
        fs::write(
            rootfs.join("etc/group"),
            "root:x:0:\nusers:x:100:\nmine:x:9997:someone",
        )
        .unwrap();
        name_host_groups(rootfs, &[3003, 9997, 20091, 50091, 100, 7]).unwrap();
        let text = fs::read_to_string(rootfs.join("etc/group")).unwrap();
        assert_eq!(
            text,
            "root:x:0:\nusers:x:100:\nmine:x:9997:someone\ninet:x:3003:\ncache_a91:x:20091:\nall_a91:x:50091:\naid_7:x:7:\n"
        );
        // Idempotent.
        name_host_groups(rootfs, &[3003, 20091]).unwrap();
        assert_eq!(fs::read_to_string(rootfs.join("etc/group")).unwrap(), text);
        // A missing file is created.
        let bare = tempfile::tempdir().unwrap();
        name_host_groups(bare.path(), &[3003]).unwrap();
        assert_eq!(
            fs::read_to_string(bare.path().join("etc/group")).unwrap(),
            "inet:x:3003:\n"
        );
    }
}
