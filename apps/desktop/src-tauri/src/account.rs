//! Account and sync runtime. Sign-in flows, the session token, account keys
//! and vault keys live in Rust; the webview sees profile data, sync status
//! and a step machine for MFA / device approval. The recovery phrase is
//! returned exactly once, from `register`, so the UI can show it.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use termoso_core::account::{
    self as core, LoginFlow, LoginStep, ReauthFlow, ReauthStep, RegisterInput,
};
use termoso_core::api::ApiClient;
use termoso_core::store::{EntityFilter, LocalVault, StoredAccount};
use termoso_core::sync::{SyncEngine, SyncEvent, SyncOptions, SyncReport};
use termoso_proto::account::ServerInfo;
use termoso_proto::auth::{Device, MfaCredential, MfaMethod};
use termoso_proto::entities::is_credential_kind;
use termoso_proto::vault::VaultMember;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

pub const SYNC_EVENT: &str = "sync";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountCard {
    pub server_url: String,
    pub user_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub is_admin: bool,
    pub device_id: Uuid,
    pub signed_in_at: DateTime<Utc>,
}

impl From<StoredAccount> for AccountCard {
    fn from(a: StoredAccount) -> Self {
        Self {
            server_url: a.server_url,
            user_id: a.user_id,
            email: a.email,
            display_name: a.display_name,
            avatar: a.avatar,
            is_admin: a.is_admin,
            device_id: a.device_id,
            signed_in_at: a.signed_in_at,
        }
    }
}

/// Where an interactive sign-in currently stands.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "step",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LoginOutcome {
    Done { account: AccountCard },
    MfaRequired { methods: Vec<MfaMethod> },
    DeviceApprovalRequired { email_hint: String },
}

