//! SSH ID: a public handle listing the user's device-bound SSH public keys.
//!
//! Private keys never reach the server. Device keys are listed only while
//! that device holds a live session — logging out or revoking the device
//! removes them from the handle immediately. FIDO2 keys follow the hardware
//! token and stay until the user removes them.
//!
//! The public list is served as plain `authorized_keys` lines at
//! `GET /sshid/{handle}` (default type, ED25519) and
//! `GET /sshid/{handle}/{type}`; `all` lists every type.

use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use termoso_crypto::encoding::unb64;
use termoso_proto::sshid::*;
use uuid::Uuid;

use crate::error::{ApiResult, Error, NoContent};
use crate::extract::{Auth, Client, Json as Body};
use crate::ratelimit;
use crate::state::AppState;
use crate::users;

/// Public fetches per IP (the list is meant to be curl'ed by servers).
const PUBLIC_IP: ratelimit::Limit = ratelimit::Limit {
    name: "sshid_pub",
    max: 120,
    window: Duration::from_secs(60),
};
const MAX_FIDO2_KEYS: i64 = 20;
const MAX_LABEL: usize = 64;
/// Handles that would collide with routes or look official.
const RESERVED: &[&str] = &[
    "admin", "api", "root", "sshid", "termoso", "support", "help", "www", "all",
];

/// Public keys `SELECT`ed for one handle: device keys only while the device
/// is signed in.
const LISTED_KEYS: &str =
    "SELECT k.id, k.key_type, k.public_key, k.device_id, k.label, k.updated_at
     FROM ssh_id_keys k
     WHERE k.user_id = $1
       AND (k.device_id IS NULL OR EXISTS (
            SELECT 1 FROM sessions s
            WHERE s.device_id = k.device_id AND s.revoked_at IS NULL AND s.expires_at > now()))
     ORDER BY k.updated_at DESC";

type KeyRow = (Uuid, String, String, Option<Uuid>, String, DateTime<Utc>);

fn key_type_of(s: &str) -> SshIdKeyType {
    SshIdKeyType::from_url_name(s).unwrap_or(SshIdKeyType::Ed25519)
}

fn public_url(state: &AppState, handle: &str) -> String {
    format!(
        "{}/sshid/{handle}",
        state.cfg.public_url.trim_end_matches('/')
    )
}

/// Accept `<algorithm> <base64>` whose algorithm matches `key_type` both in
/// the text and inside the blob; strips any comment.
fn normalize_public_key(key_type: SshIdKeyType, line: &str) -> ApiResult<String> {
    let mut parts = line.split_whitespace();
    let (Some(alg), Some(b64)) = (parts.next(), parts.next()) else {
        return Err(Error::bad_request("Public key must be `<type> <base64>`"));
    };
    if alg != key_type.wire_name() {
        return Err(Error::bad_request(format!(
            "Public key is {alg}, expected {}",
            key_type.wire_name()
        )));
    }
    let blob = unb64(b64).map_err(|_| Error::bad_request("Public key is not base64"))?;
    let embedded = blob
        .get(..4)
        .and_then(|l| usize::try_from(u32::from_be_bytes(l.try_into().ok()?)).ok())
        .and_then(|n| blob.get(4..4 + n))
        .ok_or_else(|| Error::bad_request("Public key blob is malformed"))?;
    if embedded != alg.as_bytes() {
        return Err(Error::bad_request(
            "Public key blob does not match its type",
        ));
    }
    if blob.len() > 4096 {
        return Err(Error::too_large("Public key is too large"));
    }
    Ok(format!("{alg} {b64}"))
}

