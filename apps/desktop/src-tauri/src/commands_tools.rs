//! IPC surface for keychain, port forwarding, snippets, known hosts, logs
//! and account/sync. Thin adapters; the façades own validation and secrets.

use std::collections::HashMap;

use serde::Deserialize;

use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use termoso_core::secrets::MasterKeySource;
use termoso_proto::account::{ServerInfo, UserProfile};
use termoso_proto::auth::{Device, MfaCredential};
use termoso_proto::team::{Invite, Team, TeamPresence, TeamRole};
use termoso_proto::vault::{VaultMember, VaultRole};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::account::{
    self, AccountStatus, LoginForm, LoginOutcome, ReauthOutcome, RegisterForm, Registered,
    SYNC_EVENT, SyncNotice, SyncStatus,
};
use crate::avatars;
use crate::backup::{self, BackupSummary};
use crate::cloud::{self, CloudImportReport, CloudPreview, CloudSelection};
use crate::error::{DesktopError, Result};
use crate::forwarding::{self, PfRuleCard, PfRuleForm, PfRuntime};
use crate::import::{self, ImportPreview, ImportSelection, ImportSource};
use crate::keychain::{
    self, CertificateCard, ExportOutcome, Fido2GenerateForm, Fido2LoadForm, GenerateForm,
    IdentityCard, IdentityForm, ImportForm, KeyCard, KeyPreview,
};
use crate::logs::{self, BookmarkCard, LogBody, LogCard};
use crate::multiplayer::{self, ShareInfo};
use crate::presence;
use crate::sessions;
use crate::snippets::{self, PackageNode, RunResult, SnippetCard, SnippetForm};
use crate::sshid::{self, SshIdFido2Form, SshIdView};
use crate::state::AppState;
use crate::team::{self, InviteResult, PendingKeyCard, TeamMemberCard, VaultAccess};
use crate::trust::{self, ImportReport, KnownHostCard};
use crate::update::{self, UpdateInfo};

// ───────────────────────────── keychain ─────────────────────────────

#[tauri::command]
pub async fn keys_list(state: State<'_, AppState>, vault_id: Option<Uuid>) -> Result<Vec<KeyCard>> {
    keychain::keys_list(&state.store, vault_id)
}

#[tauri::command]
pub async fn key_generate(state: State<'_, AppState>, form: GenerateForm) -> Result<KeyCard> {
    keychain::generate(&state.store, &form)
}

#[tauri::command]
pub async fn key_import(state: State<'_, AppState>, form: ImportForm) -> Result<KeyCard> {
    keychain::import(&state.store, &form)
}

fn blocking_err(e: tokio::task::JoinError) -> DesktopError {
    DesktopError::new("internal", e.to_string())
}

/// FIDO2 authenticators plugged in right now (USB HID enumeration; local only).
#[tauri::command]
pub async fn fido2_devices() -> Result<Vec<termoso_core::fido2::Fido2Device>> {
    tokio::task::spawn_blocking(keychain::fido2_devices)
        .await
        .map_err(blocking_err)
}

/// Make a credential on the token and store the `sk-*` key. Blocks until
/// the user touches the token (or it times out), so it runs off-runtime.
#[tauri::command]
pub async fn fido2_generate(
    state: State<'_, AppState>,
    form: Fido2GenerateForm,
) -> Result<KeyCard> {
    let store = state.store.clone();
    tokio::task::spawn_blocking(move || keychain::fido2_generate(&store, &form))
        .await
        .map_err(blocking_err)?
}

/// Import the resident SSH credentials of a token into a vault.
#[tauri::command]
pub async fn fido2_load_resident(
    state: State<'_, AppState>,
    form: Fido2LoadForm,
) -> Result<Vec<KeyCard>> {
    let store = state.store.clone();
    tokio::task::spawn_blocking(move || keychain::fido2_load_resident(&store, &form))
        .await
        .map_err(blocking_err)?
}

/// Import request for a private key file on disk.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileForm {
    pub vault_id: Uuid,
    pub label: String,
    pub path: String,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub remember_passphrase: bool,
    /// Pasted certificate text; ignored when `certificate_path` is set.
    #[serde(default)]
    pub certificate: Option<String>,
    /// Path to a `*-cert.pub` to attach.
    #[serde(default)]
    pub certificate_path: Option<String>,
}

