//! Vaults: encrypted containers for entities, personal or shared with a team.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::schema;

schema! {
    /// Vault kind.
    #[derive(Copy, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum VaultKind {
        /// Owned by a single user; cannot be shared.
        Personal,
        /// Owned by a team.
        Team,
    }
}

schema! {
    /// Role of a user inside a vault.
    #[derive(Copy, PartialEq, Eq, PartialOrd, Ord)]
    #[serde(rename_all = "snake_case")]
    pub enum VaultRole {
        /// Read-only access to entities.
        Viewer,
        /// Can create / edit / delete entities.
        Editor,
        /// Editor + manage members and settings.
        Manager,
    }
}

impl VaultRole {
    /// Whether this role may write entities.
    pub fn can_write(self) -> bool {
        matches!(self, VaultRole::Editor | VaultRole::Manager)
    }

    /// Whether this role may manage members.
    pub fn can_manage(self) -> bool {
        matches!(self, VaultRole::Manager)
    }
}

schema! {
    /// A vault visible to the requesting user.
    pub struct Vault {
        /// Id.
        pub id: Uuid,
        /// Kind.
        pub kind: VaultKind,
        /// Owning team (team vaults only).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub team_id: Option<Uuid>,
        /// Name (plaintext – visible to team admins).
        pub name: String,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// The requesting user's role.
        pub my_role: VaultRole,
        /// Vault key sealed to the requesting user's public key. `None` while a
        /// team admin has not yet sealed the key for this user (see
        /// `GET /teams/{id}/pending-keys`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sealed_key: Option<String>,
        /// Current key version (bumped on rotation).
        pub key_version: i32,
    }
}

schema! {
    /// `GET /vaults`
    pub struct VaultList {
        /// Vaults.
        pub vaults: Vec<Vault>,
    }
}

schema! {
    /// A vault key sealed for a specific user.
    pub struct SealedKeyFor {
        /// Recipient.
        pub user_id: Uuid,
        /// Sealed box (base64).
        pub sealed_key: String,
    }
}

schema! {
    /// `POST /teams/{team_id}/vaults`
    pub struct CreateVaultRequest {
        /// Name.
        pub name: String,
        /// Initial members with sealed keys. Must include the creator.
        pub members: Vec<VaultMemberUpsert>,
    }
}

schema! {
    /// `PATCH /vaults/{id}`
    pub struct UpdateVaultRequest {
        /// New name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
    }
}

schema! {
    /// Member of a vault.
    pub struct VaultMember {
        /// User id.
        pub user_id: Uuid,
        /// Email.
        pub email: String,
        /// Display name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        /// Role.
        pub role: VaultRole,
        /// Key version the member's sealed key corresponds to.
        pub key_version: i32,
        /// The member has no sealed key yet (needs a manager to seal it).
        pub pending: bool,
        /// Their public key (so a manager can seal the vault key).
        pub public_key: String,
        /// Added.
        pub added_at: DateTime<Utc>,
    }
}

schema! {
    /// `GET /vaults/{id}/members`
    pub struct VaultMemberList {
        /// Members.
        pub members: Vec<VaultMember>,
    }
}

schema! {
    /// `PUT /vaults/{id}/members/{user_id}` – add or update a member.
    pub struct VaultMemberUpsert {
        /// User (must be a team member).
        pub user_id: Uuid,
        /// Role.
        pub role: VaultRole,
        /// Vault key sealed to this user's public key.
        pub sealed_key: String,
    }
}

schema! {
    /// `POST /vaults/{id}/rotate` – install a new vault key after removing a member.
    /// Clients must then re-encrypt and push every entity with the new `key_version`.
    pub struct RotateVaultKeyRequest {
        /// Expected current version (optimistic concurrency).
        pub base_key_version: i32,
        /// New key sealed for every remaining member.
        pub members: Vec<SealedKeyFor>,
    }
}

schema! {
    /// Response to rotation.
    pub struct RotateVaultKeyResponse {
        /// New key version.
        pub key_version: i32,
    }
}
