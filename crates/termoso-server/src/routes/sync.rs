//! Entity sync: optimistic-concurrency push, cursor-based pull.
//!
//! Every write to a vault takes a row lock on the vault (`FOR UPDATE`) so the
//! per-vault `seq` is strictly monotonic across API instances.

use std::collections::{HashMap, HashSet};

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use sqlx::AssertSqlSafe;
use sqlx::{Postgres, Transaction};
use termoso_crypto::encoding::unb64;
use termoso_proto::entities::{is_known_kind, SyncEntity};
use termoso_proto::sync::*;
use uuid::Uuid;

use crate::error::{ApiResult, Error};
use crate::events;
use crate::extract::{Auth, Json as Body};
use crate::ratelimit;
use crate::routes::vaults::{self, Access};
use crate::state::AppState;

type Row = (
    Uuid,
    String,
    Uuid,
    i64,
    i64,
    bool,
    i32,
    String,
    DateTime<Utc>,
    Option<Uuid>,
);

fn to_entity(r: Row) -> SyncEntity {
    let (
        id,
        kind,
        vault_id,
        version,
        seq,
        deleted,
        key_version,
        data,
        updated_at,
        updated_by_device,
    ) = r;
    SyncEntity {
        id,
        kind,
        vault_id,
        version,
        seq,
        deleted,
        key_version,
        data,
        updated_at,
        updated_by_device,
    }
}

const SELECT: &str = "SELECT id, kind, vault_id, version, seq, deleted, key_version, data, updated_at, updated_by_device FROM entities";

async fn fetch_entity(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> ApiResult<Option<SyncEntity>> {
    let row: Option<Row> =
        sqlx::query_as(AssertSqlSafe(format!("{SELECT} WHERE id = $1 FOR UPDATE")))
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(row.map(to_entity))
}

/// Lock the vault row and bump its seq.
async fn next_seq(tx: &mut Transaction<'_, Postgres>, vault_id: Uuid) -> ApiResult<i64> {
    let (seq,): (i64,) = sqlx::query_as(
        "UPDATE vaults SET seq = seq + 1, updated_at = now() WHERE id = $1 RETURNING seq",
    )
    .bind(vault_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(seq)
}

fn err(id: Uuid, code: &str) -> PushResult {
    PushResult::Error {
        id,
        code: code.to_string(),
    }
}

#[utoipa::path(post, path = "/api/v1/sync/push", tag = "sync",
    request_body = PushRequest, responses((status = 200, body = PushResponse)))]
