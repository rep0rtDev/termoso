//! `russh::client::Handler` implementation: host-key verification and
//! server-initiated channels.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use russh::client::{Msg, Session};
use russh::keys::PublicKeyOrCertificate;
use russh::keys::ssh_key::PublicKey;
use russh::{Channel, ChannelOpenFailure};
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

/// Termoso's russh client handler.
pub struct ClientHandler {
    host: String,
    port: u16,
    known_hosts: KnownHosts,
    prompt: Arc<dyn HostKeyPrompt>,
    banner: Arc<Mutex<Option<String>>>,
    closed: watch::Sender<Option<String>>,
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
            closed,
            forwarded,
        }
    }

    pub(crate) fn banner(&self) -> Arc<Mutex<Option<String>>> {
        self.banner.clone()
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
        match self.prompt.decide(verdict).await {
            HostKeyDecision::Reject => {
                tracing::warn!(host = %self.host, changed, "host key rejected");
                Ok(false)
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
