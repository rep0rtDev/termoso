//! The device master key.
//!
//! One 256-bit key per profile protects everything the local database cannot
//! store in plaintext (vault keys, session token, account private key). It is
//! kept in the OS keychain (Secret Service on Linux, Credential Manager on
//! Windows, Keychain on macOS). When no keychain is reachable – headless
//! Linux, containers, some Wayland sandboxes – it falls back to a file next to
//! the database with owner-only permissions and reports that so the UI can
//! warn the user.
//!
//! Optionally the user puts the key behind a master password instead: the
//! random key is then wrapped with an Argon2id-derived key and stored in
//! `master.pw`, and no plaintext copy is kept anywhere. The database format
//! never changes — only who holds the key.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use termoso_crypto::aead::{self, Aad};
use termoso_crypto::encoding::{b64, unb64};
use termoso_crypto::kdf::{self, PASSWORD_SALT_LEN, PasswordParams};
use termoso_crypto::keys::SymmetricKey;
use zeroize::Zeroizing;

use crate::error::{CoreError, Result};

#[cfg(feature = "keychain")]
const SERVICE: &str = "termoso";
const FILE_NAME: &str = "master.key";
const PASSWORD_FILE_NAME: &str = "master.pw";
const PASSWORD_FORMAT_VERSION: u32 = 1;
/// Shortest master password accepted.
pub const MIN_PASSWORD_CHARS: usize = 8;

/// Where the master key came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MasterKeySource {
    /// OS keychain.
    Keychain,
    /// Owner-only file in the profile directory.
    File,
    /// Wrapped with a key derived from the user's master password.
    Password,
}

/// On-disk shape of `master.pw`.
#[derive(Serialize, Deserialize)]
struct PasswordRecord {
    version: u32,
    kdf: String,
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    salt: String,
    /// AEAD envelope over the raw 32-byte master key.
    wrapped: String,
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
    if password_protected(profile_dir) {
        return Err(CoreError::Invalid(
            "the master key is protected by a password".into(),
        ));
    }
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
    for name in [FILE_NAME, PASSWORD_FILE_NAME] {
        let file = profile_dir.join(name);
        if file.exists() {
            std::fs::remove_file(&file)?;
        }
    }
    keychain_delete(profile)
}

/// Whether this profile's master key is behind a master password.
pub fn password_protected(profile_dir: &Path) -> bool {
    profile_dir.join(PASSWORD_FILE_NAME).exists()
}

/// Unwrap the master key of a password-protected profile.
///
/// A wrong password is reported as [`CoreError::WrongPassword`]; the record on
/// disk is never modified. Plaintext copies left behind by an interrupted
/// [`set_password`] are removed once the password has been verified.
pub fn unlock_with_password(
    profile_dir: &Path,
    profile: &str,
    password: &str,
) -> Result<MasterKey> {
    let path = profile_dir.join(PASSWORD_FILE_NAME);
    let record: PasswordRecord = serde_json::from_slice(&std::fs::read(&path)?)?;
    let key = unwrap_record(&record, password)?;
    remove_plain_copies(profile_dir, profile);
    Ok(MasterKey {
        key,
        source: MasterKeySource::Password,
    })
}

/// Check `password` against the stored record without touching anything.
pub fn verify_password(profile_dir: &Path, password: &str) -> Result<()> {
    let path = profile_dir.join(PASSWORD_FILE_NAME);
    let record: PasswordRecord = serde_json::from_slice(&std::fs::read(&path)?)?;
    unwrap_record(&record, password).map(drop)
}

