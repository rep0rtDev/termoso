//! Administration API (server operators).

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::schema;

schema! {
    /// `GET /admin/stats`
    pub struct AdminStats {
        /// Registered users.
        pub users: i64,
        /// Users active in the last 30 days.
        pub active_users_30d: i64,
        /// Teams.
        pub teams: i64,
        /// Vaults.
        pub vaults: i64,
        /// Live entities (non-deleted).
        pub entities: i64,
        /// Devices with a valid session.
        pub active_sessions: i64,
        /// Session logs stored (bytes).
        pub log_storage_bytes: i64,
    }
}

schema! {
    /// User as seen by an administrator (no secrets, no content).
    pub struct AdminUser {
        /// Id.
        pub id: Uuid,
        /// Email.
        pub email: String,
        /// Verified.
        pub email_verified: bool,
        /// Display name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        /// Admin.
        pub is_admin: bool,
        /// Disabled.
        pub disabled: bool,
        /// MFA on.
        pub mfa_enabled: bool,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// Last activity.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_seen_at: Option<DateTime<Utc>>,
        /// Device count.
        pub devices: i64,
    }
}

schema! {
    /// Paginated user list.
    pub struct AdminUserList {
        /// Users.
        pub users: Vec<AdminUser>,
        /// Total matching.
        pub total: i64,
    }
}

schema! {
    /// `PATCH /admin/users/{id}`
    pub struct AdminUpdateUserRequest {
        /// Disable / enable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub disabled: Option<bool>,
        /// Grant / revoke admin.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_admin: Option<bool>,
        /// Force email verified.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub email_verified: Option<bool>,
    }
}

schema! {
    /// Runtime-editable server settings (`GET`/`PUT /admin/settings`).
    pub struct ServerSettings {
        /// Allow new sign-ups.
        pub registration_open: bool,
        /// Only emails in `allowed_domains` may register (empty = any).
        #[serde(default)]
        pub allowed_domains: Vec<String>,
        /// Users verified by a configured SSO provider may sign up even while
        /// `registration_open` is off (domain filter still applies).
        #[serde(default)]
        pub sso_registration: bool,
        /// Require verified email before syncing.
        pub require_email_verification: bool,
        /// Require approval code when logging in from a new device.
        pub new_device_email_approval: bool,
        /// Regular users may create teams.
        pub users_can_create_teams: bool,
        /// Session lifetime in days.
        pub session_ttl_days: u32,
        /// Maximum entity payload size in bytes.
        pub max_entity_bytes: u32,
        /// Maximum session log size in bytes.
        pub max_log_bytes: u64,
        /// Per-user total log storage quota in bytes (0 = unlimited).
        pub log_quota_bytes: u64,
        /// Keep team activity-log entries this many days (0 = forever).
        #[serde(default = "default_audit_retention_days")]
        pub audit_retention_days: u32,
    }
}

fn default_audit_retention_days() -> u32 {
    365
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            registration_open: true,
            allowed_domains: Vec::new(),
            sso_registration: false,
            require_email_verification: false,
            new_device_email_approval: true,
            users_can_create_teams: true,
            session_ttl_days: 90,
            max_entity_bytes: 256 * 1024,
            max_log_bytes: 512 * 1024 * 1024,
            log_quota_bytes: 0,
            audit_retention_days: default_audit_retention_days(),
        }
    }
}

schema! {
    /// `POST /admin/email/test`
    pub struct TestEmailRequest {
        /// Recipient.
        pub to: String,
    }
}

schema! {
    /// Team as seen by an administrator.
    pub struct AdminTeam {
        /// Id.
        pub id: Uuid,
        /// Name.
        pub name: String,
        /// Owner email.
        pub owner_email: String,
        /// Members.
        pub member_count: i64,
        /// Vaults.
        pub vault_count: i64,
        /// Created.
        pub created_at: DateTime<Utc>,
    }
}

schema! {
    /// `GET /admin/teams`
    pub struct AdminTeamList {
        /// Teams.
        pub teams: Vec<AdminTeam>,
        /// Total.
        pub total: i64,
    }
}
