//! Error surfaced to Kotlin as `TermosoException` subclasses: one variant per
//! category the UI reacts to differently, everything else under `Other`
//! with the core's kind string.

use termoso_client::ClientError;
use termoso_core::error::CoreError;
use termoso_core::fido2::Fido2Error;
use termoso_proto::error::codes;

#[derive(Debug, Clone, thiserror::Error, uniffi::Error)]
pub enum MobileError {
    #[error("{detail}")]
    Invalid { detail: String },
    #[error("{detail}")]
    NotFound { detail: String },
    #[error("vault is locked")]
    Locked,
    #[error("{detail}")]
    Ssh { detail: String },
    #[error("authentication failed ({remaining})")]
    AuthFailed { remaining: String },
    #[error("host key rejected: {detail}")]
    HostKeyRejected { detail: String },
    #[error("{detail}")]
    Key { detail: String },
    /// Security key trouble; `kind` is the stable `fido2_*` string and
    /// `retries` the PIN attempts left when the token reports them.
    #[error("{detail}")]
    SecurityKey {
        kind: String,
        detail: String,
        retries: Option<i32>,
    },
    #[error("cancelled")]
    Cancelled,
    #[error("connection closed")]
    Closed,
    /// The server wants the password (and second factor) proved again
    /// before this account change; see `TermosoApp::reauth_start`.
    #[error("re-authentication required")]
    ReauthRequired,
    #[error("{kind}: {detail}")]
    Other { kind: String, detail: String },
}

impl MobileError {
    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::Invalid {
            detail: detail.into(),
        }
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::NotFound {
            detail: detail.into(),
        }
    }

    /// Stable kind string (mirrors `CoreError::kind`).
    pub fn kind(&self) -> String {
        match self {
            Self::Invalid { .. } => "invalid".into(),
            Self::NotFound { .. } => "not_found".into(),
            Self::Locked => "vault_locked".into(),
            Self::Ssh { .. } => "ssh".into(),
            Self::AuthFailed { .. } => "auth_failed".into(),
            Self::HostKeyRejected { .. } => "host_key_rejected".into(),
            Self::Key { .. } => "key".into(),
            Self::SecurityKey { kind, .. } => kind.clone(),
            Self::Cancelled => "cancelled".into(),
            Self::Closed => "closed".into(),
            Self::ReauthRequired => "reauth_required".into(),
            Self::Other { kind, .. } => kind.clone(),
        }
    }
}

impl From<CoreError> for MobileError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::Invalid(m) => Self::Invalid { detail: m },
            CoreError::NotFound(m) => Self::NotFound { detail: m },
            CoreError::VaultLocked(_) => Self::Locked,
            CoreError::Ssh(_) => Self::Ssh {
                detail: e.to_string(),
            },
            CoreError::AuthFailed { remaining } => Self::AuthFailed {
                remaining: remaining.join(", "),
            },
            CoreError::HostKeyRejected { .. } => Self::HostKeyRejected {
                detail: e.to_string(),
            },
            CoreError::Key(m) => Self::Key { detail: m },
            CoreError::Fido2(e) => Self::from(e),
            CoreError::Cancelled => Self::Cancelled,
            CoreError::Closed => Self::Closed,
            CoreError::Api { ref code, .. } if code == codes::REAUTH_REQUIRED => {
                Self::ReauthRequired
            }
            other => Self::Other {
                kind: other.kind().to_string(),
                detail: other.to_string(),
            },
        }
    }
}

impl From<Fido2Error> for MobileError {
    fn from(e: Fido2Error) -> Self {
        let retries = match &e {
            Fido2Error::PinInvalid { retries } => *retries,
            _ => None,
        };
        Self::SecurityKey {
            kind: e.kind().to_string(),
            detail: e.to_string(),
            retries,
        }
    }
}

impl From<ClientError> for MobileError {
    fn from(e: ClientError) -> Self {
        match e.kind {
            "invalid" => Self::Invalid { detail: e.message },
            "not_found" => Self::NotFound { detail: e.message },
            "vault_locked" => Self::Locked,
            "key" => Self::Key { detail: e.message },
            kind if kind.starts_with("fido2") => Self::SecurityKey {
                kind: kind.to_string(),
                detail: e.message,
                retries: None,
            },
            "cancelled" => Self::Cancelled,
            "reauth_required" => Self::ReauthRequired,
            kind => Self::Other {
                kind: kind.to_string(),
                detail: e.message,
            },
        }
    }
}

impl From<serde_json::Error> for MobileError {
    fn from(e: serde_json::Error) -> Self {
        Self::Other {
            kind: "json".into(),
            detail: e.to_string(),
        }
    }
}

impl From<std::io::Error> for MobileError {
    fn from(e: std::io::Error) -> Self {
        Self::Other {
            kind: "io".into(),
            detail: e.to_string(),
        }
    }
}

impl From<uuid::Error> for MobileError {
    fn from(e: uuid::Error) -> Self {
        Self::Invalid {
            detail: format!("bad id: {e}"),
        }
    }
}

/// A foreign (Kotlin) callback threw something other than `MobileException`;
/// surfaces as a link error instead of unwinding through the FFI.
impl From<uniffi::UnexpectedUniFFICallbackError> for MobileError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Other {
            kind: "callback".into(),
            detail: e.reason,
        }
    }
}

pub type Result<T> = std::result::Result<T, MobileError>;
