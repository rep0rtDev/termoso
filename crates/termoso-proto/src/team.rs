//! Teams, members and invitations.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::schema;

schema! {
    /// Role inside a team.
    #[derive(Copy, PartialEq, Eq, PartialOrd, Ord)]
    #[serde(rename_all = "snake_case")]
    pub enum TeamRole {
        /// Regular member; vault access is granted per vault.
        Member,
        /// Can manage members, invites and vaults.
        Admin,
        /// Exactly one per team; can delete the team and transfer ownership.
        Owner,
    }
}

impl TeamRole {
    /// Whether this role may administer the team.
    pub fn is_admin(self) -> bool {
        matches!(self, TeamRole::Admin | TeamRole::Owner)
    }
}

schema! {
    /// A team.
    pub struct Team {
        /// Id.
        pub id: Uuid,
        /// Name.
        pub name: String,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// Requesting user's role.
        pub my_role: TeamRole,
        /// Number of members.
        pub member_count: i64,
        /// Members may share live terminal sessions with each other.
        #[serde(default = "default_true")]
        pub multiplayer_enabled: bool,
        /// Members without two-factor authentication cannot open team vaults.
        #[serde(default)]
        pub require_mfa: bool,
        /// Members see who is connected to which team-vault host right now.
        #[serde(default)]
        pub presence_enabled: bool,
    }
}

fn default_true() -> bool {
    true
}

schema! {
    /// `GET /teams`
    pub struct TeamList {
        /// Teams.
        pub teams: Vec<Team>,
    }
}

schema! {
    /// `POST /teams`
    pub struct CreateTeamRequest {
        /// Name.
        pub name: String,
    }
}

schema! {
    /// `PATCH /teams/{id}`
    #[derive(Default)]
    pub struct UpdateTeamRequest {
        /// New name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        /// Allow live terminal sharing between members.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub multiplayer_enabled: Option<bool>,
        /// Require two-factor authentication to open team vaults.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub require_mfa: Option<bool>,
        /// Show members who is connected to which team-vault host.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub presence_enabled: Option<bool>,
    }
}

schema! {
    /// Team member with public key (needed to seal vault keys to them).
    pub struct TeamMember {
        /// User id.
        pub user_id: Uuid,
        /// Email.
        pub email: String,
        /// Display name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        /// Profile picture tag (see `UserProfile::avatar`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub avatar: Option<String>,
        /// Role.
        pub role: TeamRole,
        /// X25519 public key (base64).
        pub public_key: String,
        /// Joined.
        pub joined_at: DateTime<Utc>,
    }
}

schema! {
    /// `GET /teams/{id}/members`
    pub struct TeamMemberList {
        /// Members.
        pub members: Vec<TeamMember>,
    }
}

schema! {
    /// One live connection of a team member to a host in a team vault, as
    /// reported by their device. Only routing metadata: the server never
    /// learns the host's name or address.
    #[derive(PartialEq, Eq, Hash)]
    pub struct PresenceSession {
        /// Team vault the host lives in.
        pub vault_id: Uuid,
        /// Host entity id.
        pub host_id: Uuid,
        /// `ssh` | `sftp` | `mosh` | `telnet` | `forwarding`.
        pub protocol: String,
        /// When the connection was established.
        pub since: DateTime<Utc>,
    }
}

schema! {
    /// Everything one device of one member is connected to right now.
    pub struct PresenceEntry {
        /// Member.
        pub user_id: Uuid,
        /// Email.
        pub email: String,
        /// Display name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        /// Profile picture tag (see `UserProfile::avatar`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub avatar: Option<String>,
        /// Device.
        pub device_id: Uuid,
        /// Device name as registered at login.
        pub device_name: String,
        /// `linux` | `windows` | `macos` | `android` | `ios` | `web`.
        pub platform: String,
        /// Live connections (only those in vaults the requester can see).
        pub sessions: Vec<PresenceSession>,
        /// Last heartbeat from the device.
        pub seen_at: DateTime<Utc>,
    }
}

schema! {
    /// `GET /teams/{id}/presence`
    pub struct TeamPresence {
        /// Whether the team has presence turned on; `entries` is empty otherwise.
        pub enabled: bool,
        /// Devices with at least one live connection.
        pub entries: Vec<PresenceEntry>,
    }
}

