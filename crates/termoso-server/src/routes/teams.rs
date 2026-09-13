//! Teams: membership, roles, invitations and pending vault-key requests.

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, Postgres, Transaction};
use termoso_proto::team::*;
use termoso_proto::vault::VaultRole;
use uuid::Uuid;

use crate::audit;
use crate::error::{ApiResult, Error, NoContent};
use crate::events::{self, Event};
use crate::extract::{Auth, Json as Body};
use crate::ratelimit;
use crate::state::AppState;
use crate::users;
use crate::util::{hash_token, normalize_email, random_token};

const INVITE_DAYS: i64 = 14;

pub fn team_role_str(r: TeamRole) -> &'static str {
    match r {
        TeamRole::Member => "member",
        TeamRole::Admin => "admin",
        TeamRole::Owner => "owner",
    }
}

pub fn parse_team_role(s: &str) -> TeamRole {
    match s {
        "owner" => TeamRole::Owner,
        "admin" => TeamRole::Admin,
        _ => TeamRole::Member,
    }
}

pub fn vault_role_str(r: VaultRole) -> &'static str {
    match r {
        VaultRole::Viewer => "viewer",
        VaultRole::Editor => "editor",
        VaultRole::Manager => "manager",
    }
}

pub fn parse_vault_role(s: &str) -> VaultRole {
    match s {
        "manager" => VaultRole::Manager,
        "editor" => VaultRole::Editor,
        _ => VaultRole::Viewer,
    }
}

/// Role of `user_id` in `team_id`, or 404 if not a member.
pub async fn my_role<'e, E: PgExecutor<'e>>(
    db: E,
    team_id: Uuid,
    user_id: Uuid,
) -> ApiResult<TeamRole> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2")
            .bind(team_id)
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    row.map(|(r,)| parse_team_role(&r))
        .ok_or_else(|| Error::not_found("Team"))
}

async fn require_admin(state: &AppState, team_id: Uuid, user_id: Uuid) -> ApiResult<TeamRole> {
    let role = my_role(&state.db, team_id, user_id).await?;
    if !role.is_admin() {
        return Err(Error::forbidden("Team admin role required"));
    }
    Ok(role)
}

pub async fn member_ids<'e, E: PgExecutor<'e>>(db: E, team_id: Uuid) -> ApiResult<Vec<Uuid>> {
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT user_id FROM team_members WHERE team_id = $1")
        .bind(team_id)
        .fetch_all(db)
        .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

fn validate_name(name: &str) -> ApiResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 100 {
        return Err(Error::bad_request("Invalid team name"));
    }
    Ok(n.to_string())
}

// ───────────────────────────── teams ─────────────────────────────

#[utoipa::path(get, path = "/api/v1/teams", tag = "teams", responses((status = 200, body = TeamList)))]
pub async fn list(State(state): State<AppState>, auth: Auth) -> ApiResult<Json<TeamList>> {
    let rows: Vec<(Uuid, String, DateTime<Utc>, String, i64, bool, bool)> = sqlx::query_as(
        "SELECT t.id, t.name, t.created_at, m.role,
                (SELECT count(*) FROM team_members x WHERE x.team_id = t.id),
                t.multiplayer_enabled, t.require_mfa
         FROM teams t JOIN team_members m ON m.team_id = t.id
         WHERE m.user_id = $1 ORDER BY t.created_at",
    )
    .bind(auth.user_id())
    .fetch_all(&state.db)
    .await?;
    Ok(Json(TeamList {
        teams: rows
            .into_iter()
            .map(
                |(id, name, created_at, role, member_count, multiplayer_enabled, require_mfa)| {
                    Team {
                        id,
                        name,
                        created_at,
                        my_role: parse_team_role(&role),
                        member_count,
                        multiplayer_enabled,
                        require_mfa,
                    }
                },
            )
            .collect(),
    }))
}

#[utoipa::path(post, path = "/api/v1/teams", tag = "teams",
    request_body = CreateTeamRequest, responses((status = 200, body = Team)))]
