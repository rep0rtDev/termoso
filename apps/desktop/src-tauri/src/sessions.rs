//! Live terminal sessions: connect (SSH / telnet / local shell), pump output
//! into an IPC channel, accept input, resize, close.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use termoso_core::error::CoreError;
use termoso_core::hostkey::KnownHosts;
use termoso_core::model::{Entity, HostSnippet, Identity, ResolvedHost, Snippet, SshConfig};
use termoso_core::pty::{LocalShellOptions, LocalTerminal};
use termoso_core::ssh::proxy::{ProxyConfig, ProxyKind};
use termoso_core::ssh::{Algorithms, AuthMethod, ConnectOptions, SshClient, SshTarget};
use termoso_core::store::{ConnectionHistory, LogMeta};
use termoso_core::telnet::{TelnetOptions, TelnetTerminal};
use termoso_core::terminal::{SharedTerminal, TermEvent, TermEvents, TermSize};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};
use crate::logs::Recorder;
use crate::prompts::{PromptAnswer, PromptRequest, UiHostKeyPrompt, UiInteractivePrompt};
use crate::snippets;
use crate::state::AppState;

pub const SESSION_EVENT: &str = "session";
const TERM: &str = "xterm-256color";
const MAX_PASSWORD_ATTEMPTS: usize = 3;

/// What the UI asked to open.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpenTarget {
    /// A saved host.
    Host { host_id: Uuid },
    /// Ad-hoc `user@host:port` typed into the quick-connect bar.
    Quick {
        address: String,
        #[serde(default)]
        username: Option<String>,
        #[serde(default)]
        port: Option<u16>,
    },
    /// Shell on this machine.
    Local,
}

/// Public view of a session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: Uuid,
    /// `ssh` | `telnet` | `local`.
    pub protocol: String,
    pub title: String,
    /// `user@host:port` or the local shell.
    pub target: String,
    pub host_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub state: SessionState,
    /// Negotiated SSH algorithms (`None` for local / telnet / still connecting).
    pub algorithms: Option<Algorithms>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Connecting,
    Connected,
}

/// Something happened to a session; emitted as the `session` event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    Connecting {
        id: Uuid,
        info: SessionInfo,
    },
    Connected {
        id: Uuid,
        info: SessionInfo,
    },
    Notice {
        id: Uuid,
        message: String,
    },
    Exit {
        id: Uuid,
        code: Option<u32>,
        signal: Option<String>,
    },
    Error {
        id: Uuid,
        message: String,
    },
    Closed {
        id: Uuid,
    },
}

struct Live {
    info: SessionInfo,
    term: SharedTerminal,
    /// SSH transport; SFTP and forwarding share it.
    client: Option<Arc<SshClient>>,
    /// Jump hosts, outermost first. Dropped with the session.
    #[allow(dead_code)]
    jumps: Vec<Arc<SshClient>>,
    output: Arc<Mutex<Channel<InvokeResponseBody>>>,
    pump: JoinHandle<()>,
    cancel: CancellationToken,
    history_id: Option<Uuid>,
    recorder: Option<Arc<Recorder>>,
}

/// Registry of sessions. Connection attempts are registered up-front so the UI
/// can cancel them and prompts can be routed.
#[derive(Default)]
pub struct Sessions {
    live: Mutex<HashMap<Uuid, Live>>,
    pending: Mutex<HashMap<Uuid, CancellationToken>>,
}

impl Sessions {
    pub fn list(&self) -> Vec<SessionInfo> {
        let mut v: Vec<SessionInfo> = self
            .live
            .lock()
            .expect("sessions poisoned")
            .values()
            .map(|l| l.info.clone())
            .collect();
        v.sort_by_key(|s| s.started_at);
        v
    }

    pub fn terminal(&self, id: Uuid) -> Result<SharedTerminal> {
        self.live
            .lock()
            .expect("sessions poisoned")
            .get(&id)
            .map(|l| l.term.clone())
            .ok_or_else(|| DesktopError::not_found(format!("session {id}")))
    }