/// Import from a file on disk; the private material is read here and never
/// crosses the IPC boundary.
#[tauri::command]
pub async fn key_import_file(state: State<'_, AppState>, form: ImportFileForm) -> Result<KeyCard> {
    let private_key = std::fs::read_to_string(&form.path)?;
    let certificate = match form.certificate_path {
        Some(p) => Some(std::fs::read_to_string(&p)?),
        None => form.certificate,
    };
    keychain::import(
        &state.store,
        &ImportForm {
            vault_id: form.vault_id,
            label: form.label,
            private_key,
            passphrase: form.passphrase,
            remember_passphrase: form.remember_passphrase,
            certificate,
        },
    )
}

/// Public half + format of pasted private key text; nothing is stored.
#[tauri::command]
pub async fn key_inspect(text: String) -> Result<KeyPreview> {
    keychain::inspect_private(&text)
}

/// Same for a file on disk; the private material stays in Rust.
#[tauri::command]
pub async fn key_inspect_file(path: String) -> Result<KeyPreview> {
    keychain::inspect_private(&std::fs::read_to_string(&path)?)
}

/// Parse + verify a certificate for the editor preview; nothing is stored.
#[tauri::command]
pub async fn certificate_inspect(text: String) -> Result<CertificateCard> {
    keychain::inspect_certificate(&text)
}

#[tauri::command]
pub async fn certificate_inspect_file(path: String) -> Result<CertificateCard> {
    keychain::inspect_certificate(&std::fs::read_to_string(&path)?)
}

/// Certificate text attached to a key (public data), for display/copy.
#[tauri::command]
pub async fn key_certificate(state: State<'_, AppState>, id: Uuid) -> Result<Option<String>> {
    keychain::certificate_text(&state.store, id)
}

/// Attach (`Some(text)`) or detach (`None`) the certificate of a key.
#[tauri::command]
pub async fn key_set_certificate(
    state: State<'_, AppState>,
    id: Uuid,
    certificate: Option<String>,
) -> Result<KeyCard> {
    keychain::set_certificate(&state.store, id, certificate)
}

#[tauri::command]
pub async fn key_set_certificate_file(
    state: State<'_, AppState>,
    id: Uuid,
    path: String,
) -> Result<KeyCard> {
    let text = std::fs::read_to_string(&path)?;
    keychain::set_certificate(&state.store, id, Some(text))
}

/// Copy or move a key (with its certificate) into another vault.
#[tauri::command]
pub async fn key_copy_to_vault(
    state: State<'_, AppState>,
    id: Uuid,
    vault_id: Uuid,
    move_key: bool,
) -> Result<KeyCard> {
    keychain::copy_to_vault(&state.store, id, vault_id, move_key)
}

#[tauri::command]
pub async fn key_rename(state: State<'_, AppState>, id: Uuid, label: String) -> Result<KeyCard> {
    keychain::rename(&state.store, id, &label)
}

#[tauri::command]
pub async fn key_change_passphrase(
    state: State<'_, AppState>,
    id: Uuid,
    current: Option<String>,
    next: Option<String>,
    remember: bool,
) -> Result<KeyCard> {
    keychain::change_passphrase(&state.store, id, current, next, remember)
}

#[tauri::command]
pub async fn key_remember_passphrase(
    state: State<'_, AppState>,
    id: Uuid,
    passphrase: Option<String>,
) -> Result<KeyCard> {
    keychain::remember_passphrase(&state.store, id, passphrase)
}

#[tauri::command]
pub async fn key_public(state: State<'_, AppState>, id: Uuid) -> Result<String> {
    keychain::public_key(&state.store, id)
}

/// Explicit export of private material. `export_passphrase` re-encrypts the
/// exported copy; `None` writes it unencrypted (the UI warns first).
#[tauri::command]
pub async fn key_export(
    state: State<'_, AppState>,
    id: Uuid,
    passphrase: Option<String>,
    export_passphrase: Option<String>,
) -> Result<String> {
    Ok(keychain::export(&state.store, id, passphrase, export_passphrase)?.to_string())
}

