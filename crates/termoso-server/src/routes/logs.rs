//! Session logs: metadata in Postgres, encrypted recordings in S3 via
//! pre-signed URLs (the server never proxies log bytes).
//!
//! Every log belongs to the vault whose key encrypts it. In a team vault the
//! recording is therefore readable by everyone holding the vault key, and the
//! server lets those members list and download it; pins and notes are shared
//! team state, while the encrypted metadata and body stay author-controlled.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::AssertSqlSafe;
use termoso_crypto::encoding::unb64;
use termoso_proto::logs::*;
use termoso_proto::sync::MAX_BATCH;
use termoso_proto::vault::VaultKind;
use uuid::Uuid;

use crate::audit;
use crate::error::{ApiResult, Error, NoContent};
use crate::events::{self, Event};
use crate::extract::{Auth, Json as Body};
use crate::ratelimit;
use crate::routes::vaults::{self, Access};
use crate::state::AppState;
use crate::storage::Storage;

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    user_id: Uuid,
    vault_id: Uuid,
    meta: String,
    key_version: i32,
    size_bytes: i64,
    completed: bool,
    created_at: DateTime<Utc>,
    seq: i64,
    vault_seq: i64,
    deleted: bool,
    pinned: bool,
    note: String,
    note_by: Option<Uuid>,
    email: String,
    display_name: Option<String>,
    avatar_tag: Option<String>,
    team: bool,
}

/// One row plus the bits only the server needs.
struct Stored {
    log: SessionLog,
    vault_seq: i64,
    team: bool,
}

fn stored(r: Row) -> Stored {
    let Row {
        id,
        user_id,
        vault_id,
        meta,
        key_version,
        size_bytes,
        completed,
        created_at,
        seq,
        vault_seq,
        deleted,
        pinned,
        note,
        note_by,
        email,
        display_name,
        avatar_tag,
        team,
    } = r;
    let author = (!deleted).then_some(LogAuthor {
        user_id,
        email,
        display_name,
        avatar_tag,
    });
    Stored {
        log: SessionLog {
            id,
            vault_id,
            user_id,
            author,
            meta,
            key_version,
            size_bytes,
            completed,
            created_at,
            seq,
            deleted,
            pinned,
            note,
            note_by,
        },
        vault_seq,
        team,
    }
}

/// Which counter a listing is paged by.
#[derive(Clone, Copy)]
enum Cursor {
    Author,
    Vault,
}

fn to_log(r: Row, cursor: Cursor) -> SessionLog {
    let s = stored(r);
    match cursor {
        Cursor::Author => s.log,
        Cursor::Vault => SessionLog {
            seq: s.vault_seq,
            ..s.log
        },
    }
}

const SELECT: &str =
    "SELECT s.id, s.user_id, s.vault_id, s.meta, s.key_version, s.size_bytes, s.completed,
        s.created_at, s.seq, s.vault_seq, s.deleted, s.pinned, s.note, s.note_by,
        u.email, u.display_name, u.avatar_tag, v.kind = 'team' AS team
    FROM session_logs s JOIN users u ON u.id = s.user_id JOIN vaults v ON v.id = s.vault_id";

const RETURNING: &str = "RETURNING id, user_id, vault_id, meta, key_version, size_bytes, completed,
        created_at, seq, vault_seq, deleted, pinned, note, note_by";

fn storage(state: &AppState) -> ApiResult<&Storage> {
    state
        .storage
        .as_ref()
        .ok_or_else(|| Error::feature_disabled("Session logs (object storage)"))
}

fn object_key(user_id: Uuid, id: Uuid) -> String {
    format!("logs/{user_id}/{id}.bin")
}