pub async fn create(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<CreateTeamRequest>,
) -> ApiResult<Json<Team>> {
    let name = validate_name(&req.name)?;
    if !auth.session.is_admin && !state.settings().await?.users_can_create_teams {
        return Err(Error::forbidden("Team creation is disabled on this server"));
    }
    if !auth.session.email_verified && state.mailer.is_some() {
        return Err(Error::email_unverified());
    }
    let id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    let (created_at,): (DateTime<Utc>,) = sqlx::query_as(
        "INSERT INTO teams (id, name, owner_id) VALUES ($1, $2, $3) RETURNING created_at",
    )
    .bind(id)
    .bind(&name)
    .bind(auth.user_id())
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'owner')")
        .bind(id)
        .bind(auth.user_id())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    metrics::counter!("termoso_teams_created_total").increment(1);
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "team.created").details(serde_json::json!({ "name": name })),
    )
    .await;
    Ok(Json(Team {
        id,
        name,
        created_at,
        my_role: TeamRole::Owner,
        member_count: 1,
        multiplayer_enabled: true,
        require_mfa: false,
    }))
}

#[utoipa::path(get, path = "/api/v1/teams/{id}", tag = "teams", params(("id" = Uuid, Path)), responses((status = 200, body = Team)))]
pub async fn get(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Team>> {
    let role = my_role(&state.db, id, auth.user_id()).await?;
    let (name, created_at, member_count, multiplayer_enabled, require_mfa): (
        String,
        DateTime<Utc>,
        i64,
        bool,
        bool,
    ) = sqlx::query_as(
        "SELECT name, created_at, (SELECT count(*) FROM team_members WHERE team_id = $1),
                multiplayer_enabled, require_mfa
         FROM teams WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(Team {
        id,
        name,
        created_at,
        my_role: role,
        member_count,
        multiplayer_enabled,
        require_mfa,
    }))
}

#[utoipa::path(patch, path = "/api/v1/teams/{id}", tag = "teams", params(("id" = Uuid, Path)),
    request_body = UpdateTeamRequest, responses((status = 200, body = Team)))]
pub async fn update(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<UpdateTeamRequest>,
) -> ApiResult<Json<Team>> {
    require_admin(&state, id, auth.user_id()).await?;
    if let Some(name) = &req.name {
        let name = validate_name(name)?;
        sqlx::query("UPDATE teams SET name = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(&name)
            .execute(&state.db)
            .await?;
        audit::record(
            &state.db,
            audit::Entry::new(id, &auth, "team.renamed")
                .details(serde_json::json!({ "name": name })),
        )
        .await;
    }
    if let Some(on) = req.multiplayer_enabled {
        sqlx::query("UPDATE teams SET multiplayer_enabled = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(on)
            .execute(&state.db)
            .await?;
        audit::record(
            &state.db,
            audit::Entry::new(id, &auth, "team.settings")
                .details(serde_json::json!({ "multiplayer_enabled": on })),
        )
        .await;
    }
    if let Some(on) = req.require_mfa {
        if on && !users::mfa_enabled_for(&state, auth.user_id()).await? {
            return Err(Error::forbidden(
                "Turn on two-factor authentication for your own account first",
            ));
        }
        sqlx::query("UPDATE teams SET require_mfa = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(on)
            .execute(&state.db)
            .await?;
        audit::record(
            &state.db,
            audit::Entry::new(id, &auth, "team.settings")
                .details(serde_json::json!({ "require_mfa": on })),
        )
        .await;
    }
    events::publish(
        &state,
        Event::TeamsUpdated {
            user_ids: member_ids(&state.db, id).await?,
        },
    )
    .await?;
    get(State(state), auth, Path(id)).await
}

#[utoipa::path(delete, path = "/api/v1/teams/{id}", tag = "teams", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    let role = my_role(&state.db, id, auth.user_id()).await?;
    if role != TeamRole::Owner && !auth.session.is_admin {
        return Err(Error::forbidden("Only the owner can delete a team"));
    }
    let members = member_ids(&state.db, id).await?;
    let log_keys: Vec<(String,)> =
        sqlx::query_as("SELECT l.object_key FROM session_logs l JOIN vaults v ON v.id = l.vault_id WHERE v.team_id = $1")
            .bind(id)
            .fetch_all(&state.db)
            .await?;
    sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    if let Some(storage) = &state.storage {
        for (k,) in log_keys {
            if let Err(e) = storage.delete(&k).await {
                tracing::warn!(error = %e, key = %k, "could not delete log object");
            }
        }
    }
    events::publish(
        &state,
        Event::TeamsUpdated {
            user_ids: members.clone(),
        },
    )
    .await?;
    events::publish(&state, Event::VaultsUpdated { user_ids: members }).await?;
    users::security_event(
        &state,
        auth.user_id(),
        "team_deleted",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "team_id": id })),
    )
    .await
    .map(NoContent::from)
}