#[tauri::command]
pub async fn key_export_file(
    state: State<'_, AppState>,
    id: Uuid,
    path: String,
    passphrase: Option<String>,
    export_passphrase: Option<String>,
) -> Result<()> {
    let text = keychain::export(&state.store, id, passphrase, export_passphrase)?;
    write_private(&path, text.as_bytes())?;
    let public = keychain::public_key(&state.store, id)?;
    std::fs::write(format!("{path}.pub"), format!("{public}\n"))?;
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &str, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &str, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Result of `key_export_to_host`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportToHostResult {
    pub outcome: ExportOutcome,
    pub host_label: String,
    /// `user@host:port` the key was installed for.
    pub target: String,
}

/// `ssh-copy-id`: connect to a saved host with its current credentials and
/// append the key's public half to `~/.ssh/authorized_keys` there. Prompts
/// (password, host key) are routed under a throw-away session id; the
/// transport is closed as soon as the command returns.
#[tauri::command]
pub async fn key_export_to_host<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Uuid,
    host_id: Uuid,
) -> Result<ExportToHostResult> {
    let public = keychain::public_key(&state.store, id)?;
    let conn = sessions::connect_host(&app, Uuid::new_v4(), host_id).await?;
    let out = conn
        .client
        .exec(
            keychain::EXPORT_COMMAND,
            Some(bytes::Bytes::from(format!("{public}\n"))),
        )
        .await;
    let (host_label, target) = (conn.label.clone(), conn.display.clone());
    conn.close().await;
    let out = out?;
    let outcome = keychain::export_outcome(out.exit_code, &out.stdout, &out.stderr)?;
    Ok(ExportToHostResult {
        outcome,
        host_label,
        target,
    })
}

/// Keys the system SSH agent currently holds (public halves only). Empty
/// with `available: false` when no agent is reachable.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentKeys {
    pub available: bool,
    /// Why the agent is unavailable (never contains secrets).
    pub error: Option<String>,
    pub keys: Vec<termoso_core::agent::AgentKey>,
}

#[tauri::command]
pub async fn agent_keys() -> Result<AgentKeys> {
    let mut agent = match termoso_core::agent::connect_system_agent().await {
        Ok(a) => a,
        Err(e) => {
            return Ok(AgentKeys {
                available: false,
                error: Some(e.to_string()),
                keys: Vec::new(),
            });
        }
    };
    match termoso_core::agent::list_keys(&mut agent).await {
        Ok(keys) => Ok(AgentKeys {
            available: true,
            error: None,
            keys,
        }),
        Err(e) => Ok(AgentKeys {
            available: false,
            error: Some(e.to_string()),
            keys: Vec::new(),
        }),
    }
}

#[tauri::command]
pub async fn key_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    keychain::delete(&state.store, id)
}

#[tauri::command]
pub async fn identities_list(
    state: State<'_, AppState>,
    vault_id: Option<Uuid>,
) -> Result<Vec<IdentityCard>> {
    keychain::identities(&state.store, vault_id)
}

#[tauri::command]
pub async fn identity_save(state: State<'_, AppState>, form: IdentityForm) -> Result<IdentityCard> {
    keychain::save_identity(&state.store, &form)
}

#[tauri::command]
pub async fn identity_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    keychain::delete_identity(&state.store, id)
}

/// Copy or move an identity (with its key and certificate) into another vault.
#[tauri::command]
pub async fn identity_copy_to_vault(
    state: State<'_, AppState>,
    id: Uuid,
    vault_id: Uuid,
    move_identity: bool,
) -> Result<IdentityCard> {
    keychain::copy_identity_to_vault(&state.store, id, vault_id, move_identity)
}

#[tauri::command]
pub fn master_key_migrate(state: State<'_, AppState>) -> Result<MasterKeySource> {
    state.migrate_master_key()
}

// ───────────────────────────── port forwarding ─────────────────────────────

#[tauri::command]
pub async fn pf_rules(
    state: State<'_, AppState>,
    vault_id: Option<Uuid>,
) -> Result<Vec<PfRuleCard>> {
    forwarding::rules(&state, vault_id)
}

#[tauri::command]
pub async fn pf_save(state: State<'_, AppState>, form: PfRuleForm) -> Result<PfRuleCard> {
    forwarding::save(&state, &form)
}

#[tauri::command]
pub fn pf_runtimes(state: State<'_, AppState>) -> Result<HashMap<Uuid, PfRuntime>> {
    forwarding::runtimes(&state)
}

#[tauri::command]
pub async fn pf_start<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<PfRuleCard> {
    forwarding::start(&app, id).await
}

#[tauri::command]
pub async fn pf_stop<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<()> {
    forwarding::stop(&app, id).await
}

#[tauri::command]
pub async fn pf_delete<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<()> {
    forwarding::delete(&app, id).await
}