async fn handle_of(state: &AppState, user_id: Uuid) -> ApiResult<Option<(String, DateTime<Utc>)>> {
    Ok(
        sqlx::query_as("SELECT handle, created_at FROM ssh_ids WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?,
    )
}

async fn profile(state: &AppState, auth: &Auth) -> ApiResult<Option<SshIdProfile>> {
    let Some((handle, created_at)) = handle_of(state, auth.user_id()).await? else {
        return Ok(None);
    };
    let rows: Vec<(Uuid, String, String, Option<Uuid>, String, DateTime<Utc>, Option<String>)> =
        sqlx::query_as(
            "SELECT k.id, k.key_type, k.public_key, k.device_id, k.label, k.updated_at, d.name
             FROM ssh_id_keys k LEFT JOIN devices d ON d.id = k.device_id
             WHERE k.user_id = $1
               AND (k.device_id IS NULL OR EXISTS (
                    SELECT 1 FROM sessions s
                    WHERE s.device_id = k.device_id AND s.revoked_at IS NULL AND s.expires_at > now()))
             ORDER BY k.updated_at DESC",
        )
        .bind(auth.user_id())
        .fetch_all(&state.db)
        .await?;
    let keys = rows
        .into_iter()
        .map(
            |(id, key_type, public_key, device_id, label, updated_at, device)| SshIdKey {
                id,
                key_type: key_type_of(&key_type),
                public_key,
                device_id,
                label: device.unwrap_or(label),
                current_device: device_id == Some(auth.device_id()),
                updated_at,
            },
        )
        .collect();
    Ok(Some(SshIdProfile {
        url: public_url(state, &handle),
        handle,
        created_at,
        keys,
    }))
}

#[utoipa::path(get, path = "/api/v1/account/sshid", tag = "sshid",
    responses((status = 200, body = Option<SshIdProfile>)))]
pub async fn get(
    State(state): State<AppState>,
    auth: Auth,
) -> ApiResult<Json<Option<SshIdProfile>>> {
    Ok(Json(profile(&state, &auth).await?))
}

#[utoipa::path(post, path = "/api/v1/account/sshid", tag = "sshid",
    request_body = CreateSshIdRequest, responses((status = 200, body = SshIdProfile)))]
pub async fn create(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<CreateSshIdRequest>,
) -> ApiResult<Json<SshIdProfile>> {
    let handle = normalize_handle(&req.handle).ok_or_else(|| {
        Error::bad_request(format!(
            "Handle must be {HANDLE_MIN}–{HANDLE_MAX} characters: letters, digits, - or _"
        ))
    })?;
    if RESERVED.contains(&handle.as_str()) {
        return Err(Error::conflict("This handle is reserved"));
    }
    if handle_of(&state, auth.user_id()).await?.is_some() {
        return Err(Error::conflict("You already have an SSH ID"));
    }
    let res = sqlx::query(
        "INSERT INTO ssh_ids (user_id, handle) VALUES ($1, $2) ON CONFLICT (handle) DO NOTHING",
    )
    .bind(auth.user_id())
    .bind(&handle)
    .execute(&state.db)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::conflict("This handle is already taken"));
    }
    users::security_event(
        &state,
        auth.user_id(),
        "sshid_created",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "handle": handle })),
    )
    .await?;
    let p = profile(&state, &auth)
        .await?
        .ok_or_else(|| Error::not_found("ssh id"))?;
    Ok(Json(p))
}

#[utoipa::path(delete, path = "/api/v1/account/sshid", tag = "sshid", responses((status = 204)))]
pub async fn delete(State(state): State<AppState>, auth: Auth) -> ApiResult<NoContent> {
    let row: Option<(String,)> =
        sqlx::query_as("DELETE FROM ssh_ids WHERE user_id = $1 RETURNING handle")
            .bind(auth.user_id())
            .fetch_optional(&state.db)
            .await?;
    let Some((handle,)) = row else {
        return Err(Error::not_found("ssh id"));
    };
    users::security_event(
        &state,
        auth.user_id(),
        "sshid_deleted",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "handle": handle })),
    )
    .await?;
    Ok(NoContent)
}

#[utoipa::path(put, path = "/api/v1/account/sshid/keys/device", tag = "sshid",
    request_body = PutDeviceKeysRequest, responses((status = 200, body = SshIdProfile)))]
