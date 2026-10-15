//! A real Linux userland for the local shell on Android: an official distro
//! rootfs (Alpine / Ubuntu) unpacked into the app's data directory and run
//! under `proot` (bundled in the APK as `libproot.so`, exec'd from the
//! native-library directory so Android's no-exec rule for app data does not
//! apply; guest programs are loaded by proot's own loader, never `execve`d).
//!
//! Nothing here talks to a server of ours: tarballs come from the distros'
//! official mirrors and are checked against the SHA-256 pinned below.

use std::fs;
use std::io::Write;
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
    let _ = fs::remove_file(root.join(MARKER));
    let rootfs = root.join(ROOTFS);
    if rootfs.exists() {
        remove_tree(&rootfs)?;
    }
    let tarball = root.join("download.tar.gz");

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
    let mut file = fs::File::create(&tarball)?;
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

    listener.on_progress(LinuxStage::Verifying, 0, 0);
    let digest = format!("{:x}", hasher.finalize());
    if digest != sha {
        let _ = fs::remove_file(&tarball);
        return Err(MobileError::invalid(
            "checksum mismatch — the download was corrupted or tampered with",
        ));
    }

    listener.on_progress(LinuxStage::Unpacking, 0, 0);
    let rootfs2 = rootfs.clone();
    let tarball2 = tarball.clone();
    let unpack = tokio::task::spawn_blocking(move || unpack(&tarball2, &rootfs2))
        .await
        .map_err(|e| MobileError::invalid(e.to_string()))?;
    let _ = fs::remove_file(&tarball);
    let entries = unpack?;
    listener.on_progress(LinuxStage::Unpacking, entries, entries);

    listener.on_progress(LinuxStage::Configuring, 0, 0);
    configure(&rootfs, dns)?;
    let info = LinuxInstalled {
        distro_id: src.id.into(),
        name: src.name.into(),
        version: src.version.into(),
        installed_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    };
    fs::write(root.join(MARKER), serde_json::to_vec(&info)?)?;
    Ok(())
}

/// Unpack a gzip tarball as an unprivileged user: symlinks kept, hard links
/// turned into copies (apt and apk cope), device nodes and ownership dropped.
fn unpack(tarball: &Path, rootfs: &Path) -> Result<u64> {
    fs::create_dir_all(rootfs)?;
    let file = fs::File::open(tarball)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
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
    // Ubuntu base ships no /etc/hostname; proot fakes uname, a name keeps tools quiet.
    let hostname = etc.join("hostname");
    if !hostname.exists() {
        fs::write(&hostname, "termoso\n")?;
    }
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
}
