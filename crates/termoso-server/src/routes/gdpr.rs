//! Data subject tooling for teams: where the data lives, a full export of
//! everything the server holds for the team (ciphertext included — the
//! server cannot decrypt it) and deletion with a cancellable grace period.

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Duration, Utc};
use termoso_proto::team::{ScheduleTeamDeletionRequest, TeamDataStatus, TeamRole};
use uuid::Uuid;

use crate::audit;
use crate::error::{ApiResult, Error, NoContent};
use crate::events::{self, Event};
use crate::extract::{Auth, Json as Body};
use crate::routes::teams;
use crate::state::AppState;

pub const EXPORT_PERM: &str = "team.export";

type StatusRow = (
    String,
    Option<String>,
    Option<DateTime<Utc>>,
    Option<Uuid>,
    DateTime<Utc>,
);

async fn status_row(state: &AppState, team_id: Uuid) -> ApiResult<StatusRow> {
    let row: Option<StatusRow> = sqlx::query_as(
        "SELECT name, region, deletion_scheduled_at, deletion_requested_by, created_at FROM teams WHERE id = $1",
    )
    .bind(team_id)
    .fetch_optional(&state.db)
    .await?;
    row.ok_or_else(|| Error::not_found("Team"))
}

fn to_status(state: &AppState, r: StatusRow) -> TeamDataStatus {
    TeamDataStatus {
        region: r.1,
        server_region: state.cfg.data_region.clone(),
        created_at: r.4,
        deletion_scheduled_at: r.2,
        deletion_requested_by: r.3,
        deletion_grace_days: state.cfg.team_deletion_grace_days as i32,
    }
}

/// Region, creation time and pending deletion of the team (any member).
#[utoipa::path(get, path = "/api/v1/teams/{id}/data", tag = "teams",
    params(("id" = Uuid, Path)), responses((status = 200, body = TeamDataStatus)))]
pub async fn status(
    State(state): State<AppState>,
    auth: Auth,
    Path(team_id): Path<Uuid>,
) -> ApiResult<Json<TeamDataStatus>> {
    crate::authz::membership(&state.db, team_id, auth.user_id())
        .await
        .map_err(|_| Error::not_found("Team"))?;
    let row = status_row(&state, team_id).await?;
    if row.2.is_some_and(|at| at <= Utc::now()) {
        // Grace period over: purge now rather than wait for the job.
        teams::purge(&state, team_id).await?;
        return Err(Error::not_found("Team"));
    }
    Ok(Json(to_status(&state, row)))
}

async fn table(state: &AppState, sql: &'static str, team_id: Uuid) -> ApiResult<serde_json::Value> {
    let (v,): (serde_json::Value,) = sqlx::query_as(sql)
        .bind(team_id)
        .fetch_one(&state.db)
        .await?;
    Ok(v)
}

/// Everything the server stores for the team, as one JSON document
/// (`team.export`). Entity and log payloads are the client-encrypted
/// ciphertext exactly as stored; log bodies come as short-lived download URLs.
#[utoipa::path(get, path = "/api/v1/teams/{id}/export", tag = "teams",
    params(("id" = Uuid, Path)), responses((status = 200, description = "application/json", body = serde_json::Value)))]
