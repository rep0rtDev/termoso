//! Data the terminal autocomplete needs from the session side: directory
//! listings for path completion and stored passwords typed into the
//! terminal on the user's explicit request.

use std::time::Duration;

use serde::Serialize;
use tauri::State;
use termoso_core::autocomplete::{list_local_dir, parse_ls, remote_ls_command};
use termoso_core::model::Identity;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

const LIST_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub dir: bool,
}

impl From<termoso_core::autocomplete::DirEntry> for DirEntry {
    fn from(e: termoso_core::autocomplete::DirEntry) -> Self {
        Self {
            name: e.name,
            dir: e.dir,
        }
    }
}

/// Entries of `path` as seen by the session: relative paths resolve against
/// `cwd` (reported by the shell integration), `~` against the user's home.
/// SSH sessions run `ls` on a separate channel, so the interactive shell and
/// its history never see it; local sessions read the directory directly.
#[tauri::command]
pub async fn terminal_list_dir(
    state: State<'_, AppState>,
    id: Uuid,
    cwd: Option<String>,
    path: String,
) -> Result<Vec<DirEntry>> {
    if path.contains('\0') || path.len() > 4096 {
        return Err(DesktopError::invalid("bad path"));
    }
    let info = state.sessions.info(id)?;
    match info.protocol.as_str() {
        "local" => Ok(list_local_dir(cwd.as_deref(), &path)
            .into_iter()
            .map(Into::into)
            .collect()),
        "ssh" => {
            let client = state.sessions.client(id)?;
            let cmd = remote_ls_command(cwd.as_deref(), &path);
            let out = tokio::time::timeout(LIST_TIMEOUT, client.exec(&cmd, None))
                .await
                .map_err(|_| DesktopError::invalid("listing timed out"))??;
            Ok(parse_ls(&out.stdout_str())
                .into_iter()
                .map(Into::into)
                .collect())
        }
        _ => Ok(Vec::new()),
    }
}

/// Type a stored password into the session, followed by Enter. Only ever
/// called from an explicit click / key in the autocomplete popup; the value
/// goes straight from the vault into the PTY and never crosses the IPC
/// boundary. `identity_id` = `None` means the identity the host connected
/// with.
#[tauri::command]
pub async fn terminal_insert_password(
    state: State<'_, AppState>,
    id: Uuid,
    identity_id: Option<Uuid>,
) -> Result<()> {
    let info = state.sessions.info(id)?;
    let password: Zeroizing<String> = match identity_id {
        Some(iid) => state
            .store()?
            .require::<Identity>(iid)?
            .data
            .password
            .filter(|p| !p.is_empty())
            .map(Zeroizing::new)
            .ok_or_else(|| DesktopError::invalid("identity has no password"))?,
        None => {
            let host_id = info
                .host_id
                .ok_or_else(|| DesktopError::invalid("session has no saved host"))?;
            state
                .store()?
                .resolve_host(host_id)?
                .identity
                .and_then(|i| i.data.password)
                .filter(|p| !p.is_empty())
                .map(Zeroizing::new)
                .ok_or_else(|| DesktopError::invalid("host has no stored password"))?
        }
    };
    let term = state.sessions.terminal(id)?;
    let mut line = Zeroizing::new(Vec::with_capacity(password.len() + 1));
    line.extend_from_slice(password.as_bytes());
    line.push(b'\r');
    term.write(&line).await?;
    Ok(())
}

/// Label of the identity a saved-host session authenticated with (so the UI
/// can offer "password of <label>" without seeing the password).
#[tauri::command]
pub fn terminal_host_identity(state: State<'_, AppState>, id: Uuid) -> Result<Option<String>> {
    let info = state.sessions.info(id)?;
    let Some(host_id) = info.host_id else {
        return Ok(None);
    };
    let resolved = state.store()?.resolve_host(host_id)?;
    Ok(resolved
        .identity
        .filter(|i| i.data.password.as_deref().is_some_and(|p| !p.is_empty()))
        .map(|i| {
            if i.data.label.trim().is_empty() {
                i.data.username.clone()
            } else {
                i.data.label.clone()
            }
        }))
}
