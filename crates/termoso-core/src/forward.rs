//! Port forwarding: local (`-L`), remote (`-R`) and dynamic SOCKS5 (`-D`).
//!
//! Every forward is a task tree under a [`CancellationToken`]; dropping the
//! [`Forward`] handle stops it and closes its connections.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use crate::error::{CoreError, Result};
use crate::ssh::SshClient;

/// Which kind of tunnel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ForwardSpec {
    /// Listen locally, connect on the server side.
    Local {
        /// Local bind address (`127.0.0.1`).
        bind: String,
        /// Local port (0 = ephemeral).
        port: u16,
        /// Destination as seen from the server.
        remote_host: String,
        /// Destination port.
        remote_port: u16,
    },
    /// Server listens, connections come to us and go to a local address.
    Remote {
        /// Server bind address (`""` / `localhost` / `0.0.0.0`).
        bind: String,
        /// Server port (0 = server picks).
        port: u16,
        /// Where we connect for each incoming stream.
        local_host: String,
        /// Local destination port.
        local_port: u16,
    },
    /// Local SOCKS5 proxy whose connections exit at the server.
    Dynamic {
        /// Local bind address.
        bind: String,
        /// Local port.
        port: u16,
    },
}

impl ForwardSpec {
    /// Build from a stored `PfRule` (`kind` in `local|remote|dynamic`).
    pub fn from_rule(rule: &crate::model::PfRule) -> Result<Self> {
        let bind = if rule.bound_address.is_empty() {
            "127.0.0.1".to_string()
        } else {
            rule.bound_address.clone()
        };
        Ok(match rule.kind.as_str() {
            "local" => Self::Local {
                bind,
                port: rule.local_port,
                remote_host: rule.remote_host.clone(),
                remote_port: rule.remote_port,
            },
            "remote" => Self::Remote {
                bind: if rule.bound_address == "127.0.0.1" {
                    "localhost".into()
                } else {
                    rule.bound_address.clone()
                },
                port: rule.remote_port,
                local_host: if rule.remote_host.is_empty() {
                    "127.0.0.1".into()
                } else {
                    rule.remote_host.clone()
                },
                local_port: rule.local_port,
            },
            "dynamic" => Self::Dynamic {
                bind,
                port: rule.local_port,
            },
            other => {
                return Err(CoreError::Invalid(format!(
                    "unknown forwarding kind {other:?}"
                )));
            }
        })
    }
}

/// Live counters for the UI.
#[derive(Debug, Default)]
pub struct ForwardStats {
    /// Connections accepted since start.
    pub connections: AtomicU64,
    /// Connections currently open.
    pub active: AtomicU64,
    /// Bytes local → remote.
    pub bytes_out: AtomicU64,
    /// Bytes remote → local.
    pub bytes_in: AtomicU64,
}

/// A running forward.
pub struct Forward {
    spec: ForwardSpec,
    bound: Option<SocketAddr>,
    remote_port: Option<u16>,
    cancel: CancellationToken,
    stats: Arc<ForwardStats>,
    client: Arc<SshClient>,
}

impl std::fmt::Debug for Forward {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Forward")
            .field("spec", &self.spec)
            .field("bound", &self.bound)
            .field("remote_port", &self.remote_port)
            .finish()
    }
}

impl Forward {
    /// Start the forward described by `spec` over `client`.
    pub async fn start(client: Arc<SshClient>, spec: ForwardSpec) -> Result<Self> {
        let cancel = CancellationToken::new();
        let stats = Arc::new(ForwardStats::default());
        let mut fwd = Self {
            spec: spec.clone(),
            bound: None,
            remote_port: None,
            cancel: cancel.clone(),
            stats: stats.clone(),
            client: client.clone(),
        };
        match spec {
            ForwardSpec::Local {
                bind,
                port,
                remote_host,
                remote_port,
            } => {
                let listener = TcpListener::bind((bind.as_str(), port)).await?;
                fwd.bound = Some(listener.local_addr()?);
                tokio::spawn(local_loop(
                    listener,
                    client,
                    remote_host,
                    remote_port,
                    cancel,
                    stats,
                ));
            }
            ForwardSpec::Dynamic { bind, port } => {
                let listener = TcpListener::bind((bind.as_str(), port)).await?;
                fwd.bound = Some(listener.local_addr()?);
                tokio::spawn(dynamic_loop(listener, client, cancel, stats));
            }
            ForwardSpec::Remote {
                bind,
                port,
                local_host,
                local_port,
            } => {
                let (bound, rx) = client.tcpip_forward(&bind, port).await?;
                fwd.remote_port = Some(bound);
                tokio::spawn(remote_loop(rx, local_host, local_port, cancel, stats));
            }
        }
        Ok(fwd)
    }

    /// Spec.
    pub fn spec(&self) -> &ForwardSpec {
        &self.spec
    }

    /// Local socket for `Local` / `Dynamic`.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.bound
    }

    /// Server-side port for `Remote`.
    pub fn remote_port(&self) -> Option<u16> {
        self.remote_port
    }

    /// Counters.
    pub fn stats(&self) -> &ForwardStats {
        &self.stats
    }

    /// Stop; for remote forwards also asks the server to stop listening.
    pub async fn stop(&self) {
        self.cancel.cancel();
        if let (ForwardSpec::Remote { bind, .. }, Some(port)) = (&self.spec, self.remote_port) {
            let _ = self.client.cancel_tcpip_forward(bind, port).await;
        }
    }
}

