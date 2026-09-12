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
    }
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
    pub struct UpdateTeamRequest {
        /// New name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
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