/// Where a step-up (re-authentication for sensitive account changes) stands.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "step",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ReauthOutcome {
    Done { expires_at: DateTime<Utc> },
    MfaRequired { methods: Vec<MfaMethod> },
    EmailCodeRequired { email_hint: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Registered {
    pub account: AccountCard,
    /// Shown once; never stored by the UI.
    pub recovery_phrase: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum SyncState {
    #[default]
    Idle,
    Syncing,
    Offline,
    Error,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub state: SyncState,
    pub realtime: bool,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub pushed: usize,
    pub pulled: usize,
    pub conflicts: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub account: Option<AccountCard>,
    /// A sign-in waiting for a second factor / device code.
    pub pending: Option<LoginOutcome>,
    pub sync: SyncStatus,
    pub vaults: Vec<LocalVault>,
    /// Identities, keys and certificates of the Personal vault that exist
    /// only on this device because credential sync is off (0 when it is on).
    pub local_credentials: usize,
}

/// Emitted on `SYNC_EVENT`.
#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SyncNotice {
    Status {
        status: SyncStatus,
    },
    EntitiesChanged {
        vault_id: Uuid,
    },
    VaultsChanged,
    HistoryChanged,
    LogsChanged,
    AccountChanged,
    /// Who is connected to what changed in a team.
    PresenceChanged {
        team_id: Uuid,
    },
    /// The server revoked this device; the account was signed out locally.
    SignedOut,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginForm {
    pub server_url: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterForm {
    pub server_url: String,
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub invite_token: Option<String>,
}

struct Engine {
    engine: Arc<SyncEngine>,
    cancel: CancellationToken,
    runner: JoinHandle<()>,
    watcher: JoinHandle<()>,
}

impl Engine {
    fn stop(self) {
        self.cancel.cancel();
        self.runner.abort();
        self.watcher.abort();
    }
}

#[derive(Default)]
struct Inner {
    api: Option<Arc<ApiClient>>,
    flow: Option<LoginFlow>,
    pending: Option<LoginOutcome>,
    reauth: Option<ReauthFlow>,
    engine: Option<Engine>,
}

#[derive(Default)]
pub struct AccountRuntime {
    inner: tokio::sync::Mutex<Inner>,
    status: Mutex<SyncStatus>,
}

impl AccountRuntime {
    pub fn sync_status(&self) -> SyncStatus {
        self.status.lock().expect("sync status poisoned").clone()
    }

    fn update_status(&self, f: impl FnOnce(&mut SyncStatus)) -> SyncStatus {
        let mut s = self.status.lock().expect("sync status poisoned");
        f(&mut s);
        s.clone()
    }
}

fn device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| "Termoso Desktop".into())
}

fn device(state: &AppState) -> Result<termoso_proto::auth::DeviceInfo> {
    Ok(core::device_info(
        &state.store,
        &device_name(),
        core::current_platform(),
        env!("CARGO_PKG_VERSION"),
    )?)
}

fn sync_options(state: &AppState) -> Result<SyncOptions> {
    let settings = state.settings()?;
    let interval = match settings.sync_interval_seconds {
        0 => Duration::from_secs(365 * 24 * 3600),
        s => Duration::from_secs(u64::from(s)),
    };
    Ok(SyncOptions {
        conflict: settings.conflict_policy()?,
        log_dir: Some(state.logs_dir()),
        upload_logs: settings.upload_logs,
        sync_credentials: settings.sync_credentials,
        interval,
        ..SyncOptions::default()
    })
}

fn local_credentials(state: &AppState) -> Result<usize> {
    if state.settings()?.sync_credentials {
        return Ok(0);
    }
    let Some(vault) = state.store.personal_vault()? else {
        return Ok(0);
    };
    Ok(state
        .store
        .rows(&EntityFilter {
            vault_id: Some(vault.id),
            ..EntityFilter::default()
        })?
        .iter()
        .filter(|r| is_credential_kind(&r.kind))
        .count())
}

fn outcome(step: &LoginStep) -> LoginOutcome {
    match step {
        LoginStep::Done(s) => LoginOutcome::Done {
            account: s.account.clone().into(),
        },
        LoginStep::MfaRequired { methods } => LoginOutcome::MfaRequired {
            methods: methods.clone(),
        },
        LoginStep::DeviceApprovalRequired { email_hint } => LoginOutcome::DeviceApprovalRequired {
            email_hint: email_hint.clone(),
        },
    }
}

fn reauth_outcome(step: &ReauthStep) -> ReauthOutcome {
    match step {
        ReauthStep::Done { expires_at } => ReauthOutcome::Done {
            expires_at: *expires_at,
        },
        ReauthStep::MfaRequired { methods } => ReauthOutcome::MfaRequired {
            methods: methods.clone(),
        },
        ReauthStep::EmailCodeRequired { email_hint } => ReauthOutcome::EmailCodeRequired {
            email_hint: email_hint.clone(),
        },
    }
}

fn normalize_url(url: &str) -> Result<String> {
    let url = url.trim().trim_end_matches('/');
    if url.is_empty() {
        return Err(DesktopError::invalid("server URL is required"));
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(DesktopError::invalid(
            "server URL must start with https:// or http://",
        ));
    }
    Ok(url.to_string())
}

pub fn status(state: &AppState, pending: Option<LoginOutcome>) -> Result<AccountStatus> {
    Ok(AccountStatus {
        account: state.store.account()?.map(Into::into),
        pending,
        sync: state.account.sync_status(),
        vaults: state.store.vaults()?,
        local_credentials: local_credentials(state)?,
    })
}

pub async fn current<R: Runtime>(app: &AppHandle<R>) -> Result<AccountStatus> {
    let state = app.state::<AppState>();
    let pending = state.account.inner.lock().await.pending.clone();
    status(&state, pending)
}

pub async fn server_info(url: &str) -> Result<ServerInfo> {
    let api = ApiClient::new(&normalize_url(url)?)?;
    Ok(api.server_info().await?)
}

// ───────────────────────────── sign in ─────────────────────────────

pub async fn login<R: Runtime>(app: &AppHandle<R>, form: LoginForm) -> Result<LoginOutcome> {
    let state = app.state::<AppState>();
    if state.store.account()?.is_some() {
        return Err(DesktopError::invalid("already signed in"));
    }
    if form.password.is_empty() {
        return Err(DesktopError::invalid("password is required"));
    }
    let api = Arc::new(ApiClient::new(&normalize_url(&form.server_url)?)?);
    let device = device(&state)?;
    let mut inner = state.account.inner.lock().await;
    let (flow, step) = LoginFlow::start(
        api.clone(),
        state.store.clone(),
        &form.email,
        &form.password,
        device,
        None,
    )
    .await?;
    inner.api = Some(api);
    finish_step(app, &state, &mut inner, flow, step).await
}

async fn finish_step<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    inner: &mut Inner,
    flow: LoginFlow,
    step: LoginStep,
) -> Result<LoginOutcome> {
    let out = outcome(&step);
    match step {
        LoginStep::Done(_) => {
            inner.flow = None;
            inner.pending = None;
            start_engine(app, state, inner)?;
        }
        _ => {
            inner.flow = Some(flow);
            inner.pending = Some(out.clone());
        }
    }
    Ok(out)
}

