//! SSH agent: talking to the system agent, and running our own that serves
//! vault keys to local tools (`SSH_AUTH_SOCK`) and to agent-forwarding.

use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::Arc;

#[cfg(unix)]
use russh::keys::PrivateKey;
use russh::keys::PublicKey;
use russh::keys::agent::client::{AgentClient, AgentStream};
use russh::keys::agent::server::{Agent, MessageType};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use tokio::sync::Mutex;

use crate::error::{CoreError, Result};

/// Type-erased agent client.
pub type DynAgentClient = AgentClient<Box<dyn AgentStream + Send + Unpin + 'static>>;

/// Connect to the platform SSH agent (`SSH_AUTH_SOCK`; Pageant / OpenSSH
/// named pipe on Windows).
pub async fn connect_system_agent() -> Result<DynAgentClient> {
    #[cfg(unix)]
    {
        let c = AgentClient::connect_env()
            .await
            .map_err(|e| CoreError::Ssh(format!("ssh-agent: {e}")))?;
        Ok(c.dynamic())
    }
    #[cfg(windows)]
    {
        if let Ok(c) = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await {
            return Ok(c.dynamic());
        }
        let c = AgentClient::connect_pageant()
            .await
            .map_err(|e| CoreError::Ssh(format!("pageant: {e}")))?;
        Ok(c.dynamic())
    }
}

/// Connect to an agent at an explicit socket/pipe path.
pub async fn connect_agent_at(path: &Path) -> Result<DynAgentClient> {
    #[cfg(unix)]
    {
        let c = AgentClient::connect_uds(path)
            .await
            .map_err(|e| CoreError::Ssh(format!("ssh-agent {}: {e}", path.display())))?;
        Ok(c.dynamic())
    }
    #[cfg(windows)]
    {
        let c = AgentClient::connect_named_pipe(path)
            .await
            .map_err(|e| CoreError::Ssh(format!("ssh-agent {}: {e}", path.display())))?;
        Ok(c.dynamic())
    }
}

/// A key currently held by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentKey {
    /// Algorithm name (`ssh-ed25519`, …).
    pub key_type: String,
    /// SHA-256 fingerprint.
    pub fingerprint: String,
    /// `<type> <base64>` line.
    pub public_key: String,
    /// Agent comment.
    pub comment: String,
    /// Whether this entry is a certificate.
    pub certificate: bool,
}

/// List identities held by `agent`.
pub async fn list_keys(agent: &mut DynAgentClient) -> Result<Vec<AgentKey>> {
    let ids = agent
        .request_identities()
        .await
        .map_err(|e| CoreError::Ssh(format!("ssh-agent: {e}")))?;
    Ok(ids
        .into_iter()
        .map(|id| match id {
            russh::keys::agent::AgentIdentity::PublicKey { key, comment } => AgentKey {
                key_type: key.algorithm().to_string(),
                fingerprint: crate::hostkey::fingerprint(&key),
                public_key: crate::hostkey::public_key_line(&key),
                comment,
                certificate: false,
            },
            russh::keys::agent::AgentIdentity::Certificate {
                certificate,
                comment,
            } => {
                let key = PublicKey::from(certificate.public_key().clone());
                AgentKey {
                    key_type: certificate.algorithm().to_string(),
                    fingerprint: crate::hostkey::fingerprint(&key),
                    public_key: crate::hostkey::public_key_line(&key),
                    comment,
                    certificate: true,
                }
            }
        })
        .collect())
}

/// Policy hook for the in-process agent. Signing requests are allowed by
/// default; a UI can wrap this to confirm each use.
#[derive(Clone, Default)]
pub struct AllowAll;

impl Agent for AllowAll {
    async fn confirm_request(&self, _msg: MessageType) -> bool {
        true
    }
}

/// Our own agent, listening on a Unix socket (Unix only for now).
///
/// Keys are added through the agent protocol itself, so the key store lives
/// inside russh's server and is dropped with the task.
#[cfg(unix)]
pub struct LocalAgent {
    path: PathBuf,
    task: tokio::task::JoinHandle<()>,
    client: Mutex<Option<DynAgentClient>>,
    keys: Mutex<Vec<Arc<PrivateKey>>>,
}

