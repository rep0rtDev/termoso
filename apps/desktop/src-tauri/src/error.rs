//! Error shape handed to the webview: `{ kind, message }`.

use serde::Serialize;
use termoso_core::error::CoreError;
use termoso_proto::error::codes;

/// Serialised error. `kind` is stable and machine-readable; `message` is safe
/// to show (core errors never contain secret material).
#[derive(Debug, Clone, Serialize)]
pub struct DesktopError {
    pub kind: &'static str,
    pub message: String,
}

impl DesktopError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }
}

impl std::fmt::Display for DesktopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for DesktopError {}

impl From<CoreError> for DesktopError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::Api { code, message, .. } => {
                let kind = if code == codes::MFA_REQUIRED {
                    "mfa_required"
                } else if code == codes::FORBIDDEN {
                    "forbidden"
                } else if code == codes::NOT_FOUND {
                    "not_found"
                } else if code == codes::CONFLICT {
                    "conflict"
                } else if code == codes::RATE_LIMITED {
                    "rate_limited"
                } else if code == codes::MULTIPLAYER_DISABLED {
                    "multiplayer_disabled"
                } else if code == codes::UNAUTHORIZED || code == codes::TOKEN_EXPIRED {
                    "unauthorized"
                } else {
                    "api"
                };
                Self::new(kind, message)
            }
            other => Self::new(other.kind(), other.to_string()),
        }
    }
}

impl From<serde_json::Error> for DesktopError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("json", e.to_string())
    }
}

impl From<std::io::Error> for DesktopError {
    fn from(e: std::io::Error) -> Self {
        Self::new("io", e.to_string())
    }
}

impl From<tauri::Error> for DesktopError {
    fn from(e: tauri::Error) -> Self {
        Self::new("tauri", e.to_string())
    }
}

impl From<uuid::Error> for DesktopError {
    fn from(e: uuid::Error) -> Self {
        Self::new("invalid", format!("bad id: {e}"))
    }
}

pub type Result<T> = std::result::Result<T, DesktopError>;
