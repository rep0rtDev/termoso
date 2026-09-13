//! An interactive SSH session: connect (jump hosts, proxy, saved credentials,
//! prompts for anything missing), open a shell, pump the byte stream into
//! the emulator and tell Kotlin when to repaint.
//!
//! Everything the UI needs to answer — host key confirmation, password,
//! passphrase, keyboard-interactive — arrives as a [`PromptRequest`] on the
//! listener and is answered through [`SshSession::answer`]; the Rust side
//! blocks its own connection task, never a UI thread.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use termoso_core::error::CoreError;
use termoso_core::hostkey::{
    HostKeyDecision, HostKeyInfo, HostKeyPrompt, HostKeyVerdict, KnownHosts,
};
use termoso_core::model::{Identity, ResolvedHost, SshConfig};
use termoso_core::ssh::proxy::{ProxyConfig, ProxyKind};
use termoso_core::ssh::{
    AuthMethod, ConnectOptions, ConnectPhase, ConnectProgress, InteractivePrompt,
    InteractiveQuestion, IpVersion, SshClient, SshTarget, SshTerminal,
};
use termoso_core::store::{ConnectionHistory, Store};
use termoso_core::terminal::{TermEvent, TermEvents, TermSize, TerminalSession};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{MobileError, Result};
use crate::keys::{KeyMods, SpecialKey, encode_key, encode_text};
use crate::settings::MobileSettings;
use crate::terminal::{Emulator, GridFrame, GridSnapshot, TermSignal, TerminalPalette};

const MAX_PASSWORD_ATTEMPTS: u32 = 3;

/// Where the session is.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SessionState {
    /// Connecting; `detail` names the stage (`Resolving…`, `Authenticating (password)…`).
    Connecting {
        detail: String,
    },
    Connected,
    /// The remote shell ended or the connection dropped.
    Closed {
        reason: Option<String>,
    },
    Failed {
        kind: String,
        message: String,
    },
}

/// Something only the user can answer.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum PromptRequest {
    /// First contact with this host: show the fingerprint, ask to trust.
    HostKeyUnknown {
        host: String,
        key_type: String,
        fingerprint: String,
    },
    /// The pinned key differs. Loud warning.
    HostKeyChanged {
        host: String,
        key_type: String,
        old_fingerprint: String,
        new_fingerprint: String,
    },
    Password {
        username: String,
        retry: bool,
    },
    Passphrase {
        key_label: String,
        retry: bool,
    },
    /// Server-driven keyboard-interactive dialog.
    KeyboardInteractive {
        name: String,
        instructions: String,
        questions: Vec<InteractiveQuestionInfo>,
    },
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct InteractiveQuestionInfo {
    pub prompt: String,
    /// Input may be shown while typing.
    pub echo: bool,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PromptAnswer {
    Cancel,
    /// Host key: reject / accept once / accept and save.
    HostKey {
        decision: HostKeyChoice,
    },
    /// Password or passphrase; `remember` stores it in the vault.
    Secret {
        value: String,
        remember: bool,
    },
    /// One answer per keyboard-interactive question, in order.
    Answers {
        values: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HostKeyChoice {
    Reject,
    AcceptOnce,
    AcceptAndSave,
}

/// Callbacks into Kotlin. Invoked from Rust worker threads; keep them quick
/// (post to the main thread, do not block).
#[uniffi::export(with_foreign)]
pub trait SessionListener: Send + Sync {
    fn on_state(&self, state: SessionState);
    /// The grid changed; fetch a fresh [`GridSnapshot`].
    fn on_render(&self);
    /// Answer with [`SshSession::answer`] using the same `prompt_id`.
    fn on_prompt(&self, prompt_id: u64, request: PromptRequest);
    fn on_title(&self, title: Option<String>);
    fn on_bell(&self);
    /// OSC 52 copy request from the remote.
    fn on_clipboard(&self, text: String);
    /// The remote OS was recognised (stored on the host when there is one).
    fn on_os_detected(&self, os_name: String);
}

/// How to open the terminal.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TerminalOptions {
    pub cols: u16,
    pub rows: u16,
    /// `TERM` value; empty → from settings (default `xterm-256color`).
    pub term_type: String,
    pub palette: Option<TerminalPalette>,
}

type PromptTx = oneshot::Sender<PromptAnswer>;

struct PromptBroker {
    next: AtomicU64,
    pending: Mutex<HashMap<u64, PromptTx>>,
}

impl PromptBroker {
    fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
        }
    }

    async fn ask(
        &self,
        listener: &Arc<dyn SessionListener>,
        request: PromptRequest,
    ) -> Option<PromptAnswer> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("prompts poisoned")
            .insert(id, tx);
        listener.on_prompt(id, request);
        match rx.await {
            Ok(PromptAnswer::Cancel) | Err(_) => None,
            Ok(a) => Some(a),
        }
    }

    fn answer(&self, id: u64, answer: PromptAnswer) -> bool {
        match self.pending.lock().expect("prompts poisoned").remove(&id) {
            Some(tx) => tx.send(answer).is_ok(),
            None => false,
        }
    }

    fn cancel_all(&self) {
        self.pending.lock().expect("prompts poisoned").clear();
    }
}

