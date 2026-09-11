//! Encrypted command / connection history (personal vault key).

use axum::Json;
use axum::extract::{Query, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use termoso_crypto::encoding::unb64;
use termoso_proto::sync::*;
use uuid::Uuid;

use crate::error::{ApiResult, Error, NoContent};
use crate::events::{self, Event};
use crate::extract::{Auth, Json as Body};
use crate::ratelimit;
use crate::state::AppState;

fn kind_str(k: HistoryKind) -> &'static str {
    match k {
        HistoryKind::Command => "command",
        HistoryKind::Connection => "connection",
    }
}

fn parse_kind(s: &str) -> HistoryKind {
    if s == "connection" {
        HistoryKind::Connection
    } else {
        HistoryKind::Command
    }
}

#[utoipa::path(post, path = "/api/v1/history/push", tag = "history",
    request_body = HistoryPushRequest, responses((status = 200, body = HistoryPullResponse)))]
pub async fn push(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<HistoryPushRequest>,
) -> ApiResult<Json<HistoryPullResponse>> {
    ratelimit::check(&state, ratelimit::SYNC_PUSH, &auth.user_id().to_string()).await?;
    if req.entries.len() > MAX_BATCH {
        return Err(Error::too_large(format!(
            "At most {MAX_BATCH} entries per push"
        )));
    }
    let max_bytes = state.settings().await?.max_entity_bytes as usize;
    for e in &req.entries {
        if e.data.len() > max_bytes || (!e.deleted && unb64(&e.data).is_err()) {
            return Err(Error::too_large("History entry too large"));
        }
    }
    let mut tx = state.db.begin().await?;
    let mut out = Vec::with_capacity(req.entries.len());
    let mut last_seq = 0;
    for e in req.entries {
        let (seq,): (i64,) = sqlx::query_as(
            "UPDATE users SET history_seq = history_seq + 1 WHERE id = $1 RETURNING history_seq",
        )
        .bind(auth.user_id())
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO history_entries (id, user_id, kind, data, key_version, created_at, seq, deleted)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (id) DO UPDATE SET data = EXCLUDED.data, key_version = EXCLUDED.key_version,
                 seq = EXCLUDED.seq, deleted = EXCLUDED.deleted
             WHERE history_entries.user_id = EXCLUDED.user_id",
        )
        .bind(e.id)
        .bind(auth.user_id())
        .bind(kind_str(e.kind))
        .bind(if e.deleted { "" } else { &e.data })
        .bind(e.key_version)
        .bind(e.created_at)
        .bind(seq)
        .bind(e.deleted)
        .execute(&mut *tx)
        .await?;
        last_seq = seq;
        out.push(HistoryEntry { seq, ..e });
    }
    tx.commit().await?;
    if last_seq > 0 {
        events::publish(
            &state,
            Event::HistoryChanged {
                user_id: auth.user_id(),
                seq: last_seq,
            },
        )
        .await?;
    }
    Ok(Json(HistoryPullResponse {
        entries: out,
        since: last_seq,
        has_more: false,
    }))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct PullQuery {
    #[serde(default)]
    pub since: i64,
    pub limit: Option<u32>,
}

#[utoipa::path(get, path = "/api/v1/history/pull", tag = "history", params(PullQuery),
    responses((status = 200, body = HistoryPullResponse)))]
pub async fn pull(
    State(state): State<AppState>,
    auth: Auth,
    Query(q): Query<PullQuery>,
) -> ApiResult<Json<HistoryPullResponse>> {
    let limit = q
        .limit
        .map(|l| l as usize)
        .unwrap_or(MAX_BATCH)
        .clamp(1, MAX_BATCH);
    let rows: Vec<(Uuid, String, String, i32, DateTime<Utc>, i64, bool)> = sqlx::query_as(
        "SELECT id, kind, data, key_version, created_at, seq, deleted FROM history_entries
         WHERE user_id = $1 AND seq > $2 ORDER BY seq LIMIT $3",
    )
    .bind(auth.user_id())
    .bind(q.since)
    .bind((limit + 1) as i64)
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() > limit;
    let entries: Vec<HistoryEntry> = rows
        .into_iter()
        .take(limit)
        .map(
            |(id, kind, data, key_version, created_at, seq, deleted)| HistoryEntry {
                id,
                kind: parse_kind(&kind),
                data,
                key_version,
                created_at,
                seq,
                deleted,
            },
        )
        .collect();
    let since = entries.last().map(|e| e.seq).unwrap_or(q.since);
    Ok(Json(HistoryPullResponse {
        entries,
        since,
        has_more,
    }))
}

/// Tombstones every entry (optionally of one kind) so other devices clear too.
#[utoipa::path(post, path = "/api/v1/history/clear", tag = "history",
    request_body = HistoryClearRequest, responses((status = 204)))]
pub async fn clear(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<HistoryClearRequest>,
) -> ApiResult<NoContent> {
    let mut tx = state.db.begin().await?;
    let (seq,): (i64,) = sqlx::query_as(
        "UPDATE users SET history_seq = history_seq + 1 WHERE id = $1 RETURNING history_seq",
    )
    .bind(auth.user_id())
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE history_entries SET deleted = true, data = '', seq = $2
         WHERE user_id = $1 AND deleted = false AND ($3::text IS NULL OR kind = $3)",
    )
    .bind(auth.user_id())
    .bind(seq)
    .bind(req.kind.map(kind_str))
    .execute(&mut *tx)
    .await?;
    // Purge tombstones older than 30 days; clients that have not synced for
    // that long do a full re-pull anyway.
    sqlx::query("DELETE FROM history_entries WHERE user_id = $1 AND deleted = true AND created_at < now() - interval '30 days'")
        .bind(auth.user_id())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    events::publish(
        &state,
        Event::HistoryChanged {
            user_id: auth.user_id(),
            seq,
        },
    )
    .await
    .map(NoContent::from)
}