    /// SSH transport of a live session (`None` for local / telnet).
    pub fn client(&self, id: Uuid) -> Result<Arc<SshClient>> {
        self.live
            .lock()
            .expect("sessions poisoned")
            .get(&id)
            .ok_or_else(|| DesktopError::not_found(format!("session {id}")))?
            .client
            .clone()
            .ok_or_else(|| DesktopError::invalid("session is not SSH"))
    }

    pub fn attach(&self, id: Uuid, channel: Channel<InvokeResponseBody>) -> Result<()> {
        let live = self.live.lock().expect("sessions poisoned");
        let l = live
            .get(&id)
            .ok_or_else(|| DesktopError::not_found(format!("session {id}")))?;
        *l.output.lock().expect("output poisoned") = channel;
        Ok(())
    }

    /// Cancel a connection attempt or close a live session.
    fn close(&self, id: Uuid) -> Option<Live> {
        if let Some(tok) = self.pending.lock().expect("sessions poisoned").remove(&id) {
            tok.cancel();
        }
        self.live.lock().expect("sessions poisoned").remove(&id)
    }

    fn begin(&self, id: Uuid) -> Result<CancellationToken> {
        if self
            .live
            .lock()
            .expect("sessions poisoned")
            .contains_key(&id)
        {
            return Err(DesktopError::invalid(format!("session {id} already open")));
        }
        let mut pending = self.pending.lock().expect("sessions poisoned");
        if pending.contains_key(&id) {
            return Err(DesktopError::invalid(format!(
                "session {id} already opening"
            )));
        }
        let tok = CancellationToken::new();
        pending.insert(id, tok.clone());
        Ok(tok)
    }

    fn finish_pending(&self, id: Uuid) -> bool {
        self.pending
            .lock()
            .expect("sessions poisoned")
            .remove(&id)
            .is_some()
    }
}

/// Open a session and start streaming its output into `output`. The UI may
/// pick the `id` so it can address the tab before the connection completes.
pub async fn open<R: Runtime>(
    app: AppHandle<R>,
    id: Option<Uuid>,
    target: OpenTarget,
    size: TermSize,
    output: Channel<InvokeResponseBody>,
) -> Result<SessionInfo> {
    let state = app.state::<AppState>();
    let id = id.unwrap_or_else(Uuid::new_v4);
    let cancel = state.sessions.begin(id)?;

    let result = tokio::select! {
        r = connect(&app, id, &target, size) => r,
        _ = cancel.cancelled() => Err(CoreError::Cancelled.into()),
    };
    state.prompts.cancel_session(id);
    // Cancelled while connecting: whoever cancelled already removed us.
    let still_wanted = state.sessions.finish_pending(id);

    let opened = match result {
        Ok(o) => o,
        Err(e) => {
            if still_wanted && e.kind != "cancelled" {
                let _ = app.emit(
                    SESSION_EVENT,
                    SessionEvent::Error {
                        id,
                        message: e.message.clone(),
                    },
                );
            }
            return Err(e);
        }
    };
    if !still_wanted {
        let _ = opened.term.close().await;
        return Err(CoreError::Cancelled.into());
    }

    let info = SessionInfo {
        id,
        protocol: opened.protocol.into(),
        title: opened.title,
        target: opened.target,
        host_id: opened.host_id,
        started_at: Utc::now(),
        state: SessionState::Connected,
        algorithms: opened.client.as_ref().and_then(|c| c.algorithms().cloned()),
    };
    let history_id = state
        .store
        .record_connection(&ConnectionHistory {
            host_id: info.host_id,
            label: info.title.clone(),
            target: info.target.clone(),
            protocol: info.protocol.clone(),
            duration_secs: None,
            error: None,
        })
        .ok();
    let recorder = start_recording(&state, &info, opened.vault_id, size);

    let output = Arc::new(Mutex::new(output));
    let pump = tokio::spawn(pump(
        app.clone(),
        id,
        opened.events,
        output.clone(),
        recorder.clone(),
    ));
    let live = Live {
        info: info.clone(),
        term: opened.term.clone(),
        client: opened.client,
        jumps: opened.jumps,
        output,
        pump,
        cancel,
        history_id,
        recorder,
    };
    state
        .sessions
        .live
        .lock()
        .expect("sessions poisoned")
        .insert(id, live);
    let _ = app.emit(
        SESSION_EVENT,
        SessionEvent::Connected {
            id,
            info: info.clone(),
        },
    );
    if !opened.startup.is_empty() {
        let term = opened.term;
        let script = opened.startup;
        tokio::spawn(async move {
            tokio::time::sleep(STARTUP_SNIPPET_DELAY).await;
            if let Err(e) = term.write(script.as_bytes()).await {
                tracing::debug!(session = %id, "startup snippet not delivered: {e}");
            }
        });
    }
    Ok(info)
}

