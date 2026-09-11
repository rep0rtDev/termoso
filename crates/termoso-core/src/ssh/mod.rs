//! SSH transport built on `russh`.
//!
//! [`SshClient::connect`] does host-key verification through
//! [`crate::hostkey`], tries the configured [`AuthMethod`]s in order and returns
//! a connected, authenticated client. From there you open interactive shells
//! ([`SshClient::shell`]), run commands ([`SshClient::exec`]), open SFTP
//! ([`crate::sftp::Sftp::open`]) or tunnel TCP ([`SshClient::direct_tcpip`]).
//! Jump hosts are plain recursion: connect to the jump, then
//! [`SshClient::connect_via`] through it.

mod auth;
mod handler;
pub mod proxy;
mod shell;

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use russh::Channel;
use russh::client::{Config, Handle, Msg};
use russh::{ChannelMsg, Preferred};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};

use crate::error::{CoreError, Result};
use crate::hostkey::{HostKeyPrompt, KnownHosts};
use crate::terminal::{TermEvents, TermSize};

pub use auth::{
    AuthMethod, InteractivePrompt, InteractiveQuestion, PasswordResponder, load_private_key,
};
pub use handler::{ClientHandler, ForwardedChannel};
pub use shell::SshTerminal;

/// Where to connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshTarget {
    /// Hostname or IP.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Login name.
    pub username: String,
}

impl SshTarget {
    /// `user@host:port`.
    pub fn display(&self) -> String {
        format!("{}@{}:{}", self.username, self.host, self.port)
    }
}

/// Everything needed to establish a session.
pub struct ConnectOptions {
    /// Destination.
    pub target: SshTarget,
    /// Authentication methods, tried in order.
    pub auth: Vec<AuthMethod>,
    /// Trust database.
    pub known_hosts: KnownHosts,
    /// Who to ask about unknown keys.
    pub host_key_prompt: Arc<dyn HostKeyPrompt>,
    /// Who answers keyboard-interactive prompts (falls back to the password
    /// method when `None`).
    pub interactive: Option<Arc<dyn InteractivePrompt>>,
    /// Keepalive interval (`None` = off).
    pub keepalive: Option<Duration>,
    /// TCP connect / handshake timeout.
    pub timeout: Duration,
    /// Optional proxy for the TCP leg.
    pub proxy: Option<proxy::ProxyConfig>,
    /// Environment to request on every shell (`SetEnv`).
    pub env: Vec<(String, String)>,
    /// Request agent forwarding on shells.
    pub agent_forwarding: bool,
}

impl ConnectOptions {
    /// Sensible defaults for `target`.
    pub fn new(
        target: SshTarget,
        known_hosts: KnownHosts,
        host_key_prompt: Arc<dyn HostKeyPrompt>,
    ) -> Self {
        Self {
            target,
            auth: Vec::new(),
            known_hosts,
            host_key_prompt,
            interactive: None,
            keepalive: Some(Duration::from_secs(30)),
            timeout: Duration::from_secs(20),
            proxy: None,
            env: Vec::new(),
            agent_forwarding: false,
        }
    }
}

/// Output of [`SshClient::exec`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecOutput {
    /// stdout.
    pub stdout: Vec<u8>,
    /// stderr.
    pub stderr: Vec<u8>,
    /// Exit status if the server reported one.
    pub exit_code: Option<u32>,
}

impl ExecOutput {
    /// stdout as lossy UTF-8.
    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

/// A connected and authenticated SSH session.
pub struct SshClient {
    handle: Handle<ClientHandler>,
    target: SshTarget,
    banner: Option<String>,
    closed: watch::Receiver<Option<String>>,
    forwarded: handler::ForwardRoutes,
    env: Vec<(String, String)>,
    agent_forwarding: bool,
}

impl std::fmt::Debug for SshClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshClient")
            .field("target", &self.target)
            .finish()
    }
}

fn config(opts: &ConnectOptions) -> Arc<Config> {
    Arc::new(Config {
        client_id: russh::SshId::Standard(
            format!("SSH-2.0-termoso_{}", crate::CLIENT_VERSION).into(),
        ),
        inactivity_timeout: None,
        keepalive_interval: opts.keepalive,
        keepalive_max: 3,
        preferred: Preferred::default(),
        nodelay: true,
        ..Config::default()
    })
}

impl SshClient {
    /// Connect over TCP (optionally through a proxy).
    pub async fn connect(opts: ConnectOptions) -> Result<Self> {
        let stream = tokio::time::timeout(opts.timeout, async {
            match &opts.proxy {
                Some(p) => proxy::connect(p, &opts.target.host, opts.target.port).await,
                None => Ok(
                    TcpStream::connect((opts.target.host.as_str(), opts.target.port))
                        .await
                        .map(proxy::ProxyStream::Tcp)?,
                ),
            }
        })
        .await
        .map_err(|_| {
            CoreError::Ssh(format!("timed out connecting to {}", opts.target.display()))
        })??;
        Self::connect_stream(stream, opts).await
    }

    /// Connect to `opts.target` through an already-connected jump host.
    pub async fn connect_via(jump: &SshClient, opts: ConnectOptions) -> Result<Self> {
        let channel = jump
            .handle
            .channel_open_direct_tcpip(
                opts.target.host.clone(),
                opts.target.port as u32,
                "127.0.0.1",
                0,
            )
            .await
            .map_err(|e| CoreError::Ssh(format!("jump {}: {e}", jump.target.display())))?;
        Self::connect_stream(channel.into_stream(), opts).await
    }

