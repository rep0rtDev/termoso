//! IPC surface for keychain, port forwarding, snippets, known hosts, logs
//! and account/sync. Thin adapters; the façades own validation and secrets.

use std::collections::HashMap;

use tauri::{AppHandle, Runtime, State};
use termoso_core::secrets::MasterKeySource;
use termoso_proto::account::ServerInfo;
use termoso_proto::auth::{Device, MfaCredential};
use termoso_proto::vault::VaultMember;
use uuid::Uuid;

use crate::account::{
    self, AccountStatus, LoginForm, LoginOutcome, RegisterForm, Registered, SyncStatus,
};
use crate::error::Result;
use crate::forwarding::{self, PfRuleCard, PfRuleForm, PfRuntime};
use crate::import::{self, ImportPreview, ImportSelection, ImportSource};
use crate::keychain::{
    self, ExportOutcome, GenerateForm, IdentityCard, IdentityForm, ImportForm, KeyCard,
};
use crate::logs::{self, BookmarkCard, LogBody, LogCard};
use crate::sessions;
use crate::snippets::{self, PackageNode, RunResult, SnippetCard, SnippetForm};
use crate::state::AppState;
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

#[tauri::command]
pub async fn key_import_file(
    state: State<'_, AppState>,
    vault_id: Uuid,
    label: String,
    path: String,
    passphrase: Option<String>,
    remember_passphrase: bool,
) -> Result<KeyCard> {
    let private_key = std::fs::read_to_string(&path)?;
    keychain::import(
        &state.store,
        &ImportForm {
            vault_id,
            label,
            private_key,
            passphrase,
            remember_passphrase,
        },
    )
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
pub async fn account_vault_members<R: Runtime>(
    app: AppHandle<R>,
    vault_id: Uuid,
) -> Result<Vec<VaultMember>> {
    account::vault_members(&app, vault_id).await
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
