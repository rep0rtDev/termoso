//! Session logs: metadata in Postgres, encrypted recordings in S3 via
//! pre-signed URLs (the server never proxies log bytes).

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::AssertSqlSafe;
use termoso_crypto::encoding::unb64;
use termoso_proto::logs::*;
use termoso_proto::sync::MAX_BATCH;
use uuid::Uuid;

use crate::error::{ApiResult, Error, NoContent};
use crate::events::{self, Event};
use crate::extract::{Auth, Json as Body};
use crate::ratelimit;
use crate::routes::vaults;
use crate::state::AppState;
use crate::storage::Storage;

type Row = (Uuid, Uuid, String, i32, i64, bool, DateTime<Utc>, i64, bool);

fn to_log(r: Row) -> SessionLog {
    let (id, vault_id, meta, key_version, size_bytes, completed, created_at, seq, deleted) = r;
    SessionLog {
        id,
        vault_id,
        meta,
        key_version,
        size_bytes,
        completed,
        created_at,
        seq,
        deleted,
    }
}

const SELECT: &str = "SELECT id, vault_id, meta, key_version, size_bytes, completed, created_at, seq, deleted FROM session_logs";

fn storage(state: &AppState) -> ApiResult<&Storage> {
    state
        .storage
        .as_ref()
        .ok_or_else(|| Error::feature_disabled("Session logs (object storage)"))
}

fn object_key(user_id: Uuid, id: Uuid) -> String {
    format!("logs/{user_id}/{id}.bin")
}

async fn next_seq(db: impl sqlx::PgExecutor<'_>, user_id: Uuid) -> ApiResult<i64> {
    let (seq,): (i64,) =
        sqlx::query_as("UPDATE users SET logs_seq = logs_seq + 1 WHERE id = $1 RETURNING logs_seq")
            .bind(user_id)
            .fetch_one(db)
            .await?;
    Ok(seq)
}

async fn owned(state: &AppState, user_id: Uuid, id: Uuid) -> ApiResult<SessionLog> {
    let row: Option<Row> = sqlx::query_as(AssertSqlSafe(format!(
        "{SELECT} WHERE id = $1 AND user_id = $2"
    )))
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    row.map(to_log).ok_or_else(|| Error::not_found("Log"))
}

fn validate_meta(meta: &str, max: usize) -> ApiResult<()> {
    if meta.len() > max || unb64(meta).is_err() {
        return Err(Error::too_large("Log metadata too large"));
    }
    Ok(())
}

#[utoipa::path(post, path = "/api/v1/logs", tag = "logs",
    request_body = CreateLogRequest, responses((status = 200, body = CreateLogResponse)))]
pub async fn create(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<CreateLogRequest>,
) -> ApiResult<Json<CreateLogResponse>> {
    let storage = storage(&state)?;
    ratelimit::check(&state, ratelimit::SYNC_PUSH, &auth.user_id().to_string()).await?;
    let settings = state.settings().await?;
    validate_meta(&req.meta, settings.max_entity_bytes as usize)?;
    let max_log = settings.max_log_bytes as i64;
    if req.size_bytes <= 0 || req.size_bytes > max_log {
        return Err(Error::too_large(format!(
            "Logs are limited to {max_log} bytes"
        )));
    }
    let a = vaults::require_write(&state, req.vault_id, auth.user_id()).await?;
    if a.key_version != req.key_version {
        return Err(Error::conflict("Stale vault key version"));
    }
    let (used,): (i64,) = sqlx::query_as("SELECT COALESCE(sum(size_bytes), 0)::bigint FROM session_logs WHERE user_id = $1 AND deleted = false")
        .bind(auth.user_id())
        .fetch_one(&state.db)
        .await?;
    let quota = settings.log_quota_bytes as i64;
    if quota > 0 && used + req.size_bytes > quota {
        return Err(Error::quota_exceeded("Session log storage quota exceeded"));
    }
    let key = object_key(auth.user_id(), req.id);
    let mut tx = state.db.begin().await?;
    let seq = next_seq(&mut *tx, auth.user_id()).await?;
    let inserted = sqlx::query(
        "INSERT INTO session_logs (id, user_id, vault_id, object_key, meta, key_version, size_bytes, completed, seq)
         VALUES ($1, $2, $3, $4, $5, $6, $7, false, $8) ON CONFLICT (id) DO NOTHING",
    )
    .bind(req.id)
    .bind(auth.user_id())
    .bind(req.vault_id)
    .bind(&key)
    .bind(&req.meta)
    .bind(req.key_version)
    .bind(req.size_bytes)
    .bind(seq)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(Error::conflict("Log already exists"));
    }
    tx.commit().await?;
    let p = storage
        .presign_put(&key, req.size_bytes)
        .await
        .map_err(|e| Error::Internal(e.context("presigning upload")))?;
    Ok(Json(CreateLogResponse {
        upload_url: p.url,
        upload_headers: p.headers,
        expires_in: p.expires_in,
    }))
}