struct Inner {
    store: Arc<Store>,
    listener: Arc<dyn SessionListener>,
    prompts: PromptBroker,
    emulator: Mutex<Emulator>,
    terminal: Mutex<Option<Arc<SshTerminal>>>,
    /// Bytes typed before the shell is up are queued.
    pending_input: Mutex<Vec<u8>>,
    state: Mutex<SessionState>,
    /// Set when the user dismissed a prompt; turns the resulting connect
    /// error into `Cancelled`.
    cancelled: AtomicBool,
    closed: tokio::sync::Notify,
}

/// A live terminal. Drop-safe: dropping the last reference closes the
/// connection.
#[derive(uniffi::Object)]
pub struct SshSession {
    id: Uuid,
    inner: Arc<Inner>,
    runtime: tokio::runtime::Handle,
}

pub(crate) struct Launch {
    pub store: Arc<Store>,
    pub target: SshTarget,
    pub resolved: Option<ResolvedHost>,
    pub settings: MobileSettings,
    pub options: TerminalOptions,
    pub listener: Arc<dyn SessionListener>,
}

impl SshSession {
    pub(crate) fn launch(runtime: tokio::runtime::Handle, launch: Launch) -> Arc<Self> {
        let Launch {
            store,
            target,
            resolved,
            settings,
            options,
            listener,
        } = launch;
        let palette = options
            .palette
            .clone()
            .unwrap_or_else(TerminalPalette::termoso_dark);
        let (emulator, signals) = Emulator::new(
            options.cols,
            options.rows,
            settings.scrollback_lines,
            palette,
        );
        let inner = Arc::new(Inner {
            store,
            listener,
            prompts: PromptBroker::new(),
            emulator: Mutex::new(emulator),
            terminal: Mutex::new(None),
            pending_input: Mutex::new(Vec::new()),
            state: Mutex::new(SessionState::Connecting {
                detail: "Connecting…".into(),
            }),
            cancelled: AtomicBool::new(false),
            closed: tokio::sync::Notify::new(),
        });
        let session = Arc::new(Self {
            id: Uuid::new_v4(),
            inner: inner.clone(),
            runtime: runtime.clone(),
        });
        let term_type = if options.term_type.trim().is_empty() {
            settings.term_type.clone()
        } else {
            options.term_type.clone()
        };
        runtime.spawn(run(
            inner,
            target,
            resolved,
            settings,
            term_type,
            TermSize {
                cols: options.cols.max(2),
                rows: options.rows.max(1),
            },
            signals,
        ));
        session
    }
}

#[uniffi::export]
impl SshSession {
    pub fn id(&self) -> String {
        self.id.to_string()
    }

    pub fn state(&self) -> SessionState {
        self.inner.state.lock().expect("state poisoned").clone()
    }

    /// Current frame.
    pub fn snapshot(&self) -> GridSnapshot {
        self.inner
            .emulator
            .lock()
            .expect("emulator poisoned")
            .snapshot()
    }

    /// Current frame, packed for the FFI (see [`GridFrame`]).
    pub fn frame(&self) -> GridFrame {
        self.snapshot().pack()
    }