#[tauri::command]
pub async fn pf_duplicate(state: State<'_, AppState>, id: Uuid) -> Result<PfRuleCard> {
    forwarding::duplicate(&state, id)
}

#[tauri::command]
pub async fn pf_copy_to_vault<R: Runtime>(
    app: AppHandle<R>,
    id: Uuid,
    vault_id: Uuid,
    move_rule: bool,
) -> Result<PfRuleCard> {
    if move_rule {
        forwarding::move_to_vault(&app, id, vault_id).await
    } else {
        forwarding::copy_to_vault(&app.state::<AppState>(), id, vault_id)
    }
}

// ───────────────────────────── snippets ─────────────────────────────

#[tauri::command]
pub async fn snippets_list(
    state: State<'_, AppState>,
    vault_id: Option<Uuid>,
) -> Result<Vec<SnippetCard>> {
    snippets::list(&state.store, vault_id)
}

#[tauri::command]
pub async fn snippet_save(state: State<'_, AppState>, form: SnippetForm) -> Result<SnippetCard> {
    snippets::save(&state.store, &form)
}

#[tauri::command]
pub async fn snippet_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    snippets::delete(&state.store, id)
}

#[tauri::command]
pub async fn snippet_copy_to_vault(
    state: State<'_, AppState>,
    id: Uuid,
    vault_id: Uuid,
    move_snippet: bool,
) -> Result<SnippetCard> {
    snippets::copy_to_vault(&state.store, id, vault_id, move_snippet)
}

#[tauri::command]
pub async fn snippet_set_targets(
    state: State<'_, AppState>,
    id: Uuid,
    host_ids: Vec<Uuid>,
) -> Result<SnippetCard> {
    snippets::set_targets(&state.store, id, &host_ids)
}

#[tauri::command]
pub async fn snippet_run(
    state: State<'_, AppState>,
    id: Uuid,
    session_ids: Vec<Uuid>,
    vars: HashMap<String, String>,
    paste: Option<bool>,
) -> Result<RunResult> {
    snippets::run(&state, id, &session_ids, &vars, paste.unwrap_or(false)).await
}

#[tauri::command]
pub async fn snippet_packages(
    state: State<'_, AppState>,
    vault_id: Option<Uuid>,
) -> Result<Vec<PackageNode>> {
    snippets::packages(&state.store, vault_id)
}

#[tauri::command]
pub async fn snippet_package_save(
    state: State<'_, AppState>,
    vault_id: Uuid,
    id: Option<Uuid>,
    label: String,
    parent_id: Option<Uuid>,
) -> Result<PackageNode> {
    snippets::save_package(&state.store, vault_id, id, &label, parent_id)
}

#[tauri::command]
pub async fn snippet_package_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    snippets::delete_package(&state.store, id)
}

#[tauri::command]
pub async fn snippet_package_copy_to_vault(
    state: State<'_, AppState>,
    id: Uuid,
    vault_id: Uuid,
    move_package: bool,
) -> Result<PackageNode> {
    snippets::copy_package_to_vault(&state.store, id, vault_id, move_package)
}

// ───────────────────────────── known hosts ─────────────────────────────

#[tauri::command]
pub async fn known_hosts_list(state: State<'_, AppState>) -> Result<Vec<KnownHostCard>> {
    trust::list(&state.store)
}

#[tauri::command]
pub async fn known_host_forget(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    trust::forget(&state.store, id)
}

#[tauri::command]
pub async fn known_host_forget_host(state: State<'_, AppState>, hostname: String) -> Result<usize> {
    trust::forget_host(&state.store, &hostname)
}

#[tauri::command]
pub async fn known_hosts_import_text(
    state: State<'_, AppState>,
    contents: String,
) -> Result<ImportReport> {
    trust::import_openssh(&state, &contents)
}

#[tauri::command]
pub async fn known_hosts_import_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImportReport> {
    trust::import_file(&state, &path)
}

#[tauri::command]
pub async fn known_hosts_export_text(state: State<'_, AppState>) -> Result<String> {
    trust::export_openssh(&state)
}

#[tauri::command]
pub async fn known_hosts_export_file(state: State<'_, AppState>, path: String) -> Result<usize> {
    trust::export_file(&state, &path)
}

#[tauri::command]
pub fn known_hosts_default_path() -> Option<String> {
    trust::default_openssh_path()
}

// ───────────────────────────── import ─────────────────────────────

