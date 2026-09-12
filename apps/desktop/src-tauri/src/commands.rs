//! IPC surface. Every command is a thin adapter over `termoso-core`; the
//! webview never sees keys, tokens or plaintext secrets it did not enter.

use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_deep_link::DeepLinkExt;
use termoso_core::model::AnyEntity;
use termoso_core::secrets::MasterKeySource;
use termoso_core::sftp::RemoteEntry;
use termoso_core::store::{CommandHistory, ConnectionHistory, HistoryItem};
use termoso_core::store::{EntityFilter, LocalVault};
use termoso_core::terminal::TermSize;
use termoso_proto::entities::is_known_kind;
use uuid::Uuid;

use crate::account;
use crate::edits::{self, EditInfo};
use crate::error::{DesktopError, Result};
use crate::hosts::{self, GroupForm, GroupNode, HostCard, HostForm, Inherited, TagInfo};
use crate::prompts::PromptAnswer;
use crate::sessions::{self, OpenTarget, SessionInfo};
use crate::sftp::{self, Conflict, Direction, Listing, SftpInfo, SftpTarget, TransferInfo};
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

/// Register this binary as the handler for the `termoso://`, `ssh://` and
/// `telnet://` schemes for the current user. Installers do this already; the command
/// covers AppImage / portable builds. Returns the schemes now registered.
#[tauri::command]
pub fn deep_links_register<R: Runtime>(app: AppHandle<R>) -> Result<Vec<String>> {
    let links = app.deep_link();
    links
        .register_all()
        .map_err(|e| DesktopError::new("deep_link", e.to_string()))?;
    Ok(["termoso", "ssh", "telnet"]
        .into_iter()
        .filter(|s| links.is_registered(s).unwrap_or(false))
        .map(str::to_string)
        .collect())
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
pub async fn hosts_delete(state: State<'_, AppState>, ids: Vec<Uuid>) -> Result<()> {
    for id in ids {
        hosts::delete(&state.store, id)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn host_duplicate(state: State<'_, AppState>, id: Uuid) -> Result<HostCard> {
    hosts::duplicate(&state.store, id)
}

#[tauri::command]
pub async fn hosts_move(
    state: State<'_, AppState>,
    ids: Vec<Uuid>,
    group_id: Option<Uuid>,
) -> Result<()> {
    hosts::move_hosts(&state.store, &ids, group_id)
}

#[tauri::command]
pub async fn hosts_copy_to_vault(
    state: State<'_, AppState>,
    ids: Vec<Uuid>,
    vault_id: Uuid,
    move_hosts: bool,
) -> Result<Vec<Uuid>> {
    if move_hosts {
        hosts::move_to_vault(&state.store, &ids, vault_id)
    } else {
        hosts::copy_to_vault(&state.store, &ids, vault_id)
    }
}

/// Serial devices on this machine; enumeration is local only.
#[tauri::command]
pub async fn serial_ports() -> Result<Vec<termoso_core::serial::PortInfo>> {
    tokio::task::spawn_blocking(termoso_core::serial::available_ports)
        .await
        .map_err(|e| DesktopError::new("internal", e.to_string()))
}

/// Shells installed on this machine for the "Local terminal" setting: the
/// login shell first, then `/etc/shells` (Unix) or PowerShell / cmd / WSL
/// distributions (Windows). Enumeration is local only.
#[tauri::command]
pub async fn local_shells() -> Result<Vec<String>> {
    tokio::task::spawn_blocking(sessions::local_shells)
        .await
        .map_err(|e| DesktopError::new("internal", e.to_string()))
}

#[tauri::command]
pub async fn host_inherited(
    state: State<'_, AppState>,
    group_id: Option<Uuid>,
) -> Result<Inherited> {
    hosts::inherited(&state.store, group_id)
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
pub async fn group_form(state: State<'_, AppState>, id: Uuid) -> Result<GroupForm> {
    hosts::group_form(&state.store, id)
}

#[tauri::command]
pub async fn group_save_form(state: State<'_, AppState>, form: GroupForm) -> Result<GroupNode> {
    hosts::save_group_form(&state.store, &form)
}

#[tauri::command]
pub async fn group_duplicate(state: State<'_, AppState>, id: Uuid) -> Result<GroupNode> {
    hosts::duplicate_group(&state.store, id)
}

#[tauri::command]
pub async fn group_delete(
    state: State<'_, AppState>,
    id: Uuid,
    recursive: Option<bool>,
) -> Result<()> {
    if recursive.unwrap_or(false) {
        hosts::delete_group_recursive(&state.store, id)
    } else {
        hosts::delete_group(&state.store, id)
    }
}

#[tauri::command]
pub async fn tags_list(state: State<'_, AppState>, vault_id: Option<Uuid>) -> Result<Vec<TagInfo>> {
    hosts::tags(&state.store, vault_id)
}

#[tauri::command]
pub async fn tag_update(
    state: State<'_, AppState>,
    id: Uuid,
    label: String,
    color: Option<String>,
) -> Result<TagInfo> {
    hosts::tag_update(&state.store, id, label, color)
}

#[tauri::command]
pub async fn tag_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    hosts::tag_delete(&state.store, id)
}

#[tauri::command]
pub async fn tags_merge(
    state: State<'_, AppState>,
    sources: Vec<Uuid>,
    target: Uuid,
) -> Result<TagInfo> {
    hosts::tags_merge(&state.store, &sources, target)
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

/// Record a command line the user ran in a session. Called by the terminal
/// when the shell's OSC 133 markers delimit a finished command; only the
/// command text is stored (encrypted at rest), never the output.
#[tauri::command]
pub async fn history_record_command(
    state: State<'_, AppState>,
    host_id: Option<Uuid>,
    command: String,
) -> Result<Option<Uuid>> {
    let command = command.trim();
    if command.is_empty() || command.len() > 4096 {
        return Ok(None);
    }
    Ok(Some(state.store.record_command(&CommandHistory {
        host_id,
        command: command.to_string(),
    })?))
}

#[tauri::command]
pub async fn history_commands(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<HistoryItem<CommandHistory>>> {
    Ok(state.store.commands(limit.unwrap_or(500).clamp(1, 5000))?)
}

#[tauri::command]
pub async fn history_delete(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    Ok(state.store.delete_history(id)?)
}

#[tauri::command]
pub async fn history_clear_commands(state: State<'_, AppState>) -> Result<()> {
    Ok(state
        .store
        .clear_history(Some(termoso_proto::sync::HistoryKind::Command))?)
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

/// Open a local file with the default application or `with`.
#[tauri::command]
pub fn local_open(path: String, with: Option<String>) -> Result<()> {
    let with = with.map(|w| w.trim().to_string()).filter(|w| !w.is_empty());
    edits::open_local(std::path::Path::new(&path), with.as_deref())
}

/// What already sits at the destination of a would-be transfer, if anything.
#[tauri::command]
pub async fn transfer_probe(
    state: State<'_, AppState>,
    sftp_id: Uuid,
    direction: Direction,
    local: String,
    remote: String,
) -> Result<Option<RemoteEntry>> {
    sftp::transfer_probe(&state, sftp_id, direction, local, remote).await
}

/// Start an upload or download (files or whole directories); progress and
/// completion arrive as `transfer` events. `temp` marks uploads from the
/// drop staging area, which is cleaned up afterwards.
#[tauri::command]
pub fn transfer_start<R: Runtime>(
    app: AppHandle<R>,
    sftp_id: Uuid,
    direction: Direction,
    local: String,
    remote: String,
    conflict: Option<Conflict>,
    temp: Option<bool>,
) -> Result<TransferInfo> {
    sftp::transfer_start(
        app,
        sftp_id,
        direction,
        local,
        remote,
        conflict.unwrap_or_default(),
        temp.unwrap_or(false),
    )
}

#[tauri::command]
pub async fn drop_begin() -> Result<String> {
    sftp::drop_begin().await
}

/// Raw-body command: the chunk is the request body, the target comes in
/// base64 headers (`x-drop-dir`, `x-drop-path`, `x-drop-append`).
#[tauri::command]
pub async fn drop_write(request: tauri::ipc::Request<'_>) -> Result<()> {
    use base64::Engine;
    let header = |name: &str| -> Result<String> {
        let raw = request
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| DesktopError::invalid(format!("missing {name}")))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw)
            .map_err(|_| DesktopError::invalid(format!("bad {name}")))?;
        String::from_utf8(bytes).map_err(|_| DesktopError::invalid(format!("bad {name}")))
    };
    let dir = header("x-drop-dir")?;
    let rel = header("x-drop-path")?;
    let append = request
        .headers()
        .get("x-drop-append")
        .is_some_and(|v| v == "1");
    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(b) => b.clone(),
        tauri::ipc::InvokeBody::Json(_) => {
            return Err(DesktopError::invalid("drop_write expects a raw body"));
        }
    };
    sftp::drop_write(dir, rel, bytes, append).await
}

#[tauri::command]
pub async fn drop_mkdir(dir: String, rel: String) -> Result<()> {
    sftp::drop_mkdir(dir, rel).await
}

#[tauri::command]
pub async fn drop_abort(dir: String) -> Result<()> {
    sftp::drop_abort(dir).await
}

// ───────────────────────────── edit in place ─────────────────────────────

#[tauri::command]
pub fn edits_list(state: State<'_, AppState>) -> Vec<EditInfo> {
    state.edits.list()
}

/// Download a remote file to a private temp dir, open it locally and upload
/// every save back; progress arrives as `sftp_edit` events.
#[tauri::command]
pub async fn edit_open<R: Runtime>(
    app: AppHandle<R>,
    sftp_id: Uuid,
    remote: String,
    with: Option<String>,
) -> Result<EditInfo> {
    edits::open(app, sftp_id, remote, with).await
}

#[tauri::command]
pub fn edit_upload_now(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    edits::upload_now(&state, id)
}

#[tauri::command]
pub async fn edit_close<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<()> {
    edits::close(&app, id).await
}

#[tauri::command]
pub fn transfer_cancel(state: State<'_, AppState>, id: Uuid) -> bool {
    state.sftp.cancel_transfer(id)
}