// ───────────────────────────── members ─────────────────────────────

#[utoipa::path(get, path = "/api/v1/teams/{id}/members", tag = "teams", params(("id" = Uuid, Path)),
    responses((status = 200, body = TeamMemberList)))]
pub async fn members(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TeamMemberList>> {
    my_role(&state.db, id, auth.user_id()).await?;
    let rows: Vec<(Uuid, String, Option<String>, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT u.id, u.email, u.display_name, m.role, u.public_key, m.joined_at
         FROM team_members m JOIN users u ON u.id = m.user_id
         WHERE m.team_id = $1 ORDER BY m.joined_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(TeamMemberList {
        members: rows
            .into_iter()
            .map(
                |(user_id, email, display_name, role, public_key, joined_at)| TeamMember {
                    user_id,
                    email,
                    display_name,
                    role: parse_team_role(&role),
                    public_key,
                    joined_at,
                },
            )
            .collect(),
    }))
}

#[utoipa::path(patch, path = "/api/v1/teams/{id}/members/{user_id}", tag = "teams",
    params(("id" = Uuid, Path), ("user_id" = Uuid, Path)),
    request_body = UpdateTeamMemberRequest, responses((status = 204)))]
pub async fn update_member(
    State(state): State<AppState>,
    auth: Auth,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
    Body(req): Body<UpdateTeamMemberRequest>,
) -> ApiResult<NoContent> {
    let me = my_role(&state.db, id, auth.user_id()).await?;
    let target = my_role(&state.db, id, user_id)
        .await
        .map_err(|_| Error::not_found("Member"))?;
    match req.role {
        TeamRole::Owner => {
            if me != TeamRole::Owner {
                return Err(Error::forbidden("Only the owner can transfer ownership"));
            }
            let mut tx = state.db.begin().await?;
            sqlx::query(
                "UPDATE team_members SET role = 'admin' WHERE team_id = $1 AND user_id = $2",
            )
            .bind(id)
            .bind(auth.user_id())
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE team_members SET role = 'owner' WHERE team_id = $1 AND user_id = $2",
            )
            .bind(id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE teams SET owner_id = $2, updated_at = now() WHERE id = $1")
                .bind(id)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        TeamRole::Admin | TeamRole::Member => {
            if !me.is_admin() {
                return Err(Error::forbidden("Team admin role required"));
            }
            if target == TeamRole::Owner {
                return Err(Error::forbidden("Transfer ownership first"));
            }
            sqlx::query("UPDATE team_members SET role = $3 WHERE team_id = $1 AND user_id = $2")
                .bind(id)
                .bind(user_id)
                .bind(team_role_str(req.role))
                .execute(&state.db)
                .await?;
        }
    }
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "member.role")
            .user(user_id)
            .details(serde_json::json!({
                "role": team_role_str(req.role),
                "previous_role": team_role_str(target),
            })),
    )
    .await;
    events::publish(
        &state,
        Event::TeamsUpdated {
            user_ids: member_ids(&state.db, id).await?,
        },
    )
    .await
    .map(NoContent::from)
}

/// Remove a member: drops their team membership and access to every team vault.
async fn remove(state: &AppState, team_id: Uuid, user_id: Uuid) -> ApiResult<()> {
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2")
        .bind(team_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM vault_members vm USING vaults v WHERE vm.vault_id = v.id AND v.team_id = $1 AND vm.user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut ids = member_ids(&state.db, team_id).await?;
    ids.push(user_id);
    events::publish(
        state,
        Event::TeamsUpdated {
            user_ids: ids.clone(),
        },
    )
    .await?;
    events::publish(state, Event::VaultsUpdated { user_ids: ids }).await
}

#[utoipa::path(delete, path = "/api/v1/teams/{id}/members/{user_id}", tag = "teams",
    params(("id" = Uuid, Path), ("user_id" = Uuid, Path)), responses((status = 204)))]
