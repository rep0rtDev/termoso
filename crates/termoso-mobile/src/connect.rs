//! Shared SSH connection path for terminals and SFTP: jump hosts, proxy,
//! saved credentials, and prompts for anything missing. The UI end is a
//! [`ConnectUi`]; each session kind wraps its own listener in one so the
//! prompt/answer protocol is identical for every connection.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use termoso_core::error::CoreError;
use termoso_core::fido2::{self, Fido2Error};
use termoso_core::hostkey::{
    HostKeyDecision, HostKeyInfo, HostKeyPrompt, HostKeyVerdict, KnownHosts,
};
use termoso_core::model::{Identity, ResolvedHost, SshConfig};
use termoso_core::ssh::proxy::{ProxyConfig, ProxyKind};
use termoso_core::ssh::{
    AuthMethod, ConnectOptions, ConnectPhase, ConnectProgress, InteractivePrompt,
    InteractiveQuestion, IpVersion, SshClient, SshTarget,
};
use termoso_core::store::Store;
use tokio::sync::oneshot;
use zeroize::Zeroizing;

use crate::error::{MobileError, Result};
use crate::settings::MobileSettings;

const MAX_PASSWORD_ATTEMPTS: u32 = 3;

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
    /// The security key wants its PIN before it signs. `retries` is what
    /// the token reports after a wrong PIN, when it does.
    SecurityKeyPin {
        key_label: String,
        retry: bool,
        retries: Option<i32>,
    },
    /// No security key is attached (or the attached one does not hold this
    /// credential when `wrong_device`). The UI waits for a USB plug / NFC
    /// tap, registers it, and answers [`PromptAnswer::Retry`].
    SecurityKeyInsert {
        key_label: String,
        wrong_device: bool,
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
    /// Try again (a security key is now attached).
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HostKeyChoice {
    Reject,
    AcceptOnce,
    AcceptAndSave,
}

/// The UI side of a connection attempt.
pub(crate) trait ConnectUi: Send + Sync {
    /// A new connect stage (`Resolving host…`, `Authenticating (password)…`).
    fn phase(&self, detail: String);
    /// Ask the user; the answer comes back through [`Connector::answer`].
    fn prompt(&self, prompt_id: u64, request: PromptRequest);
}

type PromptTx = oneshot::Sender<PromptAnswer>;

pub(crate) struct PromptBroker {
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

    async fn ask(&self, ui: &dyn ConnectUi, request: PromptRequest) -> Option<PromptAnswer> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("prompts poisoned")
            .insert(id, tx);
        ui.prompt(id, request);
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

/// Everything one connection attempt needs from its owner: the store for
/// credentials and known hosts, the UI for prompts, the prompt broker and
/// the "user gave up" flag.
pub(crate) struct Connector {
    pub store: Arc<Store>,
    pub ui: Arc<dyn ConnectUi>,
    prompts: PromptBroker,
    /// Set when the user dismissed a prompt; turns the resulting connect
    /// error into `Cancelled`.
    cancelled: AtomicBool,
}

impl Connector {
    pub fn new(store: Arc<Store>, ui: Arc<dyn ConnectUi>) -> Self {
        Self {
            store,
            ui,
            prompts: PromptBroker::new(),
            cancelled: AtomicBool::new(false),
        }
    }

    pub fn answer(&self, id: u64, answer: PromptAnswer) -> bool {
        self.prompts.answer(id, answer)
    }

    pub fn cancel_prompts(&self) {
        self.prompts.cancel_all();
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Map a connect error to `Cancelled` when the user dismissed a prompt.
    pub fn map_error(&self, e: MobileError) -> MobileError {
        if self.cancelled() {
            MobileError::Cancelled
        } else {
            e
        }
    }

    async fn ask(&self, request: PromptRequest) -> Option<PromptAnswer> {
        self.prompts.ask(self.ui.as_ref(), request).await
    }

    fn give_up(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

pub(crate) fn phase_label(phase: &ConnectPhase, hop: Option<&str>) -> String {
    let base = match phase {
        ConnectPhase::Resolving => "Resolving host…".to_string(),
        ConnectPhase::Connecting { via } => format!("Connecting to {via}…"),
        ConnectPhase::Handshake => "Handshake…".into(),
        ConnectPhase::HostKey => "Checking host key…".into(),
        ConnectPhase::Auth { method } => format!("Authenticating ({method})…"),
        ConnectPhase::SecurityKeyTouch { .. } => "Touch your security key…".into(),
        ConnectPhase::Authenticated => "Authenticated…".into(),
        ConnectPhase::MoshServer => "Starting mosh-server…".into(),
    };
    match hop {
        Some(h) => format!("{h}: {base}"),
        None => base,
    }
}

struct Progress {
    conn: Arc<Connector>,
    hop: Option<String>,
}

impl ConnectProgress for Progress {
    fn phase(&self, phase: ConnectPhase) {
        self.conn.ui.phase(phase_label(&phase, self.hop.as_deref()));
    }
}

struct HostKeyAsk {
    conn: Arc<Connector>,
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
            match self.conn.ask(request).await {
                Some(PromptAnswer::HostKey { decision }) => match decision {
                    HostKeyChoice::Reject => HostKeyDecision::Reject,
                    HostKeyChoice::AcceptOnce => HostKeyDecision::AcceptOnce,
                    HostKeyChoice::AcceptAndSave => HostKeyDecision::AcceptAndSave,
                },
                _ => {
                    self.conn.give_up();
                    HostKeyDecision::Reject
                }
            }
        })
    }
}

struct InteractiveAsk {
    conn: Arc<Connector>,
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
            match self.conn.ask(request).await {
                Some(PromptAnswer::Answers { values }) if values.len() == n => Some(values),
                Some(PromptAnswer::Secret { value, .. }) if n == 1 => Some(vec![value]),
                _ => {
                    self.conn.give_up();
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

/// Connect to `target` through the host's jump chain (each hop with its own
/// credentials, proxy and known-host entry), prompting for a password /
/// passphrase when the saved credentials are not enough. The returned jump
/// clients must be kept alive as long as the main client is.
pub(crate) async fn connect_resolved(
    conn: &Arc<Connector>,
    settings: &MobileSettings,
    target: SshTarget,
    resolved: Option<&ResolvedHost>,
) -> Result<(Arc<SshClient>, Vec<Arc<SshClient>>)> {
    let chain = resolved.map(|r| r.chain.clone()).unwrap_or_default();
    ssh_connect(conn, settings, target, resolved, &chain, None, None).await
}

async fn ssh_connect(
    conn: &Arc<Connector>,
    settings: &MobileSettings,
    target: SshTarget,
    resolved: Option<&ResolvedHost>,
    chain: &[termoso_core::model::Entity<termoso_core::model::Host>],
    jump: Option<Arc<SshClient>>,
    hop: Option<String>,
) -> Result<(Arc<SshClient>, Vec<Arc<SshClient>>)> {
    let store = &conn.store;
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
            conn,
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
        .or_else(|| {
            resolved
                .and_then(|r| r.ssh_id_handle.as_deref())
                .map(|h| format!("SSH ID @{h}"))
        })
        .unwrap_or_default();
    let mut pin: Option<Zeroizing<String>> = None;

    let mut attempts = 0;
    let mut inserts = 0;
    let mut asked_password = false;
    let mut asked_passphrase = false;
    loop {
        let mut auth: Vec<AuthMethod> = Vec::new();
        if let Some(key) = resolved.and_then(|r| r.key.as_ref()) {
            let certificate = resolved
                .and_then(|r| r.certificate.as_ref())
                .map(|c| c.data.certificate.clone());
            if key.data.is_agent_backed() {
                return Err(MobileError::Key {
                    detail: format!(
                        "key \"{}\" is signed by a desktop SSH agent; import its private key to use it here",
                        key.data.label
                    ),
                });
            }
            if fido2::is_sk_type(&key.data.key_type) {
                auth.push(AuthMethod::SecurityKey {
                    private_key: Zeroizing::new(key.data.private_key.clone()),
                    passphrase: passphrase.clone(),
                    pin: pin.clone(),
                    backend: Arc::new(crate::fido2::PhoneBackend::default()),
                    certificate,
                });
            } else {
                auth.push(AuthMethod::Key {
                    private_key: Zeroizing::new(key.data.private_key.clone()),
                    passphrase: passphrase.clone(),
                    certificate,
                });
            }
        }
        if let Some(i) = identity.as_ref().filter(|i| i.data.ssh_id) {
            auth.extend(crate::sshid::auth_methods(
                store,
                i.data.ssh_id_key_type,
                pin.clone(),
            )?);
        }
        if let Some(pw) = &password {
            auth.push(AuthMethod::Password(pw.clone()));
        }
        auth.push(AuthMethod::KeyboardInteractive);

        let interactive: Option<Arc<dyn InteractivePrompt>> = match password {
            Some(_) => None,
            None => Some(Arc::new(InteractiveAsk { conn: conn.clone() })),
        };
        let opts = ConnectOptions {
            target: target.clone(),
            auth,
            known_hosts: known_hosts.clone(),
            host_key_prompt: Arc::new(HostKeyAsk { conn: conn.clone() }),
            interactive,
            keepalive: keepalive(settings, &ssh_cfg),
            timeout: Duration::from_secs(ssh_cfg.timeout.unwrap_or(20).clamp(1, 600) as u64),
            proxy: proxy.clone(),
            env: ssh_cfg.env_variables.clone(),
            agent_forwarding: ssh_cfg.agent_forwarding,
            agent_socket: None,
            post_quantum_kex: settings.post_quantum_kex,
            progress: Some(Arc::new(Progress {
                conn: conn.clone(),
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
                    && !conn.cancelled()
                    && (remaining.is_empty()
                        || remaining
                            .iter()
                            .any(|m| m == "password" || m == "keyboard-interactive")) =>
            {
                attempts += 1;
                let answer = conn
                    .ask(PromptRequest::Password {
                        username: target.username.clone(),
                        retry: asked_password || password.is_some(),
                    })
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
                let answer = conn
                    .ask(PromptRequest::Passphrase {
                        key_label: key_label.clone(),
                        retry: asked_passphrase || passphrase.is_some(),
                    })
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
            Err(CoreError::Fido2(
                e @ (Fido2Error::PinRequired | Fido2Error::PinInvalid { .. }),
            )) if attempts < MAX_PASSWORD_ATTEMPTS && !conn.cancelled() => {
                attempts += 1;
                let retries = match e {
                    Fido2Error::PinInvalid { retries } => retries,
                    _ => None,
                };
                let answer = conn
                    .ask(PromptRequest::SecurityKeyPin {
                        key_label: key_label.clone(),
                        retry: pin.is_some(),
                        retries,
                    })
                    .await;
                match answer {
                    Some(PromptAnswer::Secret { value, .. }) => pin = Some(Zeroizing::new(value)),
                    _ => return Err(MobileError::Cancelled),
                }
            }
            Err(CoreError::Fido2(e @ (Fido2Error::NoDevice | Fido2Error::WrongDevice)))
                if inserts < MAX_PASSWORD_ATTEMPTS && !conn.cancelled() =>
            {
                inserts += 1;
                let answer = conn
                    .ask(PromptRequest::SecurityKeyInsert {
                        key_label: key_label.clone(),
                        wrong_device: matches!(e, Fido2Error::WrongDevice),
                    })
                    .await;
                if !matches!(answer, Some(PromptAnswer::Retry)) {
                    return Err(MobileError::Cancelled);
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}
