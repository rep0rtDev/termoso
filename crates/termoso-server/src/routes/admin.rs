//! Operator API. Admins see accounts and metadata, never vault contents.

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::AssertSqlSafe;
use termoso_proto::admin::*;
use uuid::Uuid;

use crate::error::{ApiResult, Error, NoContent};
use crate::events::{self, Event};
use crate::extract::{Admin, Json as Body};
use crate::session;
use crate::state::AppState;
use crate::users;

#[utoipa::path(get, path = "/api/v1/admin/stats", tag = "admin", responses((status = 200, body = AdminStats)))]
pub async fn stats(State(state): State<AppState>, _admin: Admin) -> ApiResult<Json<AdminStats>> {
    let (users, active_users_30d, teams, vaults, entities, active_sessions, log_storage_bytes): (i64, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT
                (SELECT count(*) FROM users),
                (SELECT count(*) FROM users WHERE last_seen_at > now() - interval '30 days'),
                (SELECT count(*) FROM teams),
                (SELECT count(*) FROM vaults WHERE deleted_at IS NULL),
                (SELECT count(*) FROM entities WHERE deleted = false),
                (SELECT count(*) FROM sessions WHERE revoked_at IS NULL AND expires_at > now()),
                (SELECT COALESCE(sum(size_bytes), 0)::bigint FROM session_logs WHERE deleted = false)",
        )
        .fetch_one(&state.db)
        .await?;
    Ok(Json(AdminStats {
        users,
        active_users_30d,
        teams,
        vaults,
        entities,
        active_sessions,
        log_storage_bytes,
    }))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListQuery {
    /// Substring match on email / display name.
    pub q: Option<String>,
    #[serde(default)]
    pub offset: i64,
    pub limit: Option<i64>,
}

type UserRow = (
    Uuid,
    String,
    bool,
    Option<String>,
    bool,
    bool,
    bool,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    i64,
);

fn to_admin_user(r: UserRow) -> AdminUser {
    let (
        id,
        email,
        email_verified,
        display_name,
        is_admin,
        disabled,
        mfa_enabled,
        created_at,
        last_seen_at,
        devices,
    ) = r;
    AdminUser {
        id,
        email,
        email_verified,
        display_name,
        is_admin,
        disabled,
        mfa_enabled,
        created_at,
        last_seen_at,
        devices,
    }
}

const USER_SELECT: &str = "SELECT u.id, u.email, u.email_verified, u.display_name, u.is_admin, u.disabled,
        (u.totp_enabled OR EXISTS (SELECT 1 FROM webauthn_credentials w WHERE w.user_id = u.id)),
        u.created_at, u.last_seen_at,
        (SELECT count(*) FROM devices d WHERE d.user_id = u.id
           AND EXISTS (SELECT 1 FROM sessions s WHERE s.device_id = d.id AND s.revoked_at IS NULL AND s.expires_at > now()))
     FROM users u";

#[utoipa::path(get, path = "/api/v1/admin/users", tag = "admin", params(ListQuery), responses((status = 200, body = AdminUserList)))]
pub async fn users(
    State(state): State<AppState>,
    _admin: Admin,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<AdminUserList>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let pattern =
        q.q.as_deref()
            .map(|s| format!("%{}%", s.trim().to_lowercase()));
    let rows: Vec<UserRow> = sqlx::query_as(AssertSqlSafe(format!(
        "{USER_SELECT} WHERE ($1::text IS NULL OR lower(u.email) LIKE $1 OR lower(COALESCE(u.display_name, '')) LIKE $1)
         ORDER BY u.created_at DESC OFFSET $2 LIMIT $3"
    )))
    .bind(&pattern)
    .bind(q.offset.max(0))
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let (total,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM users u WHERE ($1::text IS NULL OR lower(u.email) LIKE $1 OR lower(COALESCE(u.display_name, '')) LIKE $1)",
    )
    .bind(&pattern)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(AdminUserList {
        users: rows.into_iter().map(to_admin_user).collect(),
        total,
    }))
}

#[utoipa::path(get, path = "/api/v1/admin/users/{id}", tag = "admin", params(("id" = Uuid, Path)), responses((status = 200, body = AdminUser)))]
pub async fn user(
    State(state): State<AppState>,
    _admin: Admin,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AdminUser>> {
    let row: Option<UserRow> =
        sqlx::query_as(AssertSqlSafe(format!("{USER_SELECT} WHERE u.id = $1")))
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    row.map(to_admin_user)
        .map(Json)
        .ok_or_else(|| Error::not_found("User"))
}

#[utoipa::path(patch, path = "/api/v1/admin/users/{id}", tag = "admin", params(("id" = Uuid, Path)),
    request_body = AdminUpdateUserRequest, responses((status = 200, body = AdminUser)))]
