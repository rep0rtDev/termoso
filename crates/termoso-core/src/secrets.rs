//! The device master key.
//!
//! One 256-bit key per profile protects everything the local database cannot
//! store in plaintext (vault keys, session token, account private key). It is
//! kept in the OS keychain (Secret Service on Linux, Credential Manager on
//! Windows, Keychain on macOS). When no keychain is reachable – headless
//! Linux, containers, some Wayland sandboxes – it falls back to a file next to
//! the database with owner-only permissions and reports that so the UI can
//! warn the user.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use termoso_crypto::keys::SymmetricKey;

use crate::error::{CoreError, Result};

const SERVICE: &str = "termoso";
const FILE_NAME: &str = "master.key";

/// Where the master key came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MasterKeySource {
    /// OS keychain.
    Keychain,
    /// Owner-only file in the profile directory.
    File,
}

/// Loaded master key and its provenance.
pub struct MasterKey {
    /// The key.
    pub key: SymmetricKey,
    /// Where it lives.
    pub source: MasterKeySource,
}

/// Load the master key for `profile` (creating it on first run).
///
/// Order: keychain → file → generate and store in keychain (or file when the
/// keychain is unavailable). Set `allow_file = false` to refuse the fallback.
pub fn load_or_create(profile_dir: &Path, profile: &str, allow_file: bool) -> Result<MasterKey> {
    let file = profile_dir.join(FILE_NAME);

    match keychain_get(profile) {
        Ok(Some(key)) => {
            return Ok(MasterKey {
                key,
                source: MasterKeySource::Keychain,
            });
        }
        Ok(None) => {}
        Err(e) => tracing::warn!(error = %e, "keychain unavailable"),
    }

    if file.exists() {
        return Ok(MasterKey {
            key: read_file(&file)?,
            source: MasterKeySource::File,
        });
    }

    let key = SymmetricKey::generate();
    match keychain_set(profile, &key) {
        Ok(()) => Ok(MasterKey {
            key,
            source: MasterKeySource::Keychain,
        }),
        Err(e) if allow_file => {
            tracing::warn!(error = %e, "storing master key in file");
            write_file(&file, &key)?;
            Ok(MasterKey {
                key,
                source: MasterKeySource::File,
            })
        }
        Err(e) => Err(e),
    }
}

/// Move a file-stored key into the keychain (when it becomes available).
pub fn migrate_file_to_keychain(profile_dir: &Path, profile: &str) -> Result<bool> {
    let file = profile_dir.join(FILE_NAME);
    if !file.exists() {
        return Ok(false);
    }
    let key = read_file(&file)?;
    keychain_set(profile, &key)?;
    std::fs::remove_file(&file)?;
    Ok(true)
}

/// Forget the master key everywhere (profile deletion).
pub fn delete(profile_dir: &Path, profile: &str) -> Result<()> {
    let file = profile_dir.join(FILE_NAME);
    if file.exists() {
        std::fs::remove_file(&file)?;
    }
    keychain_delete(profile)
}

/// Default profile directory (`~/.local/share/termoso/<profile>` etc.).
pub fn default_profile_dir(profile: &str) -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("termoso")
        .join(profile)
}

fn read_file(path: &Path) -> Result<SymmetricKey> {
    let s = std::fs::read_to_string(path)?;
    SymmetricKey::from_b64(s.trim()).map_err(|_| CoreError::Keychain("corrupt master.key".into()))
}

fn write_file(path: &Path, key: &SymmetricKey) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    use std::io::Write;
    f.write_all(key.to_b64().as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

#[cfg(feature = "keychain")]
fn entry(profile: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("master-key/{profile}"))
        .map_err(|e| CoreError::Keychain(e.to_string()))
}

#[cfg(feature = "keychain")]
fn keychain_get(profile: &str) -> Result<Option<SymmetricKey>> {
    match entry(profile)?.get_password() {
        Ok(s) => Ok(Some(SymmetricKey::from_b64(s.trim()).map_err(|_| {
            CoreError::Keychain("corrupt keychain entry".into())
        })?)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(CoreError::Keychain(e.to_string())),
    }
}

#[cfg(feature = "keychain")]
fn keychain_set(profile: &str, key: &SymmetricKey) -> Result<()> {
    entry(profile)?
        .set_password(&key.to_b64())
        .map_err(|e| CoreError::Keychain(e.to_string()))
}

#[cfg(feature = "keychain")]
fn keychain_delete(profile: &str) -> Result<()> {
    match entry(profile)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(CoreError::Keychain(e.to_string())),
    }
}

#[cfg(not(feature = "keychain"))]
fn keychain_get(_profile: &str) -> Result<Option<SymmetricKey>> {
    Err(CoreError::Keychain(
        "keychain support not compiled in".into(),
    ))
}

#[cfg(not(feature = "keychain"))]
fn keychain_set(_profile: &str, _key: &SymmetricKey) -> Result<()> {
    Err(CoreError::Keychain(
        "keychain support not compiled in".into(),
    ))
}

#[cfg(not(feature = "keychain"))]
fn keychain_delete(_profile: &str) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_fallback_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        let key = SymmetricKey::generate();
        write_file(&path, &key).unwrap();
        assert_eq!(read_file(&path).unwrap().as_bytes(), key.as_bytes());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        assert!(
            write_file(&path, &key).is_err(),
            "never overwrite an existing key"
        );
    }
}