/// Parse everything in `~/.ssh` (or `dir`) without touching the vault.
#[tauri::command]
pub async fn import_scan_ssh(dir: Option<String>) -> Result<ImportPreview> {
    tauri::async_runtime::spawn_blocking(move || import::scan_ssh_dir(dir.as_deref()))
        .await
        .map_err(|e| crate::error::DesktopError::new("import", e.to_string()))?
        .map(import::remember)
}

#[tauri::command]
pub async fn import_parse_file(source: ImportSource, path: String) -> Result<ImportPreview> {
    tauri::async_runtime::spawn_blocking(move || import::parse_file(source, &path))
        .await
        .map_err(|e| crate::error::DesktopError::new("import", e.to_string()))?
        .map(import::remember)
}

#[tauri::command]
pub async fn import_scan_putty_registry() -> Result<ImportPreview> {
    tauri::async_runtime::spawn_blocking(import::scan_putty_registry)
        .await
        .map_err(|e| crate::error::DesktopError::new("import", e.to_string()))?
        .map(import::remember)
}

#[tauri::command]
pub fn import_ssh_dir_default() -> Option<String> {
    import::default_ssh_dir()
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn import_csv_template() -> String {
    import::csv_template()
}

#[tauri::command]
pub fn import_csv_template_save(path: String) -> Result<()> {
    std::fs::write(&path, import::csv_template())?;
    Ok(())
}

#[tauri::command]
pub async fn import_apply(
    state: State<'_, AppState>,
    vault_id: Uuid,
    preview_id: Uuid,
    selection: ImportSelection,
) -> Result<import::ImportReport> {
    import::apply_cached(&state, vault_id, preview_id, &selection)
}

#[tauri::command]
pub fn import_discard(preview_id: Uuid) {
    import::discard(preview_id)
}

// ───────────────────────────── cloud integration ─────────────────────────────

/// List machines at a provider. `config` carries the credentials for this
/// call only; nothing of it is kept or returned.
#[tauri::command]
pub async fn cloud_discover(
    state: State<'_, AppState>,
    vault_id: Uuid,
    config: termoso_core::cloud::CloudConfig,
) -> Result<CloudPreview> {
    cloud::discover(&state.store, vault_id, config).await
}

#[tauri::command]
pub async fn cloud_import(
    state: State<'_, AppState>,
    vault_id: Uuid,
    preview_id: Uuid,
    selection: CloudSelection,
) -> Result<CloudImportReport> {
    cloud::apply_cached(&state.store, vault_id, preview_id, &selection)
}

#[tauri::command]
pub fn cloud_discard(preview_id: Uuid) {
    cloud::discard(preview_id)
}

// ───────────────────────────── export / backup ─────────────────────────────

/// Write hosts as CSV. Passwords are left blank unless `include_passwords`.
#[tauri::command]
pub async fn hosts_export_csv(
    state: State<'_, AppState>,
    vault_id: Option<Uuid>,
    include_passwords: bool,
    path: String,
) -> Result<backup::CsvExportReport> {
    backup::export_hosts_csv(&state.store, vault_id, include_passwords, &path)
}

/// Encrypt the given vaults (all unlocked when empty) into a `.termoso` file.
#[tauri::command]
pub async fn backup_export(
    state: State<'_, AppState>,
    vault_ids: Vec<Uuid>,
    password: String,
    path: String,
) -> Result<BackupSummary> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        backup::export_file(&store, &vault_ids, &password, &path)
    })
    .await
    .map_err(|e| crate::error::DesktopError::new("backup", e.to_string()))?
}

/// Decrypt a backup and describe its contents; nothing is written yet.
#[tauri::command]
pub async fn backup_inspect(path: String, password: String) -> Result<BackupSummary> {
    tauri::async_runtime::spawn_blocking(move || backup::inspect_file(&path, &password))
        .await
        .map_err(|e| crate::error::DesktopError::new("backup", e.to_string()))?
}

#[tauri::command]
pub fn backup_discard(preview_id: Uuid) {
    backup::discard(preview_id)
}

/// Restore vault `source` of an inspected backup into `vault_id`.
#[tauri::command]
pub async fn backup_restore<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    preview_id: Uuid,
    source: usize,
    vault_id: Uuid,
) -> Result<backup::RestoreReport> {
    let report = backup::apply(&state.store, preview_id, source, vault_id)?;
    let _ = app.emit(SYNC_EVENT, SyncNotice::EntitiesChanged { vault_id });
    Ok(report)
}