pub async fn mfa<R: Runtime>(
    app: &AppHandle<R>,
    credential: MfaCredential,
) -> Result<LoginOutcome> {
    let state = app.state::<AppState>();
    let mut inner = state.account.inner.lock().await;
    let mut flow = inner
        .flow
        .take()
        .ok_or_else(|| DesktopError::invalid("no sign-in in progress"))?;
    match flow.mfa(credential).await {
        Ok(step) => finish_step(app, &state, &mut inner, flow, step).await,
        Err(e) => {
            inner.flow = Some(flow);
            Err(e.into())
        }
    }
}

pub async fn mfa_email_send<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let state = app.state::<AppState>();
    let inner = state.account.inner.lock().await;
    let flow = inner
        .flow
        .as_ref()
        .ok_or_else(|| DesktopError::invalid("no sign-in in progress"))?;
    Ok(flow.send_mfa_email().await?)
}

pub async fn webauthn_challenge<R: Runtime>(app: &AppHandle<R>) -> Result<serde_json::Value> {
    let state = app.state::<AppState>();
    let inner = state.account.inner.lock().await;
    let flow = inner
        .flow
        .as_ref()
        .ok_or_else(|| DesktopError::invalid("no sign-in in progress"))?;
    Ok(flow.webauthn_challenge().await?)
}

pub async fn approve_device<R: Runtime>(app: &AppHandle<R>, code: &str) -> Result<LoginOutcome> {
    let state = app.state::<AppState>();
    let mut inner = state.account.inner.lock().await;
    let mut flow = inner
        .flow
        .take()
        .ok_or_else(|| DesktopError::invalid("no sign-in in progress"))?;
    match flow.approve_device(code).await {
        Ok(step) => finish_step(app, &state, &mut inner, flow, step).await,
        Err(e) => {
            inner.flow = Some(flow);
            Err(e.into())
        }
    }
}

pub async fn resend_device_code<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let state = app.state::<AppState>();
    let inner = state.account.inner.lock().await;
    let flow = inner
        .flow
        .as_ref()
        .ok_or_else(|| DesktopError::invalid("no sign-in in progress"))?;
    Ok(flow.resend_device_code().await?)
}

pub async fn cancel_login<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let state = app.state::<AppState>();
    let mut inner = state.account.inner.lock().await;
    inner.flow = None;
    inner.pending = None;
    if state.store.account()?.is_none() {
        inner.api = None;
    }
    Ok(())
}

