//! Account profile, keys and settings.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::schema;

schema! {
    /// Public profile of the authenticated user.
    pub struct UserProfile {
        /// Id.
        pub id: Uuid,
        /// Email (lower-case).
        pub email: String,
        /// Whether the email has been verified.
        pub email_verified: bool,
        /// Display name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// Server administrator.
        pub is_admin: bool,
        /// Any MFA method enabled.
        pub mfa_enabled: bool,
    }
}

schema! {
    /// Key material the client needs after login.
    pub struct AccountKeys {
        /// X25519 public key (base64).
        pub public_key: String,
        /// Account private key wrapped with the account KEK.
        pub wrapped_private_key: String,
        /// Monotonic version, bumped on password change / recovery.
        pub key_version: i32,
    }
}

schema! {
    /// `PATCH /account/profile`
    pub struct UpdateProfileRequest {
        /// New display name (`null` clears it).
        #[serde(default)]
        pub display_name: Option<String>,
    }
}

schema! {
    /// `POST /account/email/change`
    pub struct ChangeEmailRequest {
        /// New email; a confirmation code is sent there.
        pub new_email: String,
    }
}

schema! {
    /// Generic "confirm with code" body.
    pub struct CodeRequest {
        /// The code from the email.
        pub code: String,
    }
}

schema! {
    /// A security-relevant event on the account (`GET /account/security-events`).
    ///
    /// Termoso reports *to* the user, never *on* the user: this log is visible
    /// only to the account owner.
    pub struct SecurityEvent {
        /// Id.
        pub id: Uuid,
        /// Event kind (`login`, `login_failed`, `new_device`, `device_revoked`,
        /// `password_changed`, `recovery_used`, `mfa_enabled`, `mfa_disabled`,
        /// `email_changed`, `vault_key_rotated`, …).
        pub kind: String,
        /// Device involved.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub device_id: Option<Uuid>,
        /// Client IP as seen by the server.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ip: Option<String>,
        /// User agent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user_agent: Option<String>,
        /// Extra details.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub details: Option<serde_json::Value>,
        /// When.
        pub created_at: DateTime<Utc>,
    }
}

schema! {
    /// `GET /account/security-events`
    pub struct SecurityEventList {
        /// Newest first.
        pub events: Vec<SecurityEvent>,
    }
}

schema! {
    /// Encrypted client settings blob synced across devices.
    pub struct SettingsBlob {
        /// Ciphertext envelope (encrypted with the personal vault key).
        pub data: String,
        /// Optimistic-concurrency version.
        pub version: i64,
        /// Last update.
        pub updated_at: DateTime<Utc>,
    }
}

schema! {
    /// `PUT /account/settings`
    pub struct PutSettingsRequest {
        /// New ciphertext.
        pub data: String,
        /// Version the client based this write on (`0` for first write).
        pub base_version: i64,
    }
}

schema! {
    /// Public, unauthenticated server information (`GET /server/info`).
    pub struct ServerInfo {
        /// Server display name.
        pub name: String,
        /// Server version.
        pub version: String,
        /// Whether new accounts can sign up.
        pub registration_open: bool,
        /// Configured SSO providers.
        pub sso_providers: Vec<crate::auth::SsoProvider>,
        /// Feature flags.
        pub features: ServerFeatures,
        /// Maximum entity payload size in bytes.
        pub max_entity_bytes: u32,
        /// Maximum session-log upload size in bytes.
        pub max_log_bytes: u64,
    }
}

schema! {
    /// Feature flags advertised by the server.
    #[derive(Default)]
    pub struct ServerFeatures {
        /// Session logs storage (S3) is configured.
        pub session_logs: bool,
        /// Email delivery is configured.
        pub email: bool,
        /// WebAuthn is configured (needs an RP id / origin).
        pub webauthn: bool,
        /// Teams can be created by regular users.
        pub teams: bool,
    }
}
