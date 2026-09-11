//! `russh::client::Handler` implementation: host-key verification and
//! server-initiated channels.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use russh::client::{Msg, Session};
use russh::keys::PublicKeyOrCertificate;
use russh::keys::ssh_key::PublicKey;
use russh::{Channel, ChannelOpenFailure};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};

use crate::error::CoreError;
use crate::hostkey::{HostKeyDecision, HostKeyPrompt, HostKeyVerdict, KnownHosts};

/// A channel the server opened towards us for a remote (`-R`) forward.
pub struct ForwardedChannel {
    /// Channel.
    pub channel: Channel<Msg>,
    /// Address the server accepted the connection on.
    pub connected_address: String,
    /// Port the server accepted the connection on.
    pub connected_port: u16,
    /// Peer address on the server side.
    pub originator_address: String,
    /// Peer port on the server side.
    pub originator_port: u16,
}

/// Remote-forward listeners keyed by the server-side port they were bound on.
pub(crate) type ForwardRoutes = Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<ForwardedChannel>>>>;

/// Algorithms negotiated for a connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Algorithms {
    /// Key exchange, e.g. `mlkem768x25519-sha256`.
    pub kex: String,
    /// Host key algorithm, e.g. `ssh-ed25519`.
    pub host_key: String,
    /// Symmetric cipher.
    pub cipher: String,
    /// MAC (empty for AEAD ciphers that carry their own).
    pub mac: String,
}

impl Algorithms {
    /// True when the key exchange includes a post-quantum KEM.
    pub fn post_quantum(&self) -> bool {
        self.kex.contains("mlkem") || self.kex.contains("sntrup")
    }
}

/// Termoso's russh client handler.
pub struct ClientHandler {
    host: String,
    port: u16,
    known_hosts: KnownHosts,
    prompt: Arc<dyn HostKeyPrompt>,
    banner: Arc<Mutex<Option<String>>>,
    algorithms: Arc<Mutex<Option<Algorithms>>>,
    closed: watch::Sender<Option<String>>,
    prompting: watch::Sender<bool>,
    forwarded: ForwardRoutes,
}

impl ClientHandler {
    pub(crate) fn new(
        host: String,
        port: u16,
        known_hosts: KnownHosts,
        prompt: Arc<dyn HostKeyPrompt>,
        closed: watch::Sender<Option<String>>,
        forwarded: ForwardRoutes,
    ) -> Self {
        Self {
            host,
            port,
            known_hosts,
            prompt,
            banner: Arc::new(Mutex::new(None)),
            algorithms: Arc::new(Mutex::new(None)),
            closed,
            prompting: watch::Sender::new(false),
            forwarded,
        }
    }

    pub(crate) fn banner(&self) -> Arc<Mutex<Option<String>>> {
        self.banner.clone()
    }

    /// `true` while the user is being asked about an unknown or changed host
    /// key; the handshake timeout is paused for that time.
    pub(crate) fn prompting(&self) -> watch::Receiver<bool> {
        self.prompting.subscribe()
    }

    pub(crate) fn algorithms(&self) -> Arc<Mutex<Option<Algorithms>>> {
        self.algorithms.clone()
    }

    /// The host key a certificate vouches for. Certificate authorities are
    /// not modelled yet, so the embedded key is pinned like a plain one.
    fn presented_key(key: &PublicKeyOrCertificate) -> PublicKey {
        match key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(cert) => PublicKey::from(cert.public_key().clone()),
        }
    }
}

impl russh::client::Handler for ClientHandler {
    type Error = CoreError;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = Self::presented_key(server_public_key);
        let verdict = self.known_hosts.check(&self.host, self.port, &key)?;
        if verdict == HostKeyVerdict::Known {
            return Ok(true);
        }
        let changed = matches!(verdict, HostKeyVerdict::Changed { .. });
        let _ = self.prompting.send(true);
        let decision = self.prompt.decide(verdict).await;
        let _ = self.prompting.send(false);
        match decision {
            HostKeyDecision::Reject => {
                tracing::warn!(host = %self.host, changed, "host key rejected");
                Err(CoreError::HostKeyRejected {
                    host: crate::hostkey::host_id(&self.host, self.port),
                })
            }
            HostKeyDecision::AcceptOnce => Ok(true),
            HostKeyDecision::AcceptAndSave => {
                self.known_hosts.trust(&self.host, self.port, &key)?;
                Ok(true)
            }
        }
    }

    async fn auth_banner(
        &mut self,
        banner: &str,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        *self.banner.lock().unwrap_or_else(|p| p.into_inner()) = Some(banner.to_string());
        Ok(())
    }

    async fn kex_done(
        &mut self,
        _shared_secret: Option<&[u8]>,
        names: &russh::Names,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let mut slot = self.algorithms.lock().unwrap_or_else(|p| p.into_inner());
        // Only the first exchange is interesting; rekeys reuse the same names.
        if slot.is_none() {
            *slot = Some(Algorithms {
                kex: names.kex.as_ref().to_string(),
                host_key: names.key.to_string(),
                cipher: names.cipher.as_ref().to_string(),
                mac: names.server_mac.as_ref().to_string(),
            });
        }
        Ok(())
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<Msg>,
        connected_address: &str,
        connected_port: u32,
        originator_address: &str,
        originator_port: u32,
        reply: russh::client::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let route = self
            .forwarded
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(connected_port as u16))
            .cloned();
        let Some(tx) = route.filter(|tx| !tx.is_closed()) else {
            reply
                .reject(ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        };
        reply.accept().await;
        let _ = tx.send(ForwardedChannel {
            channel,
            connected_address: connected_address.to_string(),
            connected_port: connected_port as u16,
            originator_address: originator_address.to_string(),
            originator_port: originator_port as u16,
        });
        Ok(())
    }

    async fn server_channel_open_agent_forward(
        &mut self,
        _channel: Channel<Msg>,
        reply: russh::client::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Agent forwarding to the server's requests is wired by the in-process
        // agent (see `crate::agent`) in a later step; refuse until then rather
        // than leaking key access.
        reply
            .reject(ChannelOpenFailure::AdministrativelyProhibited)
            .await;
        Ok(())
    }

    async fn disconnected(
        &mut self,
        reason: russh::client::DisconnectReason<Self::Error>,
    ) -> Result<(), Self::Error> {
        let text = match &reason {
            russh::client::DisconnectReason::ReceivedDisconnect(info) => {
                format!("server disconnected: {}", info.message)
            }
            russh::client::DisconnectReason::Error(e) => e.to_string(),
        };
        let _ = self.closed.send(Some(text));
        match reason {
            russh::client::DisconnectReason::ReceivedDisconnect(_) => Ok(()),
            russh::client::DisconnectReason::Error(e) => Err(e),
        }
    }
}
