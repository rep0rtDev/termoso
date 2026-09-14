//! An interactive SSH session: connect (jump hosts, proxy, saved credentials,
//! prompts for anything missing), open a shell, pump the byte stream into
//! the emulator and tell Kotlin when to repaint.
//!
//! Everything the UI needs to answer — host key confirmation, password,
//! passphrase, keyboard-interactive — arrives as a [`PromptRequest`] on the
//! listener and is answered through [`SshSession::answer`]; the Rust side
//! blocks its own connection task, never a UI thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use termoso_core::api::ApiClient;
use termoso_core::live::{HostShare, LiveEvent, ViewerJoin};
use termoso_core::model::ResolvedHost;
use termoso_core::ssh::SshTarget;
use termoso_core::store::{ConnectionHistory, Store};
use termoso_core::terminal::{SharedTerminal, TermEvent, TermEvents, TermSize, TerminalSession};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::connect::{ConnectUi, Connector, PromptAnswer, PromptRequest, connect_resolved};
use crate::error::{MobileError, Result};
use crate::keys::{KeyMods, SpecialKey, encode_key, encode_text};
use crate::live::{LiveListener, LiveParticipantCard, LiveShare, ShareState, ViewState};
use crate::settings::MobileSettings;
use crate::terminal::{Emulator, GridFrame, GridSnapshot, TermSignal, TerminalPalette};

/// Give the shell time to print its prompt before the startup snippet lands.
const STARTUP_SNIPPET_DELAY: Duration = Duration::from_millis(400);

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
    /// `None` → the scheme chosen in settings.
    pub palette: Option<TerminalPalette>,
}

/// Connect-stage callbacks routed onto the terminal listener.
struct TerminalUi {
    listener: Arc<dyn SessionListener>,
    state: Arc<Mutex<SessionState>>,
}

impl ConnectUi for TerminalUi {
    fn phase(&self, detail: String) {
        let state = SessionState::Connecting { detail };
        *self.state.lock().expect("state poisoned") = state.clone();
        self.listener.on_state(state);
    }

    fn prompt(&self, prompt_id: u64, request: PromptRequest) {
        self.listener.on_prompt(prompt_id, request);
    }
}

struct Inner {
    store: Arc<Store>,
    listener: Arc<dyn SessionListener>,
    conn: Arc<Connector>,
    emulator: Mutex<Emulator>,
    terminal: Mutex<Option<SharedTerminal>>,
    /// Latest view size; the PTY is opened with whatever is current then.
    size: Mutex<TermSize>,
    /// Typed bytes go through one ordered queue drained by a single writer
    /// task once the terminal is up, so keystrokes never overtake each other
    /// (and anything typed before the shell opens is delivered then).
    input: mpsc::UnboundedSender<Vec<u8>>,
    input_rx: Mutex<Option<mpsc::UnboundedReceiver<Vec<u8>>>>,
    state: Arc<Mutex<SessionState>>,
    closed: tokio::sync::Notify,
    /// Last title the remote set (what a share announces).
    title: Mutex<Option<String>>,
    /// Set while this terminal is being shared: output is mirrored into it.
    share: Mutex<Option<Arc<ShareState>>>,
    /// Set when this terminal *is* a view of somebody else's share.
    view: Option<Arc<ViewState>>,
}

impl Inner {
    /// Drain the input queue into `terminal` in order; ends when the queue
    /// closes, the terminal is gone, or a write fails.
    fn start_writer(self: &Arc<Self>, runtime: &tokio::runtime::Handle, terminal: SharedTerminal) {
        let Some(mut rx) = self.input_rx.lock().expect("input poisoned").take() else {
            return;
        };
        let weak = Arc::downgrade(self);
        runtime.spawn(async move {
            while let Some(data) = rx.recv().await {
                let open = weak.upgrade().is_some_and(|inner| {
                    inner.terminal.lock().expect("terminal poisoned").is_some()
                });
                if !open || terminal.write(&data).await.is_err() {
                    break;
                }
            }
        });
    }

    fn share(&self) -> Option<Arc<ShareState>> {
        self.share.lock().expect("share poisoned").clone()
    }

