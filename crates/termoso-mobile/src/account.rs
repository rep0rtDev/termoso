//! Account and sync runtime for the mobile façade. Mirrors the desktop
//! runtime: OPAQUE sign-in, MFA / device-approval step machine, session
//! resume, sign-out and the background [`SyncEngine`] all live in Rust;
//! Kotlin sees profile data, sync status and change notifications.
//!
//! Secrets policy: the password only feeds OPAQUE and is dropped when the
//! call returns; the session token, account keys and vault keys stay in the
//! encrypted store. The recovery phrase is returned exactly once, from
//! [`AccountRuntime::register`].

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use termoso_core::account::{self as core, LoginFlow, LoginStep, RegisterInput};
use termoso_core::api::ApiClient;
use termoso_core::store::{Store, StoredAccount};
use termoso_core::sync::{SyncEngine, SyncEvent, SyncOptions, SyncReport};
use termoso_proto::account::ServerInfo;
use termoso_proto::auth::{Device, MfaCredential};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::dto::{VaultInfo, parse_id};
use crate::error::{MobileError, Result};

/// Signed-in account as shown in the UI (no keys, no token).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountCard {
    pub server_url: String,
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
    pub device_id: String,
    /// RFC 3339.
    pub signed_in_at: String,
}

impl From<StoredAccount> for AccountCard {
    fn from(a: StoredAccount) -> Self {
        Self {
            server_url: a.server_url,
            user_id: a.user_id.to_string(),
            email: a.email,
            display_name: a.display_name,
            is_admin: a.is_admin,
            device_id: a.device_id.to_string(),
            signed_in_at: a.signed_in_at.to_rfc3339(),
        }
    }
}

/// What a server tells about itself before sign-in.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ServerCard {
    pub url: String,
    pub name: String,
    pub version: String,
    pub registration_open: bool,
    /// The server can send email (device approval / email MFA work).
    pub email: bool,
}

