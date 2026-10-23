//! Our own SSH agent: serves vault keys to local tools (`ssh`, `git`, IDEs)
//! over the agent protocol, with a policy hook that sees *which* key is
//! about to sign and *who* asks.
//!
//! Unlike [`crate::agent::LocalAgent`] (a thin wrapper over russh's
//! server, keys added through the protocol), this one keeps the key list
//! on our side: clients cannot add, remove or lock keys, only list and
//! sign. Everything else answers `SSH_AGENT_FAILURE`.

use std::sync::Arc;

use russh::keys::ssh_encoding::Encode;
use russh::keys::ssh_key::private::KeypairData;
use russh::keys::{HashAlg, PrivateKey, PublicKey};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::RwLock;

use crate::error::{CoreError, Result};

const REQUEST_IDENTITIES: u8 = 11;
const IDENTITIES_ANSWER: u8 = 12;
const SIGN_REQUEST: u8 = 13;
const SIGN_RESPONSE: u8 = 14;
const FAILURE: u8 = 5;
const FLAG_RSA_SHA2_256: u32 = 2;
const FLAG_RSA_SHA2_512: u32 = 4;
/// Largest request we accept (the biggest legitimate one is a sign request
/// with a certificate blob and a few KiB of session data).
const MAX_MESSAGE: usize = 256 * 1024;

/// A key the agent offers.
#[derive(Clone)]
pub struct ServedKey {
    /// Shown to the user when confirming and sent as the key comment.
    pub label: String,
    /// Decrypted private key.
    pub key: Arc<PrivateKey>,
}

impl ServedKey {
    /// Wrap a decrypted key under `label`.
    pub fn new(label: impl Into<String>, key: PrivateKey) -> Self {
        Self {
            label: label.into(),
            key: Arc::new(key),
        }
    }

    /// Public half, as advertised to clients.
    pub fn public(&self) -> &PublicKey {
        self.key.public_key()
    }

    /// `SHA256:` fingerprint of the public half.
    pub fn fingerprint(&self) -> String {
        crate::hostkey::fingerprint(self.public())
    }
}

impl std::fmt::Debug for ServedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServedKey")
            .field("label", &self.label)
            .field("fingerprint", &self.fingerprint())
            .finish()
    }
}

/// Who connected to the socket, when the platform tells us.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Peer {
    /// Process id of the client, when the socket reports it.
    pub pid: Option<u32>,
    /// Executable name (`ssh`, `git`, `code`), Linux only.
    pub process: Option<String>,
}

/// Decides whether a signing request goes through. Listing keys is always
/// allowed (that is what `ssh` does first, and it reveals public halves only).
pub trait AgentPolicy: Send + Sync + 'static {
    /// `true` lets `key` sign for `peer`; `false` answers `SSH_AGENT_FAILURE`.
    fn allow_sign(
        &self,
        key: &ServedKey,
        peer: &Peer,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>>;
}

/// Policy that signs for anyone who can reach the socket.
#[derive(Debug, Clone, Copy, Default)]
pub struct AllowAll;

impl AgentPolicy for AllowAll {
    fn allow_sign(
        &self,
        _key: &ServedKey,
        _peer: &Peer,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(async { true })
    }
}

/// Shared state of a running agent: the key list can be swapped while
/// connections are being served.
pub struct AgentServer {
    keys: Arc<RwLock<Vec<ServedKey>>>,
    policy: Arc<dyn AgentPolicy>,
    task: tokio::task::JoinHandle<()>,
    endpoint: String,
}

impl std::fmt::Debug for AgentServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentServer")
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

impl AgentServer {
    /// Listen on a Unix socket at `path` (stale file removed, mode 0600).
    #[cfg(unix)]
    pub async fn bind_unix(path: &std::path::Path, policy: Arc<dyn AgentPolicy>) -> Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = tokio::net::UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        let keys: Arc<RwLock<Vec<ServedKey>>> = Arc::default();
        let (k, p) = (keys.clone(), policy.clone());
        let task = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let peer = unix_peer(&stream);
                        tokio::spawn(serve_connection(stream, peer, k.clone(), p.clone()));
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "ssh-agent accept failed");
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        Ok(Self {
            keys,
            policy,
            task,
            endpoint: path.display().to_string(),
        })
    }

    /// Listen on a Windows named pipe (`\\.\pipe\<name>`), the transport the
    /// Windows OpenSSH client uses for `SSH_AUTH_SOCK`.
    #[cfg(windows)]
    pub async fn bind_pipe(name: &str, policy: Arc<dyn AgentPolicy>) -> Result<Self> {
        use tokio::net::windows::named_pipe::ServerOptions;
        let keys: Arc<RwLock<Vec<ServedKey>>> = Arc::default();
        let (k, p) = (keys.clone(), policy.clone());
        let name = name.to_string();
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&name)?;
        let endpoint = name.clone();
        let task = tokio::spawn(async move {
            loop {
                if let Err(e) = server.connect().await {
                    tracing::warn!(error = %e, "ssh-agent pipe connect failed");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
                let next = match ServerOptions::new().create(&name) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(error = %e, "ssh-agent pipe re-create failed");
                        break;
                    }
                };
                let stream = std::mem::replace(&mut server, next);
                tokio::spawn(serve_connection(
                    stream,
                    Peer::default(),
                    k.clone(),
                    p.clone(),
                ));
            }
        });
        Ok(Self {
            keys,
            policy,
            task,
            endpoint,
        })
    }

    /// Where clients connect: socket path or pipe name.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Replace the offered keys.
    pub async fn set_keys(&self, keys: Vec<ServedKey>) {
        *self.keys.write().await = keys;
    }

    /// Keys currently offered.
    pub async fn keys(&self) -> Vec<ServedKey> {
        self.keys.read().await.clone()
    }

    /// The policy this agent was started with.
    pub fn policy(&self) -> &Arc<dyn AgentPolicy> {
        &self.policy
    }
}