pub async fn push(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<PushRequest>,
) -> ApiResult<Json<PushResponse>> {
    ratelimit::check(&state, ratelimit::SYNC_PUSH, &auth.user_id().to_string()).await?;
    if req.changes.len() + req.deletes.len() > MAX_BATCH {
        return Err(Error::too_large(format!(
            "At most {MAX_BATCH} items per push"
        )));
    }
    let max_bytes = state.settings().await?.max_entity_bytes as usize;

    // Resolve write access once per vault.
    let mut access: HashMap<Uuid, Option<Access>> = HashMap::new();
    for vid in req
        .changes
        .iter()
        .map(|c| c.vault_id)
        .collect::<HashSet<_>>()
    {
        let a = vaults::require_write(&state, vid, auth.user_id())
            .await
            .ok();
        access.insert(vid, a);
    }

    let device_id = auth.device_id();
    let mut results = Vec::with_capacity(req.changes.len() + req.deletes.len());
    let mut touched: HashMap<Uuid, i64> = HashMap::new();

    let mut tx = state.db.begin().await?;

    for c in &req.changes {
        let Some(Some(a)) = access.get(&c.vault_id) else {
            results.push(err(c.id, "forbidden"));
            continue;
        };
        if !is_known_kind(&c.kind) {
            results.push(err(c.id, "unknown_kind"));
            continue;
        }
        if c.data.len() > max_bytes || unb64(&c.data).is_err() {
            results.push(err(c.id, "payload_too_large"));
            continue;
        }
        if c.key_version != a.key_version {
            results.push(err(c.id, "stale_key_version"));
            continue;
        }
        let existing = fetch_entity(&mut tx, c.id).await?;
        match (&existing, c.base_version) {
            (Some(e), _) if e.vault_id != c.vault_id => {
                results.push(err(c.id, "forbidden"));
                continue;
            }
            (Some(e), Some(base)) if e.version == base => {
                let seq = next_seq(&mut tx, c.vault_id).await?;
                let version = base + 1;
                sqlx::query(
                    "UPDATE entities SET kind = $2, version = $3, seq = $4, deleted = false, key_version = $5, data = $6,
                            updated_at = now(), updated_by_device = $7 WHERE id = $1",
                )
                .bind(c.id)
                .bind(&c.kind)
                .bind(version)
                .bind(seq)
                .bind(c.key_version)
                .bind(&c.data)
                .bind(device_id)
                .execute(&mut *tx)
                .await?;
                touched.insert(c.vault_id, seq);
                results.push(PushResult::Ok {
                    id: c.id,
                    version,
                    seq,
                });
            }
            (Some(e), _) => {
                results.push(PushResult::Conflict {
                    id: c.id,
                    server: e.clone(),
                });
            }
            (None, None) => {
                let seq = next_seq(&mut tx, c.vault_id).await?;
                sqlx::query(
                    "INSERT INTO entities (id, vault_id, kind, version, seq, deleted, key_version, data, updated_by_device)
                     VALUES ($1, $2, $3, 1, $4, false, $5, $6, $7)",
                )
                .bind(c.id)
                .bind(c.vault_id)
                .bind(&c.kind)
                .bind(seq)
                .bind(c.key_version)
                .bind(&c.data)
                .bind(device_id)
                .execute(&mut *tx)
                .await?;
                touched.insert(c.vault_id, seq);
                results.push(PushResult::Ok {
                    id: c.id,
                    version: 1,
                    seq,
                });
            }
            (None, Some(_)) => results.push(err(c.id, "not_found")),
        }
    }

    for d in &req.deletes {
        let Some(e) = fetch_entity(&mut tx, d.id).await? else {
            results.push(err(d.id, "not_found"));
            continue;
        };
        let a = match access.get(&e.vault_id) {
            Some(a) => a.clone(),
            None => {
                let a = vaults::require_write(&state, e.vault_id, auth.user_id())
                    .await
                    .ok();
                access.insert(e.vault_id, a.clone());
                a
            }
        };
        if a.is_none() {
            results.push(err(d.id, "forbidden"));
            continue;
        }
        if e.deleted {
            results.push(PushResult::Ok {
                id: d.id,
                version: e.version,
                seq: e.seq,
            });
            continue;
        }
        if e.version != d.base_version {
            results.push(PushResult::Conflict {
                id: d.id,
                server: e,
            });
            continue;
        }
        let seq = next_seq(&mut tx, e.vault_id).await?;
        let version = e.version + 1;
        sqlx::query(
            "UPDATE entities SET version = $2, seq = $3, deleted = true, data = '', updated_at = now(), updated_by_device = $4 WHERE id = $1",
        )
        .bind(d.id)
        .bind(version)
        .bind(seq)
        .bind(device_id)
        .execute(&mut *tx)
        .await?;
        touched.insert(e.vault_id, seq);
        results.push(PushResult::Ok {
            id: d.id,
            version,
            seq,
        });
    }

    tx.commit().await?;

    for (vault_id, seq) in touched {
        events::vault_changed(&state, vault_id, seq, Some(device_id)).await?;
    }
    metrics::counter!("termoso_sync_push_items_total").increment(results.len() as u64);
    Ok(Json(PushResponse { results }))
}

#[utoipa::path(post, path = "/api/v1/sync/pull", tag = "sync",
    request_body = PullRequest, responses((status = 200, body = PullResponse)))]
pub async fn pull(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<PullRequest>,
) -> ApiResult<Json<PullResponse>> {
    let limit = req
        .limit
        .map(|l| l as usize)
        .unwrap_or(MAX_BATCH)
        .clamp(1, MAX_BATCH);
    let mut cursors = HashMap::new();
    let mut entities = Vec::new();
    let mut has_more = false;

    // Deterministic vault order so paging is stable.
    let mut wanted: Vec<(Uuid, i64)> = req.cursors.into_iter().collect();
    wanted.sort();

    for (vault_id, since) in wanted {
        cursors.insert(vault_id, since);
        if has_more {
            continue;
        }
        let Ok(a) = vaults::access(&state.db, vault_id, auth.user_id()).await else {
            continue;
        };
        if a.pending {
            // Nothing to decrypt yet (key not sealed to this user).
            continue;
        }
        let remaining = limit - entities.len();
        if remaining == 0 {
            has_more = true;
            continue;
        }
        let rows: Vec<Row> = sqlx::query_as(AssertSqlSafe(format!(
            "{SELECT} WHERE vault_id = $1 AND seq > $2 ORDER BY seq LIMIT $3"
        )))
        .bind(vault_id)
        .bind(since)
        .bind((remaining + 1) as i64)
        .fetch_all(&state.db)
        .await?;
        let mut rows: Vec<SyncEntity> = rows.into_iter().map(to_entity).collect();
        if rows.len() > remaining {
            rows.truncate(remaining);
            has_more = true;
        }
        if let Some(last) = rows.last() {
            cursors.insert(vault_id, last.seq);
        }
        entities.extend(rows);
    }
    metrics::counter!("termoso_sync_pull_items_total").increment(entities.len() as u64);
    Ok(Json(PullResponse {
        entities,
        cursors,
        has_more,
    }))
}
