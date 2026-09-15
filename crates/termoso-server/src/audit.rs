//! Team activity log. Every team-scoped mutation records *who* did *what* to
//! *which* object; payloads are never inspected (they are ciphertext), so an
//! entity entry carries only its kind and id.

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgExecutor;
use termoso_proto::team::{AuditEvent, AuditEventList};
use uuid::Uuid;

use crate::error::{ApiResult, Error};
use crate::extract::Auth;
use crate::routes::teams;
use crate::state::AppState;

const PAGE: usize = 100;
const MAX_PAGE: usize = 500;

/// A pending log entry; `record` writes it.
#[derive(Debug, Clone)]
pub struct Entry {
    pub team_id: Uuid,
    pub actor: Option<(Uuid, Uuid)>,
    pub action: &'static str,
    pub vault_id: Option<Uuid>,
    pub target_user: Option<Uuid>,
    pub details: serde_json::Value,
}

impl Entry {
    pub fn new(team_id: Uuid, auth: &Auth, action: &'static str) -> Self {
        Self {
            team_id,
            actor: Some((auth.user_id(), auth.device_id())),
            action,
            vault_id: None,
            target_user: None,
            details: serde_json::json!({}),
        }
    }

    /// Same as `new` for code paths without an `Auth` (registration with an
    /// invite token, relay cleanup).
    pub fn by(team_id: Uuid, user_id: Uuid, device_id: Option<Uuid>, action: &'static str) -> Self {
        Self {
            team_id,
            actor: Some((user_id, device_id.unwrap_or(Uuid::nil()))),
            action,
            vault_id: None,
            target_user: None,
            details: serde_json::json!({}),
        }
    }

    pub fn vault(mut self, vault_id: Uuid) -> Self {
        self.vault_id = Some(vault_id);
        self
    }

    pub fn user(mut self, user_id: Uuid) -> Self {
        self.target_user = Some(user_id);
        self
    }

    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }
}

/// Insert one entry. Failures are logged, never propagated: the audit trail
/// must not turn a successful mutation into an error for the caller.
pub async fn record<'e, E: PgExecutor<'e>>(db: E, e: Entry) {
    let (actor_id, device_id) = match e.actor {
        Some((u, d)) => (Some(u), (!d.is_nil()).then_some(d)),
        None => (None, None),
    };
    let res = sqlx::query(
        "INSERT INTO team_audit_events (team_id, actor_id, device_id, action, vault_id, target_user, details)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(e.team_id)
    .bind(actor_id)
    .bind(device_id)
    .bind(e.action)
    .bind(e.vault_id)
    .bind(e.target_user)
    .bind(&e.details)
    .execute(db)
    .await;
    match res {
        Ok(_) => {
            metrics::counter!("termoso_team_audit_events_total", "action" => e.action).increment(1)
        }
        Err(err) => tracing::warn!(error = %err, action = e.action, "could not write audit event"),
    }
}

/// Team of a vault (`None` for personal vaults, which are not audited).
pub async fn team_of_vault<'e, E: PgExecutor<'e>>(db: E, vault_id: Uuid) -> Option<Uuid> {
    sqlx::query_as::<_, (Option<Uuid>,)>("SELECT team_id FROM vaults WHERE id = $1")
        .bind(vault_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .and_then(|(t,)| t)
}

/// Drop entries older than the configured retention (called opportunistically
/// from the list endpoint so no background job is needed).
async fn purge(state: &AppState, team_id: Uuid, days: u32) -> ApiResult<()> {
    if days == 0 {
        return Ok(());
    }
    sqlx::query(
        "DELETE FROM team_audit_events WHERE team_id = $1 AND created_at < now() - make_interval(days => $2)",
    )
    .bind(team_id)
    .bind(days as i32)
    .execute(&state.db)
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct AuditQuery {
    /// Return entries with `id < before` (older page).
    pub before: Option<i64>,
    /// Page size (default 100, max 500).
    pub limit: Option<u32>,
    /// Only this action (exact) or action family (`vault.` prefix).
    pub action: Option<String>,
    /// Only this actor.
    pub actor: Option<Uuid>,
    /// Only this vault.
    pub vault: Option<Uuid>,
}

type Row = (
    i64,
    Uuid,
    Option<Uuid>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    String,
    Option<Uuid>,
    Option<Uuid>,
    Option<String>,
    serde_json::Value,
    DateTime<Utc>,
);

/// Team admins see everything; members see the log too (the whole point is
/// that the software reports *to* the team, not on it) but not other members'
/// device ids.
#[utoipa::path(get, path = "/api/v1/teams/{id}/audit", tag = "teams",
    params(("id" = Uuid, Path), AuditQuery), responses((status = 200, body = AuditEventList)))]
pub async fn list(
    State(state): State<AppState>,
    auth: Auth,
    Path(team_id): Path<Uuid>,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Json<AuditEventList>> {
    let role = teams::my_role(&state.db, team_id, auth.user_id()).await?;
    let retention = state.settings().await?.audit_retention_days;
    purge(&state, team_id, retention).await?;
    let limit = q
        .limit
        .map(|l| l as usize)
        .unwrap_or(PAGE)
        .clamp(1, MAX_PAGE);
    let action_exact = q.action.as_deref().filter(|a| !a.ends_with('.'));
    let action_prefix = q
        .action
        .as_deref()
        .filter(|a| a.ends_with('.'))
        .map(|a| format!("{a}%"));
    if q.action.as_deref().is_some_and(|a| a.len() > 64) {
        return Err(Error::bad_request("action filter too long"));
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT e.id, e.team_id, e.actor_id, a.email, a.display_name, a.avatar_tag, e.device_id, e.action,
                e.vault_id, e.target_user, t.email, e.details, e.created_at
         FROM team_audit_events e
         LEFT JOIN users a ON a.id = e.actor_id
         LEFT JOIN users t ON t.id = e.target_user
         WHERE e.team_id = $1
           AND ($2::bigint IS NULL OR e.id < $2)
           AND ($3::text IS NULL OR e.action = $3)
           AND ($4::text IS NULL OR e.action LIKE $4)
           AND ($5::uuid IS NULL OR e.actor_id = $5)
           AND ($6::uuid IS NULL OR e.vault_id = $6)
         ORDER BY e.id DESC LIMIT $7",
    )
    .bind(team_id)
    .bind(q.before)
    .bind(action_exact)
    .bind(action_prefix)
    .bind(q.actor)
    .bind(q.vault)
    .bind((limit + 1) as i64)
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() > limit;
    let me = auth.user_id();
    let events: Vec<AuditEvent> = rows
        .into_iter()
        .take(limit)
        .map(
            |(
                id,
                team_id,
                actor_id,
                actor_email,
                actor_name,
                actor_avatar,
                device_id,
                action,
                vault_id,
                target_user,
                target_email,
                details,
                created_at,
            )| AuditEvent {
                id,
                team_id,
                actor_id,
                actor_email,
                actor_name,
                actor_avatar,
                device_id: device_id.filter(|_| role.is_admin() || actor_id == Some(me)),
                action,
                vault_id,
                target_user,
                target_email,
                details,
                created_at,
            },
        )
        .collect();
    let next_before = if has_more {
        events.last().map(|e| e.id)
    } else {
        None
    };
    Ok(Json(AuditEventList {
        events,
        next_before,
    }))
}