    /// Encode a special key for the terminal's current cursor mode and send
    /// it.
    pub fn send_key(&self, key: SpecialKey, mods: KeyMods) {
        let app_cursor = self
            .inner
            .emulator
            .lock()
            .expect("emulator poisoned")
            .mode()
            .contains(alacritty_terminal::term::TermMode::APP_CURSOR);
        self.write(encode_key(key, mods, app_cursor));
    }

    /// Send typed text, applying Ctrl/Alt from the key panel.
    pub fn send_text(&self, text: String, mods: KeyMods) {
        self.write(encode_text(&text, mods));
    }

    /// Visible screen as text (for copy / accessibility).
    pub fn visible_text(&self) -> Vec<String> {
        self.inner
            .emulator
            .lock()
            .expect("emulator poisoned")
            .visible_text()
    }

    /// Send raw bytes to the remote (key presses already encoded).
    pub fn write(&self, data: Vec<u8>) {
        let term = self
            .inner
            .terminal
            .lock()
            .expect("terminal poisoned")
            .clone();
        match term {
            Some(t) => {
                self.runtime.spawn(async move {
                    let _ = t.write(&data).await;
                });
            }
            None => self
                .inner
                .pending_input
                .lock()
                .expect("input poisoned")
                .extend_from_slice(&data),
        }
    }

    /// Send text as typed; wraps it in bracketed-paste markers when the
    /// application asked for them.
    pub fn paste(&self, text: String) {
        let bracketed = self
            .inner
            .emulator
            .lock()
            .expect("emulator poisoned")
            .mode()
            .contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE);
        let mut data = Vec::with_capacity(text.len() + 12);
        if bracketed {
            data.extend_from_slice(b"\x1b[200~");
        }
        data.extend_from_slice(text.replace("\r\n", "\r").replace('\n', "\r").as_bytes());
        if bracketed {
            data.extend_from_slice(b"\x1b[201~");
        }
        self.write(data);
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.inner
            .emulator
            .lock()
            .expect("emulator poisoned")
            .resize(cols, rows);
        let term = self
            .inner
            .terminal
            .lock()
            .expect("terminal poisoned")
            .clone();
        if let Some(t) = term {
            self.runtime.spawn(async move {
                let _ = t.resize(TermSize { cols, rows }).await;
            });
        }
        self.inner.listener.on_render();
    }

    /// Scroll the view: positive = towards history, negative = towards live.
    pub fn scroll(&self, lines: i32) {
        self.inner
            .emulator
            .lock()
            .expect("emulator poisoned")
            .scroll(lines);
        self.inner.listener.on_render();
    }

    pub fn scroll_to_bottom(&self) {
        self.inner
            .emulator
            .lock()
            .expect("emulator poisoned")
            .scroll_to_bottom();
        self.inner.listener.on_render();
    }

    pub fn set_palette(&self, palette: TerminalPalette) {
        self.inner
            .emulator
            .lock()
            .expect("emulator poisoned")
            .set_palette(palette);
        self.inner.listener.on_render();
    }

    /// Reply to a prompt. Returns `false` if the prompt is no longer waiting.
    pub fn answer(&self, prompt_id: u64, answer: PromptAnswer) -> bool {
        self.inner.prompts.answer(prompt_id, answer)
    }

    /// Tear the connection down; the object stays usable for `state()`/`snapshot()`.
    pub fn disconnect(&self) {
        self.inner.prompts.cancel_all();
        self.inner.closed.notify_waiters();
        self.inner.closed.notify_one();
        let term = self
            .inner
            .terminal
            .lock()
            .expect("terminal poisoned")
            .take();
        if let Some(t) = term {
            self.runtime.spawn(async move {
                let _ = t.close().await;
            });
        }
    }
}

impl Drop for SshSession {
    fn drop(&mut self) {
        self.disconnect();
    }
}

fn set_state(inner: &Inner, state: SessionState) {
    *inner.state.lock().expect("state poisoned") = state.clone();
    inner.listener.on_state(state);
}

