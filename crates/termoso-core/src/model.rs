//! Typed view over the opaque sync entities.
//!
//! The wire format (`termoso_proto::entities`) is an encrypted blob per
//! entity. Locally we work with the decrypted payload plus the routing fields
//! that both sides agree on. `Payload` ties each plaintext struct to its kind
//! string so the store can encrypt with the right AAD.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

pub use termoso_proto::entities::payload::*;

/// A plaintext entity payload with a fixed kind.
pub trait Payload: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Kind string (see `termoso_proto::entities::KINDS`).
    const KIND: &'static str;
}

macro_rules! payloads {
    ($($ty:ident => $kind:literal),* $(,)?) => {
        $(impl Payload for $ty { const KIND: &'static str = $kind; })*
    };
}

payloads! {
    Group => "group",
    Host => "host",
    SshConfig => "ssh_config",
    TelnetConfig => "telnet_config",
    SerialConfig => "serial_config",
    Identity => "identity",
    SshKey => "ssh_key",
    SshCertificate => "ssh_certificate",
    KnownHost => "known_host",
    Snippet => "snippet",
    SnippetPackage => "snippet_package",
    HostSnippet => "host_snippet",
    PfRule => "pf_rule",
    Proxy => "proxy",
    HostChain => "host_chain",
    Tag => "tag",
    TagHost => "tag_host",
    PortKnocking => "port_knocking",
    CloudImport => "cloud_import",
    Workspace => "workspace",
    LogBookmark => "log_bookmark",
}

/// Decrypted entity as handed to the UI.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Entity<T> {
    /// Id.
    pub id: Uuid,
    /// Vault.
    pub vault_id: Uuid,
    /// Server version (0 while never pushed).
    pub version: i64,
    /// Last modification.
    pub updated_at: DateTime<Utc>,
    /// Has local changes not yet pushed.
    pub dirty: bool,
    /// Payload.
    pub data: T,
}

/// Kind-agnostic entity for generic listing (payload as JSON).
pub type AnyEntity = Entity<serde_json::Value>;

/// Sync state of a local record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    /// In sync with the server (or local-only vault).
    Synced,
    /// Modified locally.
    Dirty,
    /// Deleted locally, tombstone not yet pushed.
    PendingDelete,
}

/// Everything needed to open a connection to a host, with group inheritance
/// and identity resolved. Built by [`crate::store::Store::resolve_host`].
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ResolvedHost {
    /// Host entity.
    pub host: Entity<Host>,
    /// Effective SSH config (host → group chain → default).
    pub ssh: SshConfig,
    /// Identity referenced by the SSH config, if any.
    pub identity: Option<Entity<Identity>>,
    /// Key referenced by the identity.
    pub key: Option<Entity<SshKey>>,
    /// Certificate referenced by the identity.
    pub certificate: Option<Entity<SshCertificate>>,
    /// Proxy, if any.
    pub proxy: Option<Entity<Proxy>>,
    /// Jump hosts in order (each already resolved one level).
    pub chain: Vec<Entity<Host>>,
    /// Telnet config when the host is a telnet target.
    pub telnet: Option<TelnetConfig>,
    /// Group labels from root to the host's group.
    pub group_path: Vec<String>,
    /// Tag labels.
    pub tags: Vec<String>,
}

impl ResolvedHost {
    /// Effective SSH port.
    pub fn port(&self) -> u16 {
        self.ssh.port.unwrap_or(22)
    }

    /// Effective username (`identity.username`, falling back to `root`).
    pub fn username(&self) -> String {
        self.identity
            .as_ref()
            .map(|i| i.data.username.clone())
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| "root".to_string())
    }
}
