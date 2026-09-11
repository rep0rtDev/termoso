//! IPC surface. Every command is a thin adapter over `termoso-core`; the
//! webview never sees keys, tokens or plaintext secrets it did not enter.

use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime, State};
use termoso_core::model::AnyEntity;
use termoso_core::secrets::MasterKeySource;
use termoso_core::sftp::RemoteEntry;
use termoso_core::store::{ConnectionHistory, HistoryItem};
use termoso_core::store::{EntityFilter, LocalVault};
use termoso_core::terminal::TermSize;
use termoso_proto::entities::is_known_kind;
use uuid::Uuid;

use crate::account;
use crate::error::{DesktopError, Result};
use crate::hosts::{self, GroupNode, HostCard, HostForm, TagInfo};
use crate::prompts::PromptAnswer;
use crate::sessions::{self, OpenTarget, SessionInfo};
use crate::sftp::{self, Direction, Listing, SftpInfo, SftpTarget, TransferInfo};
use crate::state::{AppState, Settings};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub profile_dir: String,
    pub device_id: Uuid,
    pub master_key_source: MasterKeySource,
    pub signed_in: bool,
    pub platform: &'static str,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> Result<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        profile_dir: state.profile_dir.display().to_string(),
        device_id: state.store.device_id()?,
        master_key_source: state.master_source(),
        signed_in: state.store.account()?.is_some(),
        platform: std::env::consts::OS,
    })
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Result<Settings> {
    state.settings()
}

#[tauri::command]
pub async fn settings_set<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings> {
    let before = state.settings()?;
    state.save_settings(&settings)?;
    if before.sync_conflict != settings.sync_conflict
        || before.sync_interval_seconds != settings.sync_interval_seconds
        || before.upload_logs != settings.upload_logs
    {
        account::reconfigure(&app).await?;
    }
    Ok(settings)
}

#[tauri::command]
pub fn vaults_list(state: State<'_, AppState>) -> Result<Vec<LocalVault>> {
    Ok(state.store.vaults()?)
}

#[tauri::command]
pub fn vault_default(state: State<'_, AppState>) -> Result<LocalVault> {
    if let Some(p) = state.store.personal_vault()?
        && p.unlocked
    {
        return Ok(p);
    }
    Ok(state.store.local_vault()?)
}

fn check_kind(kind: &str) -> Result<()> {
    if is_known_kind(kind) {
        Ok(())
    } else {
        Err(DesktopError::invalid(format!("unknown entity kind {kind}")))
    }
}

/// List entities of `kind` (all unlocked vaults when `vault_id` is omitted).
#[tauri::command]
pub async fn entities_list(
    state: State<'_, AppState>,
    kind: String,
    vault_id: Option<Uuid>,
) -> Result<Vec<AnyEntity>> {
    check_kind(&kind)?;
    Ok(state.store.list_any(&EntityFilter {
        vault_id,
        kind: Some(kind),
        include_deleted: false,
    })?)
}

#[tauri::command]
pub async fn entity_get(state: State<'_, AppState>, id: Uuid) -> Result<Option<AnyEntity>> {
    Ok(state.store.get_any(id)?)
}

/// Create (no `id`) or replace (`id`) an entity. The payload must be a full
/// object of the kind's schema; Rust validates it by deserialising into the
/// typed model before writing.
#[tauri::command]
pub async fn entity_save(
    state: State<'_, AppState>,
    kind: String,
    vault_id: Uuid,
    id: Option<Uuid>,
    data: serde_json::Value,
) -> Result<AnyEntity> {
    check_kind(&kind)?;
    validate_payload(&kind, &data)?;
    let id = id.unwrap_or_else(Uuid::new_v4);
    state.store.put_raw(vault_id, &kind, id, &data)?;
    state
        .store
        .get_any(id)?
        .ok_or_else(|| DesktopError::not_found(format!("entity {id}")))
}

#[tauri::command]
pub async fn entity_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    state.store.delete(id)?;
    Ok(())
}

