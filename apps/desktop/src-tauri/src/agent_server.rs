//! Termoso as an SSH agent: serves the vault's private keys to local tools
//! (`ssh`, `git`, IDEs) over a socket / named pipe, confirming each
//! signature in the UI when the user asks for that. Private keys stay in
//! this process; clients only ever see public halves and signatures.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use termoso_core::agent_server::{AgentPolicy, AgentServer, Peer, ServedKey};
use termoso_core::model::SshKey;
use termoso_core::ssh::load_private_key;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::Result;
use crate::prompts::{PromptAnswer, PromptRequest};
use crate::state::AppState;

#[cfg(unix)]
const SOCKET_FILE: &str = "agent.sock";
#[cfg(windows)]
const PIPE_NAME: &str = r"\\.\pipe\termoso-ssh-agent";

/// Running agent plus what it could not load, for the settings page.
#[derive(Default)]
pub struct AgentRuntime {
    inner: Mutex<Option<Running>>,
}

struct Running {
    server: AgentServer,
    skipped: Vec<SkippedKey>,
}

/// A key the agent offers (public data only).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentServedKey {
    pub label: String,
    pub key_type: String,
    pub fingerprint: String,
}

/// A vault key the agent cannot serve, and why.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedKey {
    pub label: String,
    pub reason: String,
}

/// What the settings page shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentServerStatus {
    pub running: bool,
    /// Socket path (Unix) or pipe name (Windows) to point `SSH_AUTH_SOCK` at.
    pub endpoint: Option<String>,
    /// Shell line that makes the current terminal use this agent.
    pub export_hint: Option<String>,
    pub keys: Vec<AgentServedKey>,
    pub skipped: Vec<SkippedKey>,
}

/// Asks the user before every signature when `agent_confirm` is on.
struct UiPolicy<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AgentPolicy for UiPolicy<R> {
    fn allow_sign(
        &self,
        key: &ServedKey,
        peer: &Peer,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        let (label, fingerprint, peer) = (key.label.clone(), key.fingerprint(), peer.clone());
        Box::pin(async move {
            let state = self.app.state::<AppState>();
            let confirm = state.settings().map(|s| s.agent_confirm).unwrap_or(true);
            if !confirm {
                tracing::debug!(key = %label, ?peer, "agent signature");
                return true;
            }
            let process = match (&peer.process, peer.pid) {
                (Some(p), Some(pid)) => Some(format!("{p} (pid {pid})")),
                (Some(p), None) => Some(p.clone()),
                (None, Some(pid)) => Some(format!("pid {pid}")),
                (None, None) => None,
            };
            let answer = state
                .prompts
                .ask(
                    &self.app,
                    Uuid::new_v4(),
                    label.clone(),
                    PromptRequest::AgentSign {
                        key_label: label.clone(),
                        fingerprint,
                        process,
                    },
                )
                .await;
            let ok = matches!(answer, Some(PromptAnswer::Confirm));
            tracing::info!(key = %label, ?peer, allowed = ok, "agent signature request");
            ok
        })
    }
}

/// Bring the agent in line with settings and the vault: start it when
/// `serve_ssh_agent` is on and the vault is unlocked, reload its keys,
/// stop it otherwise. Cheap enough to call after any keychain change.
pub async fn reconcile<R: Runtime>(app: &AppHandle<R>) -> Result<AgentServerStatus> {
    let state = app.state::<AppState>();
    let wanted = state
        .store()
        .ok()
        .and_then(|_| state.settings().ok())
        .map(|s| s.serve_ssh_agent)
        .unwrap_or(false);
    let mut guard = state.agent.inner.lock().await;
    if !wanted {
        if guard.take().is_some() {
            tracing::info!("ssh-agent stopped");
        }
        return Ok(status_of(None).await);
    }
    if guard.is_none() {
        let policy: Arc<dyn AgentPolicy> = Arc::new(UiPolicy { app: app.clone() });
        let server = bind(&state, policy).await?;
        tracing::info!(endpoint = server.endpoint(), "ssh-agent started");
        *guard = Some(Running {
            server,
            skipped: Vec::new(),
        });
    }
    let (keys, skipped) = load_keys(&state)?;
    let running = guard.as_mut().expect("agent just started");
    running.server.set_keys(keys).await;
    running.skipped = skipped;
    Ok(status_of(guard.as_ref()).await)
}

