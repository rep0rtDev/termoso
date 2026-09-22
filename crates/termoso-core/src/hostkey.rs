//! Server host-key trust.
//!
//! Every SSH connection goes through [`KnownHosts::check`]. Trust is stored as
//! `known_host` entities in the store (so it syncs with the account like
//! everything else). Pins from every unlocked vault count: a key pinned in a
//! team vault by an admin is trusted by all members, and a presented key that
//! contradicts *any* pin is reported as changed. Unknown or changed keys are
//! never accepted silently: the transport asks a [`HostKeyPrompt`] callback,
//! which the UI answers with an explicit user decision.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use russh::keys::ssh_key::{self, HashAlg, PublicKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::model::{Entity, KnownHost};
use crate::store::Store;

/// Result of checking a presented key against the trust database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HostKeyVerdict {
    /// Exactly this key is pinned for the host.
    Known,
    /// The host has never been seen.
    Unknown {
        /// Presented key.
        key: HostKeyInfo,
    },
    /// The host is pinned with a *different* key of the same type. This is
    /// what a man-in-the-middle looks like; the UI must be loud.
    Changed {
        /// Pinned key.
        old: HostKeyInfo,
        /// Presented key.
        new: HostKeyInfo,
    },
}

/// Displayable facts about a host key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostKeyInfo {
    /// `host:port`.
    pub host: String,
    /// Algorithm (`ssh-ed25519`, `rsa-sha2-512`…).
    pub key_type: String,
    /// `SHA256:…` fingerprint.
    pub fingerprint: String,
    /// Base64 key blob.
    pub public_key: String,
}

/// What the user decided about an unknown/changed key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostKeyDecision {
    /// Refuse and abort the connection.
    Reject,
    /// Accept for this connection only.
    AcceptOnce,
    /// Accept and pin.
    AcceptAndSave,
}

/// Asked when a key is not already trusted. Implemented by the UI layer.
pub trait HostKeyPrompt: Send + Sync {
    /// Decide.
    fn decide(
        &self,
        verdict: HostKeyVerdict,
    ) -> Pin<Box<dyn Future<Output = HostKeyDecision> + Send + '_>>;
}

/// Policy that never accepts anything it does not already know (batch jobs,
/// tests of the rejection path).
pub struct StrictPrompt;

impl HostKeyPrompt for StrictPrompt {
    fn decide(
        &self,
        _verdict: HostKeyVerdict,
    ) -> Pin<Box<dyn Future<Output = HostKeyDecision> + Send + '_>> {
        Box::pin(async { HostKeyDecision::Reject })
    }
}

/// Policy that answers with a fixed decision (tests, "trust on first use"
/// setting). Never use `AcceptAndSave` for [`HostKeyVerdict::Changed`] in
/// production code paths – this type applies the decision to unknown keys
/// only and always rejects changed keys.
pub struct FixedPrompt(pub HostKeyDecision);

impl HostKeyPrompt for FixedPrompt {
    fn decide(
        &self,
        verdict: HostKeyVerdict,
    ) -> Pin<Box<dyn Future<Output = HostKeyDecision> + Send + '_>> {
        let d = match verdict {
            HostKeyVerdict::Changed { .. } => HostKeyDecision::Reject,
            _ => self.0,
        };
        Box::pin(async move { d })
    }
}

/// Canonical `host:port` form (port omitted for 22, like OpenSSH).
pub fn host_id(host: &str, port: u16) -> String {
    let h = host.trim().to_ascii_lowercase();
    if port == 22 {
        h
    } else {
        format!("[{h}]:{port}")
    }
}

/// `SHA256:<base64 without padding>` like OpenSSH.
pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// Build the info struct for a key.
pub fn info(host: &str, port: u16, key: &PublicKey) -> HostKeyInfo {
    HostKeyInfo {
        host: host_id(host, port),
        key_type: key.algorithm().as_str().to_string(),
        fingerprint: fingerprint(key),
        public_key: key_b64(key),
    }
}

/// `<type> <base64>` as found in `authorized_keys`.
pub fn public_key_line(key: &PublicKey) -> String {
    format!("{} {}", key.algorithm().as_str(), key_b64(key))
}

fn key_b64(key: &PublicKey) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(key.to_bytes().unwrap_or_default())
}

fn key_from_b64(s: &str) -> Result<PublicKey> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|e| CoreError::Invalid(format!("known host key: {e}")))?;
    Ok(PublicKey::from_bytes(&bytes)?)
}

/// Trust database backed by the store.
#[derive(Clone)]
pub struct KnownHosts {
    store: Arc<Store>,
    vault_id: Uuid,
}