#[cfg(unix)]
impl std::fmt::Debug for LocalAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalAgent")
            .field("path", &self.path)
            .finish()
    }
}

#[cfg(unix)]
impl LocalAgent {
    /// Start listening at `path` (removed first if stale). The socket is
    /// created with mode 0600.
    pub async fn start(path: PathBuf) -> Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = tokio::net::UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let stream = futures::stream::unfold(listener, |l| async move {
            let r = l.accept().await.map(|(s, _)| s);
            Some((r, l))
        });
        let task = tokio::spawn(async move {
            if let Err(e) = russh::keys::agent::server::serve(Box::pin(stream), AllowAll).await {
                tracing::warn!(error = %e, "local ssh-agent stopped");
            }
        });
        Ok(Self {
            path,
            task,
            client: Mutex::new(None),
            keys: Mutex::new(Vec::new()),
        })
    }

    /// Socket path to export as `SSH_AUTH_SOCK`.
    pub fn socket_path(&self) -> &Path {
        &self.path
    }

    async fn client(&self) -> Result<tokio::sync::MutexGuard<'_, Option<DynAgentClient>>> {
        let mut guard = self.client.lock().await;
        if guard.is_none() {
            *guard = Some(connect_agent_at(&self.path).await?);
        }
        Ok(guard)
    }

    /// Load a key into the agent. `lifetime_secs` limits how long it stays.
    pub async fn add_key(&self, key: PrivateKey, lifetime_secs: Option<u32>) -> Result<()> {
        let mut constraints = Vec::new();
        if let Some(seconds) = lifetime_secs {
            constraints.push(russh::keys::agent::Constraint::KeyLifetime { seconds });
        }
        let mut c = self.client().await?;
        c.as_mut()
            .expect("client initialised")
            .add_identity(&key, &constraints)
            .await
            .map_err(|e| CoreError::Ssh(format!("local agent: {e}")))?;
        self.keys.lock().await.push(Arc::new(key));
        Ok(())
    }

    /// Remove one key.
    pub async fn remove_key(&self, public: &PublicKey) -> Result<()> {
        let mut c = self.client().await?;
        c.as_mut()
            .expect("client initialised")
            .remove_identity(public)
            .await
            .map_err(|e| CoreError::Ssh(format!("local agent: {e}")))?;
        self.keys.lock().await.retain(|k| k.public_key() != public);
        Ok(())
    }

    /// Remove everything.
    pub async fn clear(&self) -> Result<()> {
        let mut c = self.client().await?;
        c.as_mut()
            .expect("client initialised")
            .remove_all_identities()
            .await
            .map_err(|e| CoreError::Ssh(format!("local agent: {e}")))?;
        self.keys.lock().await.clear();
        Ok(())
    }

    /// Public halves of the keys we loaded.
    pub async fn loaded(&self) -> Vec<PublicKey> {
        self.keys
            .lock()
            .await
            .iter()
            .map(|k| k.public_key().clone())
            .collect()
    }

    /// List through the protocol (what other processes would see).
    pub async fn list(&self) -> Result<Vec<AgentKey>> {
        let mut c = self.client().await?;
        list_keys(c.as_mut().expect("client initialised")).await
    }

    /// Open a fresh protocol connection to this agent.
    pub async fn connect(&self) -> Result<DynAgentClient> {
        connect_agent_at(&self.path).await
    }
}

#[cfg(unix)]
impl Drop for LocalAgent {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_agent_serves_and_signs() {
        let dir = tempfile::tempdir().unwrap();
        let agent = LocalAgent::start(dir.path().join("agent.sock"))
            .await
            .unwrap();
        let key = PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
        let public = key.public_key().clone();
        agent.add_key(key, None).await.unwrap();

        let listed = agent.list().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].fingerprint, crate::hostkey::fingerprint(&public));

        let mut client = agent.connect().await.unwrap();
        let sig = client
            .sign_request_signature(&public, None, b"payload")
            .await
            .unwrap();
        use russh::keys::signature::Verifier;
        public.key_data().verify(b"payload", &sig).unwrap();

        agent.remove_key(&public).await.unwrap();
        assert!(agent.list().await.unwrap().is_empty());
    }
}