fn phase_label(phase: &ConnectPhase, hop: Option<&str>) -> String {
    let base = match phase {
        ConnectPhase::Resolving => "Resolving host…".to_string(),
        ConnectPhase::Connecting { via } => format!("Connecting to {via}…"),
        ConnectPhase::Handshake => "Handshake…".into(),
        ConnectPhase::HostKey => "Checking host key…".into(),
        ConnectPhase::Auth { method } => format!("Authenticating ({method})…"),
        ConnectPhase::SecurityKeyTouch { .. } => "Touch your security key…".into(),
        ConnectPhase::Authenticated => "Opening shell…".into(),
        ConnectPhase::MoshServer => "Starting mosh-server…".into(),
    };
    match hop {
        Some(h) => format!("{h}: {base}"),
        None => base,
    }
}

struct Progress {
    inner: Arc<Inner>,
    hop: Option<String>,
}

impl ConnectProgress for Progress {
    fn phase(&self, phase: ConnectPhase) {
        set_state(
            &self.inner,
            SessionState::Connecting {
                detail: phase_label(&phase, self.hop.as_deref()),
            },
        );
    }
}

struct HostKeyAsk {
    inner: Arc<Inner>,
}

fn key_host(info: &HostKeyInfo) -> String {
    info.host.clone()
}

impl HostKeyPrompt for HostKeyAsk {
    fn decide(
        &self,
        verdict: HostKeyVerdict,
    ) -> Pin<Box<dyn Future<Output = HostKeyDecision> + Send + '_>> {
        Box::pin(async move {
            let request = match &verdict {
                HostKeyVerdict::Known => return HostKeyDecision::AcceptOnce,
                HostKeyVerdict::Unknown { key } => PromptRequest::HostKeyUnknown {
                    host: key_host(key),
                    key_type: key.key_type.clone(),
                    fingerprint: key.fingerprint.clone(),
                },
                HostKeyVerdict::Changed { old, new } => PromptRequest::HostKeyChanged {
                    host: key_host(new),
                    key_type: new.key_type.clone(),
                    old_fingerprint: old.fingerprint.clone(),
                    new_fingerprint: new.fingerprint.clone(),
                },
            };
            match self.inner.prompts.ask(&self.inner.listener, request).await {
                Some(PromptAnswer::HostKey { decision }) => match decision {
                    HostKeyChoice::Reject => HostKeyDecision::Reject,
                    HostKeyChoice::AcceptOnce => HostKeyDecision::AcceptOnce,
                    HostKeyChoice::AcceptAndSave => HostKeyDecision::AcceptAndSave,
                },
                _ => {
                    self.inner.cancelled.store(true, Ordering::SeqCst);
                    HostKeyDecision::Reject
                }
            }
        })
    }
}

struct InteractiveAsk {
    inner: Arc<Inner>,
}

impl InteractivePrompt for InteractiveAsk {
    fn respond(
        &self,
        name: String,
        instructions: String,
        prompts: Vec<InteractiveQuestion>,
    ) -> Pin<Box<dyn Future<Output = Option<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            let n = prompts.len();
            let request = PromptRequest::KeyboardInteractive {
                name,
                instructions,
                questions: prompts
                    .into_iter()
                    .map(|q| InteractiveQuestionInfo {
                        prompt: q.prompt,
                        echo: q.echo,
                    })
                    .collect(),
            };
            match self.inner.prompts.ask(&self.inner.listener, request).await {
                Some(PromptAnswer::Answers { values }) if values.len() == n => Some(values),
                Some(PromptAnswer::Secret { value, .. }) if n == 1 => Some(vec![value]),
                _ => {
                    self.inner.cancelled.store(true, Ordering::SeqCst);
                    None
                }
            }
        })
    }
}

fn proxy_config(store: &Store, p: &termoso_core::model::Proxy) -> Result<ProxyConfig> {
    let kind = ProxyKind::parse(&p.kind)
        .ok_or_else(|| MobileError::invalid(format!("unsupported proxy type {}", p.kind)))?;
    let (username, password) = match p.identity_id {
        Some(id) => {
            let ident = store.get::<Identity>(id)?;
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
        password: password.filter(|_| kind.supports_password()),
    })
}

