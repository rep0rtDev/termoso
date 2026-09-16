//! API bridges: headless clients the user runs in their own infrastructure.
//!
//! A bridge is a device row + a session flagged with `bridge_id` + the set of
//! vaults whose key the cabinet sealed to the bridge's public key. The bridge
//! encrypts everything itself and talks to the server only through `/sync`,
//! so the server sees exactly what it sees from any other client: opaque
//! blobs. Access to a vault is still derived from the *owner's* membership
//! on every request — a bridge never outlives or outranks its owner.

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use termoso_crypto::encoding::unb64;
use termoso_proto::bridge::*;
use termoso_proto::vault::VaultKind;
use uuid::Uuid;

use crate::audit;
use crate::error::{ApiResult, Error, NoContent};
use crate::extract::{Auth, Json as Body, StepUp};
use crate::routes::vaults;
use crate::session;
use crate::state::AppState;
use crate::users;

const MAX_BRIDGES: i64 = 20;
const MAX_VAULTS: usize = 50;

fn validate_name(name: &str) -> ApiResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 64 {
        return Err(Error::bad_request("Invalid bridge name"));
    }
    Ok(n.to_string())
}

fn validate_public_key(key: &str) -> ApiResult<()> {
    match unb64(key) {
        Ok(b) if b.len() == 32 => Ok(()),
        _ => Err(Error::bad_request("public_key must be a base64 X25519 key")),
    }
}

fn validate_sealed(sealed: &str) -> ApiResult<()> {
    let b = unb64(sealed).map_err(|_| Error::bad_request("sealed_key is not valid base64"))?;
    if b.len() < 48 || b.len() > 512 {
        return Err(Error::bad_request("sealed_key has an unexpected length"));
    }
    Ok(())
}

/// Whether `auth` may touch `vault_id` at all. People: always (membership is
/// checked next); bridges: only vaults sealed to them.
pub async fn in_scope(state: &AppState, auth: &Auth, vault_id: Uuid) -> ApiResult<bool> {
    let Some(bridge_id) = auth.bridge_id() else {
        return Ok(true);
    };
    let row: Option<(bool,)> = sqlx::query_as(
        "SELECT sealed_key IS NOT NULL FROM bridge_vaults WHERE bridge_id = $1 AND vault_id = $2",
    )
    .bind(bridge_id)
    .bind(vault_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.is_some_and(|(sealed,)| sealed))
}

/// The owner must be able to write the vault themselves and hold its key.
async fn check_vaults(state: &AppState, user_id: Uuid, keys: &[BridgeVaultKey]) -> ApiResult<()> {
    if keys.len() > MAX_VAULTS {
        return Err(Error::bad_request("Too many vaults for one bridge"));
    }
    let mut seen = std::collections::HashSet::new();
    for k in keys {
        if !seen.insert(k.vault_id) {
            return Err(Error::bad_request("Duplicate vault"));
        }
        validate_sealed(&k.sealed_key)?;
        vaults::require_write(state, k.vault_id, user_id).await?;
    }
    Ok(())
}

type VaultRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<Uuid>,
    i32,
    Option<String>,
);

async fn vaults_of(
    state: &AppState,
    user_id: Uuid,
    bridge_ids: &[Uuid],
) -> ApiResult<std::collections::HashMap<Uuid, Vec<BridgeVault>>> {
    let rows: Vec<VaultRow> = sqlx::query_as(
        "SELECT bv.bridge_id, bv.vault_id, v.name, v.kind, v.team_id, v.key_version,
                CASE WHEN bv.key_version = v.key_version THEN bv.sealed_key END
         FROM bridge_vaults bv JOIN vaults v ON v.id = bv.vault_id
         WHERE bv.bridge_id = ANY($1) AND v.deleted_at IS NULL
         ORDER BY v.name",
    )
    .bind(bridge_ids)
    .fetch_all(&state.db)
    .await?;
    let mut out: std::collections::HashMap<Uuid, Vec<BridgeVault>> = Default::default();
    for (bridge_id, vault_id, name, kind, team_id, key_version, sealed_key) in rows {
        // Access is the owner's; a vault they lost is simply not listed.
        let Ok(a) = vaults::access(&state.db, vault_id, user_id).await else {
            continue;
        };
        out.entry(bridge_id).or_default().push(BridgeVault {
            vault_id,
            name,
            kind: if kind == "team" {
                VaultKind::Team
            } else {
                VaultKind::Personal
            },
            team_id,
            role: a.role,
            key_version,
            sealed_key,
        });
    }
    Ok(out)
}

