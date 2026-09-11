//! Unified error type → JSON `ApiError` body.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use termoso_proto::error::{codes, ApiError};

pub type ApiResult<T> = Result<T, Error>;

/// Handler return type for successful requests without a body (`204`).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoContent;

impl IntoResponse for NoContent {
    fn into_response(self) -> Response {
        StatusCode::NO_CONTENT.into_response()
    }
}

/// `result.map(NoContent::from)` for handlers whose body returns `Result<(), _>`.
impl From<()> for NoContent {
    fn from((): ()) -> Self {
        NoContent
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{1}")]
    Status(StatusCode, &'static str, String, Option<serde_json::Value>),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl Error {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Error::Status(status, code, message.into(), None)
    }
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        if let Error::Status(_, _, _, d) = &mut self {
            *d = Some(details);
        }
        self
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, codes::VALIDATION, msg)
    }
    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            codes::UNAUTHORIZED,
            "Authentication required",
        )
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, codes::FORBIDDEN, msg)
    }
    pub fn not_found(what: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            codes::NOT_FOUND,
            format!("{what} not found"),
        )
    }
    pub fn invalid_credentials() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            codes::INVALID_CREDENTIALS,
            "Invalid email or password",
        )
    }
    pub fn invalid_mfa() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            codes::INVALID_MFA,
            "Invalid second factor",
        )
    }
    pub fn invalid_code() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            codes::INVALID_MFA,
            "Invalid or expired code",
        )
    }
    pub fn token_expired() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            codes::TOKEN_EXPIRED,
            "Token expired or invalid",
        )
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, codes::CONFLICT, msg)
    }
    pub fn too_large(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, codes::TOO_LARGE, msg)
    }
    pub fn rate_limited(retry_after_secs: u64) -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            codes::RATE_LIMITED,
            "Too many requests",
        )
        .with_details(serde_json::json!({ "retry_after": retry_after_secs }))
    }
    pub fn email_taken() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            codes::EMAIL_TAKEN,
            "Email already registered",
        )
    }
    pub fn registration_closed() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            codes::REGISTRATION_CLOSED,
            "Registration is closed on this server",
        )
    }
    pub fn account_disabled() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            codes::ACCOUNT_DISABLED,
            "Account disabled",
        )
    }
    pub fn email_unverified() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            codes::EMAIL_UNVERIFIED,
            "Email not verified",
        )
    }
    pub fn quota_exceeded(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::INSUFFICIENT_STORAGE, "quota_exceeded", msg)
    }
    pub fn feature_disabled(what: &str) -> Self {
        Self::new(
            StatusCode::NOT_IMPLEMENTED,
            "feature_disabled",
            format!("{what} is not configured on this server"),
        )
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Error::Status(status, code, message, details) => (
                status,
                ApiError {
                    code: code.to_string(),
                    message,
                    details,
                },
            ),
            Error::Internal(err) => {
                tracing::error!(error = ?err, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ApiError {
                        code: codes::INTERNAL.to_string(),
                        message: "Internal server error".into(),
                        details: None,
                    },
                )
            }
        };
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        Error::Internal(anyhow::Error::new(e).context("database"))
    }
}
impl From<redis::RedisError> for Error {
    fn from(e: redis::RedisError) -> Self {
        Error::Internal(anyhow::Error::new(e).context("redis"))
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Internal(anyhow::Error::new(e).context("json"))
    }
}
impl From<termoso_crypto::CryptoError> for Error {
    fn from(e: termoso_crypto::CryptoError) -> Self {
        Error::bad_request(format!("cryptographic input rejected: {e}"))
    }
}