/// Put `key` behind `password`: writes `master.pw` atomically, then removes
/// the keychain entry and `master.key`. Also used to change the password —
/// the new record replaces the old one in a single rename, so a crash leaves
/// either the old or the new password working, never neither.
pub fn set_password(
    profile_dir: &Path,
    profile: &str,
    key: &SymmetricKey,
    password: &str,
) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(CoreError::Invalid(format!(
            "master password must be at least {MIN_PASSWORD_CHARS} characters"
        )));
    }
    let record = wrap_record(key, password, PasswordParams::default())?;
    write_atomic(
        &profile_dir.join(PASSWORD_FILE_NAME),
        &serde_json::to_vec_pretty(&record)?,
        |on_disk| {
            let record: PasswordRecord = serde_json::from_slice(on_disk)?;
            if unwrap_record(&record, password)?.as_bytes() == key.as_bytes() {
                Ok(())
            } else {
                Err(CoreError::Invalid("master.pw read-back mismatch".into()))
            }
        },
    )?;
    remove_plain_copies(profile_dir, profile);
    Ok(())
}

/// Drop the master password: store `key` in the keychain (or `master.key`
/// when `allow_file` and no keychain is reachable), then delete `master.pw`.
/// A crash in between leaves both — the profile stays password-protected and
/// the next unlock removes the plaintext copy again.
pub fn remove_password(
    profile_dir: &Path,
    profile: &str,
    key: &SymmetricKey,
    allow_file: bool,
) -> Result<MasterKeySource> {
    let source = match keychain_set(profile, key) {
        Ok(()) => MasterKeySource::Keychain,
        Err(e) if allow_file => {
            tracing::warn!(error = %e, "storing master key in file");
            let file = profile_dir.join(FILE_NAME);
            if file.exists() {
                std::fs::remove_file(&file)?;
            }
            write_file(&file, key)?;
            MasterKeySource::File
        }
        Err(e) => return Err(e),
    };
    std::fs::remove_file(profile_dir.join(PASSWORD_FILE_NAME))?;
    Ok(source)
}

fn record_aad(record: &PasswordRecord) -> Aad {
    Aad::label(&[
        "master-key",
        "password",
        &record.version.to_string(),
        &record.salt,
    ])
}

fn wrap_record(
    key: &SymmetricKey,
    password: &str,
    params: PasswordParams,
) -> Result<PasswordRecord> {
    let mut salt = [0u8; PASSWORD_SALT_LEN];
    termoso_crypto::random_bytes(&mut salt);
    let mut record = PasswordRecord {
        version: PASSWORD_FORMAT_VERSION,
        kdf: "argon2id".into(),
        m_cost: params.m_cost,
        t_cost: params.t_cost,
        p_cost: params.p_cost,
        salt: b64(&salt),
        wrapped: String::new(),
    };
    let wrapping = kdf::password_key(password.as_bytes(), &salt, params)?;
    record.wrapped = aead::encrypt_b64(&wrapping, &record_aad(&record), key.as_bytes())?;
    Ok(record)
}

fn unwrap_record(record: &PasswordRecord, password: &str) -> Result<SymmetricKey> {
    if record.version != PASSWORD_FORMAT_VERSION || record.kdf != "argon2id" {
        return Err(CoreError::Invalid(format!(
            "unsupported master.pw format (version {}, {})",
            record.version, record.kdf
        )));
    }
    // A tampered record must not be able to pin the machine.
    if record.m_cost > 4 * 1024 * 1024 || record.t_cost > 64 || record.p_cost > 16 {
        return Err(CoreError::Invalid(
            "master.pw asks for unreasonable KDF cost".into(),
        ));
    }
    let salt =
        unb64(&record.salt).map_err(|_| CoreError::Invalid("master.pw is corrupted".into()))?;
    let params = PasswordParams {
        m_cost: record.m_cost,
        t_cost: record.t_cost,
        p_cost: record.p_cost,
    };
    let wrapping = kdf::password_key(password.as_bytes(), &salt, params)?;
    let raw = Zeroizing::new(
        aead::decrypt_b64(&wrapping, &record_aad(record), &record.wrapped)
            .map_err(|_| CoreError::WrongPassword)?,
    );
    let bytes: [u8; termoso_crypto::keys::KEY_LEN] = raw
        .as_slice()
        .try_into()
        .map_err(|_| CoreError::Invalid("master.pw is corrupted".into()))?;
    Ok(SymmetricKey::from_bytes(bytes))
}

