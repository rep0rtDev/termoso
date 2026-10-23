//! Snippets on the desktop: the store logic lives in
//! [`termoso_client::snippets`]; this module only types expanded scripts
//! into live sessions.

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use termoso_core::model::Snippet;
use uuid::Uuid;

pub use termoso_client::snippets::*;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// Sessions the script was typed into.
    pub session_ids: Vec<Uuid>,
    /// The snippet asks for its terminal to be closed once the command ran.
    pub close_after_run: bool,
}

/// Type the expanded script into each session. `paste` leaves the text on
/// the command line (no trailing newline) so the user can edit it first.
pub async fn run(
    state: &AppState,
    snippet_id: Uuid,
    session_ids: &[Uuid],
    vars: &HashMap<String, String>,
    paste: bool,
) -> Result<RunResult> {
    let snippet = state.store()?.require::<Snippet>(snippet_id)?;
    let expanded = expand(&snippet.data.script, vars)?;
    let text = if paste {
        script_to_paste(&expanded)
    } else {
        script_to_send(&expanded)
    };
    if session_ids.is_empty() {
        return Err(DesktopError::invalid("pick at least one session"));
    }
    let mut done = Vec::new();
    for &sid in session_ids {
        let term = state.sessions.terminal(sid)?;
        term.write(text.as_bytes()).await?;
        done.push(sid);
    }
    Ok(RunResult {
        session_ids: done,
        close_after_run: snippet.data.close_after_run && !paste,
    })
}

/// Upper bound per host for the quiet run: connect + command.
const QUIET_TIMEOUT: Duration = Duration::from_secs(120);
/// Output kept per stream per host; the rest is dropped with a marker.
const QUIET_OUTPUT_CAP: usize = 64 * 1024;
/// Hosts worked on at the same time.
const QUIET_PARALLEL: usize = 8;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuietHostResult {
    pub host_id: Uuid,
    pub label: String,
    /// `user@host:port` the command ran on (empty when connecting failed).
    pub target: String,
    pub exit_code: Option<u32>,
    pub stdout: String,
    pub stderr: String,
    /// Connection or transport error; the command never ran.
    pub error: Option<String>,
    pub ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuietRun {
    pub run_id: Uuid,
    pub command: String,
    pub results: Vec<QuietHostResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuietProgress {
    run_id: Uuid,
    result: QuietHostResult,
}

/// Event with one [`QuietHostResult`] as each host finishes.
pub const QUIET_EVENT: &str = "snippet-quiet-progress";

fn cap(bytes: &[u8]) -> String {
    if bytes.len() <= QUIET_OUTPUT_CAP {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let mut s = String::from_utf8_lossy(&bytes[..QUIET_OUTPUT_CAP]).into_owned();
    s.push_str(&format!(
        "\n… [{} more bytes dropped]",
        bytes.len() - QUIET_OUTPUT_CAP
    ));
    s
}

/// Run the expanded script on every host over a fresh `exec` channel —
/// no terminal is opened — and collect stdout / stderr / exit code per
/// host. Prompts (password, host key) go through the usual dialogs under a
/// throw-away session id. Hosts run [`QUIET_PARALLEL`] at a time; each
/// result is also emitted as [`QUIET_EVENT`] so the UI can fill in live.
pub async fn run_quiet<R: Runtime>(
    app: &AppHandle<R>,
    snippet_id: Uuid,
    host_ids: &[Uuid],
    vars: &HashMap<String, String>,
) -> Result<QuietRun> {
    let state = app.state::<AppState>();
    let snippet = state.store()?.require::<Snippet>(snippet_id)?;
    let command = expand(&snippet.data.script, vars)?;
    if host_ids.is_empty() {
        return Err(DesktopError::invalid("pick at least one host"));
    }
    let run_id = Uuid::new_v4();
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(QUIET_PARALLEL));
    let mut handles = Vec::new();
    for &host_id in host_ids {
        let app = app.clone();
        let command = command.clone();
        let sem = sem.clone();
        handles.push(tauri::async_runtime::spawn(async move {
            let _permit = sem.acquire().await;
            let started = std::time::Instant::now();
            let mut result = QuietHostResult {
                host_id,
                label: String::new(),
                target: String::new(),
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: None,
                ms: 0,
            };
            if let Some(label) = app
                .state::<AppState>()
                .store()
                .ok()
                .and_then(|s| s.require::<termoso_core::model::Host>(host_id).ok())
                .map(|h| h.data.label.clone())
            {
                result.label = label;
            }
            let work = async {
                let conn =
                    crate::sessions::connect_host(&app, Uuid::new_v4(), host_id, None).await?;
                let out = conn.client.exec(&command, None).await;
                let (label, target) = (conn.label.clone(), conn.display.clone());
                conn.close().await;
                Ok::<_, DesktopError>((label, target, out?))
            };
            match tokio::time::timeout(QUIET_TIMEOUT, work).await {
                Ok(Ok((label, target, out))) => {
                    result.label = label;
                    result.target = target;
                    result.exit_code = out.exit_code;
                    result.stdout = cap(&out.stdout);
                    result.stderr = cap(&out.stderr);
                }
                Ok(Err(e)) => result.error = Some(e.to_string()),
                Err(_) => result.error = Some("timed out".into()),
            }
            result.ms = started.elapsed().as_millis() as u64;
            let _ = app.emit(
                QUIET_EVENT,
                QuietProgress {
                    run_id,
                    result: result.clone(),
                },
            );
            result
        }));
    }
    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        match h.await {
            Ok(r) => results.push(r),
            Err(e) => return Err(DesktopError::invalid(format!("quiet run task failed: {e}"))),
        }
    }
    Ok(QuietRun {
        run_id,
        command,
        results,
    })
}