pub async fn register<R: Runtime>(app: &AppHandle<R>, form: RegisterForm) -> Result<Registered> {
    let state = app.state::<AppState>();
    if state.store.account()?.is_some() {
        return Err(DesktopError::invalid("already signed in"));
    }
    if form.password.chars().count() < 12 {
        return Err(DesktopError::invalid(
            "password must be at least 12 characters",
        ));
    }
    let api = Arc::new(ApiClient::new(&normalize_url(&form.server_url)?)?);
    let device = device(&state)?;
    let mut inner = state.account.inner.lock().await;
    let registered = core::register(
        api.clone(),
        state.store.clone(),
        RegisterInput {
            email: form.email,
            password: form.password,
            display_name: form
                .display_name
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            device,
            invite_token: form.invite_token,
            sso_session: None,
        },
    )
    .await?;
    inner.api = Some(api);
    inner.flow = None;
    inner.pending = None;
    start_engine(app, &state, &mut inner)?;
    let phrase: Zeroizing<String> = registered.recovery_phrase;
    Ok(Registered {
        account: registered.signed_in.account.into(),
        recovery_phrase: phrase.to_string(),
    })
}

/// Restore the persisted session at startup and start syncing. Offline
/// servers are not an error: the engine will retry in the background.
pub async fn resume<R: Runtime>(app: &AppHandle<R>) -> Result<Option<AccountCard>> {
    let state = app.state::<AppState>();
    let Some(stored) = state.store.account()? else {
        return Ok(None);
    };
    let api = Arc::new(ApiClient::new(&stored.server_url)?);
    let mut inner = state.account.inner.lock().await;
    match core::resume(&api, &state.store).await {
        Ok(Some(signed)) => {
            inner.api = Some(api);
            start_engine(app, &state, &mut inner)?;
            Ok(Some(signed.account.into()))
        }
        Ok(None) => Ok(None),
        Err(e) if e.is_unauthorized() => {
            core::sign_out_local(&api, &state.store)?;
            state.account.update_status(|s| {
                s.state = SyncState::Error;
                s.last_error = Some("session revoked by the server".into());
            });
            Ok(None)
        }
        Err(e) => {
            // Offline: keep the stored token, run the engine so it reconnects.
            let secrets = state.store.account_secrets()?;
            api.set_token(Some(secrets.token));
            inner.api = Some(api);
            start_engine(app, &state, &mut inner)?;
            state.account.update_status(|s| {
                s.state = SyncState::Offline;
                s.last_error = Some(e.to_string());
            });
            Ok(Some(stored.into()))
        }
    }
}

pub async fn sign_out<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let state = app.state::<AppState>();
    let mut inner = state.account.inner.lock().await;
    stop_engine(&mut inner).await;
    inner.flow = None;
    inner.pending = None;
    inner.reauth = None;
    let api = match inner.api.take() {
        Some(api) => api,
        None => match state.store.account()? {
            Some(a) => {
                let api = Arc::new(ApiClient::new(&a.server_url)?);
                api.set_token(Some(state.store.account_secrets()?.token));
                api
            }
            None => return Ok(()),
        },
    };
    core::sign_out(&api, &state.store).await?;
    crate::avatars::clear_cache(&state);
    state.account.update_status(|s| *s = SyncStatus::default());
    let _ = app.emit(SYNC_EVENT, SyncNotice::SignedOut);
    Ok(())
}

// ───────────────────────────── sync engine ─────────────────────────────

fn start_engine<R: Runtime>(app: &AppHandle<R>, state: &AppState, inner: &mut Inner) -> Result<()> {
    if inner.engine.is_some() {
        return Ok(());
    }
    let api = inner
        .api
        .clone()
        .ok_or_else(|| DesktopError::invalid("no API client"))?;
    let engine = SyncEngine::new(api, state.store.clone(), sync_options(state)?);
    let cancel = CancellationToken::new();
    let mut rx = engine.subscribe();
    let watcher_app = app.clone();
    let watcher = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if on_event(&watcher_app, ev).await {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let runner = tokio::spawn(engine.clone().run(cancel.clone()));
    let sshid_app = app.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::sshid::refresh(&sshid_app).await {
            tracing::debug!("sshid refresh skipped: {e}");
        }
    });
    inner.engine = Some(Engine {
        engine,
        cancel,
        runner,
        watcher,
    });
    crate::presence::refresh(app);
    Ok(())
}

