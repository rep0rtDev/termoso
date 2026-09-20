//! Authentication methods and the negotiation loop.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use russh::MethodKind;
use russh::client::AuthResult;
use russh::client::{Handle, KeyboardInteractiveAuthResponse};
use russh::keys::agent::AgentIdentity;
use russh::keys::ssh_key::Certificate;
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg};
use zeroize::Zeroizing;

use super::{ClientHandler, ConnectOptions, ConnectPhase};
use crate::error::{CoreError, Result};
use crate::fido2::SkBackend;

/// One way to prove who we are, tried in the order given.
pub enum AuthMethod {
    /// `none` – lets the server tell us what it accepts, and logs in on
    /// servers that need nothing.
    None,
    /// Plain password.
    Password(Zeroizing<String>),
    /// Private key from the vault (OpenSSH / PEM / PuTTY text), optionally
    /// encrypted, optionally with an OpenSSH certificate.
    Key {
        /// Key material as stored.
        private_key: Zeroizing<String>,
        /// Passphrase if encrypted.
        passphrase: Option<Zeroizing<String>>,
        /// Certificate text (`ssh-ed25519-cert-v01@openssh.com AAAA…`).
        certificate: Option<String>,
    },
    /// FIDO2 security key (`sk-*` OpenSSH key: public part + handle). The
    /// token signs; the user touches it and, when the key demands user
    /// verification, enters the PIN.
    SecurityKey {
        /// `sk-*` key as stored (OpenSSH format, maybe passphrase-protected).
        private_key: Zeroizing<String>,
        /// Passphrase if the stored handle is encrypted.
        passphrase: Option<Zeroizing<String>>,
        /// Client PIN (needed for verify-required keys and PIN-protected tokens).
        pin: Option<Zeroizing<String>>,
        /// How to reach the token (USB HID on desktop, the device the app
        /// holds on Android).
        backend: Arc<dyn SkBackend>,
        /// Certificate text, when the sk key is certified.
        certificate: Option<String>,
    },
    /// Every key the SSH agent holds, tried in the agent's order.
    Agent,
    /// One specific key the SSH agent holds: only its public half is known
    /// to us (`IdentityFile key.pub` + `IdentitiesOnly` in OpenSSH terms);
    /// the agent signs. Fails, rather than trying other agent keys, when the
    /// agent is unreachable or does not hold the key.
    AgentKey {
        /// `<type> <base64> [comment]` line.
        public_key: String,
        /// Certificate text to present instead of the bare key.
        certificate: Option<String>,
    },
    /// Keyboard-interactive (PAM, OTP). Answers come from
    /// [`ConnectOptions::interactive`], or the password method's secret when
    /// that is not set.
    KeyboardInteractive,
}

impl std::fmt::Debug for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AuthMethod::None => "none",
            AuthMethod::Password(_) => "password",
            AuthMethod::Key { .. } => "publickey",
            AuthMethod::SecurityKey { .. } => "publickey (security key)",
            AuthMethod::Agent => "agent",
            AuthMethod::AgentKey { .. } => "publickey (agent)",
            AuthMethod::KeyboardInteractive => "keyboard-interactive",
        })
    }
}

/// A single keyboard-interactive prompt.
#[derive(Debug, Clone)]
pub struct InteractiveQuestion {
    /// Text.
    pub prompt: String,
    /// Whether the answer may be echoed.
    pub echo: bool,
}

/// Answers keyboard-interactive challenges. `None` aborts the method.
pub trait InteractivePrompt: Send + Sync {
    /// Respond to one round of prompts.
    fn respond(
        &self,
        name: String,
        instructions: String,
        prompts: Vec<InteractiveQuestion>,
    ) -> Pin<Box<dyn Future<Output = Option<Vec<String>>> + Send + '_>>;
}

/// Answers every hidden prompt with a fixed password (the "server only
/// offers keyboard-interactive" case) and echo prompts with an empty string.
pub struct PasswordResponder(pub Zeroizing<String>);

impl InteractivePrompt for PasswordResponder {
    fn respond(
        &self,
        _name: String,
        _instructions: String,
        prompts: Vec<InteractiveQuestion>,
    ) -> Pin<Box<dyn Future<Output = Option<Vec<String>>> + Send + '_>> {
        let answers = prompts
            .iter()
            .map(|p| {
                if p.echo {
                    String::new()
                } else {
                    self.0.to_string()
                }
            })
            .collect();
        Box::pin(async move { Some(answers) })
    }
}

fn method_name(k: &MethodKind) -> String {
    <&'static str>::from(k).to_string()
}

fn remaining(methods: &russh::MethodSet) -> Vec<String> {
    methods.iter().map(method_name).collect()
}

