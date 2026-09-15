//! Teams, invitations and team vaults for the mobile façade. Same rules as
//! the desktop `team` module: vault keys are generated, opened and sealed here
//! with the account keypair from the store; Kotlin only sees teams, members,
//! roles, invitation links and pending states.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use termoso_core::account as core;
use termoso_core::api::{ApiClient, AuditQuery};
use termoso_core::store::Store;
use termoso_crypto::keys::{SymmetricKey, public_key_from_b64};
use termoso_crypto::sealed;
use termoso_proto::team::{
    self as proto, CreateInviteRequest, CreateTeamRequest, UpdateTeamMemberRequest,
    UpdateTeamRequest,
};
use termoso_proto::vault::{
    CreateVaultRequest, RotateVaultKeyRequest, SealedKeyFor, UpdateVaultRequest, VaultMemberUpsert,
    VaultRole as ProtoVaultRole,
};
use uuid::Uuid;

use crate::account::{AccountRuntime, SyncChange};
use crate::dto::{VaultAccess, parse_id};
use crate::error::{MobileError, Result};
use crate::presence::{self, TeamPresenceCard};

type CoreError = termoso_core::error::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TeamRole {
    Member,
    Admin,
    Owner,
}

impl From<proto::TeamRole> for TeamRole {
    fn from(r: proto::TeamRole) -> Self {
        match r {
            proto::TeamRole::Member => Self::Member,
            proto::TeamRole::Admin => Self::Admin,
            proto::TeamRole::Owner => Self::Owner,
        }
    }
}

impl From<TeamRole> for proto::TeamRole {
    fn from(r: TeamRole) -> Self {
        match r {
            TeamRole::Member => Self::Member,
            TeamRole::Admin => Self::Admin,
            TeamRole::Owner => Self::Owner,
        }
    }
}

fn vault_access(r: ProtoVaultRole) -> VaultAccess {
    match r {
        ProtoVaultRole::Viewer => VaultAccess::View,
        ProtoVaultRole::Editor => VaultAccess::Edit,
        ProtoVaultRole::Manager => VaultAccess::Manage,
    }
}

