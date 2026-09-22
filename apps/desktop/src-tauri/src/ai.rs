//! "Ask AI": turn a short request into one shell command via the account
//! server's opt-in proxy.
//!
//! Context is derived here, not in the webview, so what leaves the machine
//! is fixed: the request text plus two coarse labels (OS identifier as
//! detected for the host's icon, shell base name). Never the buffer, the
//! host address, credentials or anything else about the session. The answer
//! is text for the user to read and paste; nothing here executes it.

use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime};
use termoso_core::model::Host;
use termoso_proto::ai::{AiCommandRequest, AiCommandResponse, AiStatus};
use uuid::Uuid;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskForm {
    pub prompt: String,
    /// Terminal session the suggestion is for; supplies the OS/shell labels.
    pub session_id: Option<Uuid>,
}

/// Provider on offer, opt-in state and today's quota.
pub async fn status<R: Runtime>(app: &AppHandle<R>) -> Result<AiStatus> {
    let api = crate::account::api(app).await?;
    Ok(api.ai_status().await?)
}

/// Opt in to (or out of) suggestions for this account.
pub async fn set_enabled<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<AiStatus> {
    let api = crate::account::api(app).await?;
    Ok(api.set_ai_enabled(enabled).await?)
}

/// Ask for one command. Returns the suggestion for display only.
pub async fn ask<R: Runtime>(app: &AppHandle<R>, form: AskForm) -> Result<AiCommandResponse> {
    let prompt = form.prompt.trim();
    if prompt.is_empty() {
        return Err(DesktopError::invalid("describe what the command should do"));
    }
    let (os, shell) = match form.session_id {
        Some(id) => context(&app.state::<AppState>(), id),
        None => (None, None),
    };
    let api = crate::account::api(app).await?;
    Ok(api
        .ai_command(&AiCommandRequest {
            prompt: prompt.to_string(),
            os,
            shell,
        })
        .await?)
}

/// `(os, shell)` labels for a session: the local platform for local shells,
/// the detected `os_name` identifier for hosts (none when unknown).
fn context(state: &AppState, session_id: Uuid) -> (Option<String>, Option<String>) {
    let Ok(info) = state.sessions.info(session_id) else {
        return (None, None);
    };
    let os = if info.protocol == "local" {
        Some(local_os().to_string())
    } else {
        info.host_id
            .and_then(|id| state.store().ok()?.require::<Host>(id).ok())
            .and_then(|h| h.data.os_name)
    };
    (os, info.shell)
}

fn local_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "osx",
        other => other,
    }
}