/// Parse the stored private key, decrypting it if needed.
pub fn load_private_key(text: &str, passphrase: Option<&str>) -> Result<PrivateKey> {
    use russh::keys::ssh_key::Error as SshKeyError;
    const ENCRYPTED: &str = "private key is encrypted; passphrase required";
    const WRONG: &str = "wrong passphrase (or the key file is corrupted)";
    match russh::keys::decode_secret_key(text, passphrase) {
        Ok(k) => Ok(k),
        Err(russh::keys::Error::KeyIsEncrypted) => Err(CoreError::Key(ENCRYPTED.into())),
        Err(russh::keys::Error::SshKey(SshKeyError::Encrypted)) => {
            Err(CoreError::Key(ENCRYPTED.into()))
        }
        Err(russh::keys::Error::SshKey(SshKeyError::Ppk(e))) => {
            let msg = e.to_string();
            if msg.contains("encrypted") {
                Err(CoreError::Key(ENCRYPTED.into()))
            } else if msg.contains("MAC") && passphrase.is_some() {
                Err(CoreError::Key(WRONG.into()))
            } else {
                Err(CoreError::Key(format!("PuTTY key: {msg}")))
            }
        }
        Err(russh::keys::Error::SshKey(SshKeyError::Crypto)) if passphrase.is_some() => {
            Err(CoreError::Key(WRONG.into()))
        }
        Err(e) => Err(CoreError::Key(e.to_string())),
    }
}

pub(super) async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    opts: &ConnectOptions,
) -> Result<()> {
    let user = opts.target.username.clone();
    let mut partial = false;

    // Probe with `none` first so we know what the server accepts.
    let mut allowed: Option<Vec<MethodKind>> = None;
    opts.report(ConnectPhase::Auth {
        method: "none".into(),
    });
    let mut last_remaining = match handle.authenticate_none(user.clone()).await? {
        AuthResult::Success => return Ok(()),
        AuthResult::Failure {
            remaining_methods, ..
        } => {
            let names = remaining(&remaining_methods);
            if !remaining_methods.is_empty() {
                allowed = Some(remaining_methods.to_vec());
            }
            names
        }
    };

    let password_fallback = opts.auth.iter().find_map(|m| match m {
        AuthMethod::Password(p) => Some(p.clone()),
        _ => None,
    });

    for method in &opts.auth {
        let kind = match method {
            AuthMethod::None => continue,
            AuthMethod::Password(_) => MethodKind::Password,
            AuthMethod::Key { .. }
            | AuthMethod::Agent
            | AuthMethod::AgentKey { .. }
            | AuthMethod::SecurityKey { .. } => MethodKind::PublicKey,
            AuthMethod::KeyboardInteractive => MethodKind::KeyboardInteractive,
        };
        if let Some(a) = &allowed
            && !a.contains(&kind)
        {
            tracing::debug!(?method, "skipped: server does not offer it");
            continue;
        }
        opts.report(ConnectPhase::Auth {
            method: match method {
                AuthMethod::Agent | AuthMethod::AgentKey { .. } => "ssh-agent".into(),
                _ => method_name(&kind),
            },
        });

        let result = match method {
            AuthMethod::None => unreachable!(),
            AuthMethod::Password(p) => {
                handle
                    .authenticate_password(user.clone(), p.to_string())
                    .await?
            }
            AuthMethod::Key {
                private_key,
                passphrase,
                certificate,
            } => {
                let key = load_private_key(private_key, passphrase.as_deref().map(|p| p.as_str()))?;
                if crate::fido2::is_security_key(key.public_key()) {
                    // An sk key that reached us as a plain `Key`: reach for
                    // the plugged-in USB token where we have one.
                    let backend = default_sk_backend()?;
                    let r = sk_auth(
                        handle,
                        opts,
                        &user,
                        key,
                        None,
                        backend,
                        certificate.as_deref(),
                    )
                    .await?;
                    match r {
                        AuthResult::Success => return Ok(()),
                        AuthResult::Failure {
                            remaining_methods,
                            partial_success,
                        } => {
                            last_remaining = remaining(&remaining_methods);
                            partial |= partial_success;
                            if !remaining_methods.is_empty() {
                                allowed = Some(remaining_methods.to_vec());
                            }
                            continue;
                        }
                    }
                }
                let hash = if key.algorithm().is_rsa() {
                    handle
                        .best_supported_rsa_hash()
                        .await?
                        .unwrap_or(Some(HashAlg::Sha512))
                } else {
                    None
                };
                match certificate {
                    Some(cert_text) => {
                        let cert = Certificate::from_openssh(cert_text.trim())?;
                        handle
                            .authenticate_openssh_cert(user.clone(), Arc::new(key), cert)
                            .await?
                    }
                    None => {
                        handle
                            .authenticate_publickey(
                                user.clone(),
                                PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                            )
                            .await?
                    }
                }
            }
            AuthMethod::SecurityKey {
                private_key,
                passphrase,
                pin,
                backend,
                certificate,
            } => {
                let key = load_private_key(private_key, passphrase.as_deref().map(|p| p.as_str()))?;
                if !crate::fido2::is_security_key(key.public_key()) {
                    return Err(CoreError::Key(
                        "not a security key (expected an sk-* key)".into(),
                    ));
                }
                sk_auth(
                    handle,
                    opts,
                    &user,
                    key,
                    pin.clone(),
                    backend.clone(),
                    certificate.as_deref(),
                )
                .await?
            }
            AuthMethod::Agent => {
                match agent_auth(handle, &user, opts.agent_socket.as_deref()).await {
                    Ok(Some(r)) => r,
                    Ok(None) => continue,
                    Err(e) => {
                        tracing::debug!(error = %e, "agent auth unavailable");
                        continue;
                    }
                }
            }
            AuthMethod::AgentKey {
                public_key,
                certificate,
            } => {
                agent_key_auth(
                    handle,
                    &user,
                    opts.agent_socket.as_deref(),
                    public_key,
                    certificate.as_deref(),
                )
                .await?
            }
            AuthMethod::KeyboardInteractive => {
                let responder: Arc<dyn InteractivePrompt> =
                    match (&opts.interactive, &password_fallback) {
                        (Some(p), _) => p.clone(),
                        (None, Some(pw)) => Arc::new(PasswordResponder(pw.clone())),
                        (None, None) => continue,
                    };
                keyboard_interactive(handle, &user, responder).await?
            }
        };

        match result {
            AuthResult::Success => return Ok(()),
            AuthResult::Failure {
                remaining_methods,
                partial_success,
            } => {
                last_remaining = remaining(&remaining_methods);
                partial |= partial_success;
                if !remaining_methods.is_empty() {
                    allowed = Some(remaining_methods.to_vec());
                }
                tracing::debug!(?method, partial_success, "auth method failed");
            }
        }
    }

    if partial {
        tracing::debug!("server accepted a method but wants more");
    }
    Err(CoreError::AuthFailed {
        remaining: last_remaining,
    })
}