// ───────────────────────────── logs ─────────────────────────────

#[tauri::command]
pub async fn logs_list(state: State<'_, AppState>) -> Result<Vec<LogCard>> {
    logs::list(&state.store)
}

#[tauri::command]
pub async fn log_read(state: State<'_, AppState>, id: Uuid) -> Result<LogBody> {
    logs::read(&state.store, id)
}

#[tauri::command]
pub async fn log_export(state: State<'_, AppState>, id: Uuid, path: String) -> Result<usize> {
    logs::export(&state.store, id, &path)
}

#[tauri::command]
pub async fn log_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    logs::delete(&state.store, id)
}

#[tauri::command]
pub async fn log_bookmarks(state: State<'_, AppState>, log_id: Uuid) -> Result<Vec<BookmarkCard>> {
    logs::bookmarks(&state.store, log_id)
}

#[tauri::command]
pub async fn log_bookmark_add(
    state: State<'_, AppState>,
    log_id: Uuid,
    offset: u64,
    note: String,
) -> Result<BookmarkCard> {
    logs::add_bookmark(&state.store, log_id, offset, &note)
}

#[tauri::command]
pub async fn log_bookmark_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    logs::delete_bookmark(&state.store, id)
}

// ───────────────────────────── account / sync ─────────────────────────────

#[tauri::command]
pub async fn account_status<R: Runtime>(app: AppHandle<R>) -> Result<AccountStatus> {
    account::current(&app).await
}

#[tauri::command]
pub async fn account_server_info(server_url: String) -> Result<ServerInfo> {
    account::server_info(&server_url).await
}

#[tauri::command]
pub async fn account_login<R: Runtime>(app: AppHandle<R>, form: LoginForm) -> Result<LoginOutcome> {
    account::login(&app, form).await
}

#[tauri::command]
pub async fn account_mfa<R: Runtime>(
    app: AppHandle<R>,
    credential: MfaCredential,
) -> Result<LoginOutcome> {
    account::mfa(&app, credential).await
}

#[tauri::command]
pub async fn account_mfa_email_send<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    account::mfa_email_send(&app).await
}

#[tauri::command]
pub async fn account_webauthn_challenge<R: Runtime>(
    app: AppHandle<R>,
) -> Result<serde_json::Value> {
    account::webauthn_challenge(&app).await
}

#[tauri::command]
pub async fn account_device_approve<R: Runtime>(
    app: AppHandle<R>,
    code: String,
) -> Result<LoginOutcome> {
    account::approve_device(&app, &code).await
}

#[tauri::command]
pub async fn account_device_resend<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    account::resend_device_code(&app).await
}

#[tauri::command]
pub async fn account_cancel_login<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    account::cancel_login(&app).await
}

#[tauri::command]
pub async fn account_register<R: Runtime>(
    app: AppHandle<R>,
    form: RegisterForm,
) -> Result<Registered> {
    account::register(&app, form).await
}

#[tauri::command]
pub async fn account_sign_out<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    account::sign_out(&app).await
}

#[tauri::command]
pub async fn account_sync_now<R: Runtime>(app: AppHandle<R>) -> Result<SyncStatus> {
    account::sync_now(&app).await
}

#[tauri::command]
pub async fn account_devices<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Device>> {
    account::devices(&app).await
}

#[tauri::command]
pub async fn account_device_revoke<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<()> {
    account::revoke_device(&app, id).await
}

#[tauri::command]
pub async fn account_reauth_start<R: Runtime>(
    app: AppHandle<R>,
    password: Zeroizing<String>,
) -> Result<ReauthOutcome> {
    account::reauth_start(&app, password).await
}

#[tauri::command]
pub async fn account_reauth_mfa<R: Runtime>(
    app: AppHandle<R>,
    credential: MfaCredential,
) -> Result<ReauthOutcome> {
    account::reauth_mfa(&app, credential).await
}

#[tauri::command]
pub async fn account_reauth_email_code<R: Runtime>(
    app: AppHandle<R>,
    code: String,
) -> Result<ReauthOutcome> {
    account::reauth_email_code(&app, code).await
}

#[tauri::command]
pub async fn account_reauth_mfa_email_send<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    account::reauth_mfa_email_send(&app).await
}