/// Begin capturing output when the user enabled recording. Failures only
/// disable the recording for this session.
fn start_recording(
    state: &AppState,
    info: &SessionInfo,
    vault_id: Option<Uuid>,
    size: TermSize,
) -> Option<Arc<Recorder>> {
    let settings = state.settings().ok()?;
    if !settings.record_sessions {
        return None;
    }
    let vault_id = match vault_id {
        Some(v) => v,
        None => state.store.local_vault().ok()?.id,
    };
    let meta = LogMeta {
        host_id: info.host_id,
        label: info.title.clone(),
        target: info.target.clone(),
        protocol: info.protocol.clone(),
        started_at: info.started_at,
        ended_at: None,
        cols: size.cols,
        rows: size.rows,
    };
    match Recorder::begin(&state.store, vault_id, meta) {
        Ok(r) => {
            tracing::debug!(session = %info.id, log = %r.id(), "recording session");
            Some(Arc::new(r))
        }
        Err(e) => {
            tracing::warn!("session recording disabled: {e}");
            None
        }
    }
}

fn finish_recording(state: &AppState, recorder: Option<Arc<Recorder>>) {
    if let Some(rec) = recorder {
        if let Err(e) = rec.finish(&state.store, &state.logs_dir()) {
            tracing::warn!("saving session recording failed: {e}");
        }
        if let Ok(settings) = state.settings()
            && let Err(e) = crate::logs::prune(&state.store, settings.log_retention_days)
        {
            tracing::debug!("log retention sweep failed: {e}");
        }
    }
}

/// Startup snippet of the host followed by its bound snippets, in order.
fn startup_script(state: &AppState, resolved: &ResolvedHost) -> String {
    let mut ids: Vec<Uuid> = resolved.host.data.startup_snippet_id.into_iter().collect();
    if let Ok(mut bound) = state
        .store
        .list::<HostSnippet>(Some(resolved.host.vault_id))
    {
        bound.retain(|b| b.data.host_id == resolved.host.id);
        bound.sort_by_key(|b| b.data.sort_order);
        ids.extend(bound.into_iter().map(|b| b.data.snippet_id));
    }
    let mut out = String::new();
    for id in ids {
        if let Ok(s) = state.store.require::<Snippet>(id) {
            out.push_str(&snippets::script_to_send(&s.data.script));
        }
    }
    out
}

/// Close a session (or abort its connection attempt).
pub async fn close<R: Runtime>(app: &AppHandle<R>, id: Uuid) -> Result<()> {
    let state = app.state::<AppState>();
    state.prompts.cancel_session(id);
    if let Some(live) = state.sessions.close(id) {
        live.cancel.cancel();
        let _ = live.term.close().await;
        live.pump.abort();
        if let Some(hid) = live.history_id {
            finish_history(&state, hid, &live.info, None);
        }
        finish_recording(&state, live.recorder);
        let _ = app.emit(SESSION_EVENT, SessionEvent::Closed { id });
    }
    Ok(())
}