pub async fn update_user(
    State(state): State<AppState>,
    Admin(auth): Admin,
    Path(id): Path<Uuid>,
    Body(req): Body<AdminUpdateUserRequest>,
) -> ApiResult<Json<AdminUser>> {
    let target = users::by_id(&state.db, id).await?;
    if id == auth.user_id() && (req.disabled == Some(true) || req.is_admin == Some(false)) {
        return Err(Error::bad_request("You cannot disable or demote yourself"));
    }
    if req.is_admin == Some(false) && state.cfg.is_bootstrap_admin(&target.email) {
        return Err(Error::bad_request(
            "This admin is configured in TERMOSO_ADMIN_EMAILS",
        ));
    }
    sqlx::query(
        "UPDATE users SET disabled = COALESCE($2, disabled), is_admin = COALESCE($3, is_admin),
                email_verified = COALESCE($4, email_verified), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(req.disabled)
    .bind(req.is_admin)
    .bind(req.email_verified)
    .execute(&state.db)
    .await?;
    if req.disabled == Some(true) {
        session::revoke_all(&state, id, None).await?;
    }
    session::invalidate_user_cache(&state, id).await?;
    users::security_event(
        &state,
        id,
        "admin_update",
        None,
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "by": auth.user_id(), "disabled": req.disabled, "is_admin": req.is_admin, "email_verified": req.email_verified })),
    )
    .await?;
    events::publish(&state, Event::AccountUpdated { user_id: id }).await?;
    user(State(state), Admin(auth), Path(id)).await
}

#[utoipa::path(delete, path = "/api/v1/admin/users/{id}", tag = "admin", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_user(
    State(state): State<AppState>,
    Admin(auth): Admin,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    if id == auth.user_id() {
        return Err(Error::bad_request(
            "Delete your own account through /account",
        ));
    }
    let target = users::by_id(&state.db, id).await?;
    let (owned,): (i64,) = sqlx::query_as("SELECT count(*) FROM teams WHERE owner_id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    if owned > 0 {
        return Err(Error::conflict(
            "User owns teams; transfer or delete them first",
        ));
    }
    let log_keys: Vec<(String,)> =
        sqlx::query_as("SELECT object_key FROM session_logs WHERE user_id = $1")
            .bind(id)
            .fetch_all(&state.db)
            .await?;
    session::revoke_all(&state, id, None).await?;
    sqlx::query("DELETE FROM users WHERE id = $1")
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
    users::security_event(
        &state,
        auth.user_id(),
        "admin_deleted_user",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "user_id": id, "email": target.email })),
    )
    .await
    .map(NoContent::from)
}

#[utoipa::path(post, path = "/api/v1/admin/users/{id}/revoke-sessions", tag = "admin", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn revoke_sessions(
    State(state): State<AppState>,
    Admin(auth): Admin,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    users::by_id(&state.db, id).await?;
    session::revoke_all(&state, id, None).await?;
    users::security_event(
        &state,
        id,
        "admin_revoked_sessions",
        None,
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "by": auth.user_id() })),
    )
    .await
    .map(NoContent::from)
}

/// Reset a user's second factors (support case: lost authenticator *and* backup codes).
#[utoipa::path(post, path = "/api/v1/admin/users/{id}/reset-mfa", tag = "admin", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn reset_mfa(
    State(state): State<AppState>,
    Admin(auth): Admin,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    users::by_id(&state.db, id).await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE users SET totp_secret = NULL, totp_enabled = false, updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM webauthn_credentials WHERE user_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM backup_codes WHERE user_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    session::revoke_all(&state, id, None).await?;
    users::security_event(
        &state,
        id,
        "admin_reset_mfa",
        None,
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "by": auth.user_id() })),
    )
    .await?;
    events::publish(&state, Event::AccountUpdated { user_id: id })
        .await
        .map(NoContent::from)
}

// ───────────────────────────── teams ─────────────────────────────

