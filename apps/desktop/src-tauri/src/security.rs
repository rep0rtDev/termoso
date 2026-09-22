//! Master password and App Lock.
//!
//! The vault is always encrypted with a random device key; the password only
//! wraps that key on disk (`master.pw`, see `termoso_core::secrets`). While the
//! vault is locked the `Store` — and with it the key — is gone from memory, so
//! every vault command fails with `locked` until the password is entered.

use serde::Serialize;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use termoso_core::secrets::{MIN_PASSWORD_CHARS, MasterKeySource};
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

pub const VAULT_EVENT: &str = "vault";

/// How often the inactivity watcher looks at the clock.
const WATCH_TICK: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub locked: bool,
    pub password_protected: bool,
    pub master_source: MasterKeySource,
    pub min_password_chars: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VaultEvent {
    Locked,
    Unlocked,
}

pub fn status(state: &AppState) -> VaultStatus {
    VaultStatus {
        locked: state.is_locked(),
        password_protected: state.password_protected(),
        master_source: state.master_source(),
        min_password_chars: MIN_PASSWORD_CHARS,
    }
}

/// Tear down everything that holds vault data or a clone of the store, then
/// drop the store itself. Idempotent.
pub async fn lock<R: Runtime>(app: &AppHandle<R>) -> Result<VaultStatus> {
    let state = app.state::<AppState>();
    if !state.password_protected() {
        return Err(DesktopError::invalid(
            "set a master password before locking the vault",
        ));
    }
    if state.is_locked() {
        return Ok(status(&state));
    }
    if state
        .lock
        .locking
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(DesktopError::invalid("the vault is already locking"));
    }
    shutdown(app).await;
    state.close_store();
    state.lock.locking.store(false, Ordering::SeqCst);
    tracing::info!("vault locked");
    let _ = app.emit(VAULT_EVENT, VaultEvent::Locked);
    Ok(status(&state))
}

async fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    crate::multiplayer::stop_all(app).await;
    crate::sessions::close_all(app).await;
    crate::sftp::close_all(app).await;
    crate::forwarding::stop_all(app).await;
    app.state::<AppState>().edits.close_all();
    crate::account::suspend(app).await;
}

/// Open the store with `password` and resume background work.
pub async fn unlock(app: &AppHandle, password: Zeroizing<String>) -> Result<VaultStatus> {
    let state = app.state::<AppState>();
    if state.lock.locking.load(Ordering::SeqCst) {
        return Err(DesktopError::invalid("the vault is still locking"));
    }
    if !state.is_locked() {
        return Ok(status(&state));
    }
    if !state.password_protected() {
        return Err(DesktopError::invalid("the vault has no master password"));
    }
    state.unlock_with_password(&password)?;
    drop(password);
    tracing::info!("vault unlocked");
    let _ = app.emit(VAULT_EVENT, VaultEvent::Unlocked);
    tauri::async_runtime::spawn(crate::startup(app.clone()));
    Ok(status(&state))
}

/// Relock after `lockAfterMinutes` without user input. One instance per
/// process; cheap enough to run for the app's lifetime.
pub async fn inactivity_watcher(app: AppHandle) {
    loop {
        tokio::time::sleep(WATCH_TICK).await;
        let state = app.state::<AppState>();
        if state.is_locked() || !state.password_protected() {
            continue;
        }
        let Ok(settings) = state.settings() else {
            continue;
        };
        if state
            .lock
            .idle_expired(settings.lock_after_minutes, Instant::now())
        {
            tracing::info!(
                minutes = settings.lock_after_minutes,
                "locking vault after inactivity"
            );
            if let Err(e) = lock(&app).await {
                tracing::warn!("inactivity lock failed: {e}");
            }
        }
    }
}

// ───────────────────────────── commands ─────────────────────────────

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> VaultStatus {
    status(&state)
}

#[tauri::command]
pub async fn vault_unlock(app: AppHandle, password: String) -> Result<VaultStatus> {
    unlock(&app, Zeroizing::new(password)).await
}

#[tauri::command]
pub async fn vault_lock(app: AppHandle) -> Result<VaultStatus> {
    lock(&app).await
}

/// The webview saw keyboard / pointer input; resets the inactivity timer.
#[tauri::command]
pub fn vault_activity(state: State<'_, AppState>) {
    state.lock.touch();
}

/// Enable a master password or change the current one (`current` required
/// when one is set). The device key is only re-wrapped; the database itself
/// is untouched.
#[tauri::command]
pub async fn master_password_set(
    state: State<'_, AppState>,
    current: Option<String>,
    password: String,
) -> Result<VaultStatus> {
    let current = current.map(Zeroizing::new);
    let password = Zeroizing::new(password);
    state.set_master_password(current.as_deref().map(String::as_str), &password)?;
    tracing::info!("master password set");
    Ok(status(&state))
}

/// Remove the master password after proving it; the key goes back to the OS
/// keychain (or the owner-only file where there is none).
#[tauri::command]
pub async fn master_password_remove(
    state: State<'_, AppState>,
    current: String,
) -> Result<VaultStatus> {
    let current = Zeroizing::new(current);
    let source = state.remove_master_password(&current)?;
    tracing::info!(source = ?source, "master password removed");
    Ok(status(&state))
}