fn finish_history(state: &AppState, history_id: Uuid, info: &SessionInfo, error: Option<String>) {
    let duration = Utc::now()
        .signed_duration_since(info.started_at)
        .num_seconds()
        .max(0) as u64;
    let _ = state.store.update_connection(
        history_id,
        &ConnectionHistory {
            host_id: info.host_id,
            label: info.title.clone(),
            target: info.target.clone(),
            protocol: info.protocol.clone(),
            duration_secs: Some(duration),
            error,
        },
    );
}

async fn pump<R: Runtime>(
    app: AppHandle<R>,
    id: Uuid,
    mut events: TermEvents,
    output: Arc<Mutex<Channel<InvokeResponseBody>>>,
    recorder: Option<Arc<Recorder>>,
) {
    let mut error: Option<String> = None;
    while let Some(ev) = events.recv().await {
        match ev {
            TermEvent::Output(bytes) => {
                if let Some(rec) = &recorder {
                    rec.append(&bytes);
                }
                let ch = output.lock().expect("output poisoned").clone();
                if let Err(e) = ch.send(InvokeResponseBody::Raw(bytes.to_vec())) {
                    tracing::debug!(error = %e, "terminal channel gone");
                }
            }
            TermEvent::Notice(message) => {
                let _ = app.emit(SESSION_EVENT, SessionEvent::Notice { id, message });
            }
            TermEvent::Exit { code, signal } => {
                let _ = app.emit(SESSION_EVENT, SessionEvent::Exit { id, code, signal });
            }
            TermEvent::Error(message) => {
                error = Some(message.clone());
                let _ = app.emit(SESSION_EVENT, SessionEvent::Error { id, message });
            }
            TermEvent::Closed => break,
        }
    }
    let state = app.state::<AppState>();
    if let Some(live) = state.sessions.close(id) {
        if let Some(hid) = live.history_id {
            finish_history(&state, hid, &live.info, error);
        }
        finish_recording(&state, live.recorder);
    }
    let _ = app.emit(SESSION_EVENT, SessionEvent::Closed { id });
}

const STARTUP_SNIPPET_DELAY: Duration = Duration::from_millis(400);

struct Opened {
    protocol: &'static str,
    title: String,
    target: String,
    host_id: Option<Uuid>,
    /// Vault of the host (recordings are stored alongside it).
    vault_id: Option<Uuid>,
    term: SharedTerminal,
    events: TermEvents,
    client: Option<Arc<SshClient>>,
    jumps: Vec<Arc<SshClient>>,
    /// Script typed into the shell right after connecting.
    startup: String,
}