type BridgeRow = (
    Uuid,
    String,
    Uuid,
    String,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<String>,
);

const SELECT_BRIDGE: &str = "SELECT b.id, b.name, b.device_id, b.public_key, b.created_at,
        (SELECT max(s.last_used_at) FROM sessions s WHERE s.bridge_id = b.id AND s.last_used_at > s.created_at),
        d.last_ip
     FROM bridges b JOIN devices d ON d.id = b.device_id";

async fn load_all(state: &AppState, user_id: Uuid) -> ApiResult<Vec<Bridge>> {
    let rows: Vec<BridgeRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECT_BRIDGE} WHERE b.user_id = $1 ORDER BY b.created_at"
    )))
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    let mut vaults = vaults_of(state, user_id, &ids).await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, name, device_id, public_key, created_at, last_used_at, last_ip)| Bridge {
                id,
                name,
                device_id,
                public_key,
                vaults: vaults.remove(&id).unwrap_or_default(),
                created_at,
                last_used_at,
                last_ip,
            },
        )
        .collect())
}

async fn load_one(state: &AppState, user_id: Uuid, id: Uuid) -> ApiResult<Bridge> {
    load_all(state, user_id)
        .await?
        .into_iter()
        .find(|b| b.id == id)
        .ok_or_else(|| Error::not_found("Bridge"))
}

#[utoipa::path(get, path = "/api/v1/account/bridges", tag = "bridges",
    responses((status = 200, body = BridgeList)))]
pub async fn list(State(state): State<AppState>, auth: Auth) -> ApiResult<Json<BridgeList>> {
    Ok(Json(BridgeList {
        bridges: load_all(&state, auth.user_id()).await?,
    }))
}

/// Create a bridge. The token in the response is shown once; the private key
/// never leaves the cabinet.
#[utoipa::path(post, path = "/api/v1/account/bridges", tag = "bridges",
    request_body = CreateBridgeRequest, responses((status = 200, body = CreateBridgeResponse)))]
pub async fn create(
    State(state): State<AppState>,
    StepUp(auth): StepUp,
    Body(req): Body<CreateBridgeRequest>,
) -> ApiResult<Json<CreateBridgeResponse>> {
    let name = validate_name(&req.name)?;
    validate_public_key(&req.public_key)?;
    check_vaults(&state, auth.user_id(), &req.vaults).await?;
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM bridges WHERE user_id = $1")
        .bind(auth.user_id())
        .fetch_one(&state.db)
        .await?;
    if count >= MAX_BRIDGES {
        return Err(Error::conflict(format!(
            "At most {MAX_BRIDGES} bridges per account"
        )));
    }

    let bridge_id = Uuid::new_v4();
    let device_id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO devices (id, user_id, name, platform, app_version, approved_at)
         VALUES ($1, $2, $3, 'cli', 'bridge', now())",
    )
    .bind(device_id)
    .bind(auth.user_id())
    .bind(format!("API bridge: {name}"))
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO bridges (id, user_id, device_id, name, public_key) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(bridge_id)
    .bind(auth.user_id())
    .bind(device_id)
    .bind(&name)
    .bind(&req.public_key)
    .execute(&mut *tx)
    .await?;
    for k in &req.vaults {
        sqlx::query(
            "INSERT INTO bridge_vaults (bridge_id, vault_id, sealed_key, key_version)
             SELECT $1, $2, $3, key_version FROM vaults WHERE id = $2",
        )
        .bind(bridge_id)
        .bind(k.vault_id)
        .bind(&k.sealed_key)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let token = session::issue_bridge(&state, auth.user_id(), device_id, bridge_id).await?;

    users::security_event(
        &state,
        auth.user_id(),
        "bridge_created",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "bridge_id": bridge_id, "name": name })),
    )
    .await?;
    let bridge = load_one(&state, auth.user_id(), bridge_id).await?;
    for v in &bridge.vaults {
        if let Some(team_id) = v.team_id {
            audit::record(
                &state.db,
                audit::Entry::new(team_id, &auth, "bridge.vault_added")
                    .vault(v.vault_id)
                    .details(serde_json::json!({ "bridge_id": bridge_id, "name": name })),
            )
            .await;
        }
    }
    Ok(Json(CreateBridgeResponse { bridge, token }))
}