/// The running sync engine, if signed in.
pub(crate) async fn engine<R: Runtime>(app: &AppHandle<R>) -> Option<Arc<SyncEngine>> {
    let state = app.state::<AppState>();
    let inner = state.account.inner.lock().await;
    inner.engine.as_ref().map(|e| e.engine.clone())
}

async fn stop_engine(inner: &mut Inner) {
    if let Some(e) = inner.engine.take() {
        e.stop();
    }
}

/// Apply a sync-engine event to the status and forward it. Returns `true`
/// when the engine has stopped for good.
async fn on_event<R: Runtime>(app: &AppHandle<R>, ev: SyncEvent) -> bool {
    let state = app.state::<AppState>();
    let notice = match ev {
        SyncEvent::Started => SyncNotice::Status {
            status: state
                .account
                .update_status(|s| s.state = SyncState::Syncing),
        },
        SyncEvent::Finished(report) => SyncNotice::Status {
            status: state.account.update_status(|s| apply_report(s, &report)),
        },
        SyncEvent::Failed(msg) => SyncNotice::Status {
            status: state.account.update_status(|s| {
                s.state = SyncState::Error;
                s.last_error = Some(msg);
            }),
        },
        SyncEvent::Connected => SyncNotice::Status {
            status: state.account.update_status(|s| {
                s.realtime = true;
                if s.state == SyncState::Offline {
                    s.state = SyncState::Idle;
                }
            }),
        },
        SyncEvent::Disconnected => SyncNotice::Status {
            status: state.account.update_status(|s| {
                s.realtime = false;
                if s.state == SyncState::Idle {
                    s.state = SyncState::Offline;
                }
            }),
        },
        SyncEvent::VaultsChanged => SyncNotice::VaultsChanged,
        SyncEvent::EntitiesChanged { vault_id } => SyncNotice::EntitiesChanged { vault_id },
        SyncEvent::HistoryChanged => SyncNotice::HistoryChanged,
        SyncEvent::LogsChanged => SyncNotice::LogsChanged,
        SyncEvent::AccountChanged => SyncNotice::AccountChanged,
        SyncEvent::PresenceChanged { team_id } => SyncNotice::PresenceChanged { team_id },
        SyncEvent::SessionRevoked => {
            let mut inner = state.account.inner.lock().await;
            // We are the watcher task: stop the runner, let ourselves return.
            if let Some(e) = inner.engine.take() {
                e.cancel.cancel();
                e.runner.abort();
            }
            if let Some(api) = inner.api.take() {
                api.set_token(None);
            }
            if let Err(e) = state.store.clear_account() {
                tracing::warn!("clearing revoked account failed: {e}");
            }
            state.account.update_status(|s| {
                *s = SyncStatus::default();
                s.state = SyncState::Error;
                s.last_error = Some("this device was signed out by the server".into());
            });
            let _ = app.emit(SYNC_EVENT, SyncNotice::SignedOut);
            return true;
        }
    };
    let _ = app.emit(SYNC_EVENT, notice);
    false
}

fn apply_report(s: &mut SyncStatus, r: &SyncReport) {
    s.state = if s.realtime {
        SyncState::Idle
    } else {
        SyncState::Offline
    };
    s.last_sync_at = Some(Utc::now());
    s.last_error = r.errors.first().map(|(id, code)| format!("{code} ({id})"));
    s.pushed = r.pushed;
    s.pulled = r.pulled;
    s.conflicts = r.conflicts;
}

pub async fn sync_now<R: Runtime>(app: &AppHandle<R>) -> Result<SyncStatus> {
    let state = app.state::<AppState>();
    let engine = {
        let inner = state.account.inner.lock().await;
        inner
            .engine
            .as_ref()
            .map(|e| e.engine.clone())
            .ok_or_else(|| DesktopError::invalid("not signed in"))?
    };
    state
        .account
        .update_status(|s| s.state = SyncState::Syncing);
    let result = engine.sync_once().await;
    let status = match &result {
        Ok(report) => state.account.update_status(|s| apply_report(s, report)),
        Err(e) => state.account.update_status(|s| {
            s.state = SyncState::Error;
            s.last_error = Some(e.to_string());
        }),
    };
    let _ = app.emit(
        SYNC_EVENT,
        SyncNotice::Status {
            status: status.clone(),
        },
    );
    if let Ok(report) = &result
        && report.changed_locally()
    {
        let _ = app.emit(SYNC_EVENT, SyncNotice::VaultsChanged);
    }
    result?;
    Ok(status)
}