async fn keyboard_interactive(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    responder: Arc<dyn InteractivePrompt>,
) -> Result<AuthResult> {
    let mut resp = handle
        .authenticate_keyboard_interactive_start(user, None)
        .await?;
    for _round in 0..16 {
        match resp {
            KeyboardInteractiveAuthResponse::Success => return Ok(AuthResult::Success),
            KeyboardInteractiveAuthResponse::Failure {
                remaining_methods,
                partial_success,
            } => {
                return Ok(AuthResult::Failure {
                    remaining_methods,
                    partial_success,
                });
            }
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                let qs = prompts
                    .into_iter()
                    .map(|p| InteractiveQuestion {
                        prompt: p.prompt,
                        echo: p.echo,
                    })
                    .collect();
                let Some(answers) = responder.respond(name, instructions, qs).await else {
                    return Err(CoreError::Cancelled);
                };
                resp = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await?;
            }
        }
    }
    Err(CoreError::Ssh(
        "keyboard-interactive: too many rounds".into(),
    ))
}

/// Backend for an `sk-*` key that arrived without one: the USB tokens on
/// desktop; nothing on platforms where the app must hand us the device.
fn default_sk_backend() -> Result<Arc<dyn SkBackend>> {
    #[cfg(feature = "fido2")]
    {
        Ok(Arc::new(crate::fido2::UsbBackend::default()))
    }
    #[cfg(not(feature = "fido2"))]
    {
        Err(crate::fido2::Fido2Error::NoDevice.into())
    }
}

/// Public-key auth where the token signs (`authenticate_publickey_with`).
/// The UI gets [`ConnectPhase::SecurityKeyTouch`] right before each
/// signature so it can say "touch your key".
async fn sk_auth(
    handle: &mut Handle<ClientHandler>,
    opts: &ConnectOptions,
    user: &str,
    key: PrivateKey,
    pin: Option<Zeroizing<String>>,
    backend: Arc<dyn SkBackend>,
    certificate: Option<&str>,
) -> Result<AuthResult> {
    let public = crate::fido2::public_key_of(&key);
    let label = public.fingerprint(HashAlg::Sha256).to_string();
    let progress = opts.progress.clone();
    let mut signer = crate::fido2::SkSigner {
        key: Arc::new(key),
        pin,
        backend,
        on_touch: Some(Box::new(move || {
            if let Some(p) = &progress {
                p.phase(ConnectPhase::SecurityKeyTouch { key: label.clone() });
            }
        })),
    };
    match certificate {
        Some(text) => {
            let cert = Certificate::from_openssh(text.trim())?;
            handle
                .authenticate_certificate_with(user, cert, None, &mut signer)
                .await
        }
        None => {
            handle
                .authenticate_publickey_with(user, public, None, &mut signer)
                .await
        }
    }
}