#[tauri::command]
pub async fn entity_move(state: State<'_, AppState>, id: Uuid, vault_id: Uuid) -> Result<Uuid> {
    Ok(state.store.move_to_vault(id, vault_id)?)
}

/// Reject payloads that do not match the typed schema so a UI bug cannot
/// write garbage into the vault.
fn validate_payload(kind: &str, data: &serde_json::Value) -> Result<()> {
    use termoso_core::model as m;
    macro_rules! check {
        ($($k:literal => $ty:ty),* $(,)?) => {
            match kind {
                $($k => { serde_json::from_value::<$ty>(data.clone())?; })*
                _ => {}
            }
        };
    }
    check! {
        "group" => m::Group,
        "host" => m::Host,
        "ssh_config" => m::SshConfig,
        "telnet_config" => m::TelnetConfig,
        "identity" => m::Identity,
        "ssh_key" => m::SshKey,
        "ssh_certificate" => m::SshCertificate,
        "known_host" => m::KnownHost,
        "snippet" => m::Snippet,
        "pf_rule" => m::PfRule,
        "proxy" => m::Proxy,
        "host_chain" => m::HostChain,
        "tag" => m::Tag,
        "tag_host" => m::TagHost,
    }
    Ok(())
}

// ───────────────────────────── hosts façade ─────────────────────────────

#[tauri::command]
pub async fn hosts_list(
    state: State<'_, AppState>,
    vault_id: Option<Uuid>,
) -> Result<Vec<HostCard>> {
    hosts::cards(&state.store, vault_id)
}

#[tauri::command]
pub async fn host_form(state: State<'_, AppState>, id: Uuid) -> Result<HostForm> {
    hosts::form(&state.store, id)
}

#[tauri::command]
pub async fn host_save(state: State<'_, AppState>, form: HostForm) -> Result<HostCard> {
    hosts::save(&state.store, &form)
}

#[tauri::command]
pub async fn host_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    hosts::delete(&state.store, id)
}

#[tauri::command]
pub async fn groups_list(
    state: State<'_, AppState>,
    vault_id: Option<Uuid>,
) -> Result<Vec<GroupNode>> {
    hosts::groups(&state.store, vault_id)
}

#[tauri::command]
pub async fn group_save(
    state: State<'_, AppState>,
    vault_id: Uuid,
    id: Option<Uuid>,
    label: String,
    parent_id: Option<Uuid>,
) -> Result<GroupNode> {
    hosts::save_group(&state.store, vault_id, id, &label, parent_id)
}

#[tauri::command]
pub async fn group_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    hosts::delete_group(&state.store, id)
}

#[tauri::command]
pub async fn tags_list(state: State<'_, AppState>, vault_id: Option<Uuid>) -> Result<Vec<TagInfo>> {
    hosts::tags(&state.store, vault_id)
}

