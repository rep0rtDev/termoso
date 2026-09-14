//! In-app updates. Manifests and packages are minisign-verified against the
//! public key baked into the binary; the feed URL is user-configurable so a
//! self-hosted server can serve it. Nothing is fetched unless the user asks
//! for it or explicitly enables the startup check.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

pub const UPDATE_EVENT: &str = "update";

/// Release feed used when `Settings::update_url` is empty.
pub const DEFAULT_FEED: &str =
    "https://github.com/rep0rtDev/termoso/releases/latest/download/latest.json";

/// Pending update found by the last check; kept in Rust so the webview only
/// ever sees metadata.
#[derive(Default)]
pub struct UpdateHub {
    pending: Mutex<Option<Update>>,
    busy: Mutex<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub download_url: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateEvent {
    Available { info: UpdateInfo },
    Progress { downloaded: u64, total: Option<u64> },
    Installed { version: String },
    Failed { message: String },
}

fn updater_err(e: tauri_plugin_updater::Error) -> DesktopError {
    DesktopError::new("update", e.to_string())
}

fn info(u: &Update) -> UpdateInfo {
    UpdateInfo {
        current_version: u.current_version.clone(),
        version: u.version.clone(),
        notes: u.body.clone(),
        published_at: u.date.and_then(|d| {
            d.format(&time::format_description::well_known::Rfc3339)
                .ok()
        }),
        download_url: u.download_url.to_string(),
        target: u.target.clone(),
    }
}

/// Resolve the feed URL: user setting, else the project default. Only
/// `https://` is accepted; the plugin enforces the same in release builds.
pub fn feed_url(raw: &str) -> Result<Url> {
    let raw = raw.trim();
    let url = Url::parse(if raw.is_empty() { DEFAULT_FEED } else { raw })
        .map_err(|e| DesktopError::invalid(format!("updateUrl: {e}")))?;
    if url.scheme() != "https" {
        return Err(DesktopError::invalid("updateUrl must use https"));
    }
    Ok(url)
}

fn emit<R: Runtime>(app: &AppHandle<R>, ev: UpdateEvent) {
    let _ = app.emit(UPDATE_EVENT, ev);
}

/// Contact the configured feed once and remember the result.
pub async fn check<R: Runtime>(app: &AppHandle<R>) -> Result<Option<UpdateInfo>> {
    let settings = app.state::<AppState>().settings()?;
    let url = feed_url(&settings.update_url)?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(updater_err)?
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(updater_err)?;
    let found = updater.check().await.map_err(updater_err)?;
    let hub = app.state::<UpdateHub>();
    let mut pending = hub.pending.lock().map_err(|_| poisoned())?;
    let out = found.as_ref().map(info);
    *pending = found;
    Ok(out)
}

/// Download, verify and install the update found by the last [`check`].
/// Emits progress; the caller decides when to relaunch.
pub async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<UpdateInfo> {
    let hub = app.state::<UpdateHub>();
    let update = {
        let mut busy = hub.busy.lock().map_err(|_| poisoned())?;
        if *busy {
            return Err(DesktopError::new("busy", "an update is already installing"));
        }
        let pending = hub.pending.lock().map_err(|_| poisoned())?;
        let Some(u) = pending.clone() else {
            return Err(DesktopError::not_found(
                "no pending update; check for updates first",
            ));
        };
        *busy = true;
        u
    };

    let meta = info(&update);
    let progress_app = app.clone();
    let mut downloaded: u64 = 0;
    let result = update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                emit(&progress_app, UpdateEvent::Progress { downloaded, total });
            },
            || {},
        )
        .await;

    if let Ok(mut busy) = hub.busy.lock() {
        *busy = false;
    }
    match result {
        Ok(()) => {
            if let Ok(mut pending) = hub.pending.lock() {
                *pending = None;
            }
            emit(
                app,
                UpdateEvent::Installed {
                    version: meta.version.clone(),
                },
            );
            Ok(meta)
        }
        Err(e) => {
            emit(
                app,
                UpdateEvent::Failed {
                    message: e.to_string(),
                },
            );
            Err(updater_err(e))
        }
    }
}

/// Startup hook: only runs when the user opted in via `updateCheck = "startup"`.
pub async fn check_on_startup<R: Runtime>(app: &AppHandle<R>) {
    match check(app).await {
        Ok(Some(i)) => {
            tracing::info!(version = %i.version, "update available");
            emit(app, UpdateEvent::Available { info: i });
        }
        Ok(None) => tracing::debug!("no update available"),
        Err(e) => tracing::warn!("update check failed: {e}"),
    }
}

fn poisoned() -> DesktopError {
    DesktopError::new("internal", "updater state poisoned")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_setting_falls_back_to_default_feed() {
        assert_eq!(feed_url("").unwrap().as_str(), DEFAULT_FEED);
        assert_eq!(feed_url("   ").unwrap().as_str(), DEFAULT_FEED);
    }

    #[test]
    fn custom_https_feed_is_kept() {
        let u =
            feed_url("https://updates.example.org/termoso/{{target}}/{{current_version}}").unwrap();
        assert_eq!(u.host_str(), Some("updates.example.org"));
    }

    #[test]
    fn plain_http_is_rejected() {
        assert!(feed_url("http://updates.example.org/latest.json").is_err());
        assert!(feed_url("ftp://x").is_err());
        assert!(feed_url("not a url").is_err());
    }
}