async fn connect<R: Runtime>(
    app: &AppHandle<R>,
    id: Uuid,
    target: &OpenTarget,
    size: TermSize,
) -> Result<Opened> {
    let state = app.state::<AppState>();
    match target {
        OpenTarget::Local => {
            let (term, events) = LocalTerminal::spawn(LocalShellOptions {
                argv: Vec::new(),
                cwd: dirs_home(),
                env: vec![("TERM".into(), TERM.into())],
                size,
            })?;
            Ok(Opened {
                protocol: "local",
                title: "Local".into(),
                target: "local shell".into(),
                host_id: None,
                vault_id: None,
                term,
                events,
                client: None,
                jumps: Vec::new(),
                startup: String::new(),
            })
        }
        OpenTarget::Quick {
            address,
            username,
            port,
        } => {
            let (user, host, p) = parse_quick(address, username.as_deref(), *port)?;
            let target = SshTarget {
                host,
                port: p,
                username: user,
            };
            let display = target.display();
            emit_connecting(app, id, "ssh", &display, &display, None);
            let (client, jumps) = ssh_connect(app, id, target, None, &[], None).await?;
            let (term, events) = client.shell(TERM, size).await?;
            Ok(Opened {
                protocol: "ssh",
                title: display.clone(),
                target: display,
                host_id: None,
                vault_id: None,
                term,
                events,
                client: Some(client),
                jumps,
                startup: String::new(),
            })
        }
        OpenTarget::Host { host_id } => {
            let resolved = state.store.resolve_host(*host_id)?;
            let label = resolved.host.data.label.clone();
            let is_telnet = resolved.telnet.is_some() && resolved.host.data.ssh_config_id.is_none();
            if is_telnet {
                let telnet = resolved.telnet.clone().unwrap_or_default();
                let port = telnet.port.unwrap_or(23);
                let display = format!("{}:{}", resolved.host.data.address, port);
                emit_connecting(app, id, "telnet", &label, &display, Some(*host_id));
                let (term, events) = TelnetTerminal::connect(TelnetOptions {
                    host: resolved.host.data.address.clone(),
                    port,
                    term: TERM.into(),
                    size,
                    timeout: Duration::from_secs(20),
                })
                .await?;
                return Ok(Opened {
                    protocol: "telnet",
                    title: label,
                    target: display,
                    host_id: Some(*host_id),
                    vault_id: Some(resolved.host.vault_id),
                    term,
                    events,
                    client: None,
                    jumps: Vec::new(),
                    startup: startup_script(&state, &resolved),
                });
            }
            let target = ssh_target(&resolved);
            let display = target.display();
            emit_connecting(app, id, "ssh", &label, &display, Some(*host_id));
            let (client, jumps) = connect_resolved(app, id, &resolved).await?;
            let (term, events) = client.shell(TERM, size).await?;
            detect_os_in_background(app, &resolved, client.clone());
            Ok(Opened {
                protocol: "ssh",
                title: label,
                target: display,
                host_id: Some(*host_id),
                vault_id: Some(resolved.host.vault_id),
                term,
                events,
                client: Some(client),
                jumps,
                startup: startup_script(&state, &resolved),
            })
        }
    }
}

/// An SSH transport to a saved host, without a shell (used by SFTP).
pub struct HostConnection {
    pub label: String,
    pub display: String,
    pub client: Arc<SshClient>,
    pub jumps: Vec<Arc<SshClient>>,
}

/// Connect to a saved SSH host, asking the UI for anything missing. Prompts
/// are routed under `session_id`.
pub async fn connect_host<R: Runtime>(
    app: &AppHandle<R>,
    session_id: Uuid,
    host_id: Uuid,
) -> Result<HostConnection> {
    let state = app.state::<AppState>();
    let resolved = state.store.resolve_host(host_id)?;
    if resolved.telnet.is_some() && resolved.host.data.ssh_config_id.is_none() {
        return Err(DesktopError::invalid("telnet hosts have no SFTP"));
    }
    let display = ssh_target(&resolved).display();
    let (client, jumps) = connect_resolved(app, session_id, &resolved).await?;
    detect_os_in_background(app, &resolved, client.clone());
    Ok(HostConnection {
        label: resolved.host.data.label.clone(),
        display,
        client,
        jumps,
    })
}

/// Fill in `os_name` for a host that has none yet, so its card gets the
/// right icon after the first successful connection. Runs off the session
/// path: a slow or refused probe never delays the shell, and failures are
/// silently dropped (the next connection tries again).
fn detect_os_in_background<R: Runtime>(
    app: &AppHandle<R>,
    resolved: &ResolvedHost,
    client: Arc<SshClient>,
) {
    let state = app.state::<AppState>();
    let enabled = state.settings().map(|s| s.detect_os).unwrap_or(true);
    if !enabled || resolved.host.data.os_name.is_some() {
        return;
    }
    let app = app.clone();
    let host_id = resolved.host.id;
    tauri::async_runtime::spawn(async move {
        let Some(os) = termoso_core::osdetect::detect(&client).await else {
            return;
        };
        let state = app.state::<AppState>();
        let saved = (|| -> Result<Uuid> {
            let mut host = state.store.require::<termoso_core::model::Host>(host_id)?;
            if host.data.os_name.is_some() {
                return Ok(host.vault_id);
            }
            host.data.os_name = Some(os.to_string());
            state.store.update(host.id, &host.data)?;
            Ok(host.vault_id)
        })();
        match saved {
            Ok(vault_id) => {
                let _ = app.emit(
                    crate::account::SYNC_EVENT,
                    crate::account::SyncNotice::EntitiesChanged { vault_id },
                );
            }
            Err(e) => tracing::debug!("saving detected os failed: {e}"),
        }
    });
}

