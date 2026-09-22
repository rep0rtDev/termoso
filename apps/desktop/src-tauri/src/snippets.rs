//! Snippets on the desktop: the store logic lives in
//! [`termoso_client::snippets`]; this module only types expanded scripts
//! into live sessions.

use std::collections::HashMap;

use serde::Serialize;
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