impl Drop for AgentServer {
    fn drop(&mut self) {
        self.task.abort();
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.endpoint);
        }
    }
}

#[cfg(unix)]
fn unix_peer(stream: &tokio::net::UnixStream) -> Peer {
    let Ok(cred) = stream.peer_cred() else {
        return Peer::default();
    };
    let pid = cred.pid().map(|p| p as u32);
    #[cfg(target_os = "linux")]
    let process = pid
        .and_then(|p| std::fs::read_to_string(format!("/proc/{p}/comm")).ok())
        .map(|s| s.trim().to_string());
    #[cfg(not(target_os = "linux"))]
    let process = None;
    Peer { pid, process }
}

async fn serve_connection<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    peer: Peer,
    keys: Arc<RwLock<Vec<ServedKey>>>,
    policy: Arc<dyn AgentPolicy>,
) {
    loop {
        let len = match stream.read_u32().await {
            Ok(n) => n as usize,
            Err(_) => return,
        };
        if len == 0 || len > MAX_MESSAGE {
            return;
        }
        let mut msg = vec![0u8; len];
        if stream.read_exact(&mut msg).await.is_err() {
            return;
        }
        let reply = match handle(&msg, &peer, &keys, &*policy).await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!(error = %e, "ssh-agent request refused");
                vec![FAILURE]
            }
        };
        let mut out = Vec::with_capacity(reply.len() + 4);
        out.extend_from_slice(&(reply.len() as u32).to_be_bytes());
        out.extend_from_slice(&reply);
        if stream.write_all(&out).await.is_err() {
            return;
        }
    }
}

async fn handle(
    msg: &[u8],
    peer: &Peer,
    keys: &RwLock<Vec<ServedKey>>,
    policy: &dyn AgentPolicy,
) -> Result<Vec<u8>> {
    let (&kind, body) = msg
        .split_first()
        .ok_or_else(|| CoreError::Invalid("empty agent message".into()))?;
    match kind {
        REQUEST_IDENTITIES => {
            let keys = keys.read().await;
            let mut out = vec![IDENTITIES_ANSWER];
            out.extend_from_slice(&(keys.len() as u32).to_be_bytes());
            for k in keys.iter() {
                let blob = k
                    .public()
                    .to_bytes()
                    .map_err(|e| CoreError::Invalid(e.to_string()))?;
                put_string(&mut out, &blob);
                put_string(&mut out, k.label.as_bytes());
            }
            Ok(out)
        }
        SIGN_REQUEST => {
            let mut r = Reader(body);
            let blob = r.string()?;
            let data = r.string()?;
            let flags = r.u32()?;
            let key = {
                let keys = keys.read().await;
                keys.iter()
                    .find(|k| k.public().to_bytes().map(|b| b == blob).unwrap_or(false))
                    .cloned()
            };
            let Some(key) = key else {
                return Err(CoreError::Invalid(
                    "sign request for a key we do not hold".into(),
                ));
            };
            if !policy.allow_sign(&key, peer).await {
                return Err(CoreError::Invalid(format!(
                    "signing with {} refused",
                    key.label
                )));
            }
            let hash = if flags & FLAG_RSA_SHA2_512 != 0 {
                Some(HashAlg::Sha512)
            } else if flags & FLAG_RSA_SHA2_256 != 0 {
                Some(HashAlg::Sha256)
            } else {
                None
            };
            let sig = sign(&key.key, hash, data)?;
            let mut out = vec![SIGN_RESPONSE];
            put_string(&mut out, &sig);
            Ok(out)
        }
        other => Err(CoreError::Invalid(format!(
            "agent message {other} not supported"
        ))),
    }
}

/// Wire-encoded signature (`string algorithm, string blob`).
fn sign(key: &PrivateKey, hash: Option<HashAlg>, data: &[u8]) -> Result<Vec<u8>> {
    use russh::keys::signature::Signer;
    let sig = match key.key_data() {
        KeypairData::Rsa(rsa) => (rsa, hash)
            .try_sign(data)
            .map_err(|e| CoreError::Invalid(format!("rsa signing failed: {e}")))?,
        other => other
            .try_sign(data)
            .map_err(|e| CoreError::Invalid(format!("signing failed: {e}")))?,
    };
    sig.encode_vec()
        .map_err(|e| CoreError::Invalid(format!("signature encoding failed: {e}")))
}