#[utoipa::path(get, path = "/api/v1/admin/teams", tag = "admin", params(ListQuery), responses((status = 200, body = AdminTeamList)))]
pub async fn teams(
    State(state): State<AppState>,
    _admin: Admin,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<AdminTeamList>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let pattern =
        q.q.as_deref()
            .map(|s| format!("%{}%", s.trim().to_lowercase()));
    let rows: Vec<(Uuid, String, String, i64, i64, DateTime<Utc>)> = sqlx::query_as(
        "SELECT t.id, t.name, u.email,
                (SELECT count(*) FROM team_members m WHERE m.team_id = t.id),
                (SELECT count(*) FROM vaults v WHERE v.team_id = t.id AND v.deleted_at IS NULL),
                t.created_at
         FROM teams t JOIN users u ON u.id = t.owner_id
         WHERE ($1::text IS NULL OR lower(t.name) LIKE $1 OR lower(u.email) LIKE $1)
         ORDER BY t.created_at DESC OFFSET $2 LIMIT $3",
    )
    .bind(&pattern)
    .bind(q.offset.max(0))
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let (total,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM teams t JOIN users u ON u.id = t.owner_id
         WHERE ($1::text IS NULL OR lower(t.name) LIKE $1 OR lower(u.email) LIKE $1)",
    )
    .bind(&pattern)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(AdminTeamList {
        teams: rows
            .into_iter()
            .map(
                |(id, name, owner_email, member_count, vault_count, created_at)| AdminTeam {
                    id,
                    name,
                    owner_email,
                    member_count,
                    vault_count,
                    created_at,
                },
            )
            .collect(),
        total,
    }))
}

#[utoipa::path(delete, path = "/api/v1/admin/teams/{id}", tag = "admin", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete_team(
    State(state): State<AppState>,
    Admin(auth): Admin,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    let members = crate::routes::teams::member_ids(&state.db, id).await?;
    let res = sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::not_found("Team"));
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
        "admin_deleted_team",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "team_id": id })),
    )
    .await
    .map(NoContent::from)
}

// ───────────────────────────── settings / email ─────────────────────────────

#[utoipa::path(get, path = "/api/v1/admin/settings", tag = "admin", responses((status = 200, body = ServerSettings)))]
pub async fn get_settings(
    State(state): State<AppState>,
    _admin: Admin,
) -> ApiResult<Json<ServerSettings>> {
    Ok(Json(state.settings().await?))
}

#[utoipa::path(put, path = "/api/v1/admin/settings", tag = "admin",
    request_body = ServerSettings, responses((status = 200, body = ServerSettings)))]
pub async fn put_settings(
    State(state): State<AppState>,
    Admin(auth): Admin,
    Body(s): Body<ServerSettings>,
) -> ApiResult<Json<ServerSettings>> {
    if s.session_ttl_days == 0 || s.session_ttl_days > 3650 {
        return Err(Error::bad_request("session_ttl_days must be 1..=3650"));
    }
    if s.max_entity_bytes < 1024 || s.max_entity_bytes > 4 * 1024 * 1024 {
        return Err(Error::bad_request(
            "max_entity_bytes must be between 1 KiB and 4 MiB",
        ));
    }
    if s.max_log_bytes < 1024 {
        return Err(Error::bad_request("max_log_bytes is too small"));
    }
    if s.audit_retention_days > 3650 {
        return Err(Error::bad_request("audit_retention_days must be 0..=3650"));
    }
    let mut s = s;
    s.allowed_domains = s
        .allowed_domains
        .iter()
        .map(|d| d.trim().trim_start_matches('@').to_lowercase())
        .filter(|d| !d.is_empty())
        .collect();
    state.save_settings(&s).await?;
    users::security_event(
        &state,
        auth.user_id(),
        "admin_settings_changed",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        None,
    )
    .await?;
    Ok(Json(s))
}

#[utoipa::path(post, path = "/api/v1/admin/email/test", tag = "admin", request_body = TestEmailRequest, responses((status = 204)))]
pub async fn test_email(
    State(state): State<AppState>,
    _admin: Admin,
    Body(req): Body<TestEmailRequest>,
) -> ApiResult<NoContent> {
    let mailer = state
        .mailer
        .as_ref()
        .ok_or_else(|| Error::feature_disabled("Email"))?;
    let to =
        crate::util::normalize_email(&req.to).ok_or_else(|| Error::bad_request("Invalid email"))?;
    mailer
        .send(
            &to,
            &format!("{}: test email", state.cfg.server_name),
            "If you can read this, outgoing email from your Termoso server works.",
        )
        .await
        .map_err(|e| Error::bad_request(format!("Sending failed: {e}")))
        .map(NoContent::from)
}