fn ssh_target(resolved: &ResolvedHost) -> SshTarget {
    SshTarget {
        host: resolved.host.data.address.clone(),
        port: resolved.port(),
        username: resolved.username(),
    }
}

async fn connect_resolved<R: Runtime>(
    app: &AppHandle<R>,
    session_id: Uuid,
    resolved: &ResolvedHost,
) -> Result<(Arc<SshClient>, Vec<Arc<SshClient>>)> {
    ssh_connect(
        app,
        session_id,
        ssh_target(resolved),
        Some(resolved),
        &resolved.chain,
        None,
    )
    .await
}

fn emit_connecting<R: Runtime>(
    app: &AppHandle<R>,
    id: Uuid,
    protocol: &str,
    title: &str,
    target: &str,
    host_id: Option<Uuid>,
) {
    let _ = app.emit(
        SESSION_EVENT,
        SessionEvent::Connecting {
            id,
            info: SessionInfo {
                id,
                protocol: protocol.into(),
                title: title.into(),
                target: target.into(),
                host_id,
                started_at: Utc::now(),
                state: SessionState::Connecting,
                algorithms: None,
            },
        },
    );
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(Into::into)
}

/// `[user@]host[:port]` → (user, host, port). IPv6 literals go in brackets.
fn parse_quick(
    address: &str,
    username: Option<&str>,
    port: Option<u16>,
) -> Result<(String, String, u16)> {
    let s = address.trim();
    if s.is_empty() {
        return Err(DesktopError::invalid("address is empty"));
    }
    let (user_part, rest) = match s.rsplit_once('@') {
        Some((u, r)) => (Some(u), r),
        None => (None, s),
    };
    let (host, port_part) = if let Some(r) = rest.strip_prefix('[') {
        let (h, tail) = r
            .split_once(']')
            .ok_or_else(|| DesktopError::invalid("unterminated IPv6 literal"))?;
        (h.to_string(), tail.strip_prefix(':'))
    } else if rest.matches(':').count() == 1 {
        let (h, p) = rest.split_once(':').unwrap_or((rest, ""));
        (h.to_string(), Some(p))
    } else {
        (rest.to_string(), None)
    };
    if host.is_empty() {
        return Err(DesktopError::invalid("host is empty"));
    }
    let port = match (port, port_part) {
        (Some(p), _) => p,
        (None, Some(p)) if !p.is_empty() => p
            .parse()
            .map_err(|_| DesktopError::invalid(format!("bad port {p}")))?,
        _ => 22,
    };
    let user = username
        .map(str::to_string)
        .or_else(|| user_part.map(str::to_string))
        .filter(|u| !u.is_empty())
        .or_else(|| std::env::var("USER").ok())
        .or_else(|| std::env::var("USERNAME").ok())
        .unwrap_or_else(|| "root".into());
    Ok((user, host, port))
}