impl KnownHosts {
    /// Known hosts saved into `vault_id` (usually the local or personal vault).
    pub fn new(store: Arc<Store>, vault_id: Uuid) -> Self {
        Self { store, vault_id }
    }

    /// All pinned keys across unlocked vaults.
    pub fn all(&self) -> Result<Vec<Entity<KnownHost>>> {
        self.store.list::<KnownHost>(None)
    }

    /// Pinned entries for a host.
    pub fn for_host(&self, host: &str, port: u16) -> Result<Vec<Entity<KnownHost>>> {
        let id = host_id(host, port);
        Ok(self
            .all()?
            .into_iter()
            .filter(|e| e.data.hostname.eq_ignore_ascii_case(&id))
            .collect())
    }

    /// Compare the presented key with the pins. A pin of the same type in any
    /// vault that holds a different key wins over a matching one elsewhere:
    /// a team pin cannot be overridden by accepting a key locally.
    pub fn check(&self, host: &str, port: u16, key: &PublicKey) -> Result<HostKeyVerdict> {
        let presented = info(host, port, key);
        let mut known = false;
        for p in self.for_host(host, port)? {
            if p.data.key_type != presented.key_type {
                continue;
            }
            if p.data.public_key.trim() == presented.public_key {
                known = true;
            } else {
                return Ok(HostKeyVerdict::Changed {
                    old: HostKeyInfo {
                        host: p.data.hostname,
                        key_type: p.data.key_type,
                        fingerprint: p.data.fingerprint,
                        public_key: p.data.public_key,
                    },
                    new: presented,
                });
            }
        }
        if known {
            return Ok(HostKeyVerdict::Known);
        }
        // No pin, or pinned with another algorithm only (server added a key
        // type): unknown for this algorithm.
        Ok(HostKeyVerdict::Unknown { key: presented })
    }

    /// Pin a key into this vault, replacing any pin of the same type that
    /// this vault holds. Pins in other vaults are left alone.
    pub fn trust(&self, host: &str, port: u16, key: &PublicKey) -> Result<Uuid> {
        let i = info(host, port, key);
        for existing in self.for_host(host, port)? {
            if existing.vault_id == self.vault_id && existing.data.key_type == i.key_type {
                self.store.delete(existing.id)?;
            }
        }
        self.store.insert(
            self.vault_id,
            &KnownHost {
                hostname: i.host,
                key_type: i.key_type,
                public_key: i.public_key,
                fingerprint: i.fingerprint,
            },
        )
    }

    /// Remove every pin this vault holds for a host.
    pub fn forget(&self, host: &str, port: u16) -> Result<usize> {
        let pins: Vec<_> = self
            .for_host(host, port)?
            .into_iter()
            .filter(|p| p.vault_id == self.vault_id)
            .collect();
        for p in &pins {
            self.store.delete(p.id)?;
        }
        Ok(pins.len())
    }

