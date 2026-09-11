//! Port forwarding: stored rules (`pf_rule` entities) plus the runtime
//! registry of tunnels currently open. Connections reuse the same SSH path
//! as terminals (jump hosts, prompts, stored credentials); the webview only
//! sees rule metadata and traffic counters.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use termoso_core::error::CoreError;
use termoso_core::forward::{Forward, ForwardSpec};
use termoso_core::model::{Host, PfRule};
use termoso_core::ssh::SshClient;
use termoso_core::store::Store;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::{DesktopError, Result};
use crate::sessions;
use crate::state::AppState;

pub const FORWARD_EVENT: &str = "forward";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PfKind {
    Local,
    Remote,
    Dynamic,
}

impl PfKind {
    fn as_str(self) -> &'static str {
        match self {
            PfKind::Local => "local",
            PfKind::Remote => "remote",
            PfKind::Dynamic => "dynamic",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "local" => Ok(PfKind::Local),
            "remote" => Ok(PfKind::Remote),
            "dynamic" => Ok(PfKind::Dynamic),
            other => Err(DesktopError::invalid(format!(
                "unknown forwarding kind {other:?}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PfState {
    Stopped,
    Starting,
    Running,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PfRuntime {
    pub state: PfState,
    pub started_at: Option<DateTime<Utc>>,
    /// Address actually bound (local/dynamic) or the port the server opened
    /// (remote).
    pub bound: Option<String>,
    pub connections: u64,
    pub active: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub last_error: Option<String>,
}

impl PfRuntime {
    fn stopped(last_error: Option<String>) -> Self {
        Self {
            state: PfState::Stopped,
            started_at: None,
            bound: None,
            connections: 0,
            active: 0,
            bytes_in: 0,
            bytes_out: 0,
            last_error,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PfRuleCard {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    pub host_id: Uuid,
    pub host_label: String,
    pub kind: PfKind,
    pub bound_address: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub auto_start: bool,
    pub updated_at: DateTime<Utc>,
    pub runtime: PfRuntime,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PfRuleForm {
    pub id: Option<Uuid>,
    pub vault_id: Uuid,
    pub label: String,
    pub host_id: Uuid,
    pub kind: PfKind,
    #[serde(default)]
    pub bound_address: String,
    pub local_port: u16,
    #[serde(default)]
    pub remote_host: String,
    #[serde(default)]
    pub remote_port: u16,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ForwardEvent {
    Changed { id: Uuid, runtime: PfRuntime },
}

struct LiveForward {
    forward: Forward,
    started_at: DateTime<Utc>,
    /// Transport (+ jump hosts) kept alive for the tunnel's lifetime.
    #[allow(dead_code)]
    client: Arc<SshClient>,
    #[allow(dead_code)]
    jumps: Vec<Arc<SshClient>>,
    watcher: JoinHandle<()>,
}

#[derive(Default)]
pub struct Forwards {
    live: Mutex<HashMap<Uuid, Arc<LiveForward>>>,
    pending: Mutex<HashMap<Uuid, CancellationToken>>,
    errors: Mutex<HashMap<Uuid, String>>,
}

impl Forwards {
    fn begin(&self, id: Uuid) -> Result<CancellationToken> {
        if self
            .live
            .lock()
            .expect("forwards poisoned")
            .contains_key(&id)
        {
            return Err(DesktopError::invalid("forwarding rule is already running"));
        }
        let mut pending = self.pending.lock().expect("forwards poisoned");
        if pending.contains_key(&id) {
            return Err(DesktopError::invalid("forwarding rule is already starting"));
        }
        let tok = CancellationToken::new();
        pending.insert(id, tok.clone());
        self.errors.lock().expect("forwards poisoned").remove(&id);
        Ok(tok)
    }

    fn finish_pending(&self, id: Uuid) -> bool {
        self.pending
            .lock()
            .expect("forwards poisoned")
            .remove(&id)
            .is_some()
    }

    fn remove(&self, id: Uuid) -> Option<Arc<LiveForward>> {
        if let Some(tok) = self.pending.lock().expect("forwards poisoned").remove(&id) {
            tok.cancel();
        }
        self.live.lock().expect("forwards poisoned").remove(&id)
    }

    fn set_error(&self, id: Uuid, message: String) {
        self.errors
            .lock()
            .expect("forwards poisoned")
            .insert(id, message);
    }

    pub fn runtime(&self, id: Uuid) -> PfRuntime {
        if let Some(live) = self.live.lock().expect("forwards poisoned").get(&id) {
            let stats = live.forward.stats();
            let bound = match live.forward.spec() {
                ForwardSpec::Remote { bind, .. } => {
                    live.forward.remote_port().map(|p| format!("{bind}:{p}"))
                }
                _ => live.forward.local_addr().map(|a| a.to_string()),
            };
            return PfRuntime {
                state: PfState::Running,
                started_at: Some(live.started_at),
                bound,
                connections: stats.connections.load(Ordering::Relaxed),
                active: stats.active.load(Ordering::Relaxed),
                bytes_in: stats.bytes_in.load(Ordering::Relaxed),
                bytes_out: stats.bytes_out.load(Ordering::Relaxed),
                last_error: None,
            };
        }
        if self
            .pending
            .lock()
            .expect("forwards poisoned")
            .contains_key(&id)
        {
            return PfRuntime {
                state: PfState::Starting,
                ..PfRuntime::stopped(None)
            };
        }
        PfRuntime::stopped(
            self.errors
                .lock()
                .expect("forwards poisoned")
                .get(&id)
                .cloned(),
        )
    }

    pub fn running_ids(&self) -> Vec<Uuid> {
        self.live
            .lock()
            .expect("forwards poisoned")
            .keys()
            .copied()
            .collect()
    }
}

// ───────────────────────────── rules (store) ─────────────────────────────

fn validate(form: &PfRuleForm) -> Result<PfRule> {
    let label = form.label.trim();
    if label.is_empty() {
        return Err(DesktopError::invalid("label is required"));
    }
    if form.local_port == 0 {
        return Err(DesktopError::invalid(match form.kind {
            PfKind::Remote => "local port is required",
            _ => "port to listen on is required",
        }));
    }
    let bound_address = form.bound_address.trim().to_string();
    let remote_host = form.remote_host.trim().to_string();
    match form.kind {
        PfKind::Local => {
            if remote_host.is_empty() {
                return Err(DesktopError::invalid("destination host is required"));
            }
            if form.remote_port == 0 {
                return Err(DesktopError::invalid("destination port is required"));
            }
        }
        PfKind::Remote => {
            if form.remote_port == 0 {
                return Err(DesktopError::invalid("remote port is required"));
            }
        }
        PfKind::Dynamic => {}
    }
    Ok(PfRule {
        label: label.to_string(),
        host_id: form.host_id,
        kind: form.kind.as_str().to_string(),
        bound_address,
        local_port: form.local_port,
        remote_host,
        remote_port: form.remote_port,
        auto_start: form.auto_start,
    })
}

fn card(
    forwards: &Forwards,
    e: &termoso_core::model::Entity<PfRule>,
    hosts: &HashMap<Uuid, String>,
) -> Result<PfRuleCard> {
    Ok(PfRuleCard {
        id: e.id,
        vault_id: e.vault_id,
        label: e.data.label.clone(),
        host_id: e.data.host_id,
        host_label: hosts
            .get(&e.data.host_id)
            .cloned()
            .unwrap_or_else(|| "(missing host)".into()),
        kind: PfKind::parse(&e.data.kind)?,
        bound_address: e.data.bound_address.clone(),
        local_port: e.data.local_port,
        remote_host: e.data.remote_host.clone(),
        remote_port: e.data.remote_port,
        auto_start: e.data.auto_start,
        updated_at: e.updated_at,
        runtime: forwards.runtime(e.id),
    })
}

fn host_labels(store: &Store, vault_id: Option<Uuid>) -> Result<HashMap<Uuid, String>> {
    Ok(store
        .list::<Host>(vault_id)?
        .into_iter()
        .map(|h| (h.id, h.data.label))
        .collect())
}

pub fn rules(state: &AppState, vault_id: Option<Uuid>) -> Result<Vec<PfRuleCard>> {
    let hosts = host_labels(&state.store, vault_id)?;
    let mut out = Vec::new();
    for e in state.store.list::<PfRule>(vault_id)? {
        out.push(card(&state.forwards, &e, &hosts)?);
    }
    out.sort_by_key(|a| a.label.to_lowercase());
    Ok(out)
}

pub fn rule(state: &AppState, id: Uuid) -> Result<PfRuleCard> {
    let e = state.store.require::<PfRule>(id)?;
    let hosts = host_labels(&state.store, Some(e.vault_id))?;
    card(&state.forwards, &e, &hosts)
}

pub fn save(state: &AppState, form: &PfRuleForm) -> Result<PfRuleCard> {
    let data = validate(form)?;
    let host = state.store.require::<Host>(form.host_id)?;
    if host.vault_id != form.vault_id {
        return Err(DesktopError::invalid("host belongs to another vault"));
    }
    if host.data.ssh_config_id.is_none() && host.data.telnet_config_id.is_some() {
        return Err(DesktopError::invalid("telnet hosts cannot forward ports"));
    }
    let id = match form.id {
        Some(id) => {
            state.store.require::<PfRule>(id)?;
            state.store.update(id, &data)?;
            id
        }
        None => state.store.insert(form.vault_id, &data)?,
    };
    rule(state, id)
}

pub async fn delete<R: Runtime>(app: &AppHandle<R>, id: Uuid) -> Result<()> {
    stop(app, id).await?;
    let state = app.state::<AppState>();
    state.store.require::<PfRule>(id)?;
    state.store.delete(id)?;
    state
        .forwards
        .errors
        .lock()
        .expect("forwards poisoned")
        .remove(&id);
    Ok(())
}

// ───────────────────────────── runtime ─────────────────────────────

fn emit<R: Runtime>(app: &AppHandle<R>, id: Uuid) {
    let state = app.state::<AppState>();
    let runtime = state.forwards.runtime(id);
    let _ = app.emit(FORWARD_EVENT, ForwardEvent::Changed { id, runtime });
}

/// Open the tunnel for rule `id`. Prompts (host key, password…) are routed
/// under the rule id, so the UI can answer or cancel them like a session.
pub async fn start<R: Runtime>(app: &AppHandle<R>, id: Uuid) -> Result<PfRuleCard> {
    let state = app.state::<AppState>();
    let e = state.store.require::<PfRule>(id)?;
    let spec = ForwardSpec::from_rule(&e.data)?;
    let cancel = state.forwards.begin(id)?;
    emit(app, id);

    let result = tokio::select! {
        r = sessions::connect_host(app, id, e.data.host_id) => r,
        _ = cancel.cancelled() => Err(CoreError::Cancelled.into()),
    };
    state.prompts.cancel_session(id);
    let still_wanted = state.forwards.finish_pending(id);
    let conn = match result {
        Ok(c) if still_wanted => c,
        Ok(_) => {
            emit(app, id);
            return Err(CoreError::Cancelled.into());
        }
        Err(err) => {
            state.forwards.set_error(id, err.message.clone());
            emit(app, id);
            return Err(err);
        }
    };

    let forward = match Forward::start(conn.client.clone(), spec).await {
        Ok(f) => f,
        Err(err) => {
            let err: DesktopError = err.into();
            state.forwards.set_error(id, err.message.clone());
            emit(app, id);
            return Err(err);
        }
    };

    let watcher = {
        let app = app.clone();
        let client = conn.client.clone();
        tokio::spawn(async move {
            let reason = client.closed().await;
            let state = app.state::<AppState>();
            if state.forwards.remove(id).is_some() {
                state
                    .forwards
                    .set_error(id, format!("connection lost: {reason}"));
                emit(&app, id);
            }
        })
    };

    state
        .forwards
        .live
        .lock()
        .expect("forwards poisoned")
        .insert(
            id,
            Arc::new(LiveForward {
                forward,
                started_at: Utc::now(),
                client: conn.client,
                jumps: conn.jumps,
                watcher,
            }),
        );
    emit(app, id);
    rule(&state, id)
}

pub async fn stop<R: Runtime>(app: &AppHandle<R>, id: Uuid) -> Result<()> {
    let state = app.state::<AppState>();
    state.prompts.cancel_session(id);
    if let Some(live) = state.forwards.remove(id) {
        live.watcher.abort();
        live.forward.stop().await;
        let _ = live.client.disconnect().await;
        emit(app, id);
    }
    Ok(())
}

/// Start every rule flagged `auto_start` that is not already running.
/// Failures are recorded per rule and do not abort the others.
pub async fn autostart<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<Uuid>> {
    let state = app.state::<AppState>();
    let running = state.forwards.running_ids();
    let mut started = Vec::new();
    for e in state.store.list::<PfRule>(None)? {
        if e.data.auto_start && !running.contains(&e.id) && start(app, e.id).await.is_ok() {
            started.push(e.id);
        }
    }
    Ok(started)
}

/// Snapshot of every rule's runtime counters (cheap; polled by the UI).
pub fn runtimes(state: &AppState) -> Result<HashMap<Uuid, PfRuntime>> {
    Ok(state
        .store
        .list::<PfRule>(None)?
        .into_iter()
        .map(|e| (e.id, state.forwards.runtime(e.id)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(kind: PfKind) -> PfRuleForm {
        PfRuleForm {
            id: None,
            vault_id: Uuid::new_v4(),
            label: "db".into(),
            host_id: Uuid::new_v4(),
            kind,
            bound_address: String::new(),
            local_port: 5432,
            remote_host: "127.0.0.1".into(),
            remote_port: 5432,
            auto_start: false,
        }
    }

    #[test]
    fn validation_per_kind() {
        assert!(validate(&form(PfKind::Local)).is_ok());
        assert!(validate(&form(PfKind::Remote)).is_ok());
        assert!(validate(&form(PfKind::Dynamic)).is_ok());

        let mut f = form(PfKind::Local);
        f.remote_host.clear();
        assert!(validate(&f).is_err());
        let mut f = form(PfKind::Local);
        f.remote_port = 0;
        assert!(validate(&f).is_err());
        let mut f = form(PfKind::Dynamic);
        f.remote_host.clear();
        f.remote_port = 0;
        assert!(validate(&f).is_ok());
        let mut f = form(PfKind::Dynamic);
        f.local_port = 0;
        assert!(validate(&f).is_err());
        let mut f = form(PfKind::Remote);
        f.label = "  ".into();
        assert!(validate(&f).is_err());
    }

    #[test]
    fn stored_rule_converts_to_core_spec() {
        let rule = validate(&form(PfKind::Local)).unwrap();
        assert_eq!(rule.kind, "local");
        let spec = ForwardSpec::from_rule(&rule).unwrap();
        assert!(matches!(
            spec,
            ForwardSpec::Local {
                port: 5432,
                remote_port: 5432,
                ..
            }
        ));
        let rule = validate(&form(PfKind::Dynamic)).unwrap();
        assert!(matches!(
            ForwardSpec::from_rule(&rule).unwrap(),
            ForwardSpec::Dynamic { port: 5432, .. }
        ));
    }

    #[test]
    fn runtime_reports_stopped_with_last_error() {
        let f = Forwards::default();
        let id = Uuid::new_v4();
        assert_eq!(f.runtime(id).state, PfState::Stopped);
        let tok = f.begin(id).unwrap();
        assert_eq!(f.runtime(id).state, PfState::Starting);
        assert!(f.begin(id).is_err());
        assert!(f.finish_pending(id));
        assert!(!tok.is_cancelled());
        f.set_error(id, "nope".into());
        let rt = f.runtime(id);
        assert_eq!(rt.state, PfState::Stopped);
        assert_eq!(rt.last_error.as_deref(), Some("nope"));
        assert!(f.begin(id).is_ok());
        assert!(f.runtime(id).last_error.is_none());
    }
}