#[utoipa::path(patch, path = "/api/v1/logs/{id}", tag = "logs", params(("id" = Uuid, Path)),
    request_body = UpdateLogRequest, responses((status = 200, body = SessionLog)))]
pub async fn update(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<UpdateLogRequest>,
) -> ApiResult<Json<SessionLog>> {
    let storage = storage(&state)?;
    let log = owned(&state, auth.user_id(), id).await?;
    if log.deleted {
        return Err(Error::not_found("Log"));
    }
    let settings = state.settings().await?;
    if let Some(m) = &req.meta {
        validate_meta(m, settings.max_entity_bytes as usize)?;
    }
    let mut size = log.size_bytes;
    let mut completed = log.completed;
    if let Some(declared) = req.size_bytes {
        let actual = storage
            .object_size(&object_key(auth.user_id(), id))
            .await
            .map_err(|e| Error::Internal(e.context("checking object")))?;
        let Some(actual) = actual else {
            return Err(Error::bad_request("Upload not found in storage"));
        };
        if actual != declared {
            return Err(Error::bad_request(
                "Declared size does not match the uploaded object",
            ));
        }
        if actual > settings.max_log_bytes as i64 {
            storage.delete(&object_key(auth.user_id(), id)).await.ok();
            return Err(Error::too_large("Uploaded object exceeds the limit"));
        }
        size = actual;
        completed = true;
    }
    let mut tx = state.db.begin().await?;
    let seq = next_seq(&mut *tx, auth.user_id()).await?;
    let row: Row = sqlx::query_as(
        "UPDATE session_logs SET meta = COALESCE($2, meta), size_bytes = $3, completed = $4, seq = $5 WHERE id = $1
         RETURNING id, vault_id, meta, key_version, size_bytes, completed, created_at, seq, deleted",
    )
    .bind(id)
    .bind(&req.meta)
    .bind(size)
    .bind(completed)
    .bind(seq)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    events::publish(
        &state,
        Event::LogsChanged {
            user_id: auth.user_id(),
            seq,
        },
    )
    .await?;
    if completed && !log.completed {
        metrics::counter!("termoso_logs_uploaded_total").increment(1);
        metrics::counter!("termoso_logs_uploaded_bytes_total").increment(size as u64);
    }
    Ok(Json(to_log(row)))
}

#[utoipa::path(get, path = "/api/v1/logs/{id}/download", tag = "logs", params(("id" = Uuid, Path)),
    responses((status = 200, body = DownloadLogResponse)))]
pub async fn download(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<DownloadLogResponse>> {
    let storage = storage(&state)?;
    let log = owned(&state, auth.user_id(), id).await?;
    if log.deleted || !log.completed {
        return Err(Error::not_found("Log"));
    }
    let p = storage
        .presign_get(&object_key(auth.user_id(), id))
        .await
        .map_err(|e| Error::Internal(e.context("presigning download")))?;
    Ok(Json(DownloadLogResponse {
        download_url: p.url,
        expires_in: p.expires_in,
    }))
}

#[utoipa::path(delete, path = "/api/v1/logs/{id}", tag = "logs", params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn delete(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    let log = owned(&state, auth.user_id(), id).await?;
    if log.deleted {
        return Ok(NoContent);
    }
    if let Some(storage) = &state.storage
        && let Err(e) = storage.delete(&object_key(auth.user_id(), id)).await
    {
        tracing::warn!(error = %e, "could not delete log object");
    }
    let mut tx = state.db.begin().await?;
    let seq = next_seq(&mut *tx, auth.user_id()).await?;
    sqlx::query(
        "UPDATE session_logs SET deleted = true, meta = '', size_bytes = 0, seq = $2 WHERE id = $1",
    )
    .bind(id)
    .bind(seq)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    events::publish(
        &state,
        Event::LogsChanged {
            user_id: auth.user_id(),
            seq,
        },
    )
    .await
    .map(NoContent::from)
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListQuery {
    #[serde(default)]
    pub since: i64,
    pub limit: Option<u32>,
}

#[utoipa::path(get, path = "/api/v1/logs", tag = "logs", params(ListQuery), responses((status = 200, body = LogListResponse)))]
pub async fn list(
    State(state): State<AppState>,
    auth: Auth,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<LogListResponse>> {
    let limit = q
        .limit
        .map(|l| l as usize)
        .unwrap_or(MAX_BATCH)
        .clamp(1, MAX_BATCH);
    let rows: Vec<Row> = sqlx::query_as(AssertSqlSafe(format!(
        "{SELECT} WHERE user_id = $1 AND seq > $2 ORDER BY seq LIMIT $3"
    )))
    .bind(auth.user_id())
    .bind(q.since)
    .bind((limit + 1) as i64)
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() > limit;
    let logs: Vec<SessionLog> = rows.into_iter().take(limit).map(to_log).collect();
    let since = logs.last().map(|l| l.seq).unwrap_or(q.since);
    Ok(Json(LogListResponse {
        logs,
        since,
        has_more,
    }))
}