schema! {
    /// `PATCH /teams/{id}/members/{user_id}`
    pub struct UpdateTeamMemberRequest {
        /// New role (`owner` transfers ownership; only the owner may do that).
        pub role: TeamRole,
    }
}

schema! {
    /// `POST /teams/{id}/invites`
    pub struct CreateInviteRequest {
        /// Invitee email.
        pub email: String,
        /// Role on acceptance.
        pub role: TeamRole,
        /// Vaults to grant on acceptance (keys are sealed later by an admin
        /// once the invitee's public key is known).
        #[serde(default)]
        pub vault_ids: Vec<Uuid>,
    }
}

schema! {
    /// Pending invitation.
    pub struct Invite {
        /// Id.
        pub id: Uuid,
        /// Email.
        pub email: String,
        /// Role.
        pub role: TeamRole,
        /// Invited by.
        pub invited_by: Uuid,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// Expires.
        pub expires_at: DateTime<Utc>,
    }
}

schema! {
    /// `POST /teams/{id}/invites` response: the invite plus its share link.
    pub struct CreatedInvite {
        /// The pending invitation.
        #[serde(flatten)]
        pub invite: Invite,
        /// Share this link with the invitee (also emailed when SMTP is configured).
        pub url: String,
    }
}

schema! {
    /// `GET /teams/{id}/invites`
    pub struct InviteList {
        /// Invites.
        pub invites: Vec<Invite>,
    }
}

schema! {
    /// `GET /invites/{token}` – public preview.
    pub struct InvitePreview {
        /// Team name.
        pub team_name: String,
        /// Inviter display name or email.
        pub inviter: String,
        /// Invitee email.
        pub email: String,
        /// Role.
        pub role: TeamRole,
        /// Whether an account with that email already exists.
        pub account_exists: bool,
    }
}

schema! {
    /// Members whose vault keys still need to be sealed (`GET /teams/{id}/pending-keys`).
    pub struct PendingVaultKeys {
        /// Items.
        pub items: Vec<PendingVaultKey>,
    }
}

schema! {
    /// A (vault, user) pair lacking a sealed key.
    pub struct PendingVaultKey {
        /// Vault.
        pub vault_id: Uuid,
        /// User.
        pub user_id: Uuid,
        /// Their public key.
        pub public_key: String,
        /// Role that was granted.
        pub role: crate::vault::VaultRole,
    }
}

schema! {
    /// One entry of the team activity log (`GET /teams/{id}/audit`). The
    /// server never sees vault payloads, so entries name kinds and ids — the
    /// client resolves them to labels where it holds the vault key.
    pub struct AuditEvent {
        /// Monotonic id; also the paging cursor.
        pub id: i64,
        /// Team.
        pub team_id: Uuid,
        /// Who did it (`None` once the account is gone).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub actor_id: Option<Uuid>,
        /// Actor email at query time.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub actor_email: Option<String>,
        /// Actor display name at query time.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub actor_name: Option<String>,
        /// Actor profile picture tag at query time (see `UserProfile::avatar`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub actor_avatar: Option<String>,
        /// Device the request came from.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub device_id: Option<Uuid>,
        /// What happened: `team.created`, `team.renamed`, `team.settings`,
        /// `member.role`, `member.removed`, `member.left`, `invite.created`,
        /// `invite.revoked`, `invite.accepted`, `vault.created`, `vault.renamed`,
        /// `vault.deleted`, `vault.key_rotated`, `vault.access_granted`,
        /// `vault.access_changed`, `vault.access_revoked`, `entity.created`,
        /// `entity.updated`, `entity.deleted`, `multiplayer.started`,
        /// `multiplayer.joined`, `multiplayer.stopped`.
        pub action: String,
        /// Vault involved, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub vault_id: Option<Uuid>,
        /// Other user involved (member, invitee), if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub target_user: Option<Uuid>,
        /// Target user's email at query time.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub target_email: Option<String>,
        /// Action-specific metadata (entity kind/id, role, name, counts).
        #[serde(default)]
        pub details: serde_json::Value,
        /// When.
        pub created_at: DateTime<Utc>,
    }
}

schema! {
    /// A page of the team activity log, newest first.
    pub struct AuditEventList {
        /// Events.
        pub events: Vec<AuditEvent>,
        /// Pass as `before` to fetch the next (older) page.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub next_before: Option<i64>,
    }
}