    /// The terminal is going away: end its share, if any.
    fn end_share(&self, runtime: &tokio::runtime::Handle) {
        if let Some(share) = self.share.lock().expect("share poisoned").take() {
            share.shutdown(runtime);
        }
    }
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

pub(crate) struct ViewerLaunch {
    pub store: Arc<Store>,
    pub settings: MobileSettings,
    pub options: TerminalOptions,
    pub listener: Arc<dyn SessionListener>,
    pub joined: ViewerJoin,
    pub live_events: mpsc::Receiver<LiveEvent>,
    pub live_listener: Arc<dyn LiveListener>,
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
            .unwrap_or_else(|| crate::themes::palette_for(&settings.terminal_theme));
        let (emulator, signals) = Emulator::new(
            options.cols,
            options.rows,
            settings.scrollback_lines,
            palette,
        );
        let state = Arc::new(Mutex::new(SessionState::Connecting {
            detail: "Connecting…".into(),
        }));
        let conn = Arc::new(Connector::new(
            store.clone(),
            Arc::new(TerminalUi {
                listener: listener.clone(),
                state: state.clone(),
            }),
        ));
        let (input, input_rx) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            store,
            listener,
            conn,
            emulator: Mutex::new(emulator),
            terminal: Mutex::new(None),
            size: Mutex::new(TermSize {
                cols: options.cols.max(2),
                rows: options.rows.max(1),
            }),
            input,
            input_rx: Mutex::new(Some(input_rx)),
            state,
            closed: tokio::sync::Notify::new(),
            title: Mutex::new(None),
            share: Mutex::new(None),
            view: None,
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
        runtime.spawn(run(inner, target, resolved, settings, term_type, signals));
        session
    }