    /// Connect over any bidirectional stream.
    pub async fn connect_stream<S>(stream: S, opts: ConnectOptions) -> Result<Self>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (closed_tx, closed_rx) = watch::channel(None);
        let forwarded: handler::ForwardRoutes = Default::default();
        let handler = ClientHandler::new(
            opts.target.host.clone(),
            opts.target.port,
            opts.known_hosts.clone(),
            opts.host_key_prompt.clone(),
            closed_tx,
            forwarded.clone(),
        );
        let banner = handler.banner();
        let mut handle = tokio::time::timeout(
            opts.timeout,
            russh::client::connect_stream(config(&opts), stream, handler),
        )
        .await
        .map_err(|_| {
            CoreError::Ssh(format!(
                "handshake with {} timed out",
                opts.target.display()
            ))
        })??;

        auth::authenticate(&mut handle, &opts).await?;

        let banner = banner.lock().unwrap_or_else(|p| p.into_inner()).clone();
        Ok(Self {
            handle,
            target: opts.target,
            banner,
            closed: closed_rx,
            forwarded,
            env: opts.env,
            agent_forwarding: opts.agent_forwarding,
        })
    }

    /// Destination.
    pub fn target(&self) -> &SshTarget {
        &self.target
    }

    /// Pre-auth banner the server sent, if any.
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }

    /// True once the transport is gone.
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed() || self.closed.borrow().is_some()
    }

    /// Resolves when the transport closes, with the reason.
    pub async fn closed(&self) -> String {
        let mut rx = self.closed.clone();
        loop {
            if let Some(reason) = rx.borrow().clone() {
                return reason;
            }
            if rx.changed().await.is_err() {
                return "connection closed".into();
            }
        }
    }

    /// Open an interactive shell with a PTY.
    pub async fn shell(
        &self,
        term: &str,
        size: TermSize,
    ) -> Result<(Arc<SshTerminal>, TermEvents)> {
        let mut channel = self.handle.channel_open_session().await?;
        for (k, v) in &self.env {
            // Servers commonly refuse env vars; that is not fatal.
            let _ = channel.set_env(false, k.clone(), v.clone()).await;
        }
        if self.agent_forwarding {
            let _ = channel.agent_forward(false).await;
        }
        channel
            .request_pty(true, term, size.cols as u32, size.rows as u32, 0, 0, &[])
            .await?;
        Self::await_reply(&mut channel, "pty").await?;
        channel.request_shell(true).await?;
        Self::await_reply(&mut channel, "shell").await?;
        Ok(shell::spawn(channel, self.banner.clone()))
    }

    /// Wait for the server's answer to a `want_reply` channel request.
    async fn await_reply(channel: &mut Channel<Msg>, what: &str) -> Result<()> {
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Success) => return Ok(()),
                Some(ChannelMsg::Failure) => {
                    return Err(CoreError::Ssh(format!("server refused {what} request")));
                }
                Some(ChannelMsg::Close) | None => return Err(CoreError::Closed),
                Some(_) => {}
            }
        }
    }

    /// Run a command without a PTY and collect its output.
    pub async fn exec(&self, command: &str, stdin: Option<Bytes>) -> Result<ExecOutput> {
        let mut channel = self.handle.channel_open_session().await?;
        for (k, v) in &self.env {
            let _ = channel.set_env(false, k.clone(), v.clone()).await;
        }
        channel.exec(true, command).await?;
        if let Some(data) = stdin {
            channel.data_bytes(data).await?;
        }
        channel.eof().await?;
        let mut out = ExecOutput::default();
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => out.stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, .. } => out.stderr.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => out.exit_code = Some(exit_status),
                ChannelMsg::Close => break,
                _ => {}
            }
        }
        Ok(out)
    }

    /// Open a raw session channel (used by SFTP for the subsystem request).
    pub(crate) async fn open_session(&self) -> Result<russh::Channel<russh::client::Msg>> {
        Ok(self.handle.channel_open_session().await?)
    }

    /// Open a TCP tunnel to `host:port` as seen from the server.
    pub async fn direct_tcpip(
        &self,
        host: &str,
        port: u16,
        originator: (&str, u16),
    ) -> Result<russh::ChannelStream<russh::client::Msg>> {
        let ch = self
            .handle
            .channel_open_direct_tcpip(host, port as u32, originator.0, originator.1 as u32)
            .await?;
        Ok(ch.into_stream())
    }

    /// Ask the server to listen on `bind:port` and forward connections to us.
    /// Returns the port actually bound (matters for `port = 0`) and the
    /// receiver of incoming channels for it.
    pub async fn tcpip_forward(
        &self,
        bind: &str,
        port: u16,
    ) -> Result<(u16, mpsc::UnboundedReceiver<ForwardedChannel>)> {
        let bound = self.handle.tcpip_forward(bind, port as u32).await?;
        let bound = if bound == 0 { port } else { bound as u16 };
        let (tx, rx) = mpsc::unbounded_channel();
        self.forwarded
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(bound, tx);
        Ok((bound, rx))
    }

    /// Cancel a remote forward.
    pub async fn cancel_tcpip_forward(&self, bind: &str, port: u16) -> Result<()> {
        self.forwarded
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&port);
        Ok(self.handle.cancel_tcpip_forward(bind, port as u32).await?)
    }

    /// Close the connection.
    pub async fn disconnect(&self) -> Result<()> {
        if self.handle.is_closed() {
            return Ok(());
        }
        self.handle
            .disconnect(russh::Disconnect::ByApplication, "bye", "")
            .await?;
        Ok(())
    }
}