/// Advance both counters a change is published under: the author's (for
/// `GET /logs`) and the vault's (for `GET /vaults/{id}/logs`).
async fn next_seqs(
    tx: &mut sqlx::PgConnection,
    user_id: Uuid,
    vault_id: Uuid,
) -> ApiResult<(i64, i64)> {
    let (seq,): (i64,) =
        sqlx::query_as("UPDATE users SET logs_seq = logs_seq + 1 WHERE id = $1 RETURNING logs_seq")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
    let (vseq,): (i64,) = sqlx::query_as(
        "UPDATE vaults SET logs_seq = logs_seq + 1 WHERE id = $1 RETURNING logs_seq",
    )
    .bind(vault_id)
    .fetch_one(&mut *tx)
    .await?;
    Ok((seq, vseq))
}

async fn notify(state: &AppState, s: &Stored) -> ApiResult<()> {
    events::publish(
        state,
        Event::LogsChanged {
            user_id: s.log.user_id,
            seq: s.log.seq,
        },
    )
    .await?;
    if s.team {
        events::publish(
            state,
            Event::VaultLogsChanged {
                vault_id: s.log.vault_id,
                seq: s.vault_seq,
            },
        )
        .await?;
    }
    Ok(())
}

async fn load(state: &AppState, id: Uuid) -> ApiResult<Stored> {
    let row: Option<Row> = sqlx::query_as(AssertSqlSafe(format!("{SELECT} WHERE s.id = $1")))
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    row.map(stored).ok_or_else(|| Error::not_found("Log"))
}

/// What the caller may do with a log: the author owns it outright; other
/// members of a team vault act through their vault role.
struct Grant {
    stored: Stored,
    owner: bool,
    access: Option<Access>,
}

impl Grant {
    fn can_write(&self) -> bool {
        self.owner || self.access.as_ref().is_some_and(|a| a.role.can_write())
    }
    fn can_manage(&self) -> bool {
        self.owner || self.access.as_ref().is_some_and(|a| a.role.can_manage())
    }
    fn team_id(&self) -> Option<Uuid> {
        self.access.as_ref().and_then(|a| a.team_id)
    }
}

/// Load a log the caller may at least read. Teammates need a sealed vault key
/// (a pending member could not decrypt anything anyway); anyone else gets the
/// same 404 as for a log that does not exist.
async fn readable(state: &AppState, auth: &Auth, id: Uuid) -> ApiResult<Grant> {
    let s = load(state, id).await?;
    let owner = s.log.user_id == auth.user_id();
    let access = match vaults::access(&state.db, s.log.vault_id, auth.user_id()).await {
        Ok(a) => Some(a),
        Err(Error::Status(StatusCode::NOT_FOUND, ..)) if owner => None,
        Err(e) => return Err(e),
    };
    let teammate = access
        .as_ref()
        .is_some_and(|a| a.kind == VaultKind::Team && !a.pending);
    if !owner && !teammate {
        return Err(Error::not_found("Log"));
    }
    Ok(Grant {
        stored: s,
        owner,
        access,
    })
}

fn validate_meta(meta: &str, max: usize) -> ApiResult<()> {
    if meta.len() > max || unb64(meta).is_err() {
        return Err(Error::too_large("Log metadata too large"));
    }
    Ok(())
}