pub async fn put_device_keys(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<PutDeviceKeysRequest>,
) -> ApiResult<Json<SshIdProfile>> {
    if handle_of(&state, auth.user_id()).await?.is_none() {
        return Err(Error::not_found("ssh id"));
    }
    let mut keys = Vec::with_capacity(req.keys.len());
    for k in &req.keys {
        if k.key_type.is_hardware() {
            return Err(Error::bad_request(
                "Hardware keys are added with POST /account/sshid/keys/fido2",
            ));
        }
        if keys.iter().any(|(t, _)| *t == k.key_type) {
            return Err(Error::bad_request("Duplicate key type"));
        }
        keys.push((k.key_type, normalize_public_key(k.key_type, &k.public_key)?));
    }
    let (_, device_name): (Uuid, String) =
        sqlx::query_as("SELECT id, name FROM devices WHERE id = $1 AND user_id = $2")
            .bind(auth.device_id())
            .bind(auth.user_id())
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| Error::not_found("device"))?;

    let mut tx = state.db.begin().await?;
    let types: Vec<String> = keys.iter().map(|(t, _)| t.url_name().to_string()).collect();
    sqlx::query("DELETE FROM ssh_id_keys WHERE device_id = $1 AND NOT (key_type = ANY($2))")
        .bind(auth.device_id())
        .bind(&types)
        .execute(&mut *tx)
        .await?;
    for (t, public_key) in &keys {
        sqlx::query(
            "INSERT INTO ssh_id_keys (id, user_id, device_id, key_type, public_key, label)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (device_id, key_type) WHERE device_id IS NOT NULL
             DO UPDATE SET public_key = EXCLUDED.public_key, label = EXCLUDED.label,
                           updated_at = CASE WHEN ssh_id_keys.public_key = EXCLUDED.public_key
                                             THEN ssh_id_keys.updated_at ELSE now() END",
        )
        .bind(Uuid::new_v4())
        .bind(auth.user_id())
        .bind(auth.device_id())
        .bind(t.url_name())
        .bind(public_key)
        .bind(&device_name)
        .execute(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref d) if d.is_unique_violation() => {
                Error::conflict("This public key is already published")
            }
            other => other.into(),
        })?;
    }
    tx.commit().await?;
    let p = profile(&state, &auth)
        .await?
        .ok_or_else(|| Error::not_found("ssh id"))?;
    Ok(Json(p))
}

#[utoipa::path(post, path = "/api/v1/account/sshid/keys/fido2", tag = "sshid",
    request_body = AddFido2KeyRequest, responses((status = 200, body = SshIdKey)))]
pub async fn add_fido2_key(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<AddFido2KeyRequest>,
) -> ApiResult<Json<SshIdKey>> {
    if handle_of(&state, auth.user_id()).await?.is_none() {
        return Err(Error::not_found("ssh id"));
    }
    if !req.key_type.is_hardware() {
        return Err(Error::bad_request(
            "Only FIDO2 (sk-*) keys can be added here",
        ));
    }
    let label = req.label.trim();
    if label.is_empty() || label.chars().count() > MAX_LABEL {
        return Err(Error::bad_request(format!(
            "Label must be 1–{MAX_LABEL} characters"
        )));
    }
    let public_key = normalize_public_key(req.key_type, &req.public_key)?;
    let (n,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM ssh_id_keys WHERE user_id = $1 AND device_id IS NULL")
            .bind(auth.user_id())
            .fetch_one(&state.db)
            .await?;
    if n >= MAX_FIDO2_KEYS {
        return Err(Error::quota_exceeded("Too many FIDO2 keys"));
    }
    let id = Uuid::new_v4();
    let row: (DateTime<Utc>,) = sqlx::query_as(
        "INSERT INTO ssh_id_keys (id, user_id, device_id, key_type, public_key, label)
         VALUES ($1, $2, NULL, $3, $4, $5) RETURNING updated_at",
    )
    .bind(id)
    .bind(auth.user_id())
    .bind(req.key_type.url_name())
    .bind(&public_key)
    .bind(label)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref d) if d.is_unique_violation() => {
            Error::conflict("This public key is already published")
        }
        other => other.into(),
    })?;
    Ok(Json(SshIdKey {
        id,
        key_type: req.key_type,
        public_key,
        device_id: None,
        label: label.to_string(),
        current_device: false,
        updated_at: row.0,
    }))
}