/// Restart the engine with fresh options (after settings change).
pub async fn reconfigure<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let state = app.state::<AppState>();
    let mut inner = state.account.inner.lock().await;
    if inner.engine.is_none() {
        return Ok(());
    }
    stop_engine(&mut inner).await;
    start_engine(app, &state, &mut inner)
}

/// Switch credential sync for the Personal vault. Turning it off takes the
/// identities, keys and certificates off the server (they stay here);
/// turning it on pushes the local ones and pulls what other devices have.
/// Works offline too: the setting is saved and the engine picks it up.
pub async fn set_credential_sync<R: Runtime>(
    app: &AppHandle<R>,
    on: bool,
) -> Result<AccountStatus> {
    let state = app.state::<AppState>();
    let mut settings = state.settings()?;
    if settings.sync_credentials != on {
        settings.sync_credentials = on;
        state.save_settings(&settings)?;
    }
    reconfigure(app).await?;
    if let Some(engine) = engine(app).await {
        state
            .account
            .update_status(|s| s.state = SyncState::Syncing);
        let result = if on {
            engine.resync_credentials().await.map(Some)
        } else {
            engine.purge_credentials().await.map(|_| None)
        };
        let status = match &result {
            Ok(Some(report)) => state.account.update_status(|s| apply_report(s, report)),
            Ok(None) => state.account.update_status(|s| {
                s.state = SyncState::Idle;
                s.last_sync_at = Some(Utc::now());
            }),
            Err(e) => state.account.update_status(|s| {
                s.state = SyncState::Error;
                s.last_error = Some(e.to_string());
            }),
        };
        let _ = app.emit(SYNC_EVENT, SyncNotice::Status { status });
        let _ = app.emit(SYNC_EVENT, SyncNotice::VaultsChanged);
        result?;
    }
    current(app).await
}

// ───────────────────────────── devices ─────────────────────────────

/// The signed-in API client, or "not signed in".
pub(crate) async fn api<R: Runtime>(app: &AppHandle<R>) -> Result<Arc<ApiClient>> {
    let state = app.state::<AppState>();
    let inner = state.account.inner.lock().await;
    inner
        .api
        .clone()
        .filter(|a| a.token().is_some())
        .ok_or_else(|| DesktopError::invalid("not signed in"))
}

// ───────────────────────────── step-up ─────────────────────────────

/// Prove the password again so the session may perform sensitive changes
/// (the server answers `reauth_required` otherwise).
pub async fn reauth_start<R: Runtime>(
    app: &AppHandle<R>,
    password: Zeroizing<String>,
) -> Result<ReauthOutcome> {
    let state = app.state::<AppState>();
    let account = state
        .store
        .account()?
        .ok_or_else(|| DesktopError::invalid("not signed in"))?;
    let api = api(app).await?;
    let mut inner = state.account.inner.lock().await;
    inner.reauth = None;
    let (flow, step) = ReauthFlow::start(api, &account.email, &password).await?;
    Ok(keep_reauth(&mut inner, flow, step))
}

fn keep_reauth(inner: &mut Inner, flow: ReauthFlow, step: ReauthStep) -> ReauthOutcome {
    let out = reauth_outcome(&step);
    inner.reauth = match step {
        ReauthStep::Done { .. } => None,
        _ => Some(flow),
    };
    out
}

enum ReauthAnswer {
    Mfa(MfaCredential),
    EmailCode(String),
}