fn clean_note(note: &str) -> ApiResult<String> {
    let note = note.trim();
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(Error::too_large(format!(
            "Notes are limited to {MAX_NOTE_CHARS} characters"
        )));
    }
    if note
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(Error::bad_request("Note contains control characters"));
    }
    Ok(note.to_owned())
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
    let (seq, vseq) = next_seqs(&mut tx, auth.user_id(), req.vault_id).await?;
    let inserted = sqlx::query(
        "INSERT INTO session_logs (id, user_id, vault_id, object_key, meta, key_version, size_bytes, completed, seq, vault_seq)
         VALUES ($1, $2, $3, $4, $5, $6, $7, false, $8, $9) ON CONFLICT (id) DO NOTHING",
    )
    .bind(req.id)
    .bind(auth.user_id())
    .bind(req.vault_id)
    .bind(&key)
    .bind(&req.meta)
    .bind(req.key_version)
    .bind(req.size_bytes)
    .bind(seq)
    .bind(vseq)
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
    let grant = readable(&state, &auth, id).await?;
    let log = &grant.stored.log;
    if log.deleted {
        return Err(Error::not_found("Log"));
    }
    if (req.meta.is_some() || req.size_bytes.is_some()) && !grant.owner {
        return Err(Error::forbidden("Only the author can change a recording"));
    }
    if (req.pinned.is_some() || req.note.is_some()) && !grant.can_write() {
        return Err(Error::forbidden("Editor role required to pin or annotate"));
    }
    let settings = state.settings().await?;
    if let Some(m) = &req.meta {
        validate_meta(m, settings.max_entity_bytes as usize)?;
    }
    let note = req.note.as_deref().map(clean_note).transpose()?;
    let mut size = log.size_bytes;
    let mut completed = log.completed;
    if let Some(declared) = req.size_bytes {
        let storage = storage(&state)?;
        let key = object_key(log.user_id, id);
        let actual = storage
            .object_size(&key)
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
            storage.delete(&key).await.ok();
            return Err(Error::too_large("Uploaded object exceeds the limit"));
        }
        size = actual;
        completed = true;
    }
    let pinned = req.pinned.unwrap_or(log.pinned);
    let note_by = match &note {
        Some(n) if n.is_empty() => None,
        Some(_) => Some(auth.user_id()),
        None => log.note_by,
    };
    let mut tx = state.db.begin().await?;
    let (seq, vseq) = next_seqs(&mut tx, log.user_id, log.vault_id).await?;
    let row: Row = sqlx::query_as(AssertSqlSafe(format!(
        "WITH upd AS (UPDATE session_logs SET meta = COALESCE($2, meta), size_bytes = $3, completed = $4,
                seq = $5, vault_seq = $6, pinned = $7, note = COALESCE($8, note), note_by = $9
             WHERE id = $1 {RETURNING})
         {SELECT_FROM_UPD}"
    )))
    .bind(id)
    .bind(&req.meta)
    .bind(size)
    .bind(completed)
    .bind(seq)
    .bind(vseq)
    .bind(pinned)
    .bind(&note)
    .bind(note_by)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    let updated = stored(row);
    if let Some(team_id) = grant.team_id() {
        let mut changes = Vec::new();
        if let Some(p) = req.pinned.filter(|p| *p != log.pinned) {
            changes.push(if p { "pinned" } else { "unpinned" });
        }
        if note.as_deref().is_some_and(|n| n != log.note) {
            changes.push("note");
        }
        if !changes.is_empty() {
            audit::record(
                &state.db,
                audit::Entry::new(team_id, &auth, "log.updated")
                    .vault(log.vault_id)
                    .details(serde_json::json!({
                        "log_id": id,
                        "author_id": log.user_id,
                        "changes": changes,
                    })),
            )
            .await;
        }
    }
    notify(&state, &updated).await?;
    if completed && !log.completed {
        metrics::counter!("termoso_logs_uploaded_total").increment(1);
        metrics::counter!("termoso_logs_uploaded_bytes_total").increment(size as u64);
    }
    Ok(Json(updated.log))
}

/// Re-select the updated row with its author/vault columns.
const SELECT_FROM_UPD: &str =
    "SELECT s.id, s.user_id, s.vault_id, s.meta, s.key_version, s.size_bytes, s.completed,
        s.created_at, s.seq, s.vault_seq, s.deleted, s.pinned, s.note, s.note_by,
        u.email, u.display_name, u.avatar_tag, v.kind = 'team' AS team
    FROM upd s JOIN users u ON u.id = s.user_id JOIN vaults v ON v.id = s.vault_id";

#[utoipa::path(get, path = "/api/v1/logs/{id}/download", tag = "logs", params(("id" = Uuid, Path)),
    responses((status = 200, body = DownloadLogResponse)))]