    /// A terminal that mirrors a share somebody else is hosting. The grid
    /// follows the host's geometry; input is silently dropped until the host
    /// grants control (see [`LiveListener::on_control`]).
    pub(crate) fn launch_viewer(
        runtime: tokio::runtime::Handle,
        launch: ViewerLaunch,
    ) -> Arc<Self> {
        let ViewerLaunch {
            store,
            settings,
            options,
            listener,
            joined,
            live_events,
            live_listener,
        } = launch;
        let ViewerJoin {
            term,
            events,
            user_id,
            participants,
        } = joined;
        let palette = options
            .palette
            .clone()
            .unwrap_or_else(|| crate::themes::palette_for(&settings.terminal_theme));
        let (emulator, signals) = Emulator::new(
            options.cols,
            options.rows,
            settings.scrollback_lines,
            palette,
        );
        let state = Arc::new(Mutex::new(SessionState::Connected));
        let conn = Arc::new(Connector::new(
            store.clone(),
            Arc::new(TerminalUi {
                listener: listener.clone(),
                state: state.clone(),
            }),
        ));
        let view = Arc::new(ViewState {
            me: user_id,
            participants: Mutex::new(participants),
            can_write: AtomicBool::new(false),
        });
        let (input, input_rx) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            store,
            listener: listener.clone(),
            conn,
            emulator: Mutex::new(emulator),
            terminal: Mutex::new(Some(term.clone())),
            size: Mutex::new(TermSize {
                cols: options.cols.max(2),
                rows: options.rows.max(1),
            }),
            input,
            input_rx: Mutex::new(Some(input_rx)),
            state,
            closed: tokio::sync::Notify::new(),
            title: Mutex::new(None),
            share: Mutex::new(None),
            view: Some(view.clone()),
        });
        let session = Arc::new(Self {
            id: Uuid::new_v4(),
            inner: inner.clone(),
            runtime: runtime.clone(),
        });
        inner.start_writer(&runtime, term.clone());
        set_state(&inner, SessionState::Connected);
        live_listener.on_participants(view.cards());
        {
            let resize_inner = inner.clone();
            let resize: Box<dyn Fn(u16, u16) + Send + Sync> = Box::new(move |cols, rows| {
                let cols = cols.max(2);
                let rows = rows.max(1);
                *resize_inner.size.lock().expect("size poisoned") = TermSize { cols, rows };
                resize_inner
                    .emulator
                    .lock()
                    .expect("emulator poisoned")
                    .resize(cols, rows);
                resize_inner.listener.on_render();
            });
            let title_inner = inner.clone();
            let title: Box<dyn Fn(String) + Send + Sync> = Box::new(move |t| {
                let t = (!t.trim().is_empty()).then_some(t);
                *title_inner.title.lock().expect("title poisoned") = t.clone();
                title_inner.listener.on_title(t);
            });
            runtime.spawn(crate::live::forward_viewer(
                live_events,
                view,
                live_listener,
                resize,
                title,
            ));
        }
        runtime.spawn(run_viewer(inner, term, events, signals));
        session
    }

    /// Start sharing this terminal. Fails unless the shell is open.
    pub(crate) async fn share(
        self: &Arc<Self>,
        api: Arc<ApiClient>,
        listener: Arc<dyn LiveListener>,
        label: String,
    ) -> Result<Arc<LiveShare>> {
        if self.inner.view.is_some() {
            return Err(MobileError::invalid(
                "this terminal is already a shared view",
            ));
        }
        if self.inner.share().is_some() {
            return Err(MobileError::invalid("this terminal is already shared"));
        }
        let term = self
            .inner
            .terminal
            .lock()
            .expect("terminal poisoned")
            .clone()
            .ok_or_else(|| MobileError::invalid("connect before sharing"))?;
        let size = *self.inner.size.lock().expect("size poisoned");
        let title = self
            .inner
            .title
            .lock()
            .expect("title poisoned")
            .clone()
            .unwrap_or(label);
        let (tx, rx) = mpsc::channel(64);
        let share = HostShare::start(api, term, size, title, tx).await?;
        let weak = Arc::downgrade(&self.inner);
        let detach: Box<dyn Fn() + Send + Sync> = Box::new(move || {
            if let Some(inner) = weak.upgrade() {
                *inner.share.lock().expect("share poisoned") = None;
            }
        });
        let (live, state) = LiveShare::attach(self.runtime.clone(), share, rx, listener, detach);
        *self.inner.share.lock().expect("share poisoned") = Some(state);
        Ok(live)
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
        let _ = self.inner.input.send(data);
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

    /// `true` for a terminal that mirrors somebody else's share.
    pub fn is_view(&self) -> bool {
        self.inner.view.is_some()
    }

    /// Viewer only: whether the host has granted control right now.
    pub fn can_write(&self) -> bool {
        self.inner
            .view
            .as_ref()
            .is_none_or(|v| v.can_write.load(Ordering::Relaxed))
    }

    /// Everyone connected to the share this terminal belongs to (viewer side;
    /// hosts ask their [`LiveShare`]).
    pub fn live_participants(&self) -> Vec<LiveParticipantCard> {
        self.inner
            .view
            .as_ref()
            .map(|v| v.cards())
            .unwrap_or_default()
    }

    /// `true` while this terminal is being shared.
    pub fn is_shared(&self) -> bool {
        self.inner.share().is_some()
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if self.inner.view.is_some() {
            // The grid follows the host; the view scales to fit.
            return;
        }
        let cols = cols.max(2);
        let rows = rows.max(1);
        *self.inner.size.lock().expect("size poisoned") = TermSize { cols, rows };
        if let Some(share) = self.inner.share() {
            share.resized(TermSize { cols, rows });
        }
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
        self.inner.conn.answer(prompt_id, answer)
    }

    /// Tear the connection down; the object stays usable for `state()`/`snapshot()`.
    pub fn disconnect(&self) {
        self.inner.conn.cancel_prompts();
        self.inner.closed.notify_waiters();
        self.inner.closed.notify_one();
        self.inner.end_share(&self.runtime);
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

async fn run(
    inner: Arc<Inner>,
    target: SshTarget,
    resolved: Option<ResolvedHost>,
    settings: MobileSettings,
    term_type: String,
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

    let connect = connect_resolved(&inner.conn, &settings, target.clone(), resolved.as_ref());
    let (client, _jumps) = tokio::select! {
        r = connect => match r {
            Ok(c) => c,
            Err(e) => {
                let e = inner.conn.map_error(e);
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
    let size = *inner.size.lock().expect("size poisoned");
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
    inner.start_writer(&tokio::runtime::Handle::current(), terminal.clone());
    set_state(&inner, SessionState::Connected);
    if let Some(script) = crate::snippets::startup_script(
        &inner.store,
        resolved
            .as_ref()
            .and_then(|r| r.host.data.startup_snippet_id),
    ) {
        let terminal = terminal.clone();
        tokio::spawn(async move {
            tokio::time::sleep(STARTUP_SNIPPET_DELAY).await;
            let _ = terminal.write(script.as_bytes()).await;
        });
    }

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

    let terminal: SharedTerminal = terminal;
    let reason = pump(&inner, &terminal, events, signals).await;
    finish(reason.clone());
    *inner.terminal.lock().expect("terminal poisoned") = None;
    inner.end_share(&tokio::runtime::Handle::current());
    let _ = client.disconnect().await;
    set_state(&inner, SessionState::Closed { reason });
}

/// Viewer counterpart of [`run`]: the relay stream is already open, so just
/// pump it until the host stops or the tab closes.
async fn run_viewer(
    inner: Arc<Inner>,
    terminal: SharedTerminal,
    events: TermEvents,
    signals: mpsc::UnboundedReceiver<TermSignal>,
) {
    let reason = pump(&inner, &terminal, events, signals).await;
    *inner.terminal.lock().expect("terminal poisoned") = None;
    let _ = terminal.close().await;
    set_state(&inner, SessionState::Closed { reason });
}

/// Feed remote output into the emulator and route emulator signals back.
/// Returns the close reason (`None` = clean exit).
async fn pump(
    inner: &Arc<Inner>,
    terminal: &SharedTerminal,
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
                let share = inner.share();
                for ev in batch {
                    match ev {
                        TermEvent::Output(bytes) => {
                            if let Some(share) = &share {
                                share.publisher.publish(Bytes::copy_from_slice(&bytes));
                            }
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
                    Some(TermSignal::Title(t)) => {
                        *inner.title.lock().expect("title poisoned") = t.clone();
                        if let (Some(share), Some(t)) = (inner.share(), &t) {
                            share.retitled(t.clone());
                        }
                        inner.listener.on_title(t);
                    }
                    Some(TermSignal::Bell) => inner.listener.on_bell(),
                    Some(TermSignal::Clipboard(text)) => inner.listener.on_clipboard(text),
                    None => {}
                }
            }
            _ = inner.closed.notified() => return None,
        }
    }
}