async fn continue_reauth<R: Runtime>(
    app: &AppHandle<R>,
    answer: ReauthAnswer,
) -> Result<ReauthOutcome> {
    let state = app.state::<AppState>();
    let mut inner = state.account.inner.lock().await;
    let mut flow = inner
        .reauth
        .take()
        .ok_or_else(|| DesktopError::invalid("no re-authentication in progress"))?;
    let result = match answer {
        ReauthAnswer::Mfa(credential) => flow.mfa(credential).await,
        ReauthAnswer::EmailCode(code) => flow.email_code(&code).await,
    };
    match result {
        Ok(step) => Ok(keep_reauth(&mut inner, flow, step)),
        Err(e) => {
            inner.reauth = Some(flow);
            Err(e.into())
        }
    }
}

pub async fn reauth_mfa<R: Runtime>(
    app: &AppHandle<R>,
    credential: MfaCredential,
) -> Result<ReauthOutcome> {
    continue_reauth(app, ReauthAnswer::Mfa(credential)).await
}

pub async fn reauth_email_code<R: Runtime>(
    app: &AppHandle<R>,
    code: String,
) -> Result<ReauthOutcome> {
    continue_reauth(app, ReauthAnswer::EmailCode(code)).await
}

pub async fn reauth_mfa_email_send<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let state = app.state::<AppState>();
    let inner = state.account.inner.lock().await;
    let flow = inner
        .reauth
        .as_ref()
        .ok_or_else(|| DesktopError::invalid("no re-authentication in progress"))?;
    Ok(flow.send_mfa_email().await?)
}

pub async fn reauth_webauthn_challenge<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<serde_json::Value> {
    let state = app.state::<AppState>();
    let inner = state.account.inner.lock().await;
    let flow = inner
        .reauth
        .as_ref()
        .ok_or_else(|| DesktopError::invalid("no re-authentication in progress"))?;
    Ok(flow.webauthn_challenge().await?)
}

pub async fn reauth_cancel<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let state = app.state::<AppState>();
    state.account.inner.lock().await.reauth = None;
    Ok(())
}

pub async fn devices<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<Device>> {
    Ok(api(app).await?.devices().await?)
}

pub async fn vault_members<R: Runtime>(
    app: &AppHandle<R>,
    vault_id: Uuid,
) -> Result<Vec<VaultMember>> {
    Ok(api(app).await?.vault_members(vault_id).await?.members)
}

pub async fn revoke_device<R: Runtime>(app: &AppHandle<R>, id: Uuid) -> Result<()> {
    let state = app.state::<AppState>();
    if state.store.account()?.map(|a| a.device_id) == Some(id) {
        return Err(DesktopError::invalid(
            "use sign out to remove the current device",
        ));
    }
    Ok(api(app).await?.revoke_device(id).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_normalisation() {
        assert_eq!(
            normalize_url(" https://termoso.example.com/ ").unwrap(),
            "https://termoso.example.com"
        );
        assert!(normalize_url("termoso.example.com").is_err());
        assert!(normalize_url("").is_err());
    }

    #[test]
    fn report_updates_status() {
        let mut s = SyncStatus {
            realtime: true,
            ..SyncStatus::default()
        };
        apply_report(
            &mut s,
            &SyncReport {
                pushed: 2,
                pulled: 3,
                conflicts: 1,
                errors: vec![(Uuid::nil(), "too_large".into())],
                ..SyncReport::default()
            },
        );
        assert_eq!(s.state, SyncState::Idle);
        assert_eq!((s.pushed, s.pulled, s.conflicts), (2, 3, 1));
        assert!(s.last_error.as_deref().unwrap().starts_with("too_large"));
        assert!(s.last_sync_at.is_some());
    }

    #[test]
    fn outcome_serialises_step_tag() {
        let v = serde_json::to_value(LoginOutcome::MfaRequired {
            methods: vec![MfaMethod::Totp],
        })
        .unwrap();
        assert_eq!(v["step"], "mfaRequired");
        assert_eq!(v["methods"][0], "totp");
    }
}