impl ServerCard {
    fn new(url: String, info: ServerInfo) -> Self {
        Self {
            url,
            name: info.name,
            version: info.version,
            registration_open: info.registration_open,
            email: info.features.email,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MfaMethod {
    Totp,
    Webauthn,
    BackupCode,
    Email,
}

impl From<termoso_proto::auth::MfaMethod> for MfaMethod {
    fn from(m: termoso_proto::auth::MfaMethod) -> Self {
        use termoso_proto::auth::MfaMethod as M;
        match m {
            M::Totp => Self::Totp,
            M::Webauthn => Self::Webauthn,
            M::BackupCode => Self::BackupCode,
            M::Email => Self::Email,
        }
    }
}

/// Where an interactive sign-in stands.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum LoginOutcome {
    Done { account: AccountCard },
    MfaRequired { methods: Vec<MfaMethod> },
    DeviceApprovalRequired { email_hint: String },
}

impl From<&LoginStep> for LoginOutcome {
    fn from(step: &LoginStep) -> Self {
        match step {
            LoginStep::Done(s) => Self::Done {
                account: s.account.clone().into(),
            },
            LoginStep::MfaRequired { methods } => Self::MfaRequired {
                methods: methods.iter().copied().map(Into::into).collect(),
            },
            LoginStep::DeviceApprovalRequired { email_hint } => Self::DeviceApprovalRequired {
                email_hint: email_hint.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Registered {
    pub account: AccountCard,
    /// 24 words. Shown once; never stored by the UI.
    pub recovery_phrase: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Enum)]
pub enum SyncState {
    #[default]
    Idle,
    Syncing,
    Offline,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, uniffi::Record)]
pub struct SyncStatus {
    pub state: SyncState,
    /// Realtime channel connected: changes from other devices arrive at once.
    pub realtime: bool,
    /// RFC 3339.
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    pub pushed: u32,
    pub pulled: u32,
    pub conflicts: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AccountStatus {
    pub account: Option<AccountCard>,
    /// A sign-in waiting for a second factor / device code.
    pub pending: Option<LoginOutcome>,
    pub sync: SyncStatus,
    pub vaults: Vec<VaultInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DeviceCard {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub last_seen_at: String,
    pub current: bool,
}

impl From<Device> for DeviceCard {
    fn from(d: Device) -> Self {
        Self {
            current: d.current,
            id: d.id.to_string(),
            name: d.name,
            platform: format!("{:?}", d.platform).to_lowercase(),
            app_version: d.app_version,
            created_at: d.created_at.to_rfc3339(),
            last_seen_at: d.last_seen_at.to_rfc3339(),
        }
    }
}

/// Local data changed because of a sync pull; list screens should reload.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SyncChange {
    Vaults,
    Entities { vault_id: String },
    History,
    Logs,
    Account,
}

/// Callbacks into Kotlin. Invoked from Rust worker threads; keep them quick
/// (post to the main thread, do not block).
#[uniffi::export(with_foreign)]
pub trait SyncListener: Send + Sync {
    fn on_status(&self, status: SyncStatus);
    fn on_changed(&self, change: SyncChange);
    /// The account is gone (sign-out or revoked by the server).
    fn on_signed_out(&self, reason: Option<String>);
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LoginForm {
    pub server_url: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RegisterForm {
    pub server_url: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
    pub invite_token: Option<String>,
}

const MIN_PASSWORD_CHARS: usize = 12;

/// Trim, drop trailing slashes, require an http(s) scheme.
pub fn normalize_server_url(url: &str) -> Result<String> {
    let url = url.trim().trim_end_matches('/');
    if url.is_empty() {
        return Err(MobileError::invalid("server URL is required"));
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(MobileError::invalid(
            "server URL must start with https:// or http://",
        ));
    }
    if url[url.find("://").unwrap_or(0) + 3..].is_empty() {
        return Err(MobileError::invalid("server URL has no host"));
    }
    Ok(url.to_string())
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
    engine: Option<Engine>,
}

/// One store's account state. Held by [`crate::TermosoApp`]; background
/// tasks only hold a [`Weak`] so an unlocked vault can be dropped.
pub struct AccountRuntime {
    store: Arc<Store>,
    device_name: Mutex<String>,
    inner: tokio::sync::Mutex<Inner>,
    status: Mutex<SyncStatus>,
    listener: Mutex<Option<Arc<dyn SyncListener>>>,
}

impl AccountRuntime {
    pub fn new(store: Arc<Store>) -> Arc<Self> {
        Arc::new(Self {
            store,
            device_name: Mutex::new("Android".into()),
            inner: tokio::sync::Mutex::new(Inner::default()),
            status: Mutex::new(SyncStatus::default()),
            listener: Mutex::new(None),
        })
    }

    pub fn set_device_name(&self, name: String) {
        let name = name.trim().to_string();
        if !name.is_empty() {
            *self.device_name.lock().unwrap_or_else(|p| p.into_inner()) = name;
        }
    }

    pub fn set_listener(&self, listener: Option<Arc<dyn SyncListener>>) {
        *self.listener.lock().unwrap_or_else(|p| p.into_inner()) = listener;
    }

    fn listener(&self) -> Option<Arc<dyn SyncListener>> {
        self.listener
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub(crate) fn store(&self) -> &Store {
        &self.store
    }

    pub(crate) fn notify_changed(&self, change: SyncChange) {
        if let Some(l) = self.listener() {
            l.on_changed(change);
        }
    }

    /// Run one sync round on the runtime without waiting for it (after a
    /// vault-list change that may have queued re-encrypted rows).
    pub(crate) fn sync_in_background(self: &Arc<Self>) {
        let rt = self.clone();
        tokio::spawn(async move {
            if let Err(e) = rt.sync_now().await {
                tracing::debug!("sync after vault change: {e}");
            }
        });
    }

    pub fn sync_status(&self) -> SyncStatus {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn update_status(&self, f: impl FnOnce(&mut SyncStatus)) -> SyncStatus {
        let mut s = self.status.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut s);
        s.clone()
    }

    fn publish_status(&self, f: impl FnOnce(&mut SyncStatus)) {
        let status = self.update_status(f);
        if let Some(l) = self.listener() {
            l.on_status(status);
        }
    }

    fn device(&self) -> Result<termoso_proto::auth::DeviceInfo> {
        let name = self
            .device_name
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        Ok(core::device_info(
            &self.store,
            &name,
            core::current_platform(),
            env!("CARGO_PKG_VERSION"),
        )?)
    }

    pub async fn status(&self) -> Result<AccountStatus> {
        let pending = self.inner.lock().await.pending.clone();
        Ok(AccountStatus {
            account: self.store.account()?.map(Into::into),
            pending,
            sync: self.sync_status(),
            vaults: self
                .store
                .vaults()?
                .into_iter()
                .map(VaultInfo::from)
                .collect(),
        })
    }

    pub async fn server_info(url: &str) -> Result<ServerCard> {
        let url = normalize_server_url(url)?;
        let api = ApiClient::new(&url)?;
        Ok(ServerCard::new(url, api.server_info().await?))
    }

    // ---- sign in ------------------------------------------------------

    pub async fn login(self: &Arc<Self>, form: LoginForm) -> Result<LoginOutcome> {
        if self.store.account()?.is_some() {
            return Err(MobileError::invalid("already signed in"));
        }
        if form.email.trim().is_empty() {
            return Err(MobileError::invalid("email is required"));
        }
        if form.password.is_empty() {
            return Err(MobileError::invalid("password is required"));
        }
        let api = Arc::new(ApiClient::new(&normalize_server_url(&form.server_url)?)?);
        let device = self.device()?;
        let mut inner = self.inner.lock().await;
        let (flow, step) = LoginFlow::start(
            api.clone(),
            self.store.clone(),
            form.email.trim(),
            &form.password,
            device,
            None,
        )
        .await?;
        inner.api = Some(api);
        self.finish_step(&mut inner, flow, step)
    }

    fn finish_step(
        self: &Arc<Self>,
        inner: &mut Inner,
        flow: LoginFlow,
        step: LoginStep,
    ) -> Result<LoginOutcome> {
        let out = LoginOutcome::from(&step);
        match step {
            LoginStep::Done(_) => {
                inner.flow = None;
                inner.pending = None;
                self.start_engine(inner)?;
            }
            _ => {
                inner.flow = Some(flow);
                inner.pending = Some(out.clone());
            }
        }
        Ok(out)
    }

    pub async fn mfa(self: &Arc<Self>, method: MfaMethod, code: String) -> Result<LoginOutcome> {
        let code = code.trim().to_string();
        if code.is_empty() {
            return Err(MobileError::invalid("code is required"));
        }
        let credential = match method {
            MfaMethod::Totp => MfaCredential::Totp { code },
            MfaMethod::BackupCode => MfaCredential::BackupCode { code },
            MfaMethod::Email => MfaCredential::Email { code },
            MfaMethod::Webauthn => {
                return Err(MobileError::invalid(
                    "security keys are not supported on Android yet; use another method",
                ));
            }
        };
        let mut inner = self.inner.lock().await;
        let mut flow = inner
            .flow
            .take()
            .ok_or_else(|| MobileError::invalid("no sign-in in progress"))?;
        match flow.mfa(credential).await {
            Ok(step) => self.finish_step(&mut inner, flow, step),
            Err(e) => {
                inner.flow = Some(flow);
                Err(e.into())
            }
        }
    }

    pub async fn mfa_email_send(&self) -> Result<()> {
        let inner = self.inner.lock().await;
        let flow = inner
            .flow
            .as_ref()
            .ok_or_else(|| MobileError::invalid("no sign-in in progress"))?;
        Ok(flow.send_mfa_email().await?)
    }

    pub async fn approve_device(self: &Arc<Self>, code: String) -> Result<LoginOutcome> {
        let code = code.trim().to_string();
        if code.is_empty() {
            return Err(MobileError::invalid("code is required"));
        }
        let mut inner = self.inner.lock().await;
        let mut flow = inner
            .flow
            .take()
            .ok_or_else(|| MobileError::invalid("no sign-in in progress"))?;
        match flow.approve_device(&code).await {
            Ok(step) => self.finish_step(&mut inner, flow, step),
            Err(e) => {
                inner.flow = Some(flow);
                Err(e.into())
            }
        }
    }

    pub async fn resend_device_code(&self) -> Result<()> {
        let inner = self.inner.lock().await;
        let flow = inner
            .flow
            .as_ref()
            .ok_or_else(|| MobileError::invalid("no sign-in in progress"))?;
        Ok(flow.resend_device_code().await?)
    }

    pub async fn cancel_login(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        inner.flow = None;
        inner.pending = None;
        if self.store.account()?.is_none() {
            inner.api = None;
        }
        Ok(())
    }

    pub async fn register(self: &Arc<Self>, form: RegisterForm) -> Result<Registered> {
        if self.store.account()?.is_some() {
            return Err(MobileError::invalid("already signed in"));
        }
        if form.email.trim().is_empty() {
            return Err(MobileError::invalid("email is required"));
        }
        if form.password.chars().count() < MIN_PASSWORD_CHARS {
            return Err(MobileError::invalid(format!(
                "password must be at least {MIN_PASSWORD_CHARS} characters"
            )));
        }
        let api = Arc::new(ApiClient::new(&normalize_server_url(&form.server_url)?)?);
        let device = self.device()?;
        let mut inner = self.inner.lock().await;
        let registered = core::register(
            api.clone(),
            self.store.clone(),
            RegisterInput {
                email: form.email.trim().to_string(),
                password: form.password,
                display_name: form
                    .display_name
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
                device,
                invite_token: form
                    .invite_token
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
                sso_session: None,
            },
        )
        .await?;
        inner.api = Some(api);
        inner.flow = None;
        inner.pending = None;
        self.start_engine(&mut inner)?;
        Ok(Registered {
            account: registered.signed_in.account.into(),
            recovery_phrase: registered.recovery_phrase.to_string(),
        })
    }

    /// Restore the persisted session and start syncing. An unreachable
    /// server is not an error: the engine retries in the background.
    pub async fn resume(self: &Arc<Self>) -> Result<Option<AccountCard>> {
        let Some(stored) = self.store.account()? else {
            return Ok(None);
        };
        let api = Arc::new(ApiClient::new(&stored.server_url)?);
        let mut inner = self.inner.lock().await;
        if inner.engine.is_some() {
            return Ok(Some(stored.into()));
        }
        match core::resume(&api, &self.store).await {
            Ok(Some(signed)) => {
                inner.api = Some(api);
                self.start_engine(&mut inner)?;
                Ok(Some(signed.account.into()))
            }
            Ok(None) => Ok(None),
            Err(e) if e.is_unauthorized() => {
                core::sign_out_local(&api, &self.store)?;
                self.publish_status(|s| {
                    *s = SyncStatus::default();
                    s.state = SyncState::Error;
                    s.last_error = Some("session revoked by the server".into());
                });
                if let Some(l) = self.listener() {
                    l.on_signed_out(Some("This device was signed out by the server.".into()));
                }
                Ok(None)
            }
            Err(e) => {
                let secrets = self.store.account_secrets()?;
                api.set_token(Some(secrets.token));
                inner.api = Some(api);
                self.start_engine(&mut inner)?;
                self.publish_status(|s| {
                    s.state = SyncState::Offline;
                    s.last_error = Some(e.to_string());
                });
                Ok(Some(stored.into()))
            }
        }
    }

    pub async fn sign_out(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        if let Some(e) = inner.engine.take() {
            e.stop();
        }
        inner.flow = None;
        inner.pending = None;
        let api = match inner.api.take() {
            Some(api) => api,
            None => match self.store.account()? {
                Some(a) => {
                    let api = Arc::new(ApiClient::new(&a.server_url)?);
                    api.set_token(Some(self.store.account_secrets()?.token));
                    api
                }
                None => return Ok(()),
            },
        };
        core::sign_out(&api, &self.store).await?;
        self.update_status(|s| *s = SyncStatus::default());
        if let Some(l) = self.listener() {
            l.on_signed_out(None);
        }
        Ok(())
    }

    // ---- sync engine --------------------------------------------------

    fn start_engine(self: &Arc<Self>, inner: &mut Inner) -> Result<()> {
        if inner.engine.is_some() {
            return Ok(());
        }
        let api = inner
            .api
            .clone()
            .ok_or_else(|| MobileError::invalid("no API client"))?;
        let opts = SyncOptions {
            log_dir: None,
            upload_logs: false,
            interval: Duration::from_secs(15 * 60),
            ..SyncOptions::default()
        };
        let engine = SyncEngine::new(api, self.store.clone(), opts);
        let cancel = CancellationToken::new();
        let mut rx = engine.subscribe();
        let weak: Weak<Self> = Arc::downgrade(self);
        let watcher = tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        let Some(rt) = weak.upgrade() else { break };
                        if rt.on_event(ev).await {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        let runner = tokio::spawn(engine.clone().run(cancel.clone()));
        inner.engine = Some(Engine {
            engine,
            cancel,
            runner,
            watcher,
        });
        Ok(())
    }

    /// Apply an engine event to the status and forward it. Returns `true`
    /// when the engine has stopped for good.
    async fn on_event(&self, ev: SyncEvent) -> bool {
        let listener = self.listener();
        let notify = |change: SyncChange| {
            if let Some(l) = &listener {
                l.on_changed(change);
            }
        };
        match ev {
            SyncEvent::Started => self.publish_status(|s| s.state = SyncState::Syncing),
            SyncEvent::Finished(report) => {
                let changed = report.changed_locally();
                self.publish_status(|s| apply_report(s, &report));
                if changed {
                    notify(SyncChange::Vaults);
                }
            }
            SyncEvent::Failed(msg) => self.publish_status(|s| {
                s.state = SyncState::Error;
                s.last_error = Some(msg);
            }),
            SyncEvent::Connected => self.publish_status(|s| {
                s.realtime = true;
                if s.state == SyncState::Offline {
                    s.state = SyncState::Idle;
                }
            }),
            SyncEvent::Disconnected => self.publish_status(|s| {
                s.realtime = false;
                if s.state == SyncState::Idle {
                    s.state = SyncState::Offline;
                }
            }),
            SyncEvent::VaultsChanged => notify(SyncChange::Vaults),
            SyncEvent::EntitiesChanged { vault_id } => notify(SyncChange::Entities {
                vault_id: vault_id.to_string(),
            }),
            SyncEvent::HistoryChanged => notify(SyncChange::History),
            SyncEvent::LogsChanged => notify(SyncChange::Logs),
            SyncEvent::AccountChanged => notify(SyncChange::Account),
            SyncEvent::SessionRevoked => {
                let mut inner = self.inner.lock().await;
                // We are the watcher task: stop the runner, let ourselves return.
                if let Some(e) = inner.engine.take() {
                    e.cancel.cancel();
                    e.runner.abort();
                }
                if let Some(api) = inner.api.take() {
                    api.set_token(None);
                }
                if let Err(e) = self.store.clear_account() {
                    tracing::warn!("clearing revoked account failed: {e}");
                }
                self.publish_status(|s| {
                    *s = SyncStatus::default();
                    s.state = SyncState::Error;
                    s.last_error = Some("this device was signed out by the server".into());
                });
                if let Some(l) = &listener {
                    l.on_signed_out(Some("This device was signed out by the server.".into()));
                }
                return true;
            }
        }
        false
    }

    pub async fn sync_now(&self) -> Result<SyncStatus> {
        let engine = {
            let inner = self.inner.lock().await;
            inner
                .engine
                .as_ref()
                .map(|e| e.engine.clone())
                .ok_or_else(|| MobileError::invalid("not signed in"))?
        };
        self.publish_status(|s| s.state = SyncState::Syncing);
        let result = engine.sync_once().await;
        let status = match &result {
            Ok(report) => self.update_status(|s| apply_report(s, report)),
            Err(e) => self.update_status(|s| {
                s.state = SyncState::Error;
                s.last_error = Some(e.to_string());
            }),
        };
        if let Some(l) = self.listener() {
            l.on_status(status.clone());
            if matches!(&result, Ok(r) if r.changed_locally()) {
                l.on_changed(SyncChange::Vaults);
            }
        }
        result?;
        Ok(status)
    }

    // ---- devices ------------------------------------------------------

    pub(crate) async fn api(&self) -> Result<Arc<ApiClient>> {
        let inner = self.inner.lock().await;
        inner
            .api
            .clone()
            .filter(|a| a.token().is_some())
            .ok_or_else(|| MobileError::invalid("not signed in"))
    }

    pub async fn devices(&self) -> Result<Vec<DeviceCard>> {
        Ok(self
            .api()
            .await?
            .devices()
            .await?
            .into_iter()
            .map(DeviceCard::from)
            .collect())
    }

    pub async fn revoke_device(&self, id: String) -> Result<()> {
        let id = parse_id(&id)?;
        if self.store.account()?.map(|a| a.device_id) == Some(id) {
            return Err(MobileError::invalid(
                "use sign out to remove the current device",
            ));
        }
        Ok(self.api().await?.revoke_device(id).await?)
    }

    /// Stop background work. Called when the vault is closed.
    pub fn shutdown(self: &Arc<Self>, handle: &tokio::runtime::Handle) {
        match self.inner.try_lock() {
            Ok(mut inner) => {
                if let Some(e) = inner.engine.take() {
                    e.stop();
                }
            }
            Err(_) => {
                let rt = self.clone();
                handle.spawn(async move {
                    let mut inner = rt.inner.lock().await;
                    if let Some(e) = inner.engine.take() {
                        e.stop();
                    }
                });
            }
        }
        self.set_listener(None);
    }
}

fn apply_report(s: &mut SyncStatus, r: &SyncReport) {
    s.state = if s.realtime {
        SyncState::Idle
    } else {
        SyncState::Offline
    };
    s.last_sync_at = Some(chrono::Utc::now().to_rfc3339());
    s.last_error = r.errors.first().map(|(id, code)| format!("{code} ({id})"));
    s.pushed = u32::try_from(r.pushed).unwrap_or(u32::MAX);
    s.pulled = u32::try_from(r.pulled).unwrap_or(u32::MAX);
    s.conflicts = u32::try_from(r.conflicts).unwrap_or(u32::MAX);
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn normalizes_server_url() {
        assert_eq!(
            normalize_server_url("  https://termoso.example/// ").unwrap(),
            "https://termoso.example"
        );
        assert_eq!(
            normalize_server_url("http://10.0.2.2:8080").unwrap(),
            "http://10.0.2.2:8080"
        );
        assert!(normalize_server_url("").is_err());
        assert!(normalize_server_url("termoso.example").is_err());
        assert!(normalize_server_url("https://").is_err());
    }

    #[test]
    fn report_updates_status() {
        let mut s = SyncStatus {
            realtime: true,
            ..SyncStatus::default()
        };
        let r = SyncReport {
            pushed: 3,
            pulled: 5,
            conflicts: 1,
            errors: vec![(Uuid::nil(), "too_large".into())],
            history: (0, 0),
            logs: (0, 0),
        };
        apply_report(&mut s, &r);
        assert_eq!(s.state, SyncState::Idle);
        assert_eq!((s.pushed, s.pulled, s.conflicts), (3, 5, 1));
        assert!(s.last_error.as_deref().unwrap().starts_with("too_large"));
        assert!(s.last_sync_at.is_some());

        s.realtime = false;
        apply_report(&mut s, &SyncReport::default());
        assert_eq!(s.state, SyncState::Offline);
        assert_eq!(s.last_error, None);
    }
}