/// Best effort: once a password protects the key, no plaintext copy may stay
/// behind. Failures are logged, not fatal — the next unlock retries.
fn remove_plain_copies(profile_dir: &Path, profile: &str) {
    let file = profile_dir.join(FILE_NAME);
    if file.exists()
        && let Err(e) = std::fs::remove_file(&file)
    {
        tracing::warn!(error = %e, "cannot remove master.key");
    }
    if let Err(e) = keychain_delete(profile) {
        tracing::warn!(error = %e, "cannot remove master key from keychain");
    }
}

/// Owner-only temp file in the same directory, flushed to disk, read back
/// through `verify`, then renamed over `path` (rename replaces atomically on
/// every supported platform). Whatever was at `path` survives any failure.
fn write_atomic(
    path: &Path,
    contents: &[u8],
    verify: impl FnOnce(&[u8]) -> Result<()>,
) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        "{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("master.pw")
    ));
    if tmp.exists() {
        std::fs::remove_file(&tmp)?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(contents)?;
    f.sync_all()?;
    drop(f);
    let on_disk = std::fs::read(&tmp);
    if let Err(e) = on_disk.map_err(Into::into).and_then(|bytes| verify(&bytes)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, path)?;
    if let Ok(dir) = std::fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// `<data dir>/termoso/<profile>` (e.g. `~/.local/share/termoso/default`).
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
    f.write_all(key.to_b64().as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

#[cfg(feature = "keychain")]
fn entry(profile: &str) -> std::result::Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(SERVICE, &format!("master-key/{profile}"))
}

#[cfg(feature = "keychain")]
fn keychain_err(e: keyring::Error) -> CoreError {
    CoreError::Keychain(e.to_string())
}

#[cfg(feature = "keychain")]
fn keychain_get(profile: &str) -> Result<Option<SymmetricKey>> {
    match entry(profile).map_err(keychain_err)?.get_password() {
        Ok(s) => Ok(Some(SymmetricKey::from_b64(s.trim()).map_err(|_| {
            CoreError::Keychain("corrupt keychain entry".into())
        })?)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(keychain_err(e)),
    }
}

#[cfg(feature = "keychain")]
fn keychain_set(profile: &str, key: &SymmetricKey) -> Result<()> {
    entry(profile)
        .map_err(keychain_err)?
        .set_password(&key.to_b64())
        .map_err(keychain_err)
}

#[cfg(feature = "keychain")]
fn keychain_delete(profile: &str) -> Result<()> {
    let entry = match entry(profile) {
        Ok(e) => e,
        // No credential store on this machine: there is nothing to delete.
        Err(keyring::Error::NoDefaultStore) => return Ok(()),
        Err(e) => return Err(keychain_err(e)),
    };
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(keychain_err(e)),
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

    fn test_profile() -> String {
        format!("test-{}", uuid::Uuid::new_v4())
    }

    #[test]
    fn password_wraps_and_unwraps_the_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let profile = test_profile();
        let key = SymmetricKey::generate();
        write_file(&dir.path().join(FILE_NAME), &key).unwrap();

        set_password(dir.path(), &profile, &key, "correct horse battery").unwrap();
        assert!(password_protected(dir.path()));
        assert!(
            !dir.path().join(FILE_NAME).exists(),
            "plaintext copy removed once the password protects the key"
        );
        assert!(!dir.path().join("master.pw.tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(PASSWORD_FILE_NAME))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        assert!(matches!(
            load_or_create(dir.path(), &profile, true),
            Err(CoreError::Invalid(_))
        ));

        let unlocked = unlock_with_password(dir.path(), &profile, "correct horse battery").unwrap();
        assert_eq!(unlocked.source, MasterKeySource::Password);
        assert_eq!(unlocked.key.as_bytes(), key.as_bytes());
        delete(dir.path(), &profile).unwrap();
    }

    #[test]
    fn wrong_password_is_rejected_and_record_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let profile = test_profile();
        let key = SymmetricKey::generate();
        set_password(dir.path(), &profile, &key, "correct horse battery").unwrap();
        let before = std::fs::read(dir.path().join(PASSWORD_FILE_NAME)).unwrap();

        assert!(matches!(
            unlock_with_password(dir.path(), &profile, "Correct horse battery"),
            Err(CoreError::WrongPassword)
        ));
        assert!(matches!(
            verify_password(dir.path(), ""),
            Err(CoreError::WrongPassword)
        ));
        assert_eq!(
            std::fs::read(dir.path().join(PASSWORD_FILE_NAME)).unwrap(),
            before
        );
        assert!(verify_password(dir.path(), "correct horse battery").is_ok());
        delete(dir.path(), &profile).unwrap();
    }

    #[test]
    fn short_password_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let key = SymmetricKey::generate();
        assert!(matches!(
            set_password(dir.path(), &test_profile(), &key, "short"),
            Err(CoreError::Invalid(_))
        ));
        assert!(!password_protected(dir.path()));
    }

    #[test]
    fn change_password_replaces_record_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let profile = test_profile();
        let key = SymmetricKey::generate();
        set_password(dir.path(), &profile, &key, "first password").unwrap();
        set_password(dir.path(), &profile, &key, "second password").unwrap();
        assert!(matches!(
            unlock_with_password(dir.path(), &profile, "first password"),
            Err(CoreError::WrongPassword)
        ));
        let unlocked = unlock_with_password(dir.path(), &profile, "second password").unwrap();
        assert_eq!(unlocked.key.as_bytes(), key.as_bytes());
        delete(dir.path(), &profile).unwrap();
    }

    #[test]
    fn stale_temp_file_does_not_block_writing() {
        let dir = tempfile::tempdir().unwrap();
        let profile = test_profile();
        std::fs::write(dir.path().join("master.pw.tmp"), b"garbage").unwrap();
        let key = SymmetricKey::generate();
        set_password(dir.path(), &profile, &key, "correct horse battery").unwrap();
        assert!(!dir.path().join("master.pw.tmp").exists());
        assert!(unlock_with_password(dir.path(), &profile, "correct horse battery").is_ok());
        delete(dir.path(), &profile).unwrap();
    }

    #[test]
    fn corrupt_record_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let profile = test_profile();
        std::fs::write(dir.path().join(PASSWORD_FILE_NAME), b"{not json").unwrap();
        assert!(matches!(
            unlock_with_password(dir.path(), &profile, "x"),
            Err(CoreError::Json(_))
        ));
        let record = PasswordRecord {
            version: PASSWORD_FORMAT_VERSION,
            kdf: "argon2id".into(),
            m_cost: u32::MAX,
            t_cost: 1,
            p_cost: 1,
            salt: b64(&[0u8; PASSWORD_SALT_LEN]),
            wrapped: String::new(),
        };
        std::fs::write(
            dir.path().join(PASSWORD_FILE_NAME),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            unlock_with_password(dir.path(), &profile, "x"),
            Err(CoreError::Invalid(_))
        ));
    }

    #[test]
    fn remove_password_restores_plain_storage() {
        let dir = tempfile::tempdir().unwrap();
        let profile = test_profile();
        let key = SymmetricKey::generate();
        set_password(dir.path(), &profile, &key, "correct horse battery").unwrap();
        let source = remove_password(dir.path(), &profile, &key, true).unwrap();
        assert!(!password_protected(dir.path()));
        let loaded = load_or_create(dir.path(), &profile, true).unwrap();
        assert_eq!(loaded.source, source);
        assert_eq!(loaded.key.as_bytes(), key.as_bytes());
        delete(dir.path(), &profile).unwrap();
    }
}