/// Replace the vault set of a bridge (add, remove, or re-seal after a key
/// rotation). Only vaults in the request stay.
#[utoipa::path(put, path = "/api/v1/account/bridges/{id}/vaults", tag = "bridges",
    params(("id" = Uuid, Path)), request_body = Vec<BridgeVaultKey>,
    responses((status = 200, body = Bridge)))]
pub async fn set_vaults(
    State(state): State<AppState>,
    StepUp(auth): StepUp,
    Path(id): Path<Uuid>,
    Body(keys): Body<Vec<BridgeVaultKey>>,
) -> ApiResult<Json<Bridge>> {
    let before = load_one(&state, auth.user_id(), id).await?;
    check_vaults(&state, auth.user_id(), &keys).await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM bridge_vaults WHERE bridge_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for k in &keys {
        sqlx::query(
            "INSERT INTO bridge_vaults (bridge_id, vault_id, sealed_key, key_version)
             SELECT $1, $2, $3, key_version FROM vaults WHERE id = $2",
        )
        .bind(id)
        .bind(k.vault_id)
        .bind(&k.sealed_key)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let after = load_one(&state, auth.user_id(), id).await?;
    let was: std::collections::HashSet<Uuid> = before.vaults.iter().map(|v| v.vault_id).collect();
    let now: std::collections::HashSet<Uuid> = after.vaults.iter().map(|v| v.vault_id).collect();
    for v in before.vaults.iter().filter(|v| !now.contains(&v.vault_id)) {
        if let Some(team_id) = v.team_id {
            audit::record(
                &state.db,
                audit::Entry::new(team_id, &auth, "bridge.vault_removed")
                    .vault(v.vault_id)
                    .details(serde_json::json!({ "bridge_id": id, "name": after.name })),
            )
            .await;
        }
    }
    for v in after.vaults.iter().filter(|v| !was.contains(&v.vault_id)) {
        if let Some(team_id) = v.team_id {
            audit::record(
                &state.db,
                audit::Entry::new(team_id, &auth, "bridge.vault_added")
                    .vault(v.vault_id)
                    .details(serde_json::json!({ "bridge_id": id, "name": after.name })),
            )
            .await;
        }
    }
    Ok(Json(after))
}

#[utoipa::path(delete, path = "/api/v1/account/bridges/{id}", tag = "bridges",
    params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn revoke(
    State(state): State<AppState>,
    StepUp(auth): StepUp,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    let bridge = load_one(&state, auth.user_id(), id).await?;
    session::revoke_device(&state, auth.user_id(), bridge.device_id).await?;
    sqlx::query("DELETE FROM bridges WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.user_id())
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM devices WHERE id = $1 AND user_id = $2")
        .bind(bridge.device_id)
        .bind(auth.user_id())
        .execute(&state.db)
        .await?;
    users::security_event(
        &state,
        auth.user_id(),
        "bridge_revoked",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "bridge_id": id, "name": bridge.name })),
    )
    .await?;
    for v in &bridge.vaults {
        if let Some(team_id) = v.team_id {
            audit::record(
                &state.db,
                audit::Entry::new(team_id, &auth, "bridge.revoked")
                    .vault(v.vault_id)
                    .details(serde_json::json!({ "bridge_id": id, "name": bridge.name })),
            )
            .await;
        }
    }
    Ok(NoContent)
}

/// What a bridge learns about itself with its own token.
#[utoipa::path(get, path = "/api/v1/bridge/me", tag = "bridges",
    responses((status = 200, body = BridgeSelf)))]
pub async fn me(State(state): State<AppState>, auth: Auth) -> ApiResult<Json<BridgeSelf>> {
    let Some(bridge_id) = auth.bridge_id() else {
        return Err(Error::forbidden("Not an API bridge session"));
    };
    let b = load_one(&state, auth.user_id(), bridge_id).await?;
    Ok(Json(BridgeSelf {
        id: b.id,
        name: b.name,
        user_id: auth.user_id(),
        device_id: b.device_id,
        vaults: b.vaults,
    }))
}
