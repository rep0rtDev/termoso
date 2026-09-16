//! CI smoke hook, debug builds only. With `TERMOSO_SMOKE_SCRIPT=<file>` set,
//! the file is evaluated in the main webview once its page has loaded; the
//! script drives the UI through the DOM and reports through `smoke_report`,
//! which appends one line per call to `TERMOSO_SMOKE_REPORT`. Release builds
//! ignore both variables and reject the command, so a shipped binary carries
//! no way to inject script.

use std::io::Write;

use tauri::webview::{PageLoadEvent, PageLoadPayload};
use tauri::{Runtime, Webview};

use crate::error::{DesktopError, Result};

pub const SCRIPT_ENV: &str = "TERMOSO_SMOKE_SCRIPT";
pub const REPORT_ENV: &str = "TERMOSO_SMOKE_REPORT";

fn enabled() -> bool {
    cfg!(debug_assertions)
}

/// Builder `on_page_load` handler: inject the smoke script into `main`.
pub fn on_page_load<R: Runtime>(webview: &Webview<R>, payload: &PageLoadPayload<'_>) {
    if !enabled() || payload.event() != PageLoadEvent::Finished || webview.label() != "main" {
        return;
    }
    let Some(path) = std::env::var_os(SCRIPT_ENV) else {
        return;
    };
    let js = match std::fs::read_to_string(&path) {
        Ok(js) => js,
        Err(e) => {
            tracing::error!(path = %path.to_string_lossy(), "cannot read smoke script: {e}");
            return;
        }
    };
    tracing::info!(path = %path.to_string_lossy(), bytes = js.len(), "smoke script injected");
    if let Err(e) = webview.eval(js) {
        tracing::error!("smoke script eval failed: {e}");
    }
}

#[tauri::command]
pub fn smoke_report(line: String) -> Result<()> {
    if !enabled() {
        return Err(DesktopError::forbidden(
            "smoke reporting is a debug-build feature",
        ));
    }
    let Some(path) = std::env::var_os(REPORT_ENV) else {
        return Err(DesktopError::invalid(format!("{REPORT_ENV} is not set")));
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", line.replace(['\n', '\r'], " "))?;
    Ok(())
}