pub async fn export(
    State(state): State<AppState>,
    auth: Auth,
    Path(team_id): Path<Uuid>,
) -> ApiResult<impl axum::response::IntoResponse> {
    crate::authz::require(&state, team_id, auth.user_id(), EXPORT_PERM).await?;
    crate::ratelimit::check(
        &state,
        crate::ratelimit::SYNC_PUSH,
        &auth.user_id().to_string(),
    )
    .await?;
    let team = table(&state,
        "SELECT row_to_json(x) FROM (SELECT id, name, region, created_at, updated_at, multiplayer_enabled, presence_enabled,
                deletion_scheduled_at FROM teams WHERE id = $1) x", team_id).await?;
    let members = table(&state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT m.user_id, u.email, u.display_name, m.role AS level, r.slug AS role, m.role_source, m.joined_at
            FROM team_members m JOIN users u ON u.id = m.user_id JOIN team_roles r ON r.id = m.role_id
            WHERE m.team_id = $1 ORDER BY m.joined_at) x", team_id).await?;
    let roles = table(&state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT id, slug, name, description, permissions, builtin, position FROM team_roles WHERE team_id = $1 ORDER BY position, slug) x", team_id).await?;
    let policies =
        serde_json::to_value(crate::policy::load(&state.db, team_id).await?).unwrap_or_default();
    let vaults = table(&state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT v.id, v.name, v.created_at, v.key_version, v.session_logging,
                   (SELECT json_agg(json_build_object('user_id', vm.user_id, 'role', vm.role, 'key_version', vm.key_version,
                           'pending', vm.sealed_key IS NULL, 'expires_at', vm.expires_at, 'added_at', vm.added_at))
                    FROM vault_members vm WHERE vm.vault_id = v.id) AS members
            FROM vaults v WHERE v.team_id = $1 AND v.deleted_at IS NULL ORDER BY v.created_at) x", team_id).await?;
    let entities = table(&state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT e.id, e.vault_id, e.kind, e.version, e.key_version, e.data, e.created_at, e.updated_at, e.secret_rotated_at
            FROM entities e JOIN vaults v ON v.id = e.vault_id
            WHERE v.team_id = $1 AND v.deleted_at IS NULL AND e.deleted = false ORDER BY e.vault_id, e.seq) x", team_id).await?;
    let mut logs = table(&state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT l.id, l.vault_id, l.user_id, l.meta, l.key_version, l.size_bytes, l.completed, l.pinned, l.note, l.created_at,
                   l.object_key
            FROM session_logs l JOIN vaults v ON v.id = l.vault_id
            WHERE v.team_id = $1 AND l.deleted = false ORDER BY l.created_at) x", team_id).await?;
    if let Some(arr) = logs.as_array_mut() {
        for l in arr.iter_mut() {
            let Some(obj) = l.as_object_mut() else {
                continue;
            };
            let key = obj
                .remove("object_key")
                .and_then(|k| k.as_str().map(str::to_owned));
            if let (Some(storage), Some(key), true) = (
                &state.storage,
                key,
                obj.get("completed")
                    .and_then(|c| c.as_bool())
                    .unwrap_or(false),
            ) {
                match storage.presign_get(&key).await {
                    Ok(p) => {
                        obj.insert("download_url".into(), p.url.into());
                        obj.insert("download_expires_in".into(), p.expires_in.into());
                    }
                    Err(e) => tracing::warn!(error = %e, "presign for export"),
                }
            }
        }
    }
    let audit_events = table(
        &state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT id, action, actor_id, device_id, vault_id, target_user, details, created_at
            FROM team_audit_events WHERE team_id = $1 ORDER BY created_at DESC LIMIT 50000) x",
        team_id,
    )
    .await?;
    let webhooks = table(
        &state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT id, url, kind, actions, enabled, created_at, last_status, last_delivered_at FROM team_webhooks WHERE team_id = $1) x",
        team_id,
    )
    .await?;
    let acl = table(&state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT id, vault_id, entity_id, entity_kind, principal_kind, role_id, user_id, group_name, mode, created_at
            FROM entity_acl WHERE team_id = $1) x", team_id).await?;
    let access_requests = table(&state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT id, vault_id, user_id, role, hours, reason, status, decided_by, decided_at, expires_at, created_at
            FROM access_requests WHERE team_id = $1) x", team_id).await?;
    let rotation_jobs = table(&state,
        "SELECT COALESCE(json_agg(row_to_json(x)), '[]'::json) FROM (
            SELECT id, vault_id, entity_id, entity_kind, status, note, error, requested_by, claimed_by, created_at, finished_at
            FROM rotation_jobs WHERE team_id = $1) x", team_id).await?;
    let idp = table(&state,
        "SELECT COALESCE((SELECT row_to_json(x) FROM (
            SELECT kind, name, issuer, client_id, scopes, saml_idp_entity_id, domains, enabled, jit_join, updated_at
            FROM team_idp WHERE team_id = $1) x), 'null'::json)", team_id).await?;
    let doc = serde_json::json!({
        "format": "termoso-team-export/1",
        "exported_at": Utc::now(),
        "server": { "name": state.cfg.server_name, "region": state.cfg.data_region, "version": env!("CARGO_PKG_VERSION") },
        "note": "Entity `data` and log `meta`/bodies are end-to-end encrypted with the vault key; the server cannot read them.",
        "team": team,
        "members": members,
        "roles": roles,
        "policies": policies,
        "vaults": vaults,
        "entities": entities,
        "session_logs": logs,
        "audit_events": audit_events,
        "webhooks": webhooks,
        "entity_acl": acl,
        "access_requests": access_requests,
        "rotation_jobs": rotation_jobs,
        "idp": idp,
    });
    audit::record(
        &state.db,
        audit::Entry::new(team_id, &auth, "team.exported").details(serde_json::json!({
            "entities": entities.as_array().map_or(0, Vec::len),
            "logs": logs.as_array().map_or(0, Vec::len),
        })),
    )
    .await;
    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/json".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"termoso-team-{team_id}.json\""),
            ),
        ],
        doc.to_string(),
    ))
}