    /// Import plain (unhashed) lines from an OpenSSH `known_hosts` file.
    /// Hashed (`|1|…`) and marker (`@cert-authority`) lines are skipped.
    /// Returns the number of pins added.
    pub fn import_openssh(&self, contents: &str) -> Result<usize> {
        let mut n = 0;
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let (Some(hosts), Some(algo), Some(blob)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if hosts.starts_with("|1|") {
                continue;
            }
            let Ok(key) = key_from_b64(blob) else {
                continue;
            };
            if key.algorithm().as_str() != algo {
                continue;
            }
            for h in hosts.split(',') {
                let (host, port) = parse_host_port(h);
                if host.contains('*') || host.contains('?') {
                    continue;
                }
                if matches!(self.check(&host, port, &key)?, HostKeyVerdict::Known) {
                    continue;
                }
                self.trust(&host, port, &key)?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Export pins as OpenSSH `known_hosts` lines.
    pub fn export_openssh(&self) -> Result<String> {
        let mut out = String::new();
        for e in self.all()? {
            out.push_str(&format!(
                "{} {} {}\n",
                e.data.hostname, e.data.key_type, e.data.public_key
            ));
        }
        Ok(out)
    }
}

fn parse_host_port(s: &str) -> (String, u16) {
    if let Some(rest) = s.strip_prefix('[')
        && let Some((host, port)) = rest.split_once("]:")
    {
        return (host.to_string(), port.parse().unwrap_or(22));
    }
    (s.to_string(), 22)
}

/// Parse an OpenSSH public key line (`ssh-ed25519 AAAA… comment`).
pub fn parse_public_key(line: &str) -> Result<PublicKey> {
    Ok(PublicKey::from_openssh(line.trim())?)
}

/// Re-export for callers that only need the key type.
pub use ssh_key::Algorithm;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::LocalVaultKind;
    use russh::keys::ssh_key::PrivateKey;
    use termoso_crypto::keys::SymmetricKey;
    use termoso_proto::vault::VaultRole;

    fn key() -> PublicKey {
        PrivateKey::random(&mut rand::rng(), ssh_key::Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone()
    }

    fn kh() -> KnownHosts {
        let store = Arc::new(Store::open_in_memory(SymmetricKey::generate()).unwrap());
        let v = store.local_vault().unwrap().id;
        KnownHosts::new(store, v)
    }

    #[test]
    fn unknown_then_known_then_changed() {
        let kh = kh();
        let k1 = key();
        let k2 = key();
        assert!(matches!(
            kh.check("Example.com", 22, &k1).unwrap(),
            HostKeyVerdict::Unknown { .. }
        ));
        kh.trust("example.com", 22, &k1).unwrap();
        assert_eq!(
            kh.check("EXAMPLE.COM", 22, &k1).unwrap(),
            HostKeyVerdict::Known
        );
        match kh.check("example.com", 22, &k2).unwrap() {
            HostKeyVerdict::Changed { old, new } => {
                assert_eq!(old.fingerprint, fingerprint(&k1));
                assert_eq!(new.fingerprint, fingerprint(&k2));
                assert!(new.fingerprint.starts_with("SHA256:"));
            }
            other => panic!("{other:?}"),
        }
        // Different port is a different host.
        assert!(matches!(
            kh.check("example.com", 2222, &k1).unwrap(),
            HostKeyVerdict::Unknown { .. }
        ));
        assert_eq!(kh.forget("example.com", 22).unwrap(), 1);
        assert!(matches!(
            kh.check("example.com", 22, &k1).unwrap(),
            HostKeyVerdict::Unknown { .. }
        ));
    }

    #[test]
    fn openssh_import_export() {
        let kh = kh();
        let k = key();
        let line = format!(
            "[git.example]:2222,10.0.0.5 ssh-ed25519 {} comment\n|1|abc|def ssh-ed25519 {}\n# c\n",
            key_b64(&k),
            key_b64(&k)
        );
        assert_eq!(kh.import_openssh(&line).unwrap(), 2);
        assert_eq!(kh.import_openssh(&line).unwrap(), 0, "idempotent");
        assert_eq!(
            kh.check("git.example", 2222, &k).unwrap(),
            HostKeyVerdict::Known
        );
        assert_eq!(kh.check("10.0.0.5", 22, &k).unwrap(), HostKeyVerdict::Known);
        let exported = kh.export_openssh().unwrap();
        assert!(exported.contains("[git.example]:2222 ssh-ed25519 "));
        assert!(exported.contains("10.0.0.5 ssh-ed25519 "));
    }

    #[test]
    fn pin_in_another_vault_is_trusted_and_cannot_be_overridden() {
        let store = Arc::new(Store::open_in_memory(SymmetricKey::generate()).unwrap());
        let local = store.local_vault().unwrap().id;
        let team = Uuid::new_v4();
        store
            .upsert_vault(
                team,
                LocalVaultKind::Team,
                "Team",
                Some(Uuid::new_v4()),
                VaultRole::Editor,
                Some(&SymmetricKey::generate()),
                1,
            )
            .unwrap();
        let admin = KnownHosts::new(store.clone(), team);
        let member = KnownHosts::new(store.clone(), local);
        let (k1, k2) = (key(), key());

        // Admin pins in the team vault: members trust it without a prompt.
        admin.trust("db.internal", 22, &k1).unwrap();
        assert_eq!(
            member.check("db.internal", 22, &k1).unwrap(),
            HostKeyVerdict::Known
        );

        // A different key is "changed" even if the member pins it locally.
        member.trust("db.internal", 22, &k2).unwrap();
        assert!(matches!(
            member.check("db.internal", 22, &k2).unwrap(),
            HostKeyVerdict::Changed { .. }
        ));
        // The local pin did not touch the team pin.
        assert_eq!(member.for_host("db.internal", 22).unwrap().len(), 2);
        assert_eq!(member.forget("db.internal", 22).unwrap(), 1);
        assert_eq!(
            member.check("db.internal", 22, &k1).unwrap(),
            HostKeyVerdict::Known
        );
    }

    #[test]
    fn fixed_prompt_never_accepts_changed_keys() {
        let p = FixedPrompt(HostKeyDecision::AcceptAndSave);
        let k = key();
        let i = info("h", 22, &k);
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(
            rt.block_on(p.decide(HostKeyVerdict::Changed {
                old: i.clone(),
                new: i.clone()
            })),
            HostKeyDecision::Reject
        );
        assert_eq!(
            rt.block_on(p.decide(HostKeyVerdict::Unknown { key: i })),
            HostKeyDecision::AcceptAndSave
        );
    }
}
