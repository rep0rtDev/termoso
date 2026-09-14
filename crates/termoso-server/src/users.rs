//! User rows, profile mapping and the per-user security log.

use chrono::{DateTime, Utc};
use sqlx::AssertSqlSafe;
use sqlx::FromRow;
use termoso_proto::account::{AccountKeys, UserProfile};
use uuid::Uuid;

use crate::error::{ApiResult, Error};
use crate::state::AppState;

#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub email_verified: bool,
    pub display_name: Option<String>,
    pub opaque_record: Option<Vec<u8>>,
    pub public_key: String,
    pub wrapped_private_key: String,
    pub recovery_wrapped_private_key: String,
    pub recovery_verifier_hash: String,
    pub key_version: i32,
    pub totp_secret: Option<Vec<u8>>,
    pub totp_enabled: bool,
    pub is_admin: bool,
    pub disabled: bool,
    pub created_at: DateTime<Utc>,
    pub reset_scheduled_for: Option<DateTime<Utc>>,
}

const COLUMNS: &str =
    "id, email, email_verified, display_name, opaque_record, public_key, wrapped_private_key,
    recovery_wrapped_private_key, recovery_verifier_hash, key_version, totp_secret, totp_enabled,
    is_admin, disabled, created_at, reset_scheduled_for";

pub async fn by_id<'e, E>(db: E, id: Uuid) -> ApiResult<UserRow>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as::<_, UserRow>(AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM users WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| Error::not_found("User"))
}

pub async fn by_email<'e, E>(db: E, email: &str) -> ApiResult<Option<UserRow>>
where
    E: sqlx::PgExecutor<'e>,
{
    Ok(sqlx::query_as::<_, UserRow>(AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM users WHERE lower(email) = $1"
    )))
    .bind(email.to_lowercase())
    .fetch_optional(db)
    .await?)
}

/// Whether `user_id` has TOTP or at least one WebAuthn credential enrolled.
pub async fn mfa_enabled_for(state: &AppState, user_id: Uuid) -> ApiResult<bool> {
    let (on,): (bool,) = sqlx::query_as(
        "SELECT u.totp_enabled OR EXISTS (SELECT 1 FROM webauthn_credentials w WHERE w.user_id = u.id)
         FROM users u WHERE u.id = $1",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(on)
}

pub async fn mfa_enabled(state: &AppState, user: &UserRow) -> ApiResult<bool> {
    if user.totp_enabled {
        return Ok(true);
    }
    let (n,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM webauthn_credentials WHERE user_id = $1")
            .bind(user.id)
            .fetch_one(&state.db)
            .await?;
    Ok(n > 0)
}

pub fn profile(u: &UserRow, mfa_enabled: bool) -> UserProfile {
    UserProfile {
        id: u.id,
        email: u.email.clone(),
        email_verified: u.email_verified,
        display_name: u.display_name.clone(),
        created_at: u.created_at,
        is_admin: u.is_admin,
        mfa_enabled,
        reset_scheduled_for: u.reset_scheduled_for,
    }
}

/// Best-effort security notification to the account owner. Plain text, no
/// links to click, nothing to track; failures are logged, never surfaced.
pub async fn notify(state: &AppState, email: &str, subject: &str, text: &str) {
    let Some(mailer) = &state.mailer else {
        return;
    };
    let text = format!("{text}\n\nIf this was not you, sign in and review your security events.");
    if let Err(e) = mailer
        .send(
            email,
            &format!("{}: {subject}", state.cfg.server_name),
            &text,
        )
        .await
    {
        tracing::warn!(error = %e, "could not send security notification");
    }
}

pub fn keys(u: &UserRow) -> AccountKeys {
    AccountKeys {
        public_key: u.public_key.clone(),
        wrapped_private_key: u.wrapped_private_key.clone(),
        key_version: u.key_version,
    }
}

/// Record a security event for the account owner to review.
pub async fn security_event(
    state: &AppState,
    user_id: Uuid,
    kind: &str,
    device_id: Option<Uuid>,
    ip: Option<&str>,
    user_agent: Option<&str>,
    details: Option<serde_json::Value>,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO security_events (user_id, kind, device_id, ip, user_agent, details) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(kind)
    .bind(device_id)
    .bind(ip)
    .bind(user_agent)
    .bind(details)
    .execute(&state.db)
    .await?;
    metrics::counter!("termoso_security_events_total", "kind" => kind.to_string()).increment(1);
    Ok(())
}

/// Create the personal vault for a fresh user inside `tx`.
pub async fn create_personal_vault(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    sealed_key: &str,
) -> ApiResult<Uuid> {
    let vault_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO vaults (id, kind, owner_id, name) VALUES ($1, 'personal', $2, 'Personal')",
    )
    .bind(vault_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO vault_members (vault_id, user_id, role, sealed_key, key_version, added_by) VALUES ($1, $2, 'manager', $3, 1, $2)",
    )
    .bind(vault_id)
    .bind(user_id)
    .bind(sealed_key)
    .execute(&mut **tx)
    .await?;
    Ok(vault_id)
}