/// Schedule deletion of the team after the server's grace period (owner
/// only; the team name must be typed to confirm). Members are notified and
/// the owner can cancel until the deadline.
#[utoipa::path(post, path = "/api/v1/teams/{id}/deletion", tag = "teams",
    params(("id" = Uuid, Path)), request_body = ScheduleTeamDeletionRequest,
    responses((status = 200, body = TeamDataStatus)))]
pub async fn schedule(
    State(state): State<AppState>,
    auth: Auth,
    Path(team_id): Path<Uuid>,
    Body(req): Body<ScheduleTeamDeletionRequest>,
) -> ApiResult<Json<TeamDataStatus>> {
    let role = teams::my_role(&state.db, team_id, auth.user_id()).await?;
    if role != TeamRole::Owner {
        return Err(Error::forbidden("Only the owner can delete a team"));
    }
    let row = status_row(&state, team_id).await?;
    if req.confirm_name.trim() != row.0 {
        return Err(Error::bad_request("Type the team name to confirm"));
    }
    if row.2.is_some() {
        return Err(Error::conflict("Deletion is already scheduled"));
    }
    let grace = state.cfg.team_deletion_grace_days as i64;
    if grace == 0 {
        let members = teams::purge(&state, team_id).await?;
        audit_security(&state, &auth, team_id, "team_deleted").await;
        let _ = members;
        return Ok(Json(TeamDataStatus {
            region: row.1,
            server_region: state.cfg.data_region.clone(),
            created_at: row.4,
            deletion_scheduled_at: Some(Utc::now()),
            deletion_requested_by: Some(auth.user_id()),
            deletion_grace_days: 0,
        }));
    }
    let at = Utc::now() + Duration::days(grace);
    sqlx::query(
        "UPDATE teams SET deletion_scheduled_at = $2, deletion_requested_by = $3, updated_at = now() WHERE id = $1",
    )
    .bind(team_id)
    .bind(at)
    .bind(auth.user_id())
    .execute(&state.db)
    .await?;
    audit::record(
        &state.db,
        audit::Entry::new(team_id, &auth, "team.deletion_scheduled")
            .details(serde_json::json!({ "at": at, "grace_days": grace })),
    )
    .await;
    notify_members(&state, team_id, &row.0, Some(at)).await;
    let members = teams::member_ids(&state.db, team_id).await?;
    events::publish(&state, Event::TeamsUpdated { user_ids: members }).await?;
    let row = status_row(&state, team_id).await?;
    Ok(Json(to_status(&state, row)))
}

