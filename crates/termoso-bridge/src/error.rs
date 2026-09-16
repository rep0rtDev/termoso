use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Bridge error. Messages never contain credential material.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// Bad request from the caller of the REST API.
    #[error("{0}")]
    Invalid(String),
    /// Referenced vault / group / host does not exist (or is not sealed to this bridge).
    #[error("{0}")]
    NotFound(String),
    /// The bridge has no usable key for the vault (pending re-seal after rotation).
    #[error("{0}")]
    Pending(String),
    /// Caller did not present the local API key.
    #[error("missing or invalid API key")]
    Unauthorized,
    /// Local request budget exhausted.
    #[error("too many requests; slow down")]
    RateLimited,
    /// Server rejected a request.
    #[error("server responded {status} {code}: {message}")]
    Server {
        status: u16,
        code: String,
        message: String,
    },
    /// Sync push kept conflicting after retries.
    #[error("sync conflict on {0}; retry")]
    Conflict(String),
    /// Transport failure talking to the server.
    #[error("cannot reach server: {0}")]
    Transport(String),
    /// Credentials file / key material problem.
    #[error("{0}")]
    Credentials(String),
    /// Local encryption / decryption failure.
    #[error("crypto: {0}")]
    Crypto(#[from] termoso_crypto::CryptoError),
    /// JSON (de)serialisation failure.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BridgeError>;

impl From<reqwest::Error> for BridgeError {
    fn from(e: reqwest::Error) -> Self {
        // reqwest's Display may include the URL but never request bodies.
        BridgeError::Transport(e.without_url().to_string())
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: String,
}

impl BridgeError {
    pub fn status(&self) -> StatusCode {
        match self {
            BridgeError::Invalid(_) | BridgeError::Json(_) => StatusCode::BAD_REQUEST,
            BridgeError::NotFound(_) => StatusCode::NOT_FOUND,
            BridgeError::Unauthorized => StatusCode::UNAUTHORIZED,
            BridgeError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            BridgeError::Pending(_) => StatusCode::CONFLICT,
            BridgeError::Conflict(_) => StatusCode::CONFLICT,
            BridgeError::Server { status, .. } => match *status {
                429 => StatusCode::TOO_MANY_REQUESTS,
                422 => StatusCode::UNPROCESSABLE_ENTITY,
                _ => StatusCode::BAD_GATEWAY,
            },
            BridgeError::Transport(_) => StatusCode::BAD_GATEWAY,
            BridgeError::Credentials(_) | BridgeError::Crypto(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            BridgeError::Invalid(_) | BridgeError::Json(_) => "invalid_request",
            BridgeError::NotFound(_) => "not_found",
            BridgeError::Unauthorized => "unauthorized",
            BridgeError::RateLimited => "rate_limited",
            BridgeError::Pending(_) => "vault_key_pending",
            BridgeError::Conflict(_) => "conflict",
            BridgeError::Server { .. } => "server_error",
            BridgeError::Transport(_) => "server_unreachable",
            BridgeError::Credentials(_) => "credentials",
            BridgeError::Crypto(_) => "crypto",
        }
    }
}

impl IntoResponse for BridgeError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            tracing::error!(error = %self, "request failed");
        } else if matches!(self, BridgeError::RateLimited) {
            tracing::debug!("request rate-limited");
        } else {
            tracing::warn!(error = %self, "request rejected");
        }
        let body = ErrorBody {
            code: self.code(),
            message: self.to_string(),
        };
        let mut resp = (status, axum::Json(body)).into_response();
        if status == StatusCode::TOO_MANY_REQUESTS {
            resp.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("1"),
            );
        }
        resp
    }
}