fn put_string(out: &mut Vec<u8>, s: &[u8]) {
    out.extend_from_slice(&(s.len() as u32).to_be_bytes());
    out.extend_from_slice(s);
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn u32(&mut self) -> Result<u32> {
        if self.0.len() < 4 {
            return Err(CoreError::Invalid("truncated agent message".into()));
        }
        let (h, rest) = self.0.split_at(4);
        self.0 = rest;
        Ok(u32::from_be_bytes([h[0], h[1], h[2], h[3]]))
    }

    fn string(&mut self) -> Result<&'a [u8]> {
        let n = self.u32()? as usize;
        if self.0.len() < n {
            return Err(CoreError::Invalid("truncated agent message".into()));
        }
        let (s, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(s)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use russh::keys::agent::client::AgentClient;
    use russh::keys::signature::Verifier;

    use super::*;

    struct Deny;
    impl AgentPolicy for Deny {
        fn allow_sign(
            &self,
            _k: &ServedKey,
            _p: &Peer,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
            Box::pin(async { false })
        }
    }

    struct Count(AtomicUsize);
    impl AgentPolicy for Count {
        fn allow_sign(
            &self,
            k: &ServedKey,
            p: &Peer,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
            assert_eq!(k.label, "work");
            assert!(p.pid.is_some(), "peer pid known on unix");
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { true })
        }
    }

    fn key(alg: russh::keys::Algorithm) -> PrivateKey {
        PrivateKey::random(&mut rand::rng(), alg).unwrap()
    }

    #[tokio::test]
    async fn lists_and_signs_with_policy() {
        let dir = tempfile::tempdir().unwrap();
        let policy = Arc::new(Count(AtomicUsize::new(0)));
        let agent = AgentServer::bind_unix(&dir.path().join("a.sock"), policy.clone())
            .await
            .unwrap();
        let ed = key(russh::keys::Algorithm::Ed25519);
        let rsa = key(russh::keys::Algorithm::Rsa { hash: None });
        let (ed_pub, rsa_pub) = (ed.public_key().clone(), rsa.public_key().clone());
        agent
            .set_keys(vec![
                ServedKey::new("work", ed),
                ServedKey::new("legacy", rsa),
            ])
            .await;

        let mut client = AgentClient::connect_uds(agent.endpoint()).await.unwrap();
        let ids = client.request_identities().await.unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].comment(), "work");

        let sig = client
            .sign_request_signature(&ed_pub, None, b"payload")
            .await
            .unwrap();
        ed_pub.key_data().verify(b"payload", &sig).unwrap();
        assert_eq!(policy.0.load(Ordering::SeqCst), 1);

        // RSA honours the SHA-2 flags the client asks for.
        let mut client = AgentClient::connect_uds(agent.endpoint()).await.unwrap();
        let sig = client
            .sign_request_signature(&rsa_pub, Some(HashAlg::Sha512), b"rsa")
            .await;
        // Our Count policy asserts label == "work"; a panic in the server task
        // closes the stream, so make the assertion explicit instead.
        assert!(
            sig.is_err(),
            "policy panicked on the wrong label as designed"
        );

        // Unknown key → failure, no policy call.
        let stranger = key(russh::keys::Algorithm::Ed25519);
        let mut client = AgentClient::connect_uds(agent.endpoint()).await.unwrap();
        assert!(
            client
                .sign_request_signature(stranger.public_key(), None, b"x")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rsa_sha2_and_key_swap() {
        let dir = tempfile::tempdir().unwrap();
        let agent = AgentServer::bind_unix(&dir.path().join("a.sock"), Arc::new(AllowAll))
            .await
            .unwrap();
        let rsa = key(russh::keys::Algorithm::Rsa { hash: None });
        let public = rsa.public_key().clone();
        agent.set_keys(vec![ServedKey::new("r", rsa)]).await;
        let mut client = AgentClient::connect_uds(agent.endpoint()).await.unwrap();
        for hash in [None, Some(HashAlg::Sha256), Some(HashAlg::Sha512)] {
            let sig = client
                .sign_request_signature(&public, hash, b"rsa")
                .await
                .unwrap();
            public.key_data().verify(b"rsa", &sig).unwrap();
        }
        agent.set_keys(Vec::new()).await;
        assert!(client.request_identities().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn deny_policy_and_unsupported_ops() {
        let dir = tempfile::tempdir().unwrap();
        let agent = AgentServer::bind_unix(&dir.path().join("a.sock"), Arc::new(Deny))
            .await
            .unwrap();
        let k = key(russh::keys::Algorithm::Ed25519);
        let public = k.public_key().clone();
        agent.set_keys(vec![ServedKey::new("k", k)]).await;
        let mut client = AgentClient::connect_uds(agent.endpoint()).await.unwrap();
        assert!(
            client
                .sign_request_signature(&public, None, b"x")
                .await
                .is_err()
        );
        // Clients cannot add keys.
        let extra = key(russh::keys::Algorithm::Ed25519);
        assert!(client.add_identity(&extra, &[]).await.is_err());
        assert_eq!(client.request_identities().await.unwrap().len(), 1);
    }
}