#[tauri::command]
pub async fn account_reauth_webauthn_challenge<R: Runtime>(
    app: AppHandle<R>,
) -> Result<serde_json::Value> {
    account::reauth_webauthn_challenge(&app).await
}

#[tauri::command]
pub async fn account_reauth_cancel<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    account::reauth_cancel(&app).await
}

#[tauri::command]
pub async fn account_vault_members<R: Runtime>(
    app: AppHandle<R>,
    vault_id: Uuid,
) -> Result<Vec<VaultMember>> {
    account::vault_members(&app, vault_id).await
}

// ───────────────────────────── SSH ID ─────────────────────────────

#[tauri::command]
pub async fn sshid_view<R: Runtime>(app: AppHandle<R>) -> Result<SshIdView> {
    sshid::view(&app).await
}

#[tauri::command]
pub async fn sshid_create<R: Runtime>(app: AppHandle<R>, handle: String) -> Result<SshIdView> {
    sshid::create(&app, &handle).await
}

#[tauri::command]
pub async fn sshid_delete<R: Runtime>(app: AppHandle<R>) -> Result<SshIdView> {
    sshid::delete(&app).await
}

#[tauri::command]
pub async fn sshid_publish<R: Runtime>(app: AppHandle<R>) -> Result<SshIdView> {
    sshid::publish_now(&app).await
}

#[tauri::command]
pub async fn sshid_rotate<R: Runtime>(app: AppHandle<R>) -> Result<SshIdView> {
    sshid::rotate(&app).await
}

/// Blocks until the token is touched; the generation runs off-runtime.
#[tauri::command]
pub async fn sshid_add_fido2<R: Runtime>(
    app: AppHandle<R>,
    form: SshIdFido2Form,
) -> Result<SshIdView> {
    sshid::add_fido2(&app, form).await
}

#[tauri::command]
pub async fn sshid_remove_key<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<SshIdView> {
    sshid::remove_key(&app, id).await
}

#[tauri::command]
pub async fn sshid_remove_device<R: Runtime>(
    app: AppHandle<R>,
    device_id: Uuid,
) -> Result<SshIdView> {
    sshid::remove_device(&app, device_id).await
}

// ───────────────────────────── teams ─────────────────────────────

#[tauri::command]
pub async fn teams_list<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Team>> {
    team::list(&app).await
}

#[tauri::command]
pub async fn team_create<R: Runtime>(app: AppHandle<R>, name: String) -> Result<Team> {
    team::create(&app, &name).await
}

#[tauri::command]
pub async fn team_rename<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
    name: String,
) -> Result<Team> {
    team::rename(&app, team_id, &name).await
}

#[tauri::command]
pub async fn team_set_security<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
    multiplayer_enabled: Option<bool>,
    require_mfa: Option<bool>,
    presence_enabled: Option<bool>,
) -> Result<Team> {
    team::set_security(
        &app,
        team_id,
        multiplayer_enabled,
        require_mfa,
        presence_enabled,
    )
    .await
}

#[tauri::command]
pub async fn team_delete<R: Runtime>(app: AppHandle<R>, team_id: Uuid) -> Result<()> {
    team::delete(&app, team_id).await
}

#[tauri::command]
pub async fn team_leave<R: Runtime>(app: AppHandle<R>, team_id: Uuid) -> Result<()> {
    team::leave(&app, team_id).await
}

#[tauri::command]
pub async fn team_accept_invite<R: Runtime>(app: AppHandle<R>, link: String) -> Result<Team> {
    team::accept_invite(&app, &link).await
}

#[tauri::command]
pub async fn team_members<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
) -> Result<Vec<TeamMemberCard>> {
    team::members(&app, team_id).await
}

#[tauri::command]
pub async fn team_member_set_role<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
    user_id: Uuid,
    role: TeamRole,
) -> Result<()> {
    team::set_member_role(&app, team_id, user_id, role).await
}

#[tauri::command]
pub async fn team_member_remove<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<()> {
    team::remove_member(&app, team_id, user_id).await
}

#[tauri::command]
pub async fn team_invites<R: Runtime>(app: AppHandle<R>, team_id: Uuid) -> Result<Vec<Invite>> {
    team::invites(&app, team_id).await
}

#[tauri::command]
pub async fn team_invite<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
    emails: Vec<String>,
    role: TeamRole,
    vault_ids: Vec<Uuid>,
) -> Result<Vec<InviteResult>> {
    team::invite(&app, team_id, emails, role, vault_ids).await
}

