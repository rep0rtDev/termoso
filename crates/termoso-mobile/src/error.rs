//! Error surfaced to Kotlin as `TermosoException` subclasses: one variant per
//! category the UI reacts to differently, everything else under `Other`
//! with the core's kind string.

use termoso_client::ClientError;
use termoso_core::error::CoreError;

#[derive(Debug, Clone, thiserror::Error, uniffi::Error)]
pub enum MobileError {
    #[error("{message}")]
    Invalid { message: String },
    #[error("{message}")]
    NotFound { message: String },
    #[error("vault is locked")]
    Locked,
    #[error("{message}")]
    Ssh { message: String },
    #[error("authentication failed ({remaining})")]
    AuthFailed { remaining: String },
    #[error("host key rejected: {message}")]
    HostKeyRejected { message: String },
    #[error("{message}")]
    Key { message: String },
    #[error("cancelled")]
    Cancelled,
    #[error("connection closed")]
    Closed,
    #[error("{kind}: {message}")]
    Other { kind: String, message: String },
}

impl MobileError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
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
            Self::Cancelled => "cancelled".into(),
            Self::Closed => "closed".into(),
            Self::Other { kind, .. } => kind.clone(),
        }
    }
}

impl From<CoreError> for MobileError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::Invalid(m) => Self::Invalid { message: m },
            CoreError::NotFound(m) => Self::NotFound { message: m },
            CoreError::VaultLocked(_) => Self::Locked,
            CoreError::Ssh(_) => Self::Ssh {
                message: e.to_string(),
            },
            CoreError::AuthFailed { remaining } => Self::AuthFailed {
                remaining: remaining.join(", "),
            },
            CoreError::HostKeyRejected { .. } => Self::HostKeyRejected {
                message: e.to_string(),
            },
            CoreError::Key(m) => Self::Key { message: m },
            CoreError::Cancelled => Self::Cancelled,
            CoreError::Closed => Self::Closed,
            other => Self::Other {
                kind: other.kind().to_string(),
                message: other.to_string(),
            },
        }
    }
}

impl From<ClientError> for MobileError {
    fn from(e: ClientError) -> Self {
        match e.kind {
            "invalid" => Self::Invalid { message: e.message },
            "not_found" => Self::NotFound { message: e.message },
            "vault_locked" => Self::Locked,
            "key" => Self::Key { message: e.message },
            "cancelled" => Self::Cancelled,
            kind => Self::Other {
                kind: kind.to_string(),
                message: e.message,
            },
        }
    }
}

impl From<serde_json::Error> for MobileError {
    fn from(e: serde_json::Error) -> Self {
        Self::Other {
            kind: "json".into(),
            message: e.to_string(),
        }
    }
}

impl From<std::io::Error> for MobileError {
    fn from(e: std::io::Error) -> Self {
        Self::Other {
            kind: "io".into(),
            message: e.to_string(),
        }
    }
}

impl From<uuid::Error> for MobileError {
    fn from(e: uuid::Error) -> Self {
        Self::Invalid {
            message: format!("bad id: {e}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, MobileError>;