#[utoipa::path(delete, path = "/api/v1/account/sshid/keys/{id}", tag = "sshid",
    params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn remove_key(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    let res = sqlx::query("DELETE FROM ssh_id_keys WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.user_id())
        .execute(&state.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::not_found("key"));
    }
    Ok(NoContent)
}

/// Remove the device keys of `device_id`; called when the device logs out or
/// is revoked so the handle stops listing them even before the session rows
/// expire.
pub async fn forget_device(state: &AppState, device_id: Uuid) -> ApiResult<()> {
    sqlx::query("DELETE FROM ssh_id_keys WHERE device_id = $1")
        .bind(device_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

// ───────────────────────── public ─────────────────────────

async fn listed(
    state: &AppState,
    handle: &str,
    filter: Option<SshIdKeyType>,
) -> ApiResult<Response> {
    let Some(handle) = normalize_handle(handle) else {
        return Err(Error::not_found("ssh id"));
    };
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT user_id FROM ssh_ids WHERE handle = $1")
        .bind(&handle)
        .fetch_optional(&state.db)
        .await?;
    let Some((user_id,)) = row else {
        return Err(Error::not_found("ssh id"));
    };
    let rows: Vec<KeyRow> = sqlx::query_as(LISTED_KEYS)
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;
    let mut body = String::new();
    for (_, key_type, public_key, _, _, _) in rows {
        let t = key_type_of(&key_type);
        if filter.is_some_and(|f| f != t) {
            continue;
        }
        body.push_str(&public_key);
        body.push_str(" #SSH ID - @");
        body.push_str(&handle);
        body.push('\n');
    }
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=60"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        body,
    )
        .into_response())
}

#[utoipa::path(get, path = "/sshid/{handle}", tag = "sshid",
    params(("handle" = String, Path)),
    responses((status = 200, description = "authorized_keys lines of the default type (ED25519)", content_type = "text/plain")))]
pub async fn public_default(
    State(state): State<AppState>,
    client: Client,
    Path(handle): Path<String>,
) -> ApiResult<Response> {
    ratelimit::check_ip(&state, PUBLIC_IP, client.ip.as_deref()).await?;
    listed(&state, &handle, Some(SshIdKeyType::Ed25519)).await
}

#[utoipa::path(get, path = "/sshid/{handle}/{type}", tag = "sshid",
    params(("handle" = String, Path), ("type" = String, Path, description = "ED25519, ECDSA, RSA, ECDSA-SK, ED25519-SK or all")),
    responses((status = 200, description = "authorized_keys lines", content_type = "text/plain")))]
pub async fn public_typed(
    State(state): State<AppState>,
    client: Client,
    Path((handle, kind)): Path<(String, String)>,
) -> ApiResult<Response> {
    ratelimit::check_ip(&state, PUBLIC_IP, client.ip.as_deref()).await?;
    let filter = if kind.eq_ignore_ascii_case("all") {
        None
    } else {
        Some(SshIdKeyType::from_url_name(&kind).ok_or_else(|| Error::not_found("key type"))?)
    };
    listed(&state, &handle, filter).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_key_normalisation() {
        // `ssh-ed25519` blob: string "ssh-ed25519" + 32 zero bytes.
        let mut blob = Vec::new();
        blob.extend_from_slice(&11u32.to_be_bytes());
        blob.extend_from_slice(b"ssh-ed25519");
        blob.extend_from_slice(&32u32.to_be_bytes());
        blob.extend_from_slice(&[0u8; 32]);
        let b64 = termoso_crypto::encoding::b64(&blob);
        let line = format!("ssh-ed25519 {b64} me@laptop");
        assert_eq!(
            normalize_public_key(SshIdKeyType::Ed25519, &line).unwrap(),
            format!("ssh-ed25519 {b64}")
        );
        assert!(normalize_public_key(SshIdKeyType::Ecdsa, &line).is_err());
        assert!(normalize_public_key(SshIdKeyType::Ed25519, "ssh-ed25519 !!!").is_err());
        let lying = format!("ecdsa-sha2-nistp256 {b64}");
        assert!(normalize_public_key(SshIdKeyType::Ecdsa, &lying).is_err());
    }
}