pub async fn download(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<DownloadLogResponse>> {
    let storage = storage(&state)?;
    let grant = readable(&state, &auth, id).await?;
    let log = &grant.stored.log;
    if log.deleted || !log.completed {
        return Err(Error::not_found("Log"));
    }
    let p = storage
        .presign_get(&object_key(log.user_id, id))
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
    let grant = readable(&state, &auth, id).await?;
    let log = &grant.stored.log;
    if log.deleted {
        return Ok(NoContent);
    }
    if !grant.can_manage() {
        return Err(Error::forbidden(
            "Only the author or a vault manager can delete a recording",
        ));
    }
    if let Some(storage) = &state.storage
        && let Err(e) = storage.delete(&object_key(log.user_id, id)).await
    {
        tracing::warn!(error = %e, "could not delete log object");
    }
    let mut tx = state.db.begin().await?;
    let (seq, vseq) = next_seqs(&mut tx, log.user_id, log.vault_id).await?;
    sqlx::query(
        "UPDATE session_logs SET deleted = true, meta = '', size_bytes = 0, pinned = false, note = '',
                note_by = NULL, seq = $2, vault_seq = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(seq)
    .bind(vseq)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if let Some(team_id) = grant.team_id() {
        audit::record(
            &state.db,
            audit::Entry::new(team_id, &auth, "log.deleted")
                .vault(log.vault_id)
                .details(serde_json::json!({ "log_id": id, "author_id": log.user_id })),
        )
        .await;
    }
    let deleted = Stored {
        log: SessionLog {
            seq,
            deleted: true,
            ..grant.stored.log
        },
        vault_seq: vseq,
        team: grant.stored.team,
    };
    notify(&state, &deleted).await.map(NoContent::from)
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListQuery {
    #[serde(default)]
    pub since: i64,
    pub limit: Option<u32>,
}

fn page(rows: Vec<Row>, limit: usize, since: i64, cursor: Cursor) -> LogListResponse {
    let has_more = rows.len() > limit;
    let logs: Vec<SessionLog> = rows
        .into_iter()
        .take(limit)
        .map(|r| to_log(r, cursor))
        .collect();
    let since = logs.last().map(|l| l.seq).unwrap_or(since);
    LogListResponse {
        logs,
        since,
        has_more,
    }
}

fn clamp(limit: Option<u32>) -> usize {
    limit
        .map(|l| l as usize)
        .unwrap_or(MAX_BATCH)
        .clamp(1, MAX_BATCH)
}

#[utoipa::path(get, path = "/api/v1/logs", tag = "logs", params(ListQuery), responses((status = 200, body = LogListResponse)))]
pub async fn list(
    State(state): State<AppState>,
    auth: Auth,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<LogListResponse>> {
    let limit = clamp(q.limit);
    let rows: Vec<Row> = sqlx::query_as(AssertSqlSafe(format!(
        "{SELECT} WHERE s.user_id = $1 AND s.seq > $2 ORDER BY s.seq LIMIT $3"
    )))
    .bind(auth.user_id())
    .bind(q.since)
    .bind((limit + 1) as i64)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(page(rows, limit, q.since, Cursor::Author)))
}

/// Every member's recordings in a vault, paged by the vault counter. Team
/// vaults only make sense here; a personal vault simply yields the owner's
/// logs.
#[utoipa::path(get, path = "/api/v1/vaults/{id}/logs", tag = "logs",
    params(("id" = Uuid, Path), ListQuery), responses((status = 200, body = LogListResponse)))]
pub async fn list_vault(
    State(state): State<AppState>,
    auth: Auth,
    Path(vault_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<LogListResponse>> {
    let a = vaults::access(&state.db, vault_id, auth.user_id()).await?;
    if a.pending {
        return Err(Error::forbidden("No key for this vault yet"));
    }
    let limit = clamp(q.limit);
    let rows: Vec<Row> = sqlx::query_as(AssertSqlSafe(format!(
        "{SELECT} WHERE s.vault_id = $1 AND s.vault_seq > $2 ORDER BY s.vault_seq LIMIT $3"
    )))
    .bind(vault_id)
    .bind(q.since)
    .bind((limit + 1) as i64)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(page(rows, limit, q.since, Cursor::Vault)))
}