pub async fn remove_member(
    State(state): State<AppState>,
    auth: Auth,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<NoContent> {
    require_admin(&state, id, auth.user_id()).await?;
    let target = my_role(&state.db, id, user_id)
        .await
        .map_err(|_| Error::not_found("Member"))?;
    if target == TeamRole::Owner {
        return Err(Error::forbidden("The owner cannot be removed"));
    }
    remove(&state, id, user_id).await?;
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "member.removed")
            .user(user_id)
            .details(serde_json::json!({ "role": team_role_str(target) })),
    )
    .await;
    Ok(NoContent)
}

#[utoipa::path(post, path = "/api/v1/teams/{id}/leave", tag = "teams", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn leave(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    let role = my_role(&state.db, id, auth.user_id()).await?;
    if role == TeamRole::Owner {
        return Err(Error::forbidden("Transfer ownership before leaving"));
    }
    remove(&state, id, auth.user_id()).await?;
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "member.left")
            .details(serde_json::json!({ "role": team_role_str(role) })),
    )
    .await;
    Ok(NoContent)
}

// ───────────────────────────── invites ─────────────────────────────

#[utoipa::path(get, path = "/api/v1/teams/{id}/invites", tag = "teams", params(("id" = Uuid, Path)),
    responses((status = 200, body = InviteList)))]
pub async fn invites(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<InviteList>> {
    require_admin(&state, id, auth.user_id()).await?;
    let rows: Vec<(Uuid, String, String, Uuid, DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, email, role, invited_by, created_at, expires_at FROM team_invites
         WHERE team_id = $1 AND accepted_at IS NULL AND expires_at > now() ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(InviteList {
        invites: rows
            .into_iter()
            .map(
                |(id, email, role, invited_by, created_at, expires_at)| Invite {
                    id,
                    email,
                    role: parse_team_role(&role),
                    invited_by,
                    created_at,
                    expires_at,
                },
            )
            .collect(),
    }))
}

#[utoipa::path(post, path = "/api/v1/teams/{id}/invites", tag = "teams", params(("id" = Uuid, Path)),
    request_body = CreateInviteRequest, responses((status = 200, body = CreatedInvite)))]