#[tauri::command]
pub async fn history_connections(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<HistoryItem<ConnectionHistory>>> {
    Ok(state
        .store
        .connections(limit.unwrap_or(50).clamp(1, 1000))?)
}

#[tauri::command]
pub fn sessions_list(state: State<'_, AppState>) -> Vec<SessionInfo> {
    state.sessions.list()
}

/// Open a terminal session; output bytes stream through `output`.
#[tauri::command]
pub async fn terminal_open<R: Runtime>(
    app: AppHandle<R>,
    id: Option<Uuid>,
    target: OpenTarget,
    cols: u16,
    rows: u16,
    output: Channel<InvokeResponseBody>,
) -> Result<SessionInfo> {
    let size = TermSize {
        cols: cols.max(2),
        rows: rows.max(1),
    };
    sessions::open(app, id, target, size, output).await
}

/// Re-attach an output channel (after the webview reloaded).
#[tauri::command]
pub fn terminal_attach(
    state: State<'_, AppState>,
    id: Uuid,
    output: Channel<InvokeResponseBody>,
) -> Result<()> {
    state.sessions.attach(id, output)
}

#[tauri::command]
pub async fn terminal_write(state: State<'_, AppState>, id: Uuid, data: String) -> Result<()> {
    let term = state.sessions.terminal(id)?;
    term.write(data.as_bytes()).await?;
    Ok(())
}

#[tauri::command]
pub async fn terminal_resize(
    state: State<'_, AppState>,
    id: Uuid,
    cols: u16,
    rows: u16,
) -> Result<()> {
    let term = state.sessions.terminal(id)?;
    term.resize(TermSize {
        cols: cols.max(2),
        rows: rows.max(1),
    })
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn terminal_close<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<()> {
    sessions::close(&app, id).await
}

/// Answer a prompt raised by a connection attempt.
#[tauri::command]
pub fn prompt_answer(state: State<'_, AppState>, id: Uuid, answer: PromptAnswer) -> bool {
    state.prompts.answer(id, answer)
}

// ───────────────────────────── SFTP ─────────────────────────────

#[tauri::command]
pub fn sftp_sessions_list(state: State<'_, AppState>) -> Vec<SftpInfo> {
    state.sftp.list()
}

#[tauri::command]
pub async fn sftp_open<R: Runtime>(
    app: AppHandle<R>,
    id: Option<Uuid>,
    target: SftpTarget,
) -> Result<SftpInfo> {
    sftp::open(app, id, target).await
}

#[tauri::command]
pub async fn sftp_close<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<()> {
    sftp::close(&app, id).await
}

#[tauri::command]
pub async fn sftp_list(
    state: State<'_, AppState>,
    id: Uuid,
    path: Option<String>,
) -> Result<Listing> {
    sftp::remote_list(&state, id, path).await
}

#[tauri::command]
pub async fn sftp_stat(state: State<'_, AppState>, id: Uuid, path: String) -> Result<RemoteEntry> {
    sftp::remote_stat(&state, id, path).await
}

#[tauri::command]
pub async fn sftp_mkdir(state: State<'_, AppState>, id: Uuid, path: String) -> Result<()> {
    sftp::remote_mkdir(&state, id, path).await
}

#[tauri::command]
pub async fn sftp_rename(
    state: State<'_, AppState>,
    id: Uuid,
    from: String,
    to: String,
) -> Result<()> {
    sftp::remote_rename(&state, id, from, to).await
}

#[tauri::command]
pub async fn sftp_remove(
    state: State<'_, AppState>,
    id: Uuid,
    path: String,
    recursive: bool,
) -> Result<()> {
    sftp::remote_remove(&state, id, path, recursive).await
}

#[tauri::command]
pub async fn sftp_chmod(
    state: State<'_, AppState>,
    id: Uuid,
    path: String,
    mode: u32,
) -> Result<()> {
    sftp::remote_chmod(&state, id, path, mode).await
}

#[tauri::command]
pub fn local_home() -> String {
    sftp::local_home()
}

#[tauri::command]
pub async fn local_list(path: Option<String>) -> Result<Listing> {
    sftp::local_list(path).await
}

#[tauri::command]
pub async fn local_stat(path: String) -> Result<RemoteEntry> {
    sftp::local_stat(path).await
}

#[tauri::command]
pub async fn local_mkdir(path: String) -> Result<()> {
    sftp::local_mkdir(path).await
}

#[tauri::command]
pub async fn local_rename(from: String, to: String) -> Result<()> {
    sftp::local_rename(from, to).await
}

#[tauri::command]
pub async fn local_remove(path: String, recursive: bool) -> Result<()> {
    sftp::local_remove(path, recursive).await
}

/// Start an upload or download (files or whole directories); progress and
/// completion arrive as `transfer` events.
#[tauri::command]
pub fn transfer_start<R: Runtime>(
    app: AppHandle<R>,
    sftp_id: Uuid,
    direction: Direction,
    local: String,
    remote: String,
    resume: Option<bool>,
) -> Result<TransferInfo> {
    sftp::transfer_start(
        app,
        sftp_id,
        direction,
        local,
        remote,
        resume.unwrap_or(false),
    )
}

#[tauri::command]
pub fn transfer_cancel(state: State<'_, AppState>, id: Uuid) -> bool {
    state.sftp.cancel_transfer(id)
}