/// Store a password the user asked to remember: on the host's identity when
/// it has one, otherwise on a new hidden identity attached to the host's
/// SSH config.
fn remember_password(store: &Store, resolved: &ResolvedHost, value: &Zeroizing<String>) {
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
                ssh_id: false,
                ssh_id_key_type: None,
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
                        ..SshConfig::default()
                    },
                )?;
                let mut host = resolved.host.data.clone();
                host.ssh_config_id = Some(cfg_id);
                store.update(resolved.host.id, &host)
            }
        }
    })();
    if let Err(e) = result {
        tracing::warn!("remember password: {e}");
    }
}

fn keepalive(settings: &MobileSettings, cfg: &SshConfig) -> Option<Duration> {
    let secs = cfg
        .keep_alive_interval
        .unwrap_or(settings.keep_alive_seconds);
    (secs > 0).then(|| Duration::from_secs(secs as u64))
}

/// Connect to `target`, going through `chain` jump hosts first (each with
/// its own credentials, proxy and known-host entry). Prompts for a password /
/// passphrase when the saved credentials are not enough.
async fn ssh_connect(
    inner: &Arc<Inner>,
    settings: &MobileSettings,
    target: SshTarget,
    resolved: Option<&ResolvedHost>,
    chain: &[termoso_core::model::Entity<termoso_core::model::Host>],
    jump: Option<Arc<SshClient>>,
    hop: Option<String>,
) -> Result<(Arc<SshClient>, Vec<Arc<SshClient>>)> {
    let store = &inner.store;
    let mut jumps: Vec<Arc<SshClient>> = Vec::new();
    let mut via = jump;
    for link in chain {
        let hop_resolved = store.resolve_host(link.id)?;
        let hop_target = SshTarget {
            host: hop_resolved.host.data.address.clone(),
            port: hop_resolved.port(),
            username: hop_resolved.username(),
        };
        let (client, _) = Box::pin(ssh_connect(
            inner,
            settings,
            hop_target,
            Some(&hop_resolved),
            &[],
            via.clone(),
            Some(hop_resolved.host.data.label.clone()),
        ))
        .await?;
        jumps.push(client.clone());
        via = Some(client);
    }

    let known_hosts = KnownHosts::new(store.clone(), store.local_vault()?.id);
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
        Some(p) => Some(proxy_config(store, &p.data)?),
        None => None,
    };
    let key_label = resolved
        .and_then(|r| r.key.as_ref())
        .map(|k| k.data.label.clone())
        .unwrap_or_default();

    let mut attempts = 0;
    let mut asked_password = false;
    let mut asked_passphrase = false;
    loop {
        let mut auth: Vec<AuthMethod> = Vec::new();
        if let Some(key) = resolved.and_then(|r| r.key.as_ref()) {
            let certificate = resolved
                .and_then(|r| r.certificate.as_ref())
                .map(|c| c.data.certificate.clone());
            auth.push(AuthMethod::Key {
                private_key: Zeroizing::new(key.data.private_key.clone()),
                passphrase: passphrase.clone(),
                certificate,
            });
        }
        if let Some(pw) = &password {
            auth.push(AuthMethod::Password(pw.clone()));
        }
        auth.push(AuthMethod::KeyboardInteractive);

        let interactive: Option<Arc<dyn InteractivePrompt>> = match password {
            Some(_) => None,
            None => Some(Arc::new(InteractiveAsk {
                inner: inner.clone(),
            })),
        };
        let opts = ConnectOptions {
            target: target.clone(),
            auth,
            known_hosts: known_hosts.clone(),
            host_key_prompt: Arc::new(HostKeyAsk {
                inner: inner.clone(),
            }),
            interactive,
            keepalive: keepalive(settings, &ssh_cfg),
            timeout: Duration::from_secs(ssh_cfg.timeout.unwrap_or(20).clamp(1, 600) as u64),
            proxy: proxy.clone(),
            env: ssh_cfg.env_variables.clone(),
            agent_forwarding: ssh_cfg.agent_forwarding,
            post_quantum_kex: settings.post_quantum_kex,
            progress: Some(Arc::new(Progress {
                inner: inner.clone(),
                hop: hop.clone(),
            })),
            ip_version: resolved
                .map(|r| IpVersion::parse(&r.host.data.ip_version))
                .unwrap_or_default(),
        };

        let result = match &via {
            Some(j) => SshClient::connect_via(j, opts).await,
            None => SshClient::connect(opts).await,
        };
        match result {
            Ok(client) => return Ok((Arc::new(client), jumps)),
            Err(CoreError::AuthFailed { remaining })
                if attempts < MAX_PASSWORD_ATTEMPTS
                    && !inner.cancelled.load(Ordering::SeqCst)
                    && (remaining.is_empty()
                        || remaining
                            .iter()
                            .any(|m| m == "password" || m == "keyboard-interactive")) =>
            {
                attempts += 1;
                let answer = inner
                    .prompts
                    .ask(
                        &inner.listener,
                        PromptRequest::Password {
                            username: target.username.clone(),
                            retry: asked_password || password.is_some(),
                        },
                    )
                    .await;
                asked_password = true;
                match answer {
                    Some(PromptAnswer::Secret { value, remember }) => {
                        let value = Zeroizing::new(value);
                        if remember && let Some(r) = resolved {
                            remember_password(store, r, &value);
                        }
                        password = Some(value);
                    }
                    _ => return Err(MobileError::Cancelled),
                }
            }
            Err(CoreError::Key(msg))
                if attempts < MAX_PASSWORD_ATTEMPTS && msg.contains("passphrase") =>
            {
                attempts += 1;
                let answer = inner
                    .prompts
                    .ask(
                        &inner.listener,
                        PromptRequest::Passphrase {
                            key_label: key_label.clone(),
                            retry: asked_passphrase || passphrase.is_some(),
                        },
                    )
                    .await;
                asked_passphrase = true;
                match answer {
                    Some(PromptAnswer::Secret { value, remember }) => {
                        let value = Zeroizing::new(value);
                        if remember && let Some(key) = resolved.and_then(|r| r.key.as_ref()) {
                            let mut data = key.data.clone();
                            data.passphrase = Some(value.to_string());
                            if let Err(e) = store.update(key.id, &data) {
                                tracing::warn!("remember passphrase: {e}");
                            }
                        }
                        passphrase = Some(value);
                    }
                    _ => return Err(MobileError::Cancelled),
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run(
    inner: Arc<Inner>,
    target: SshTarget,
    resolved: Option<ResolvedHost>,
    settings: MobileSettings,
    term_type: String,
    size: TermSize,
    signals: mpsc::UnboundedReceiver<TermSignal>,
) {
    let started = Instant::now();
    let history_id = inner
        .store
        .record_connection(&ConnectionHistory {
            host_id: resolved.as_ref().map(|r| r.host.id),
            label: resolved
                .as_ref()
                .map(|r| r.host.data.label.clone())
                .unwrap_or_else(|| target.host.clone()),
            target: target.display(),
            protocol: "ssh".into(),
            duration_secs: None,
            error: None,
        })
        .ok();
    let finish = |error: Option<String>| {
        if let Some(id) = history_id {
            let _ = inner.store.update_connection(
                id,
                &ConnectionHistory {
                    host_id: resolved.as_ref().map(|r| r.host.id),
                    label: resolved
                        .as_ref()
                        .map(|r| r.host.data.label.clone())
                        .unwrap_or_else(|| target.host.clone()),
                    target: target.display(),
                    protocol: "ssh".into(),
                    duration_secs: Some(started.elapsed().as_secs()),
                    error,
                },
            );
        }
    };

    let chain = resolved
        .as_ref()
        .map(|r| r.chain.clone())
        .unwrap_or_default();
    let connect = ssh_connect(
        &inner,
        &settings,
        target.clone(),
        resolved.as_ref(),
        &chain,
        None,
        None,
    );
    let (client, _jumps) = tokio::select! {
        r = connect => match r {
            Ok(c) => c,
            Err(e) => {
                let e = if inner.cancelled.load(Ordering::SeqCst) { MobileError::Cancelled } else { e };
                finish(Some(e.to_string()));
                set_state(&inner, SessionState::Failed { kind: e.kind(), message: e.to_string() });
                return;
            }
        },
        _ = inner.closed.notified() => {
            finish(Some("cancelled".into()));
            set_state(&inner, SessionState::Closed { reason: None });
            return;
        }
    };

    set_state(
        &inner,
        SessionState::Connecting {
            detail: "Opening shell…".into(),
        },
    );
    let (terminal, events) = match client.shell(&term_type, size).await {
        Ok(v) => v,
        Err(e) => {
            let e = MobileError::from(e);
            finish(Some(e.to_string()));
            set_state(
                &inner,
                SessionState::Failed {
                    kind: e.kind(),
                    message: e.to_string(),
                },
            );
            return;
        }
    };
    *inner.terminal.lock().expect("terminal poisoned") = Some(terminal.clone());
    let queued = std::mem::take(&mut *inner.pending_input.lock().expect("input poisoned"));
    if !queued.is_empty() {
        let _ = terminal.write(&queued).await;
    }
    set_state(&inner, SessionState::Connected);

    if settings.detect_os
        && resolved
            .as_ref()
            .is_none_or(|r| r.host.data.os_name.is_none())
    {
        let inner = inner.clone();
        let client = client.clone();
        let host_id = resolved.as_ref().map(|r| r.host.id);
        tokio::spawn(async move {
            let Some(os) = termoso_core::osdetect::detect(&client).await else {
                return;
            };
            let saved = (|| -> termoso_core::error::Result<()> {
                let Some(host_id) = host_id else {
                    return Ok(());
                };
                let mut host = inner.store.require::<termoso_core::model::Host>(host_id)?;
                if host.data.os_name.is_some() {
                    return Ok(());
                }
                host.data.os_name = Some(os.to_string());
                inner.store.update(host.id, &host.data)
            })();
            if saved.is_ok() {
                inner.listener.on_os_detected(os.to_string());
            }
        });
    }

    let reason = pump(&inner, &terminal, events, signals).await;
    finish(reason.clone());
    *inner.terminal.lock().expect("terminal poisoned") = None;
    let _ = client.disconnect().await;
    set_state(&inner, SessionState::Closed { reason });
}

/// Feed remote output into the emulator and route emulator signals back.
/// Returns the close reason (`None` = clean exit).
async fn pump(
    inner: &Arc<Inner>,
    terminal: &Arc<SshTerminal>,
    mut events: TermEvents,
    mut signals: mpsc::UnboundedReceiver<TermSignal>,
) -> Option<String> {
    loop {
        tokio::select! {
            ev = events.recv() => {
                let ev = ev?;
                let mut batch = vec![ev];
                while let Ok(more) = events.try_recv() {
                    batch.push(more);
                    if batch.len() >= 64 {
                        break;
                    }
                }
                let mut dirty = false;
                for ev in batch {
                    match ev {
                        TermEvent::Output(bytes) => {
                            inner.emulator.lock().expect("emulator poisoned").feed(&bytes);
                            dirty = true;
                        }
                        TermEvent::Notice(text) => {
                            let line = format!("\r\n\x1b[2m{text}\x1b[0m\r\n");
                            inner.emulator.lock().expect("emulator poisoned").feed(line.as_bytes());
                            dirty = true;
                        }
                        TermEvent::Error(text) => {
                            if dirty {
                                inner.listener.on_render();
                            }
                            return Some(text);
                        }
                        TermEvent::Exit { code, signal } => {
                            if dirty {
                                inner.listener.on_render();
                            }
                            return match (code, signal) {
                                (Some(0), _) | (None, None) => None,
                                (Some(c), _) => Some(format!("exit status {c}")),
                                (None, Some(s)) => Some(format!("signal {s}")),
                            };
                        }
                        TermEvent::Closed => {
                            if dirty {
                                inner.listener.on_render();
                            }
                            return None;
                        }
                    }
                }
                if dirty {
                    inner.listener.on_render();
                }
            }
            sig = signals.recv() => {
                match sig {
                    Some(TermSignal::PtyWrite(bytes)) => {
                        let _ = terminal.write(&bytes).await;
                    }
                    Some(TermSignal::Title(t)) => inner.listener.on_title(t),
                    Some(TermSignal::Bell) => inner.listener.on_bell(),
                    Some(TermSignal::Clipboard(text)) => inner.listener.on_clipboard(text),
                    None => {}
                }
            }
            _ = inner.closed.notified() => return None,
        }
    }
}