fn vault_role(a: VaultAccess) -> ProtoVaultRole {
    match a {
        VaultAccess::View => ProtoVaultRole::Viewer,
        VaultAccess::Edit => ProtoVaultRole::Editor,
        VaultAccess::Manage => ProtoVaultRole::Manager,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TeamCard {
    pub id: String,
    pub name: String,
    pub my_role: TeamRole,
    pub member_count: u32,
    pub multiplayer_enabled: bool,
    pub require_mfa: bool,
    /// Members can see which team-vault hosts teammates are connected to.
    pub presence_enabled: bool,
    /// RFC 3339.
    pub created_at: String,
}

impl From<proto::Team> for TeamCard {
    fn from(t: proto::Team) -> Self {
        Self {
            id: t.id.to_string(),
            name: t.name,
            my_role: t.my_role.into(),
            member_count: u32::try_from(t.member_count).unwrap_or(u32::MAX),
            multiplayer_enabled: t.multiplayer_enabled,
            require_mfa: t.require_mfa,
            presence_enabled: t.presence_enabled,
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

/// Team member as shown in the UI (the public key stays in Rust).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TeamMemberCard {
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub role: TeamRole,
    /// RFC 3339.
    pub joined_at: String,
    /// This is the signed-in user.
    pub me: bool,
    /// Second factor enrolled (TOTP or security key). Only team admins and
    /// the member themself get to know; `None` otherwise.
    pub mfa_enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InviteCard {
    pub id: String,
    pub email: String,
    pub role: TeamRole,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub expires_at: String,
}

impl From<proto::Invite> for InviteCard {
    fn from(i: proto::Invite) -> Self {
        Self {
            id: i.id.to_string(),
            email: i.email,
            role: i.role.into(),
            created_at: i.created_at.to_rfc3339(),
            expires_at: i.expires_at.to_rfc3339(),
        }
    }
}

/// Outcome of one invitation in a batch.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InviteSent {
    pub email: String,
    /// Share link (also emailed when the server has SMTP).
    pub url: Option<String>,
    pub error: Option<String>,
}

/// Member of a team vault (or still waiting for a manager to seal the key).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VaultMemberCard {
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub access: VaultAccess,
    pub pending: bool,
    pub me: bool,
}

/// Team-vault member waiting for their sealed copy of the vault key.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PendingKeyCard {
    pub vault_id: String,
    pub vault_name: String,
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub access: VaultAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VaultAccessDraft {
    pub user_id: String,
    pub access: VaultAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AuditEventCard {
    pub id: i64,
    pub action: String,
    pub actor: Option<String>,
    pub target: Option<String>,
    pub vault_id: Option<String>,
    /// Flattened `key: value` pairs of the event details.
    pub details: Vec<String>,
    /// RFC 3339.
    pub created_at: String,
}

impl From<proto::AuditEvent> for AuditEventCard {
    fn from(e: proto::AuditEvent) -> Self {
        let details = match e.details {
            serde_json::Value::Object(map) => map
                .into_iter()
                .map(|(k, v)| match v {
                    serde_json::Value::String(s) => format!("{k}: {s}"),
                    other => format!("{k}: {other}"),
                })
                .collect(),
            serde_json::Value::Null => Vec::new(),
            other => vec![other.to_string()],
        };
        Self {
            id: e.id,
            action: e.action,
            actor: e.actor_name.or(e.actor_email),
            target: e.target_email,
            vault_id: e.vault_id.map(|v| v.to_string()),
            details,
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AuditPage {
    pub events: Vec<AuditEventCard>,
    pub next_before: Option<i64>,
}

fn clean_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 80 {
        return Err(MobileError::invalid("name must be 1–80 characters"));
    }
    Ok(n.to_string())
}

/// Token from an invitation link (`…/invite/<token>`) or a bare token.
pub fn invite_token(link: &str) -> &str {
    let path = link.trim().split(['?', '#']).next().unwrap_or_default();
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
}

fn seal_for(public_key_b64: &str, key: &SymmetricKey) -> Result<String> {
    let pk = public_key_from_b64(public_key_b64).map_err(CoreError::from)?;
    Ok(sealed::seal_vault_key(&pk, key).map_err(CoreError::from)?)
}

fn me(store: &Store) -> Result<Uuid> {
    Ok(store
        .account()?
        .ok_or_else(|| MobileError::invalid("not signed in"))?
        .user_id)
}

async fn members_raw(api: &ApiClient, team_id: Uuid) -> Result<Vec<proto::TeamMember>> {
    Ok(api.team_members(team_id).await?.members)
}

/// Public keys of team members, by user id.
async fn public_keys(api: &ApiClient, team_id: Uuid) -> Result<HashMap<Uuid, String>> {
    Ok(members_raw(api, team_id)
        .await?
        .into_iter()
        .map(|m| (m.user_id, m.public_key))
        .collect())
}

/// New vault key sealed to every member who already holds one; pending
/// members keep waiting for a manager to grant the new key. Local rows are
/// re-encrypted by the vault refresh that follows.
async fn rotate_with(api: &ApiClient, store: &Store, vault_id: Uuid) -> Result<()> {
    let vault = store.vault(vault_id)?;
    if !vault.unlocked {
        return Err(MobileError::invalid("you need the vault key to rotate it"));
    }
    let members = api.vault_members(vault_id).await?.members;
    let key = SymmetricKey::generate();
    let sealed_for = members
        .iter()
        .filter(|m| !m.pending)
        .map(|m| {
            Ok(SealedKeyFor {
                user_id: m.user_id,
                sealed_key: seal_for(&m.public_key, &key)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    api.rotate_vault_key(
        vault_id,
        &RotateVaultKeyRequest {
            base_key_version: vault.key_version,
            members: sealed_for,
        },
    )
    .await?;
    Ok(())
}

impl AccountRuntime {
    /// Pull the vault list again (new vault, key granted, rotation, removal),
    /// tell Kotlin and push any re-encrypted rows in the background.
    async fn refresh_vaults(self: &Arc<Self>) -> Result<()> {
        let api = self.api().await?;
        core::refresh_vaults(&api, self.store()).await?;
        self.notify_changed(SyncChange::Vaults);
        self.sync_in_background();
        Ok(())
    }

    // ---- teams ----

    pub async fn teams(self: &Arc<Self>) -> Result<Vec<TeamCard>> {
        Ok(self
            .api()
            .await?
            .teams()
            .await?
            .teams
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Create a team and its first vault (named after the team) with us as
    /// manager.
    pub async fn create_team(self: &Arc<Self>, name: String) -> Result<TeamCard> {
        let name = clean_name(&name)?;
        let api = self.api().await?;
        let team = api
            .create_team(&CreateTeamRequest { name: name.clone() })
            .await?;
        let store = self.store();
        let secrets = store.account_secrets()?;
        let key = SymmetricKey::generate();
        let req = CreateVaultRequest {
            name,
            members: vec![VaultMemberUpsert {
                user_id: me(store)?,
                role: ProtoVaultRole::Manager,
                sealed_key: sealed::seal_vault_key(secrets.private_key.public(), &key)
                    .map_err(CoreError::from)?,
            }],
        };
        if let Err(e) = api.create_team_vault(team.id, &req).await {
            tracing::warn!(team = %team.id, "team created without a vault: {e}");
        }
        self.refresh_vaults().await?;
        Ok(team.into())
    }

    pub async fn rename_team(self: &Arc<Self>, team_id: String, name: String) -> Result<TeamCard> {
        let name = clean_name(&name)?;
        let team = self
            .api()
            .await?
            .update_team(
                parse_id(&team_id)?,
                &UpdateTeamRequest {
                    name: Some(name),
                    ..Default::default()
                },
            )
            .await?;
        self.refresh_vaults().await?;
        Ok(team.into())
    }

    /// Multiplayer / require-2FA / presence switches (admins only).
    pub async fn set_team_security(
        self: &Arc<Self>,
        team_id: String,
        multiplayer_enabled: Option<bool>,
        require_mfa: Option<bool>,
        presence_enabled: Option<bool>,
    ) -> Result<TeamCard> {
        Ok(self
            .api()
            .await?
            .update_team(
                parse_id(&team_id)?,
                &UpdateTeamRequest {
                    name: None,
                    multiplayer_enabled,
                    require_mfa,
                    presence_enabled,
                },
            )
            .await?
            .into())
    }

    /// Who is connected to the team's hosts right now (empty entries while
    /// the team has presence switched off).
    pub async fn team_presence(self: &Arc<Self>, team_id: String) -> Result<TeamPresenceCard> {
        let me = self.store().account()?.map(|a| a.user_id);
        let presence = self.api().await?.team_presence(parse_id(&team_id)?).await?;
        Ok(presence::card(presence, me))
    }

    /// Normalized WebP of a user's profile picture, `None` when they have none.
    pub async fn user_avatar(self: &Arc<Self>, user_id: String) -> Result<Option<Vec<u8>>> {
        Ok(self.api().await?.user_avatar(parse_id(&user_id)?).await?)
    }

    /// Whether this account hides itself from teammates' presence views.
    pub async fn presence_hidden(self: &Arc<Self>) -> Result<bool> {
        Ok(self.api().await?.account().await?.user.presence_hidden)
    }

    /// Hide (or show again) this account in teammates' presence views.
    pub async fn set_presence_hidden(self: &Arc<Self>, hidden: bool) -> Result<bool> {
        Ok(self
            .api()
            .await?
            .set_presence_hidden(hidden)
            .await?
            .presence_hidden)
    }

    pub async fn delete_team(self: &Arc<Self>, team_id: String) -> Result<()> {
        self.api().await?.delete_team(parse_id(&team_id)?).await?;
        self.refresh_vaults().await
    }

    pub async fn leave_team(self: &Arc<Self>, team_id: String) -> Result<()> {
        self.api().await?.leave_team(parse_id(&team_id)?).await?;
        self.refresh_vaults().await
    }

    /// Join the team behind an invitation link (or bare token).
    pub async fn accept_invite(self: &Arc<Self>, link: String) -> Result<TeamCard> {
        let team = self.api().await?.accept_invite(invite_token(&link)).await?;
        self.refresh_vaults().await?;
        Ok(team.into())
    }

    // ---- members ----

    pub async fn team_members(self: &Arc<Self>, team_id: String) -> Result<Vec<TeamMemberCard>> {
        let api = self.api().await?;
        let me = me(self.store())?;
        Ok(members_raw(&api, parse_id(&team_id)?)
            .await?
            .into_iter()
            .map(|m| TeamMemberCard {
                me: m.user_id == me,
                user_id: m.user_id.to_string(),
                email: m.email,
                display_name: m.display_name,
                avatar: m.avatar,
                role: m.role.into(),
                joined_at: m.joined_at.to_rfc3339(),
                mfa_enabled: m.mfa_enabled,
            })
            .collect())
    }

    pub async fn set_team_member_role(
        self: &Arc<Self>,
        team_id: String,
        user_id: String,
        role: TeamRole,
    ) -> Result<()> {
        self.api()
            .await?
            .update_team_member(
                parse_id(&team_id)?,
                parse_id(&user_id)?,
                &UpdateTeamMemberRequest { role: role.into() },
            )
            .await?;
        Ok(())
    }

    /// Remove a teammate, then rotate the key of every team vault we manage
    /// and hold a key for, so their stale copy stops working.
    pub async fn remove_team_member(
        self: &Arc<Self>,
        team_id: String,
        user_id: String,
    ) -> Result<()> {
        let team_id = parse_id(&team_id)?;
        let api = self.api().await?;
        api.remove_team_member(team_id, parse_id(&user_id)?).await?;
        let store = self.store();
        for v in store
            .vaults()?
            .iter()
            .filter(|v| v.team_id == Some(team_id) && v.unlocked && v.role.can_manage())
        {
            if let Err(e) = rotate_with(&api, store, v.id).await {
                tracing::warn!(vault = %v.id, "rotate after member removal: {e}");
            }
        }
        self.refresh_vaults().await
    }

    // ---- invites ----

    pub async fn team_invites(self: &Arc<Self>, team_id: String) -> Result<Vec<InviteCard>> {
        Ok(self
            .api()
            .await?
            .team_invites(parse_id(&team_id)?)
            .await?
            .invites
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn invite(
        self: &Arc<Self>,
        team_id: String,
        emails: Vec<String>,
        role: TeamRole,
        vault_ids: Vec<String>,
    ) -> Result<Vec<InviteSent>> {
        if role == TeamRole::Owner {
            return Err(MobileError::invalid("cannot invite as owner"));
        }
        let team_id = parse_id(&team_id)?;
        let vault_ids = vault_ids
            .iter()
            .map(|v| parse_id(v))
            .collect::<Result<Vec<_>>>()?;
        let api = self.api().await?;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for raw in emails {
            let email = raw.trim().to_ascii_lowercase();
            if email.is_empty() || !seen.insert(email.clone()) {
                continue;
            }
            let req = CreateInviteRequest {
                email: email.clone(),
                role: role.into(),
                vault_ids: vault_ids.clone(),
            };
            out.push(match api.create_invite(team_id, &req).await {
                Ok(c) => InviteSent {
                    email,
                    url: Some(c.url),
                    error: None,
                },
                Err(e) => InviteSent {
                    email,
                    url: None,
                    error: Some(e.to_string()),
                },
            });
        }
        if out.is_empty() {
            return Err(MobileError::invalid("enter at least one email address"));
        }
        Ok(out)
    }

    pub async fn revoke_invite(self: &Arc<Self>, team_id: String, invite_id: String) -> Result<()> {
        self.api()
            .await?
            .delete_invite(parse_id(&team_id)?, parse_id(&invite_id)?)
            .await?;
        Ok(())
    }

    pub async fn team_audit(
        self: &Arc<Self>,
        team_id: String,
        before: Option<i64>,
        limit: Option<u32>,
    ) -> Result<AuditPage> {
        let page = self
            .api()
            .await?
            .team_audit(
                parse_id(&team_id)?,
                &AuditQuery {
                    before,
                    limit,
                    ..AuditQuery::default()
                },
            )
            .await?;
        Ok(AuditPage {
            events: page.events.into_iter().map(Into::into).collect(),
            next_before: page.next_before,
        })
    }

    // ---- team vaults ----

    /// Members of every team vault we manage who still wait for a key,
    /// resolved to names via the team member list.
    pub async fn pending_keys(self: &Arc<Self>, team_id: String) -> Result<Vec<PendingKeyCard>> {
        let team_id = parse_id(&team_id)?;
        let api = self.api().await?;
        let store = self.store();
        let names: HashMap<Uuid, (String, Option<String>)> = members_raw(&api, team_id)
            .await?
            .into_iter()
            .map(|m| (m.user_id, (m.email, m.display_name)))
            .collect();
        let vaults: HashMap<Uuid, String> = store
            .vaults()?
            .into_iter()
            .filter(|v| v.team_id == Some(team_id))
            .map(|v| (v.id, v.name))
            .collect();
        Ok(api
            .team_pending_keys(team_id)
            .await?
            .items
            .into_iter()
            .map(|p| {
                let (email, display_name) = names.get(&p.user_id).cloned().unwrap_or_default();
                PendingKeyCard {
                    vault_name: vaults.get(&p.vault_id).cloned().unwrap_or_default(),
                    vault_id: p.vault_id.to_string(),
                    user_id: p.user_id.to_string(),
                    email,
                    display_name,
                    access: vault_access(p.role),
                }
            })
            .collect())
    }

    /// Create a team vault; we are always included as a manager.
    pub async fn create_team_vault(
        self: &Arc<Self>,
        team_id: String,
        name: String,
        access: Vec<VaultAccessDraft>,
    ) -> Result<()> {
        let team_id = parse_id(&team_id)?;
        let name = clean_name(&name)?;
        let api = self.api().await?;
        let store = self.store();
        let secrets = store.account_secrets()?;
        let me = me(store)?;
        let keys = public_keys(&api, team_id).await?;
        let key = SymmetricKey::generate();
        let mut members = vec![VaultMemberUpsert {
            user_id: me,
            role: ProtoVaultRole::Manager,
            sealed_key: sealed::seal_vault_key(secrets.private_key.public(), &key)
                .map_err(CoreError::from)?,
        }];
        for a in access {
            let user_id = parse_id(&a.user_id)?;
            if user_id == me {
                continue;
            }
            let pk = keys
                .get(&user_id)
                .ok_or_else(|| MobileError::not_found("user is not a member of this team"))?;
            members.push(VaultMemberUpsert {
                user_id,
                role: vault_role(a.access),
                sealed_key: seal_for(pk, &key)?,
            });
        }
        api.create_team_vault(team_id, &CreateVaultRequest { name, members })
            .await?;
        self.refresh_vaults().await
    }

    pub async fn rename_team_vault(self: &Arc<Self>, vault_id: String, name: String) -> Result<()> {
        let name = clean_name(&name)?;
        self.api()
            .await?
            .update_vault(
                parse_id(&vault_id)?,
                &UpdateVaultRequest {
                    name: Some(name),
                    session_logging: None,
                },
            )
            .await?;
        self.refresh_vaults().await
    }

    pub async fn delete_team_vault(self: &Arc<Self>, vault_id: String) -> Result<()> {
        self.api().await?.delete_vault(parse_id(&vault_id)?).await?;
        self.refresh_vaults().await
    }

    pub async fn team_vault_members(
        self: &Arc<Self>,
        vault_id: String,
    ) -> Result<Vec<VaultMemberCard>> {
        let me = me(self.store())?;
        Ok(self
            .api()
            .await?
            .vault_members(parse_id(&vault_id)?)
            .await?
            .members
            .into_iter()
            .map(|m| VaultMemberCard {
                me: m.user_id == me,
                user_id: m.user_id.to_string(),
                email: m.email,
                display_name: m.display_name,
                avatar: m.avatar,
                access: vault_access(m.role),
                pending: m.pending,
            })
            .collect())
    }

    /// Grant (or change) a teammate's access: seal our copy of the vault key
    /// to their account public key. Works for pending members and role
    /// changes alike.
    pub async fn set_vault_access(
        self: &Arc<Self>,
        vault_id: String,
        user_id: String,
        access: VaultAccess,
    ) -> Result<()> {
        let vault_id = parse_id(&vault_id)?;
        let user_id = parse_id(&user_id)?;
        let api = self.api().await?;
        let store = self.store();
        let vault = store.vault(vault_id)?;
        let team_id = vault
            .team_id
            .ok_or_else(|| MobileError::invalid("not a team vault"))?;
        let key = store.vault_key(vault_id)?;
        let keys = public_keys(&api, team_id).await?;
        let pk = keys
            .get(&user_id)
            .ok_or_else(|| MobileError::not_found("user is not a member of this team"))?;
        api.upsert_vault_member(
            vault_id,
            &VaultMemberUpsert {
                user_id,
                role: vault_role(access),
                sealed_key: seal_for(pk, &key)?,
            },
        )
        .await?;
        Ok(())
    }

    /// Revoke access and rotate the vault key for everyone who remains.
    pub async fn remove_vault_access(
        self: &Arc<Self>,
        vault_id: String,
        user_id: String,
    ) -> Result<()> {
        let vault_id = parse_id(&vault_id)?;
        let user_id = parse_id(&user_id)?;
        let api = self.api().await?;
        api.remove_vault_member(vault_id, user_id).await?;
        let store = self.store();
        if me(store)? != user_id {
            rotate_with(&api, store, vault_id).await?;
        }
        self.refresh_vaults().await
    }

    pub async fn rotate_team_vault_key(self: &Arc<Self>, vault_id: String) -> Result<()> {
        let api = self.api().await?;
        rotate_with(&api, self.store(), parse_id(&vault_id)?).await?;
        self.refresh_vaults().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_token_is_last_path_segment() {
        assert_eq!(invite_token("https://x.io/invite/abc_-9?x=1"), "abc_-9");
        assert_eq!(invite_token("abc"), "abc");
        assert_eq!(invite_token(" https://x.io/invite/abc/ "), "abc");
        assert_eq!(invite_token(""), "");
    }

    #[test]
    fn audit_details_flatten_to_pairs() {
        let e = proto::AuditEvent {
            id: 7,
            team_id: Uuid::nil(),
            actor_id: None,
            actor_email: Some("a@x.io".into()),
            actor_name: None,
            actor_avatar: None,
            device_id: None,
            action: "member.role_changed".into(),
            vault_id: None,
            target_user: None,
            target_email: Some("b@x.io".into()),
            details: serde_json::json!({"role": "admin", "n": 2}),
            created_at: chrono::Utc::now(),
        };
        let c = AuditEventCard::from(e);
        assert_eq!(c.actor.as_deref(), Some("a@x.io"));
        assert_eq!(
            c.details,
            vec!["n: 2".to_string(), "role: admin".to_string()]
        );
    }
}