impl Drop for Forward {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

async fn pipe<A, B>(mut a: A, mut b: B, cancel: CancellationToken, stats: Arc<ForwardStats>)
where
    A: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    B: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    stats.connections.fetch_add(1, Ordering::Relaxed);
    stats.active.fetch_add(1, Ordering::Relaxed);
    let r = tokio::select! {
        r = tokio::io::copy_bidirectional(&mut a, &mut b) => r.ok(),
        _ = cancel.cancelled() => None,
    };
    if let Some((out, inn)) = r {
        stats.bytes_out.fetch_add(out, Ordering::Relaxed);
        stats.bytes_in.fetch_add(inn, Ordering::Relaxed);
    }
    let _ = a.shutdown().await;
    let _ = b.shutdown().await;
    stats.active.fetch_sub(1, Ordering::Relaxed);
}

async fn local_loop(
    listener: TcpListener,
    client: Arc<SshClient>,
    host: String,
    port: u16,
    cancel: CancellationToken,
    stats: Arc<ForwardStats>,
) {
    loop {
        let (sock, peer) = tokio::select! {
            r = listener.accept() => match r { Ok(x) => x, Err(_) => break },
            _ = cancel.cancelled() => break,
        };
        let client = client.clone();
        let host = host.clone();
        let cancel = cancel.child_token();
        let stats = stats.clone();
        tokio::spawn(async move {
            match client
                .direct_tcpip(&host, port, (&peer.ip().to_string(), peer.port()))
                .await
            {
                Ok(chan) => pipe(sock, chan, cancel, stats).await,
                Err(e) => tracing::debug!(error = %e, "local forward: channel open failed"),
            }
        });
    }
}

async fn remote_loop(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<crate::ssh::ForwardedChannel>,
    host: String,
    port: u16,
    cancel: CancellationToken,
    stats: Arc<ForwardStats>,
) {
    loop {
        let fwd = tokio::select! {
            r = rx.recv() => match r { Some(x) => x, None => break },
            _ = cancel.cancelled() => break,
        };
        let host = host.clone();
        let cancel = cancel.child_token();
        let stats = stats.clone();
        tokio::spawn(async move {
            match TcpStream::connect((host.as_str(), port)).await {
                Ok(sock) => pipe(fwd.channel.into_stream(), sock, cancel, stats).await,
                Err(e) => tracing::debug!(error = %e, "remote forward: local connect failed"),
            }
        });
    }
}

async fn dynamic_loop(
    listener: TcpListener,
    client: Arc<SshClient>,
    cancel: CancellationToken,
    stats: Arc<ForwardStats>,
) {
    loop {
        let (sock, peer) = tokio::select! {
            r = listener.accept() => match r { Ok(x) => x, Err(_) => break },
            _ = cancel.cancelled() => break,
        };
        let client = client.clone();
        let cancel = cancel.child_token();
        let stats = stats.clone();
        tokio::spawn(async move {
            if let Err(e) = socks5_serve(sock, peer, client, cancel, stats).await {
                tracing::debug!(error = %e, "dynamic forward: socks5 session failed");
            }
        });
    }
}

/// Minimal SOCKS5 server side: no auth, CONNECT only.
async fn socks5_serve(
    mut sock: TcpStream,
    peer: SocketAddr,
    client: Arc<SshClient>,
    cancel: CancellationToken,
    stats: Arc<ForwardStats>,
) -> Result<()> {
    let mut hdr = [0u8; 2];
    sock.read_exact(&mut hdr).await?;
    if hdr[0] != 5 {
        return Err(CoreError::Invalid("socks: not version 5".into()));
    }
    let mut methods = vec![0u8; hdr[1] as usize];
    sock.read_exact(&mut methods).await?;
    if !methods.contains(&0) {
        sock.write_all(&[5, 0xff]).await?;
        return Err(CoreError::Invalid("socks: client requires auth".into()));
    }
    sock.write_all(&[5, 0]).await?;

    let mut req = [0u8; 4];
    sock.read_exact(&mut req).await?;
    if req[1] != 1 {
        sock.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        return Err(CoreError::Invalid("socks: only CONNECT supported".into()));
    }
    let host = match req[3] {
        1 => {
            let mut a = [0u8; 4];
            sock.read_exact(&mut a).await?;
            std::net::Ipv4Addr::from(a).to_string()
        }
        4 => {
            let mut a = [0u8; 16];
            sock.read_exact(&mut a).await?;
            std::net::Ipv6Addr::from(a).to_string()
        }
        3 => {
            let mut l = [0u8; 1];
            sock.read_exact(&mut l).await?;
            let mut name = vec![0u8; l[0] as usize];
            sock.read_exact(&mut name).await?;
            String::from_utf8_lossy(&name).into_owned()
        }
        _ => {
            sock.write_all(&[5, 8, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Err(CoreError::Invalid("socks: bad address type".into()));
        }
    };
    let mut p = [0u8; 2];
    sock.read_exact(&mut p).await?;
    let port = u16::from_be_bytes(p);

    match client
        .direct_tcpip(&host, port, (&peer.ip().to_string(), peer.port()))
        .await
    {
        Ok(chan) => {
            sock.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            pipe(sock, chan, cancel, stats).await;
            Ok(())
        }
        Err(e) => {
            sock.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            Err(e)
        }
    }
}
