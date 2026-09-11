//! Keychain façade: SSH keys and identities. Private key text never leaves
//! Rust except through the explicit `export` path; list DTOs carry only the
//! public half and metadata derived by `termoso_core::keys`.

use serde::{Deserialize, Serialize};
use termoso_core::keys::{self, KeyAlgorithm, KeyInfo};
use termoso_core::model::{Entity, Identity, SshKey};
use termoso_core::store::Store;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};

/// Metadata for one stored key. Never contains the private half.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyCard {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    /// `ssh-ed25519`, `ssh-rsa`, `ecdsa-sha2-nistp256`, … or the stored
    /// `key_type` when the material cannot be parsed.
    pub key_type: String,
    pub bits: usize,
    pub fingerprint: String,
    pub public_key: String,
    pub comment: String,
    /// Private key is passphrase-protected on disk.
    pub encrypted: bool,
    /// A passphrase is stored alongside the key (auto-unlock).
    pub has_passphrase: bool,
    /// The material is not parseable (foreign/broken import); only public
    /// data is shown and the key cannot be exported or re-encrypted.
    pub unreadable: bool,
    /// Identities (visible and inline) that reference this key.
    pub used_by: usize,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateForm {
    pub vault_id: Uuid,
    pub label: String,
    pub algorithm: KeyAlgorithm,
    #[serde(default)]
    pub comment: String,
    /// Encrypt the private key with this passphrase.
    #[serde(default)]
    pub passphrase: Option<String>,
    /// Keep the passphrase in the vault so connections do not prompt.
    #[serde(default)]
    pub remember_passphrase: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportForm {
    pub vault_id: Uuid,
    pub label: String,
    /// Private key text in any supported format (OpenSSH, PKCS#8, PEM).
    pub private_key: String,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub remember_passphrase: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityCard {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    pub username: String,
    pub has_password: bool,
    pub ssh_key_id: Option<Uuid>,
    pub ssh_key_label: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityForm {
    pub id: Option<Uuid>,
    pub vault_id: Uuid,
    pub label: String,
    pub username: String,
    /// `None` keeps the stored password, `Some("")` clears it.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub ssh_key_id: Option<Uuid>,
}

fn label_of(s: &str, what: &str) -> Result<String> {
    let label = s.trim();
    if label.is_empty() {
        return Err(DesktopError::invalid(format!("{what} label is required")));
    }
    if label.chars().count() > 200 {
        return Err(DesktopError::invalid(format!("{what} label is too long")));
    }
    Ok(label.to_string())
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|p| !p.is_empty())
}

fn key_usage(
    store: &Store,
    vault_id: Option<Uuid>,
) -> Result<std::collections::HashMap<Uuid, usize>> {
    let mut usage = std::collections::HashMap::new();
    for ident in store.list::<Identity>(vault_id)? {
        if let Some(k) = ident.data.ssh_key_id {
            *usage.entry(k).or_insert(0) += 1;
        }
    }
    Ok(usage)
}

fn card(entity: &Entity<SshKey>, used_by: usize) -> KeyCard {
    let k = &entity.data;
    let info = keys::inspect(&k.private_key).ok();
    let public_line = info
        .as_ref()
        .map(|i| i.public_key.clone())
        .or_else(|| k.public_key.clone())
        .unwrap_or_default();
    let (key_type, bits, fingerprint, comment, encrypted) = match &info {
        Some(i) => (
            i.key_type.clone(),
            i.bits,
            i.fingerprint.clone(),
            i.comment.clone(),
            i.encrypted,
        ),
        None => {
            let pk = k
                .public_key
                .as_deref()
                .and_then(|l| keys::parse_public(l).ok());
            (
                pk.as_ref()
                    .map(|p| p.key_type.clone())
                    .unwrap_or_else(|| k.key_type.clone()),
                pk.as_ref().map(|p| p.bits).unwrap_or(0),
                pk.as_ref()
                    .map(|p| p.fingerprint.clone())
                    .unwrap_or_default(),
                pk.as_ref().map(|p| p.comment.clone()).unwrap_or_default(),
                false,
            )
        }
    };
    KeyCard {
        id: entity.id,
        vault_id: entity.vault_id,
        label: k.label.clone(),
        key_type,
        bits,
        fingerprint,
        public_key: public_line,
        comment,
        encrypted,
        has_passphrase: k.passphrase.as_deref().is_some_and(|p| !p.is_empty()),
        unreadable: info.is_none(),
        used_by,
        updated_at: entity.updated_at,
        dirty: entity.dirty,
    }
}

pub fn keys_list(store: &Store, vault_id: Option<Uuid>) -> Result<Vec<KeyCard>> {
    let usage = key_usage(store, vault_id)?;
    let mut out: Vec<KeyCard> = store
        .list::<SshKey>(vault_id)?
        .iter()
        .map(|e| card(e, usage.get(&e.id).copied().unwrap_or(0)))
        .collect();
    out.sort_by_key(|a| a.label.to_lowercase());
    Ok(out)
}

fn key_card(store: &Store, id: Uuid) -> Result<KeyCard> {
    let e = store.require::<SshKey>(id)?;
    let usage = key_usage(store, Some(e.vault_id))?;
    Ok(card(&e, usage.get(&id).copied().unwrap_or(0)))
}

fn short_type(info: &KeyInfo) -> String {
    match info.key_type.as_str() {
        "ssh-ed25519" => "ed25519".into(),
        "ssh-rsa" => "rsa".into(),
        t if t.starts_with("ecdsa") => "ecdsa".into(),
        t => t.to_string(),
    }
}

pub fn generate(store: &Store, form: &GenerateForm) -> Result<KeyCard> {
    let label = label_of(&form.label, "key")?;
    let passphrase = non_empty(form.passphrase.clone());
    let material = keys::generate(form.algorithm, form.comment.trim(), passphrase.as_deref())?;
    let key = SshKey {
        label,
        private_key: material.private_key.to_string(),
        public_key: Some(material.public_key.clone()),
        passphrase: if form.remember_passphrase {
            passphrase
        } else {
            None
        },
        key_type: short_type(&material.info),
        fido2_credential_id: None,
    };
    let id = store.insert(form.vault_id, &key)?;
    key_card(store, id)
}

pub fn import(store: &Store, form: &ImportForm) -> Result<KeyCard> {
    let label = label_of(&form.label, "key")?;
    let passphrase = non_empty(form.passphrase.clone());
    let material = keys::import(&form.private_key, passphrase.as_deref())?;
    let key = SshKey {
        label,
        private_key: material.private_key.to_string(),
        public_key: Some(material.public_key.clone()),
        passphrase: if form.remember_passphrase {
            passphrase
        } else {
            None
        },
        key_type: short_type(&material.info),
        fido2_credential_id: None,
    };
    let id = store.insert(form.vault_id, &key)?;
    key_card(store, id)
}

pub fn rename(store: &Store, id: Uuid, label: &str) -> Result<KeyCard> {
    let mut e = store.require::<SshKey>(id)?;
    e.data.label = label_of(label, "key")?;
    store.update(id, &e.data)?;
    key_card(store, id)
}

/// Re-encrypt the private key. `current` is needed only when the key is
/// encrypted and no passphrase is stored; `next = None` removes encryption.
pub fn change_passphrase(
    store: &Store,
    id: Uuid,
    current: Option<String>,
    next: Option<String>,
    remember: bool,
) -> Result<KeyCard> {
    let mut e = store.require::<SshKey>(id)?;
    let current = non_empty(current).or_else(|| non_empty(e.data.passphrase.clone()));
    let next = non_empty(next);
    let material =
        keys::change_passphrase(&e.data.private_key, current.as_deref(), next.as_deref())?;
    e.data.private_key = material.private_key.to_string();
    e.data.public_key = Some(material.public_key.clone());
    e.data.passphrase = if remember { next } else { None };
    store.update(id, &e.data)?;
    key_card(store, id)
}

/// Forget (or set) the stored passphrase without touching the key material.
pub fn remember_passphrase(store: &Store, id: Uuid, passphrase: Option<String>) -> Result<KeyCard> {
    let mut e = store.require::<SshKey>(id)?;
    let passphrase = non_empty(passphrase);
    if let Some(p) = &passphrase {
        // Verify before storing so a typo does not get persisted.
        keys::export_openssh(&e.data.private_key, Some(p), None)?;
    }
    e.data.passphrase = passphrase;
    store.update(id, &e.data)?;
    key_card(store, id)
}

/// Public key line (`type base64 comment`).
pub fn public_key(store: &Store, id: Uuid) -> Result<String> {
    let e = store.require::<SshKey>(id)?;
    if let Ok(info) = keys::inspect(&e.data.private_key) {
        return Ok(info.public_key);
    }
    e.data
        .public_key
        .clone()
        .ok_or_else(|| DesktopError::invalid("key has no public half"))
}

/// Private key in OpenSSH format. The only path that hands private material
/// to the webview; the caller decides whether to re-encrypt it on the way out.
pub fn export(
    store: &Store,
    id: Uuid,
    passphrase: Option<String>,
    export_passphrase: Option<String>,
) -> Result<Zeroizing<String>> {
    let e = store.require::<SshKey>(id)?;
    let passphrase = non_empty(passphrase).or_else(|| non_empty(e.data.passphrase.clone()));
    Ok(keys::export_openssh(
        &e.data.private_key,
        passphrase.as_deref(),
        non_empty(export_passphrase).as_deref(),
    )?)
}

/// Remote side of "Export to host" (`ssh-copy-id` semantics). The public key
/// line arrives on stdin — never inside the command string — so no part of
/// it is ever parsed by the remote shell. Idempotent: an existing entry with
/// the same type+blob is left alone. POSIX sh + coreutils/busybox only; one
/// line without single quotes so it survives `sh -c '…'` under any login
/// shell (fish, csh, …).
macro_rules! authorized_keys_script {
    () => {
        concat!(
            "umask 077; d=\"${HOME:?}/.ssh\"; f=\"$d/authorized_keys\"; ",
            "mkdir -p \"$d\" && chmod 700 \"$d\" && touch \"$f\" && chmod 600 \"$f\" || exit 2; ",
            "k=$(head -n 1); set -f; set -- $k; [ -n \"$2\" ] || exit 3; blob=\"$1 $2\"; ",
            "if grep -qF -- \"$blob\" \"$f\"; then echo EXISTS; else ",
            "{ [ -s \"$f\" ] && [ -n \"$(tail -c 1 \"$f\")\" ] && echo >> \"$f\"; ",
            "printf \"%s\\n\" \"$k\" >> \"$f\" && echo ADDED; }; fi",
        )
    };
}

#[cfg(test)]
const AUTHORIZED_KEYS_SCRIPT: &str = authorized_keys_script!();

/// Command sent over `exec`: forces POSIX sh regardless of the login shell.
pub const EXPORT_COMMAND: &str = concat!("exec sh -c '", authorized_keys_script!(), "'");

/// What `EXPORT_COMMAND` reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportOutcome {
    Added,
    AlreadyPresent,
}

/// Interpret the script's exit status and output.
pub fn export_outcome(
    exit_code: Option<u32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<ExportOutcome> {
    let out = String::from_utf8_lossy(stdout);
    match (exit_code, out.trim()) {
        (Some(0), "ADDED") => Ok(ExportOutcome::Added),
        (Some(0), "EXISTS") => Ok(ExportOutcome::AlreadyPresent),
        (Some(2), _) => Err(DesktopError::invalid(
            "could not create ~/.ssh/authorized_keys on the host (permissions?)",
        )),
        (Some(3), _) => Err(DesktopError::invalid(
            "the host did not receive the key (stdin was empty)",
        )),
        (code, _) => {
            let err = String::from_utf8_lossy(stderr);
            let detail = err.trim();
            Err(DesktopError::invalid(match (code, detail.is_empty()) {
                (Some(c), false) => format!("remote command failed (exit {c}): {detail}"),
                (Some(c), true) => format!("remote command failed (exit {c})"),
                (None, false) => format!("remote command failed: {detail}"),
                (None, true) => "remote command failed without an exit status".to_string(),
            }))
        }
    }
}

/// Delete a key and detach it from every identity that referenced it.
pub fn delete(store: &Store, id: Uuid) -> Result<()> {
    let e = store.require::<SshKey>(id)?;
    for mut ident in store.list::<Identity>(Some(e.vault_id))? {
        if ident.data.ssh_key_id == Some(id) {
            ident.data.ssh_key_id = None;
            store.update(ident.id, &ident.data)?;
        }
    }
    store.delete(id)?;
    Ok(())
}

// ───────────────────────────── identities ─────────────────────────────

pub fn identities(store: &Store, vault_id: Option<Uuid>) -> Result<Vec<IdentityCard>> {
    let labels: std::collections::HashMap<Uuid, String> = store
        .list::<SshKey>(vault_id)?
        .into_iter()
        .map(|k| (k.id, k.data.label))
        .collect();
    let mut out: Vec<IdentityCard> = store
        .list::<Identity>(vault_id)?
        .into_iter()
        .filter(|i| i.data.is_visible)
        .map(|i| IdentityCard {
            id: i.id,
            vault_id: i.vault_id,
            label: i.data.label,
            username: i.data.username,
            has_password: i.data.password.as_deref().is_some_and(|p| !p.is_empty()),
            ssh_key_id: i.data.ssh_key_id,
            ssh_key_label: i.data.ssh_key_id.and_then(|k| labels.get(&k).cloned()),
            updated_at: i.updated_at,
        })
        .collect();
    out.sort_by_key(|a| a.label.to_lowercase());
    Ok(out)
}

pub fn save_identity(store: &Store, form: &IdentityForm) -> Result<IdentityCard> {
    let label = label_of(&form.label, "identity")?;
    let username = form.username.trim().to_string();
    if username.is_empty() {
        return Err(DesktopError::invalid("username is required"));
    }
    if let Some(k) = form.ssh_key_id {
        let key = store.require::<SshKey>(k)?;
        if key.vault_id != form.vault_id {
            return Err(DesktopError::invalid("key belongs to another vault"));
        }
    }
    let existing = match form.id {
        Some(id) => Some(store.require::<Identity>(id)?),
        None => None,
    };
    let password = match &form.password {
        Some(p) if p.is_empty() => None,
        Some(p) => Some(p.clone()),
        None => existing.as_ref().and_then(|e| e.data.password.clone()),
    };
    let data = Identity {
        label,
        username,
        password,
        ssh_key_id: form.ssh_key_id,
        ssh_certificate_id: existing.as_ref().and_then(|e| e.data.ssh_certificate_id),
        is_visible: true,
    };
    let id = match existing {
        Some(e) => {
            store.update(e.id, &data)?;
            e.id
        }
        None => store.insert(form.vault_id, &data)?,
    };
    identities(store, Some(form.vault_id))?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| DesktopError::not_found(format!("identity {id}")))
}

pub fn delete_identity(store: &Store, id: Uuid) -> Result<()> {
    let e = store.require::<Identity>(id)?;
    if !e.data.is_visible {
        return Err(DesktopError::invalid(
            "inline identities are managed by their host",
        ));
    }
    store.delete(id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_core::termoso_crypto::keys::SymmetricKey;

    fn store() -> Store {
        Store::open_in_memory(SymmetricKey::generate()).expect("store")
    }

    fn make_key(store: &Store, vault: Uuid, passphrase: Option<&str>, remember: bool) -> KeyCard {
        generate(
            store,
            &GenerateForm {
                vault_id: vault,
                label: "laptop".into(),
                algorithm: KeyAlgorithm::Ed25519,
                comment: "me@laptop".into(),
                passphrase: passphrase.map(str::to_string),
                remember_passphrase: remember,
            },
        )
        .expect("generate")
    }

    #[test]
    fn export_outcome_maps_script_results() {
        assert_eq!(
            export_outcome(Some(0), b"ADDED\n", b"").unwrap(),
            ExportOutcome::Added
        );
        assert_eq!(
            export_outcome(Some(0), b"EXISTS\n", b"").unwrap(),
            ExportOutcome::AlreadyPresent
        );
        let e = export_outcome(Some(2), b"", b"").unwrap_err().to_string();
        assert!(e.contains("authorized_keys"), "{e}");
        let e = export_outcome(Some(127), b"", b"sh: awk: not found\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains("exit 127") && e.contains("awk"), "{e}");
        assert!(export_outcome(None, b"", b"").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn authorized_keys_script_is_idempotent_and_keeps_key_out_of_argv() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let store = store();
        let vault = store.local_vault().unwrap().id;
        let card = make_key(&store, vault, None, false);
        let line = public_key(&store, card.id).unwrap();
        let home = tempfile::tempdir().unwrap();

        // Pre-existing file without a trailing newline must not get glued to.
        std::fs::create_dir(home.path().join(".ssh")).unwrap();
        let ak = home.path().join(".ssh/authorized_keys");
        std::fs::write(&ak, "ssh-ed25519 AAAAexisting old@box").unwrap();

        // Goes through the login-shell wrapper exactly like the remote side.
        let run = |input: &str| {
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(EXPORT_COMMAND)
                .env_clear()
                .env("HOME", home.path())
                .env("PATH", "/usr/bin:/bin")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(format!("{input}\n").as_bytes())
                .unwrap();
            let out = child.wait_with_output().unwrap();
            export_outcome(
                out.status.code().map(|c| c as u32),
                &out.stdout,
                &out.stderr,
            )
        };

        assert_eq!(run(&line).unwrap(), ExportOutcome::Added);
        assert_eq!(run(&line).unwrap(), ExportOutcome::AlreadyPresent);
        // Same key, different comment: still a duplicate.
        let mut parts: Vec<&str> = line.split_whitespace().collect();
        parts.truncate(2);
        let no_comment = parts.join(" ");
        assert_eq!(run(&no_comment).unwrap(), ExportOutcome::AlreadyPresent);
        // A different key is appended as a second line.
        let other = make_key(&store, vault, None, false);
        assert_eq!(
            run(&public_key(&store, other.id).unwrap()).unwrap(),
            ExportOutcome::Added
        );

        let text = std::fs::read_to_string(&ak).unwrap();
        assert_eq!(text.lines().count(), 3);
        assert_eq!(text.lines().nth(1), Some(line.as_str()));
        assert!(text.ends_with('\n'));
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&ak).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(home.path().join(".ssh"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert!(!AUTHORIZED_KEYS_SCRIPT.contains("ssh-ed25519"));
        assert!(!AUTHORIZED_KEYS_SCRIPT.contains('\'') && !AUTHORIZED_KEYS_SCRIPT.contains('\n'));
        // Malformed input (no blob) is refused rather than matched as a prefix.
        assert!(run("ssh-ed25519").is_err());

        // Hostile input never reaches the shell as code.
        let evil = "ssh-ed25519 AAAA$(touch /tmp/pwned_termoso)`id` x";
        assert_eq!(run(evil).unwrap(), ExportOutcome::Added);
        assert!(!std::path::Path::new("/tmp/pwned_termoso").exists());
        assert!(std::fs::read_to_string(&ak).unwrap().contains(evil));
    }

    #[test]
    fn list_never_contains_private_material() {
        let store = store();
        let vault = store.local_vault().unwrap().id;
        let card = make_key(&store, vault, None, false);
        let json = serde_json::to_string(&keys_list(&store, Some(vault)).unwrap()).unwrap();
        assert!(!json.contains("PRIVATE KEY"));
        assert!(json.contains(&card.fingerprint));
        assert_eq!(card.key_type, "ssh-ed25519");
        assert_eq!(card.comment, "me@laptop");
        assert!(!card.encrypted);
        assert!(!card.has_passphrase);
        assert_eq!(card.used_by, 0);
    }

    #[test]
    fn passphrase_lifecycle() {
        let store = store();
        let vault = store.local_vault().unwrap().id;
        let card = make_key(&store, vault, Some("pw"), true);
        assert!(card.encrypted && card.has_passphrase);

        // Export uses the stored passphrase; re-encrypting on export works.
        let plain = export(&store, card.id, None, None).unwrap();
        assert!(plain.contains("OPENSSH PRIVATE KEY"));
        let info = keys::inspect(&plain).unwrap();
        assert!(!info.encrypted);
        let enc = export(&store, card.id, None, Some("other".into())).unwrap();
        assert!(keys::inspect(&enc).unwrap().encrypted);

        // Forget the passphrase: export without one must fail.
        let card = remember_passphrase(&store, card.id, None).unwrap();
        assert!(!card.has_passphrase);
        assert!(export(&store, card.id, None, None).is_err());
        assert!(remember_passphrase(&store, card.id, Some("wrong".into())).is_err());
        assert!(export(&store, card.id, Some("pw".into()), None).is_ok());

        // Remove encryption entirely.
        let card = change_passphrase(&store, card.id, Some("pw".into()), None, false).unwrap();
        assert!(!card.encrypted);
        assert!(export(&store, card.id, None, None).is_ok());
    }

    #[test]
    fn import_roundtrip_and_public_line() {
        let store = store();
        let vault = store.local_vault().unwrap().id;
        let material = keys::generate(KeyAlgorithm::EcdsaP256, "c", None).unwrap();
        let card = import(
            &store,
            &ImportForm {
                vault_id: vault,
                label: " imported ".into(),
                private_key: material.private_key.to_string(),
                passphrase: None,
                remember_passphrase: false,
            },
        )
        .unwrap();
        assert_eq!(card.label, "imported");
        assert_eq!(card.fingerprint, material.info.fingerprint);
        assert_eq!(public_key(&store, card.id).unwrap(), material.public_key);
        assert!(
            import(
                &store,
                &ImportForm {
                    vault_id: vault,
                    label: "bad".into(),
                    private_key: "not a key".into(),
                    passphrase: None,
                    remember_passphrase: false,
                },
            )
            .is_err()
        );
    }

    #[test]
    fn identities_reference_keys_and_detach_on_delete() {
        let store = store();
        let vault = store.local_vault().unwrap().id;
        let key = make_key(&store, vault, None, false);
        let ident = save_identity(
            &store,
            &IdentityForm {
                id: None,
                vault_id: vault,
                label: "deploy".into(),
                username: "deploy".into(),
                password: Some("pw".into()),
                ssh_key_id: Some(key.id),
            },
        )
        .unwrap();
        assert!(ident.has_password);
        assert_eq!(ident.ssh_key_label.as_deref(), Some("laptop"));
        assert_eq!(keys_list(&store, Some(vault)).unwrap()[0].used_by, 1);

        // Saving with password None keeps it, Some("") clears it.
        let ident2 = save_identity(
            &store,
            &IdentityForm {
                id: Some(ident.id),
                vault_id: vault,
                label: "deploy".into(),
                username: "deploy".into(),
                password: None,
                ssh_key_id: Some(key.id),
            },
        )
        .unwrap();
        assert!(ident2.has_password);
        let ident3 = save_identity(
            &store,
            &IdentityForm {
                id: Some(ident.id),
                vault_id: vault,
                label: "deploy".into(),
                username: "deploy".into(),
                password: Some(String::new()),
                ssh_key_id: Some(key.id),
            },
        )
        .unwrap();
        assert!(!ident3.has_password);

        delete(&store, key.id).unwrap();
        let after = identities(&store, Some(vault)).unwrap();
        assert_eq!(after.len(), 1);
        assert!(after[0].ssh_key_id.is_none());
        assert!(
            save_identity(
                &store,
                &IdentityForm {
                    id: None,
                    vault_id: vault,
                    label: "x".into(),
                    username: "".into(),
                    password: None,
                    ssh_key_id: None,
                },
            )
            .is_err()
        );
    }
}
