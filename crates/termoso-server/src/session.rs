//! Bearer sessions: issue, validate (Redis-cached), touch, revoke.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use termoso_proto::auth::{DeviceInfo, Platform};
use uuid::Uuid;

use crate::error::{ApiResult, Error};
use crate::events;
use crate::state::AppState;
use crate::util::{hash_token, random_token};

const CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub is_admin: bool,
    pub disabled: bool,
    pub email_verified: bool,
    pub last_used_at: DateTime<Utc>,
}

fn cache_key(token_hash: &str) -> String {
    format!("sess:{token_hash}")
}

pub fn platform_str(p: Platform) -> &'static str {
    match p {
        Platform::Windows => "windows",
        Platform::Linux => "linux",
        Platform::Macos => "macos",
        Platform::Android => "android",
        Platform::Ios => "ios",
        Platform::Web => "web",
        Platform::Cli => "cli",
    }
}

pub fn parse_platform(s: &str) -> Platform {
    match s {
        "windows" => Platform::Windows,
        "linux" => Platform::Linux,
        "macos" => Platform::Macos,
        "android" => Platform::Android,
        "ios" => Platform::Ios,
        "cli" => Platform::Cli,
        _ => Platform::Web,
    }
}

/// Find an existing device for this user by client id, or create a new one.
/// Returns `(device_id, is_new)`.
pub async fn upsert_device(
    db: &PgPool,
    user_id: Uuid,
    info: &DeviceInfo,
    ip: Option<&str>,
) -> ApiResult<(Uuid, bool)> {
    if let Some(cid) = info.client_device_id {
        let existing: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM devices WHERE id = $1 AND user_id = $2")
                .bind(cid)
                .bind(user_id)
                .fetch_optional(db)
                .await?;
        if let Some((id,)) = existing {
            sqlx::query(
                "UPDATE devices SET name = $2, platform = $3, app_version = $4, last_seen_at = now(), last_ip = $5 WHERE id = $1",
            )
            .bind(id)
            .bind(&info.name)
            .bind(platform_str(info.platform))
            .bind(&info.app_version)
            .bind(ip)
            .execute(db)
            .await?;
            return Ok((id, false));
        }
    }
    let id = info.client_device_id.unwrap_or_else(Uuid::new_v4);
    sqlx::query(
        "INSERT INTO devices (id, user_id, name, platform, app_version, last_ip) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(user_id)
    .bind(&info.name)
    .bind(platform_str(info.platform))
    .bind(&info.app_version)
    .bind(ip)
    .execute(db)
    .await?;
    Ok((id, true))
}

/// Create a session for an (already approved) device. Returns the bearer token.
pub async fn issue(
    state: &AppState,
    user_id: Uuid,
    device_id: Uuid,
) -> ApiResult<(String, DateTime<Utc>)> {
    let settings = state.settings().await?;
    let token = random_token();
    let expires_at = Utc::now() + chrono::Duration::days(settings.session_ttl_days.max(1) as i64);
    sqlx::query(
        "INSERT INTO sessions (id, user_id, device_id, token_hash, expires_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(device_id)
    .bind(hash_token(&token))
    .bind(expires_at)
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE devices SET approved_at = COALESCE(approved_at, now()), last_seen_at = now() WHERE id = $1")
        .bind(device_id)
        .execute(&state.db)
        .await?;
    Ok((token, expires_at))
}

pub async fn validate(state: &AppState, token: &str) -> ApiResult<SessionInfo> {
    let h = hash_token(token);
    let key = cache_key(&h);
    if let Some(info) = state.cache.get_json::<SessionInfo>(&key).await? {
        if info.expires_at > Utc::now() {
            return Ok(info);
        }
    }
    let row: Option<(Uuid, Uuid, Uuid, DateTime<Utc>, bool, bool, bool, DateTime<Utc>)> = sqlx::query_as(
        "SELECT s.id, s.user_id, s.device_id, s.expires_at, u.is_admin, u.disabled, u.email_verified, s.last_used_at
         FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1 AND s.revoked_at IS NULL AND s.expires_at > now()",
    )
    .bind(&h)
    .fetch_optional(&state.db)
    .await?;
    let Some((
        session_id,
        user_id,
        device_id,
        expires_at,
        is_admin,
        disabled,
        email_verified,
        last_used_at,
    )) = row
    else {
        return Err(Error::unauthorized());
    };
    let info = SessionInfo {
        session_id,
        user_id,
        device_id,
        expires_at,
        is_admin,
        disabled,
        email_verified,
        last_used_at,
    };
    state.cache.set_json(&key, &info, CACHE_TTL).await?;
    Ok(info)
}

/// Sliding expiry + last-seen bookkeeping, at most every 5 minutes per session.
pub async fn touch(state: &AppState, info: &SessionInfo, ip: Option<&str>) -> ApiResult<()> {
    if Utc::now() - info.last_used_at < chrono::Duration::minutes(5) {
        return Ok(());
    }
    let settings = state.settings().await?;
    let new_exp = Utc::now() + chrono::Duration::days(settings.session_ttl_days.max(1) as i64);
    sqlx::query("UPDATE sessions SET last_used_at = now(), expires_at = $2 WHERE id = $1")
        .bind(info.session_id)
        .bind(new_exp)
        .execute(&state.db)
        .await?;
    sqlx::query(
        "UPDATE devices SET last_seen_at = now(), last_ip = COALESCE($2, last_ip) WHERE id = $1",
    )
    .bind(info.device_id)
    .bind(ip)
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE users SET last_seen_at = now() WHERE id = $1")
        .bind(info.user_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn revoke_session(state: &AppState, session_id: Uuid) -> ApiResult<()> {
    let row: Option<(String, Uuid)> = sqlx::query_as(
        "UPDATE sessions SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL RETURNING token_hash, user_id",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await?;
    if let Some((h, user_id)) = row {
        state.cache.del(&cache_key(&h)).await?;
        events::publish(
            state,
            events::Event::SessionRevoked {
                user_id,
                session_id,
            },
        )
        .await?;
    }
    Ok(())
}

/// Revoke all sessions of a device.
pub async fn revoke_device(state: &AppState, user_id: Uuid, device_id: Uuid) -> ApiResult<()> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "UPDATE sessions SET revoked_at = now() WHERE device_id = $1 AND user_id = $2 AND revoked_at IS NULL RETURNING id, token_hash",
    )
    .bind(device_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    for (session_id, h) in rows {
        state.cache.del(&cache_key(&h)).await?;
        events::publish(
            state,
            events::Event::SessionRevoked {
                user_id,
                session_id,
            },
        )
        .await?;
    }
    Ok(())
}

/// Revoke every session of the user except `keep`.
pub async fn revoke_all(state: &AppState, user_id: Uuid, keep: Option<Uuid>) -> ApiResult<()> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "UPDATE sessions SET revoked_at = now()
         WHERE user_id = $1 AND revoked_at IS NULL AND ($2::uuid IS NULL OR id <> $2)
         RETURNING id, token_hash",
    )
    .bind(user_id)
    .bind(keep)
    .fetch_all(&state.db)
    .await?;
    for (session_id, h) in rows {
        state.cache.del(&cache_key(&h)).await?;
        events::publish(
            state,
            events::Event::SessionRevoked {
                user_id,
                session_id,
            },
        )
        .await?;
    }
    Ok(())
}

pub async fn is_revoked(state: &AppState, session_id: Uuid) -> ApiResult<bool> {
    let row: Option<(bool,)> = sqlx::query_as(
        "SELECT revoked_at IS NOT NULL OR expires_at <= now() FROM sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|(r,)| r).unwrap_or(true))
}

/// Invalidate the cached copy (e.g. after the user was disabled / promoted).
pub async fn invalidate_user_cache(state: &AppState, user_id: Uuid) -> ApiResult<()> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT token_hash FROM sessions WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
    for (h,) in rows {
        state.cache.del(&cache_key(&h)).await?;
    }
    Ok(())
}