/// Connect to `target`, going through `chain` jump hosts first. Asks the UI
/// for a password/passphrase when the stored credentials are not enough.
async fn ssh_connect<R: Runtime>(
    app: &AppHandle<R>,
    session_id: Uuid,
    target: SshTarget,
    resolved: Option<&ResolvedHost>,
    chain: &[Entity<termoso_core::model::Host>],
    jump: Option<Arc<SshClient>>,
) -> Result<(Arc<SshClient>, Vec<Arc<SshClient>>)> {
    let state = app.state::<AppState>();
    let mut jumps: Vec<Arc<SshClient>> = Vec::new();

    // Jump hosts first (each resolved on its own, one level deep).
    let mut via = jump;
    for hop in chain {
        let hop_resolved = state.store.resolve_host(hop.id)?;
        let hop_target = SshTarget {
            host: hop_resolved.host.data.address.clone(),
            port: hop_resolved.port(),
            username: hop_resolved.username(),
        };
        let (client, _) = Box::pin(ssh_connect(
            app,
            session_id,
            hop_target,
            Some(&hop_resolved),
            &[],
            via.clone(),
        ))
        .await?;
        jumps.push(client.clone());
        via = Some(client);
    }

    let known_hosts = KnownHosts::new(state.store.clone(), state.store.local_vault()?.id);
    let display = target.display();
    let ssh_cfg = resolved.map(|r| r.ssh.clone()).unwrap_or_default();
    let identity = resolved.and_then(|r| r.identity.clone());
    let mut password: Option<Zeroizing<String>> = identity
        .as_ref()
        .and_then(|i| i.data.password.clone())
        .filter(|p| !p.is_empty())
        .map(Zeroizing::new);
    let mut passphrase: Option<Zeroizing<String>> = resolved
        .and_then(|r| r.key.as_ref())
        .and_then(|k| k.data.passphrase.clone())
        .filter(|p| !p.is_empty())
        .map(Zeroizing::new);
    let proxy = match resolved.and_then(|r| r.proxy.as_ref()) {
        Some(p) => Some(proxy_config(&state, &p.data)?),
        None => None,
    };

    let mut attempts = 0;
    loop {
        let mut auth: Vec<AuthMethod> = Vec::new();
        if let Some(key) = resolved.and_then(|r| r.key.as_ref()) {
            auth.push(AuthMethod::Key {
                private_key: Zeroizing::new(key.data.private_key.clone()),
                passphrase: passphrase.clone(),
                certificate: resolved
                    .and_then(|r| r.certificate.as_ref())
                    .map(|c| c.data.certificate.clone()),
            });
        }
        auth.push(AuthMethod::Agent);
        if let Some(pw) = &password {
            auth.push(AuthMethod::Password(pw.clone()));
        }
        auth.push(AuthMethod::KeyboardInteractive);

        let interactive: Option<Arc<dyn termoso_core::ssh::InteractivePrompt>> = match password {
            Some(_) => None,
            None => Some(Arc::new(UiInteractivePrompt {
                app: app.clone(),
                session_id,
                target: display.clone(),
            })),
        };
        let opts = ConnectOptions {
            target: target.clone(),
            auth,
            known_hosts: known_hosts.clone(),
            host_key_prompt: Arc::new(UiHostKeyPrompt {
                app: app.clone(),
                session_id,
                target: display.clone(),
            }),
            interactive,
            keepalive: keepalive(&state, &ssh_cfg),
            timeout: Duration::from_secs(ssh_cfg.timeout.unwrap_or(20).clamp(1, 600) as u64),
            proxy: proxy.clone(),
            env: ssh_cfg.env_variables.clone(),
            agent_forwarding: ssh_cfg.agent_forwarding,
            post_quantum_kex: state.settings().map(|s| s.post_quantum_kex).unwrap_or(true),
        };

        let result = match &via {
            Some(j) => SshClient::connect_via(j, opts).await,
            None => SshClient::connect(opts).await,
        };
        match result {
            Ok(client) => return Ok((Arc::new(client), jumps)),
            Err(CoreError::AuthFailed { remaining })
                if attempts < MAX_PASSWORD_ATTEMPTS
                    && remaining
                        .iter()
                        .any(|m| m == "password" || m == "keyboard-interactive") =>
            {
                attempts += 1;
                let answer = state
                    .prompts
                    .ask(
                        app,
                        session_id,
                        display.clone(),
                        PromptRequest::Password {
                            username: target.username.clone(),
                            retry: password.is_some(),
                        },
                    )
                    .await;
                match answer {
                    Some(PromptAnswer::Secret { value, remember }) => {
                        if remember && let Some(r) = resolved {
                            remember_password(&state, r, &value);
                        }
                        password = Some(value);
                    }
                    _ => return Err(CoreError::Cancelled.into()),
                }
            }
            Err(CoreError::Key(msg))
                if attempts < MAX_PASSWORD_ATTEMPTS && msg.contains("passphrase") =>
            {
                attempts += 1;
                let label = resolved
                    .and_then(|r| r.key.as_ref())
                    .map(|k| k.data.label.clone())
                    .unwrap_or_default();
                let answer = state
                    .prompts
                    .ask(
                        app,
                        session_id,
                        display.clone(),
                        PromptRequest::Passphrase { key_label: label },
                    )
                    .await;
                match answer {
                    Some(PromptAnswer::Secret { value, remember }) => {
                        if remember && let Some(key) = resolved.and_then(|r| r.key.as_ref()) {
                            let mut data = key.data.clone();
                            data.passphrase = Some(value.to_string());
                            let _ = state.store.update(key.id, &data);
                        }
                        passphrase = Some(value);
                    }
                    _ => return Err(CoreError::Cancelled.into()),
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn keepalive(state: &AppState, cfg: &SshConfig) -> Option<Duration> {
    let secs = cfg
        .keep_alive_interval
        .or_else(|| state.settings().ok().map(|s| s.keep_alive_seconds))
        .unwrap_or(30);
    (secs > 0).then(|| Duration::from_secs(secs as u64))
}

fn proxy_config(state: &AppState, p: &termoso_core::model::Proxy) -> Result<ProxyConfig> {
    let kind = ProxyKind::parse(&p.kind)
        .ok_or_else(|| DesktopError::invalid(format!("unsupported proxy type {}", p.kind)))?;
    let (username, password) = match p.identity_id {
        Some(id) => {
            let ident = state.store.get::<Identity>(id)?;
            (
                ident.as_ref().map(|i| i.data.username.clone()),
                ident
                    .and_then(|i| i.data.password)
                    .filter(|p| !p.is_empty())
                    .map(Zeroizing::new),
            )
        }
        None => (None, None),
    };
    Ok(ProxyConfig {
        kind,
        host: p.host.clone(),
        port: p.port,
        username,
        password,
    })
}

/// Store a password the user asked to remember: on the host's identity when
/// it has one, otherwise on a new hidden identity attached to the host's SSH
/// config (created if missing).
fn remember_password(state: &AppState, resolved: &ResolvedHost, value: &Zeroizing<String>) {
    let store = &state.store;
    let result = (|| -> termoso_core::error::Result<()> {
        if let Some(ident) = &resolved.identity {
            let mut data = ident.data.clone();
            data.password = Some(value.to_string());
            return store.update(ident.id, &data);
        }
        let vault_id = resolved.host.vault_id;
        let identity_id = store.insert(
            vault_id,
            &Identity {
                label: format!("{}@{}", resolved.username(), resolved.host.data.address),
                username: resolved.username(),
                password: Some(value.to_string()),
                ssh_key_id: None,
                ssh_certificate_id: None,
                is_visible: false,
            },
        )?;
        match resolved.host.data.ssh_config_id {
            Some(cfg_id) => {
                let cfg = store.require::<SshConfig>(cfg_id)?;
                let mut data = cfg.data;
                data.identity_id = Some(identity_id);
                store.update(cfg_id, &data)
            }
            None => {
                let cfg_id = store.insert(
                    vault_id,
                    &SshConfig {
                        identity_id: Some(identity_id),
                        ..resolved.ssh.clone()
                    },
                )?;
                let mut host = resolved.host.data.clone();
                host.ssh_config_id = Some(cfg_id);
                store.update(resolved.host.id, &host)
            }
        }
    })();
    if let Err(e) = result {
        tracing::warn!(error = %e, "could not remember password");
    }
}