#[tauri::command]
pub async fn team_invite_revoke<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
    invite_id: Uuid,
) -> Result<()> {
    team::revoke_invite(&app, team_id, invite_id).await
}

#[tauri::command]
pub async fn team_pending_keys<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
) -> Result<Vec<PendingKeyCard>> {
    team::pending_keys(&app, team_id).await
}

#[tauri::command]
pub async fn team_audit<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
    filter: Option<team::AuditFilter>,
) -> Result<team::AuditPage> {
    team::audit(&app, team_id, filter.unwrap_or_default()).await
}

#[tauri::command]
pub async fn team_presence<R: Runtime>(app: AppHandle<R>, team_id: Uuid) -> Result<TeamPresence> {
    presence::team(&app, team_id).await
}

#[tauri::command]
pub async fn account_profile<R: Runtime>(app: AppHandle<R>) -> Result<UserProfile> {
    presence::profile(&app).await
}

/// Raw WebP bytes (empty when there is no picture) so the webview can build
/// a blob URL without a base64 round trip.
#[tauri::command]
pub async fn user_avatar<R: Runtime>(
    app: AppHandle<R>,
    user_id: Uuid,
    tag: String,
) -> Result<tauri::ipc::Response> {
    let bytes = avatars::user_avatar(&app, user_id, tag).await?;
    Ok(tauri::ipc::Response::new(bytes.unwrap_or_default()))
}

#[tauri::command]
pub async fn account_set_presence_hidden<R: Runtime>(
    app: AppHandle<R>,
    hidden: bool,
) -> Result<UserProfile> {
    presence::set_hidden(&app, hidden).await
}

#[tauri::command]
pub async fn team_vault_create<R: Runtime>(
    app: AppHandle<R>,
    team_id: Uuid,
    name: String,
    access: Vec<VaultAccess>,
) -> Result<()> {
    team::create_vault(&app, team_id, &name, access).await
}

#[tauri::command]
pub async fn team_vault_rename<R: Runtime>(
    app: AppHandle<R>,
    vault_id: Uuid,
    name: String,
) -> Result<()> {
    team::rename_vault(&app, vault_id, &name).await
}

#[tauri::command]
pub async fn team_vault_delete<R: Runtime>(app: AppHandle<R>, vault_id: Uuid) -> Result<()> {
    team::delete_vault(&app, vault_id).await
}

#[tauri::command]
pub async fn team_vault_set_access<R: Runtime>(
    app: AppHandle<R>,
    vault_id: Uuid,
    user_id: Uuid,
    role: VaultRole,
) -> Result<()> {
    team::set_vault_access(&app, vault_id, user_id, role).await
}

#[tauri::command]
pub async fn team_vault_remove_access<R: Runtime>(
    app: AppHandle<R>,
    vault_id: Uuid,
    user_id: Uuid,
) -> Result<()> {
    team::remove_vault_access(&app, vault_id, user_id).await
}

#[tauri::command]
pub async fn team_vault_rotate_key<R: Runtime>(app: AppHandle<R>, vault_id: Uuid) -> Result<()> {
    team::rotate_vault_key(&app, vault_id).await
}

// ───────────────────────────── multiplayer ─────────────────────────────

#[tauri::command]
pub async fn multiplayer_start<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<ShareInfo> {
    multiplayer::start(&app, id).await
}

#[tauri::command]
pub async fn multiplayer_stop<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<()> {
    multiplayer::stop(&app, id).await
}

#[tauri::command]
pub fn multiplayer_info(state: State<'_, AppState>, id: Uuid) -> Option<ShareInfo> {
    state.multiplayer.info(id)
}

#[tauri::command]
pub fn multiplayer_set_control<R: Runtime>(
    app: AppHandle<R>,
    id: Uuid,
    user_id: Uuid,
    enabled: bool,
) -> Result<()> {
    multiplayer::set_control(&app, id, user_id, enabled)
}

// ───────────────────────────── updates ─────────────────────────────

#[tauri::command]
pub async fn update_check<R: Runtime>(app: AppHandle<R>) -> Result<Option<UpdateInfo>> {
    update::check(&app).await
}

#[tauri::command]
pub async fn update_install<R: Runtime>(app: AppHandle<R>) -> Result<UpdateInfo> {
    update::install(&app).await
}

#[tauri::command]
pub fn update_restart<R: Runtime>(app: AppHandle<R>) {
    app.restart()
}