/// Try every identity the agent holds. `Ok(None)` when the agent is not
/// reachable or has no keys.
async fn agent_auth(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    socket: Option<&std::path::Path>,
) -> Result<Option<AuthResult>> {
    let mut agent = crate::agent::connect_agent(socket).await?;
    let identities = agent
        .request_identities()
        .await
        .map_err(|e| CoreError::Ssh(format!("agent: {e}")))?;
    if identities.is_empty() {
        return Ok(None);
    }
    let mut last = None;
    for id in identities {
        let key = identity_key(&id);
        let hash = rsa_hash(handle, &key).await?;
        let r = handle
            .authenticate_publickey_with(user, key, hash, &mut agent)
            .await
            .map_err(|e| CoreError::Ssh(format!("agent: {e}")))?;
        if r.success() {
            return Ok(Some(r));
        }
        last = Some(r);
    }
    Ok(last)
}

/// Public key an agent identity is about (the certified key for certificates).
fn identity_key(id: &AgentIdentity) -> russh::keys::PublicKey {
    match id {
        AgentIdentity::PublicKey { key, .. } => key.clone(),
        AgentIdentity::Certificate { certificate, .. } => {
            russh::keys::PublicKey::from(certificate.public_key().clone())
        }
    }
}

async fn rsa_hash(
    handle: &mut Handle<ClientHandler>,
    key: &russh::keys::PublicKey,
) -> Result<Option<HashAlg>> {
    if key.algorithm().is_rsa() {
        Ok(handle
            .best_supported_rsa_hash()
            .await?
            .unwrap_or(Some(HashAlg::Sha512)))
    } else {
        Ok(None)
    }
}

/// Signs with one fixed agent key. Certificate sign requests are rewritten to
/// the bare key (as OpenSSH does), so an agent that holds only the private
/// key, not the certificate, still works.
struct AgentKeySigner {
    agent: crate::agent::DynAgentClient,
    key: russh::keys::PublicKey,
}

impl russh::Signer for AgentKeySigner {
    type Error = CoreError;

    fn auth_sign(
        &mut self,
        _key: &AgentIdentity,
        hash_alg: Option<HashAlg>,
        to_sign: Vec<u8>,
    ) -> impl Future<Output = Result<Vec<u8>>> + Send {
        let id = AgentIdentity::PublicKey {
            key: self.key.clone(),
            comment: String::new(),
        };
        async move {
            self.agent
                .sign_request(&id, hash_alg, to_sign)
                .await
                .map_err(|e| CoreError::Ssh(format!("agent refused to sign: {e}")))
        }
    }
}

/// Authenticate with exactly one agent-held key, selected by its public half.
async fn agent_key_auth(
    handle: &mut Handle<ClientHandler>,
    user: &str,
    socket: Option<&std::path::Path>,
    public_key: &str,
    certificate: Option<&str>,
) -> Result<AuthResult> {
    let wanted = crate::hostkey::parse_public_key(public_key)?;
    let fp = wanted.fingerprint(Default::default()).to_string();
    let mut agent = crate::agent::connect_agent(socket)
        .await
        .map_err(|e| CoreError::Ssh(format!("{e} (needed for agent key {fp})")))?;
    let identities = agent
        .request_identities()
        .await
        .map_err(|e| CoreError::Ssh(format!("agent: {e}")))?;
    if !identities
        .iter()
        .any(|id| identity_key(id).key_data() == wanted.key_data())
    {
        return Err(CoreError::Key(format!(
            "the SSH agent does not hold key {fp}; add it (ssh-add / KeePassXC) and retry"
        )));
    }
    let hash = rsa_hash(handle, &wanted).await?;
    let mut signer = AgentKeySigner {
        agent,
        key: wanted.clone(),
    };
    let r = match certificate {
        Some(text) => {
            let cert = Certificate::from_openssh(text.trim())
                .map_err(|e| CoreError::Key(format!("certificate: {e}")))?;
            if cert.public_key() != wanted.key_data() {
                return Err(CoreError::Key(
                    "certificate was issued for a different key than the agent key".into(),
                ));
            }
            handle
                .authenticate_certificate_with(user, cert, hash, &mut signer)
                .await?
        }
        None => {
            handle
                .authenticate_publickey_with(user, wanted, hash, &mut signer)
                .await?
        }
    };
    Ok(r)
}
