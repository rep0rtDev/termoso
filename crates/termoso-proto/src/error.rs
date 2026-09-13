//! Uniform error body returned by every failing endpoint.

use crate::schema;

schema! {
    /// Error response body.
    pub struct ApiError {
        /// Stable machine-readable code, e.g. `invalid_credentials`.
        pub code: String,
        /// Human-readable message (English, safe to show to the user).
        pub message: String,
        /// Optional structured details (validation errors, retry-after…).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub details: Option<serde_json::Value>,
    }
}

/// Well-known error codes.
pub mod codes {
    /// Request body failed validation.
    pub const VALIDATION: &str = "validation_failed";
    /// Missing or invalid session token.
    pub const UNAUTHORIZED: &str = "unauthorized";
    /// Authenticated but not allowed.
    pub const FORBIDDEN: &str = "forbidden";
    /// Resource not found.
    pub const NOT_FOUND: &str = "not_found";
    /// Wrong email/password (client-side OPAQUE failure is reported as this too).
    pub const INVALID_CREDENTIALS: &str = "invalid_credentials";
    /// Email already registered.
    pub const EMAIL_TAKEN: &str = "email_taken";
    /// Server has registration disabled.
    pub const REGISTRATION_CLOSED: &str = "registration_closed";
    /// MFA code invalid or expired.
    pub const INVALID_MFA: &str = "invalid_mfa";
    /// Token (login/mfa/approval/invite) expired or unknown.
    pub const TOKEN_EXPIRED: &str = "token_expired";
    /// Optimistic-concurrency conflict.
    pub const CONFLICT: &str = "conflict";
    /// Too many requests.
    pub const RATE_LIMITED: &str = "rate_limited";
    /// Payload exceeds configured limit.
    pub const TOO_LARGE: &str = "payload_too_large";
    /// Internal error.
    pub const INTERNAL: &str = "internal_error";
    /// Account disabled by an administrator.
    pub const ACCOUNT_DISABLED: &str = "account_disabled";
    /// Email must be verified first.
    pub const EMAIL_UNVERIFIED: &str = "email_unverified";
    /// The team requires two-factor authentication for vault access.
    pub const MFA_REQUIRED: &str = "mfa_required";
}
