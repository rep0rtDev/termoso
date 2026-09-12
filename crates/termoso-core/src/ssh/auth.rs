//! Authentication methods and the negotiation loop.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use russh::MethodKind;
use russh::client::AuthResult;
use russh::client::{Handle, KeyboardInteractiveAuthResponse};
use russh::keys::ssh_key::Certificate;
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg};
use zeroize::Zeroizing;

use super::{ClientHandler, ConnectOptions, ConnectPhase};
use crate::error::{CoreError, Result};

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
    /// Keys held by the system SSH agent (`SSH_AUTH_SOCK` / Pageant).
    Agent,
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
            AuthMethod::Agent => "agent",
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
            AuthMethod::Key { .. } | AuthMethod::Agent => MethodKind::PublicKey,
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
                AuthMethod::Agent => "ssh-agent".into(),
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
            AuthMethod::Agent => match agent_auth(handle, &user).await {
                Ok(Some(r)) => r,
                Ok(None) => continue,
                Err(e) => {
                    tracing::debug!(error = %e, "agent auth unavailable");
                    continue;
                }
            },
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

/// Try every identity the system agent holds. `Ok(None)` when the agent is
/// not reachable or has no keys.
async fn agent_auth(handle: &mut Handle<ClientHandler>, user: &str) -> Result<Option<AuthResult>> {
    let mut agent = crate::agent::connect_system_agent().await?;
    let identities = agent
        .request_identities()
        .await
        .map_err(|e| CoreError::Ssh(format!("agent: {e}")))?;
    if identities.is_empty() {
        return Ok(None);
    }
    let mut last = None;
    for id in identities {
        let key = match &id {
            russh::keys::agent::AgentIdentity::PublicKey { key, .. } => key.clone(),
            russh::keys::agent::AgentIdentity::Certificate { certificate, .. } => {
                russh::keys::PublicKey::from(certificate.public_key().clone())
            }
        };
        let hash = if key.algorithm().is_rsa() {
            handle
                .best_supported_rsa_hash()
                .await?
                .unwrap_or(Some(HashAlg::Sha512))
        } else {
            None
        };
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