pub async fn create_invite(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<CreateInviteRequest>,
) -> ApiResult<Json<CreatedInvite>> {
    require_admin(&state, id, auth.user_id()).await?;
    if req.role == TeamRole::Owner {
        return Err(Error::bad_request("Cannot invite as owner"));
    }
    let email = normalize_email(&req.email).ok_or_else(|| Error::bad_request("Invalid email"))?;
    let already: Option<(Uuid,)> = sqlx::query_as(
        "SELECT u.id FROM users u JOIN team_members m ON m.user_id = u.id WHERE m.team_id = $1 AND lower(u.email) = $2",
    )
    .bind(id)
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;
    if already.is_some() {
        return Err(Error::conflict("Already a member"));
    }
    for vid in &req.vault_ids {
        let ok: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM vaults WHERE id = $1 AND team_id = $2 AND deleted_at IS NULL",
        )
        .bind(vid)
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
        if ok.is_none() {
            return Err(Error::bad_request("Vault does not belong to this team"));
        }
    }
    let token = random_token();
    let invite_id = Uuid::new_v4();
    let expires_at = Utc::now() + chrono::Duration::days(INVITE_DAYS);
    sqlx::query(
        "DELETE FROM team_invites WHERE team_id = $1 AND lower(email) = $2 AND accepted_at IS NULL",
    )
    .bind(id)
    .bind(&email)
    .execute(&state.db)
    .await?;
    let (created_at,): (DateTime<Utc>,) = sqlx::query_as(
        "INSERT INTO team_invites (id, team_id, email, role, token_hash, invited_by, vault_ids, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING created_at",
    )
    .bind(invite_id)
    .bind(id)
    .bind(&email)
    .bind(team_role_str(req.role))
    .bind(hash_token(&token))
    .bind(auth.user_id())
    .bind(&req.vault_ids)
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;
    let url = format!(
        "{}/invite/{}",
        state.cfg.web_url().trim_end_matches('/'),
        token
    );
    if let Some(mailer) = &state.mailer
        && ratelimit::check(&state, ratelimit::EMAIL, &email)
            .await
            .is_ok()
    {
        let (team_name,): (String,) = sqlx::query_as("SELECT name FROM teams WHERE id = $1")
            .bind(id)
            .fetch_one(&state.db)
            .await?;
        let inviter = users::by_id(&state.db, auth.user_id()).await?;
        let text = format!(
            "{} invited you to join the team \"{}\" on {}.\n\nOpen this link to accept:\n\n    {}\n\nThe invitation expires in {} days.",
            inviter
                .display_name
                .clone()
                .unwrap_or(inviter.email.clone()),
            team_name,
            state.cfg.server_name,
            url,
            INVITE_DAYS
        );
        if let Err(e) = mailer
            .send(&email, &format!("Invitation to {team_name}"), &text)
            .await
        {
            tracing::warn!(error = %e, "could not send invite email");
        }
    }
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "invite.created").details(serde_json::json!({
            "invite_id": invite_id,
            "email": email,
            "role": team_role_str(req.role),
            "vault_ids": req.vault_ids,
        })),
    )
    .await;
    Ok(Json(CreatedInvite {
        invite: Invite {
            id: invite_id,
            email,
            role: req.role,
            invited_by: auth.user_id(),
            created_at,
            expires_at,
        },
        url,
    }))
}