/// Drop the agent (vault locking, app shutdown).
pub async fn stop<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    if state.agent.inner.lock().await.take().is_some() {
        tracing::info!("ssh-agent stopped");
    }
}

pub async fn status<R: Runtime>(app: &AppHandle<R>) -> AgentServerStatus {
    let state = app.state::<AppState>();
    let guard = state.agent.inner.lock().await;
    status_of(guard.as_ref()).await
}

async fn bind(state: &AppState, policy: Arc<dyn AgentPolicy>) -> Result<AgentServer> {
    #[cfg(unix)]
    {
        Ok(AgentServer::bind_unix(&state.profile_dir.join(SOCKET_FILE), policy).await?)
    }
    #[cfg(windows)]
    {
        let _ = &state.profile_dir;
        Ok(AgentServer::bind_pipe(PIPE_NAME, policy).await?)
    }
}

/// Every private key in the vault we can decrypt right now. Keys that live
/// on a token (`sk-*`, FIDO2), agent-backed public-only keys and encrypted
/// keys without a remembered passphrase are reported, not served.
fn load_keys(state: &AppState) -> Result<(Vec<ServedKey>, Vec<SkippedKey>)> {
    let store = state.store()?;
    let mut keys = Vec::new();
    let mut skipped = Vec::new();
    let mut entries = store.list::<SshKey>(None)?;
    entries.sort_by_key(|e| e.data.label.to_lowercase());
    for e in entries {
        let k = &e.data;
        if k.ssh_id {
            continue;
        }
        if k.fido2_credential_id.is_some() || k.key_type.starts_with("sk-") {
            skipped.push(SkippedKey {
                label: k.label.clone(),
                reason: "security key: signs only inside Termoso".into(),
            });
            continue;
        }
        if k.private_key.trim().is_empty() {
            skipped.push(SkippedKey {
                label: k.label.clone(),
                reason: "backed by another agent, no private key in the vault".into(),
            });
            continue;
        }
        match load_private_key(&k.private_key, k.passphrase.as_deref()) {
            Ok(pk) => keys.push(ServedKey::new(k.label.clone(), pk)),
            Err(_) if k.passphrase.is_none() => skipped.push(SkippedKey {
                label: k.label.clone(),
                reason: "encrypted: remember its passphrase in the keychain to serve it".into(),
            }),
            Err(e) => skipped.push(SkippedKey {
                label: k.label.clone(),
                reason: e.to_string(),
            }),
        }
    }
    Ok((keys, skipped))
}

async fn status_of(running: Option<&Running>) -> AgentServerStatus {
    let Some(r) = running else {
        return AgentServerStatus {
            running: false,
            endpoint: None,
            export_hint: None,
            keys: Vec::new(),
            skipped: Vec::new(),
        };
    };
    let endpoint = r.server.endpoint().to_string();
    let keys = r
        .server
        .keys()
        .await
        .into_iter()
        .map(|k| AgentServedKey {
            label: k.label.clone(),
            key_type: k.public().algorithm().to_string(),
            fingerprint: k.fingerprint(),
        })
        .collect();
    AgentServerStatus {
        running: true,
        export_hint: Some(export_hint(&endpoint)),
        endpoint: Some(endpoint),
        keys,
        skipped: r.skipped.clone(),
    }
}

fn export_hint(endpoint: &str) -> String {
    if cfg!(windows) {
        format!("$env:SSH_AUTH_SOCK = '{endpoint}'")
    } else {
        format!("export SSH_AUTH_SOCK=\"{endpoint}\"")
    }
}

#[tauri::command]
pub async fn agent_server_status<R: Runtime>(app: AppHandle<R>) -> Result<AgentServerStatus> {
    Ok(status(&app).await)
}

/// Re-read settings and the keychain; also how the UI applies the toggle.
#[tauri::command]
pub async fn agent_server_refresh<R: Runtime>(app: AppHandle<R>) -> Result<AgentServerStatus> {
    reconcile(&app).await
}