/// Cancel a scheduled deletion (owner only).
#[utoipa::path(delete, path = "/api/v1/teams/{id}/deletion", tag = "teams",
    params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn cancel(
    State(state): State<AppState>,
    auth: Auth,
    Path(team_id): Path<Uuid>,
) -> ApiResult<NoContent> {
    let role = teams::my_role(&state.db, team_id, auth.user_id()).await?;
    if role != TeamRole::Owner {
        return Err(Error::forbidden("Only the owner can cancel the deletion"));
    }
    let row = status_row(&state, team_id).await?;
    if row.2.is_none() {
        return Err(Error::conflict("No deletion is scheduled"));
    }
    sqlx::query(
        "UPDATE teams SET deletion_scheduled_at = NULL, deletion_requested_by = NULL, updated_at = now() WHERE id = $1",
    )
    .bind(team_id)
    .execute(&state.db)
    .await?;
    audit::record(
        &state.db,
        audit::Entry::new(team_id, &auth, "team.deletion_cancelled"),
    )
    .await;
    notify_members(&state, team_id, &row.0, None).await;
    let members = teams::member_ids(&state.db, team_id).await?;
    events::publish(&state, Event::TeamsUpdated { user_ids: members })
        .await
        .map(NoContent::from)
}

async fn audit_security(state: &AppState, auth: &Auth, team_id: Uuid, kind: &'static str) {
    let _ = crate::users::security_event(
        state,
        auth.user_id(),
        kind,
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "team_id": team_id })),
    )
    .await;
}

async fn notify_members(
    state: &AppState,
    team_id: Uuid,
    team_name: &str,
    at: Option<DateTime<Utc>>,
) {
    let Some(mailer) = &state.mailer else { return };
    let emails: Vec<(String,)> = match sqlx::query_as(
        "SELECT u.email FROM team_members m JOIN users u ON u.id = m.user_id WHERE m.team_id = $1 LIMIT 500",
    )
    .bind(team_id)
    .fetch_all(&state.db)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "member lookup");
            return;
        }
    };
    let url = format!(
        "{}/team?tab=data",
        state.cfg.public_url.trim_end_matches('/')
    );
    for (email,) in emails {
        let msg = match at {
            Some(at) => crate::mail::Email::new(
                format!("Team “{team_name}” will be deleted"),
                "Scheduled team deletion",
            )
            .paragraph(format!(
                "The owner scheduled deletion of the team “{team_name}” on {} for {}. All team vaults, hosts, recordings and audit history on {} will be erased then. Export what you need before that date.",
                state.cfg.server_name,
                at.format("%Y-%m-%d %H:%M UTC"),
                state.cfg.server_name
            ))
            .button("Open team", &url),
            None => crate::mail::Email::new(
                format!("Deletion of “{team_name}” cancelled"),
                "Team deletion cancelled",
            )
            .paragraph(format!(
                "The scheduled deletion of the team “{team_name}” on {} was cancelled by the owner. Nothing was removed.",
                state.cfg.server_name
            ))
            .button("Open team", &url),
        };
        if let Err(e) = mailer.send(&email, &msg).await {
            tracing::warn!(error = %e, "team deletion email");
        }
    }
}

/// Every 10 minutes purge teams whose grace period has ended.
pub async fn run_purge(state: AppState) {
    crate::jobs::exclusive(
        state,
        "team-purge",
        std::time::Duration::from_secs(600),
        purge_pass,
    )
    .await
}

pub async fn purge_pass(state: AppState) -> anyhow::Result<u64> {
    let due: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, name FROM teams WHERE deletion_scheduled_at IS NOT NULL AND deletion_scheduled_at < now()",
    )
    .fetch_all(&state.db)
    .await?;
    let mut n = 0;
    for (id, name) in due {
        match teams::purge(&state, id).await {
            Ok(_) => {
                tracing::info!(team = %id, name = %name, "team purged after grace period");
                n += 1;
            }
            Err(e) => tracing::warn!(team = %id, error = %e, "team purge failed"),
        }
    }
    Ok(n)
}