#[utoipa::path(delete, path = "/api/v1/teams/{id}/invites/{invite_id}", tag = "teams",
    params(("id" = Uuid, Path), ("invite_id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_invite(
    State(state): State<AppState>,
    auth: Auth,
    Path((id, invite_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<NoContent> {
    require_admin(&state, id, auth.user_id()).await?;
    let res: Option<(String,)> = sqlx::query_as(
        "DELETE FROM team_invites WHERE id = $1 AND team_id = $2 AND accepted_at IS NULL RETURNING email",
    )
    .bind(invite_id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some((email,)) = res else {
        return Err(Error::not_found("Invite"));
    };
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "invite.revoked")
            .details(serde_json::json!({ "invite_id": invite_id, "email": email })),
    )
    .await;
    Ok(NoContent)
}

#[utoipa::path(get, path = "/api/v1/invites/{token}", tag = "teams", params(("token" = String, Path)),
    responses((status = 200, body = InvitePreview)))]
pub async fn invite_preview(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> ApiResult<Json<InvitePreview>> {
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT t.name, COALESCE(u.display_name, u.email), i.email, i.role
         FROM team_invites i JOIN teams t ON t.id = i.team_id JOIN users u ON u.id = i.invited_by
         WHERE i.token_hash = $1 AND i.accepted_at IS NULL AND i.expires_at > now()",
    )
    .bind(hash_token(&token))
    .fetch_optional(&state.db)
    .await?;
    let Some((team_name, inviter, email, role)) = row else {
        return Err(Error::token_expired());
    };
    let account_exists = users::by_email(&state.db, &email).await?.is_some();
    Ok(Json(InvitePreview {
        team_name,
        inviter,
        email,
        role: parse_team_role(&role),
        account_exists,
    }))
}

/// Join the team and create pending (unsealed) vault memberships. Used by
/// `accept_invite` and by registration with an invite token.
pub async fn apply_invite(
    tx: &mut Transaction<'_, Postgres>,
    invite_id: Uuid,
    team_id: Uuid,
    role: &str,
    vault_ids: &[Uuid],
    user_id: Uuid,
    device_id: Option<Uuid>,
) -> ApiResult<NoContent> {
    let role = if role == "owner" { "member" } else { role };
    sqlx::query(
        "INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (team_id, user_id) DO NOTHING",
    )
    .bind(team_id)
    .bind(user_id)
    .bind(role)
    .execute(&mut **tx)
    .await?;
    for vid in vault_ids {
        sqlx::query(
            "INSERT INTO vault_members (vault_id, user_id, role, sealed_key, key_version)
             SELECT id, $2, 'viewer', NULL, key_version FROM vaults WHERE id = $1 AND team_id = $3 AND deleted_at IS NULL
             ON CONFLICT (vault_id, user_id) DO NOTHING",
        )
        .bind(vid)
        .bind(user_id)
        .bind(team_id)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query("UPDATE team_invites SET accepted_at = now(), accepted_by = $2 WHERE id = $1")
        .bind(invite_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    audit::record(
        &mut **tx,
        audit::Entry::by(team_id, user_id, device_id, "invite.accepted")
            .user(user_id)
            .details(serde_json::json!({
                "invite_id": invite_id,
                "role": role,
                "vault_ids": vault_ids,
            })),
    )
    .await;
    Ok(NoContent)
}

#[utoipa::path(post, path = "/api/v1/invites/{token}/accept", tag = "teams", params(("token" = String, Path)),
    responses((status = 200, body = Team)))]
pub async fn accept_invite(
    State(state): State<AppState>,
    auth: Auth,
    Path(token): Path<String>,
) -> ApiResult<Json<Team>> {
    let row: Option<(Uuid, Uuid, String, String, Vec<Uuid>)> = sqlx::query_as(
        "SELECT id, team_id, email, role, vault_ids FROM team_invites
         WHERE token_hash = $1 AND accepted_at IS NULL AND expires_at > now()",
    )
    .bind(hash_token(&token))
    .fetch_optional(&state.db)
    .await?;
    let Some((invite_id, team_id, email, role, vault_ids)) = row else {
        return Err(Error::token_expired());
    };
    let me = users::by_id(&state.db, auth.user_id()).await?;
    if !me.email.eq_ignore_ascii_case(&email) {
        return Err(Error::forbidden(
            "This invitation was sent to a different email address",
        ));
    }
    let mut tx = state.db.begin().await?;
    apply_invite(
        &mut tx,
        invite_id,
        team_id,
        &role,
        &vault_ids,
        me.id,
        Some(auth.device_id()),
    )
    .await?;
    tx.commit().await?;
    let mut ids = member_ids(&state.db, team_id).await?;
    ids.push(me.id);
    events::publish(
        &state,
        Event::TeamsUpdated {
            user_ids: ids.clone(),
        },
    )
    .await?;
    if !vault_ids.is_empty() {
        events::publish(&state, Event::VaultsUpdated { user_ids: ids }).await?;
    }
    get(State(state), auth, Path(team_id)).await
}

// ───────────────────────────── pending vault keys ─────────────────────────────

/// Members of team vaults who still need the vault key sealed to their public
/// key. Only vault managers (or team admins) get to see them.
#[utoipa::path(get, path = "/api/v1/teams/{id}/pending-keys", tag = "teams", params(("id" = Uuid, Path)),
    responses((status = 200, body = PendingVaultKeys)))]
pub async fn pending_keys(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<PendingVaultKeys>> {
    let role = my_role(&state.db, id, auth.user_id()).await?;
    let rows: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as(
        "SELECT vm.vault_id, vm.user_id, u.public_key, vm.role
         FROM vault_members vm
         JOIN vaults v ON v.id = vm.vault_id
         JOIN users u ON u.id = vm.user_id
         WHERE v.team_id = $1 AND v.deleted_at IS NULL AND vm.sealed_key IS NULL
           AND ($3 OR EXISTS (SELECT 1 FROM vault_members me WHERE me.vault_id = v.id AND me.user_id = $2
                              AND me.role = 'manager' AND me.sealed_key IS NOT NULL))",
    )
    .bind(id)
    .bind(auth.user_id())
    .bind(role.is_admin())
    .fetch_all(&state.db)
    .await?;
    Ok(Json(PendingVaultKeys {
        items: rows
            .into_iter()
            .map(|(vault_id, user_id, public_key, role)| PendingVaultKey {
                vault_id,
                user_id,
                public_key,
                role: parse_vault_role(&role),
            })
            .collect(),
    }))
}
