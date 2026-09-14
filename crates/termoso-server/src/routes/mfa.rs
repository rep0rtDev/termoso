//! Second-factor management: TOTP, backup codes, WebAuthn credentials.

use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use termoso_proto::auth::*;
use uuid::Uuid;
use webauthn_rs::prelude::{
    CreationChallengeResponse, PasskeyRegistration, RegisterPublicKeyCredential,
};

use crate::codes;
use crate::error::{ApiResult, Error, NoContent};
use crate::events::{self, Event};
use crate::extract::{Auth, Json as Body, StepUp};
use crate::routes::auth::{check_totp, load_passkeys, totp_for};
use crate::state::AppState;
use crate::users;
use crate::util::{backup_code, hash_token};

const P_TOTP_SETUP: &str = "totp_setup";
const P_WEBAUTHN_REG: &str = "webauthn_reg";
const SETUP_TTL: Duration = Duration::from_secs(600);
const BACKUP_CODE_COUNT: usize = 10;

#[utoipa::path(get, path = "/api/v1/account/mfa", tag = "mfa", responses((status = 200, body = MfaStatus)))]
pub async fn status(State(state): State<AppState>, auth: Auth) -> ApiResult<Json<MfaStatus>> {
    let u = users::by_id(&state.db, auth.user_id()).await?;
    let creds: Vec<(Uuid, String, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, name, created_at, last_used_at FROM webauthn_credentials WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(u.id)
    .fetch_all(&state.db)
    .await?;
    let (remaining,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM backup_codes WHERE user_id = $1 AND used_at IS NULL")
            .bind(u.id)
            .fetch_one(&state.db)
            .await?;
    Ok(Json(MfaStatus {
        totp_enabled: u.totp_enabled,
        webauthn_credentials: creds
            .into_iter()
            .map(
                |(id, name, created_at, last_used_at)| WebauthnCredentialInfo {
                    id,
                    name,
                    created_at,
                    last_used_at,
                },
            )
            .collect(),
        backup_codes_remaining: remaining as u32,
    }))
}

// ───────────────────────────── TOTP ─────────────────────────────

#[utoipa::path(post, path = "/api/v1/account/mfa/totp/setup", tag = "mfa", responses((status = 200, body = TotpSetupResponse)))]
pub async fn totp_setup(
    State(state): State<AppState>,
    StepUp(auth): StepUp,
) -> ApiResult<Json<TotpSetupResponse>> {
    let u = users::by_id(&state.db, auth.user_id()).await?;
    if u.totp_enabled {
        return Err(Error::conflict("TOTP is already enabled"));
    }
    let secret = totp_rs::Secret::generate_secret();
    let bytes = secret
        .to_bytes()
        .map_err(|e| Error::Internal(anyhow::anyhow!("totp secret: {e:?}")))?;
    let totp = totp_for(&state, &u, &bytes)?;
    let encrypted = state.encrypt_secret("totp", &bytes)?;
    codes::set_flow(
        &state,
        P_TOTP_SETUP,
        &u.id.to_string(),
        &encrypted,
        SETUP_TTL,
    )
    .await?;
    Ok(Json(TotpSetupResponse {
        secret: secret.to_encoded().to_string(),
        otpauth_url: totp.get_url(),
    }))
}

#[utoipa::path(post, path = "/api/v1/account/mfa/totp/confirm", tag = "mfa",
    request_body = TotpCodeRequest, responses((status = 200, body = BackupCodes)))]
pub async fn totp_confirm(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<TotpCodeRequest>,
) -> ApiResult<Json<BackupCodes>> {
    let mut u = users::by_id(&state.db, auth.user_id()).await?;
    if u.totp_enabled {
        return Err(Error::conflict("TOTP is already enabled"));
    }
    let encrypted: Vec<u8> = codes::get_flow(&state, P_TOTP_SETUP, &u.id.to_string()).await?;
    u.totp_secret = Some(encrypted.clone());
    if !check_totp(&state, &u, &req.code).await? {
        return Err(Error::invalid_mfa());
    }
    let had_mfa = users::mfa_enabled(&state, &u).await?;
    sqlx::query(
        "UPDATE users SET totp_secret = $2, totp_enabled = true, updated_at = now() WHERE id = $1",
    )
    .bind(u.id)
    .bind(&encrypted)
    .execute(&state.db)
    .await?;
    codes::del_flow(&state, P_TOTP_SETUP, &u.id.to_string()).await?;
    users::security_event(
        &state,
        u.id,
        "totp_enabled",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        None,
    )
    .await?;
    let codes = if had_mfa {
        Vec::new()
    } else {
        regenerate_backup_codes(&state, u.id).await?
    };
    events::publish(&state, Event::AccountUpdated { user_id: u.id }).await?;
    Ok(Json(BackupCodes { codes }))
}

#[utoipa::path(delete, path = "/api/v1/account/mfa/totp", tag = "mfa",
    request_body = TotpCodeRequest, responses((status = 204)))]
pub async fn totp_disable(
    State(state): State<AppState>,
    StepUp(auth): StepUp,
    Body(req): Body<TotpCodeRequest>,
) -> ApiResult<NoContent> {
    let u = users::by_id(&state.db, auth.user_id()).await?;
    if !u.totp_enabled {
        return Ok(NoContent);
    }
    let ok = check_totp(&state, &u, &req.code).await?
        || crate::routes::auth::consume_backup_code(&state, u.id, &req.code).await?;
    if !ok {
        return Err(Error::invalid_mfa());
    }
    sqlx::query("UPDATE users SET totp_secret = NULL, totp_enabled = false, updated_at = now() WHERE id = $1")
        .bind(u.id)
        .execute(&state.db)
        .await?;
    cleanup_backup_codes_if_no_mfa(&state, u.id).await?;
    users::security_event(
        &state,
        u.id,
        "totp_disabled",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        None,
    )
    .await?;
    users::notify(
        &state,
        &u.email,
        "authenticator app removed",
        "Two-factor authentication with an authenticator app was turned off for your account.",
    )
    .await;
    events::publish(&state, Event::AccountUpdated { user_id: u.id })
        .await
        .map(NoContent::from)
}

// ───────────────────────────── backup codes ─────────────────────────────

pub async fn regenerate_backup_codes(state: &AppState, user_id: Uuid) -> ApiResult<Vec<String>> {
    let codes: Vec<String> = (0..BACKUP_CODE_COUNT).map(|_| backup_code()).collect();
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM backup_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for c in &codes {
        sqlx::query("INSERT INTO backup_codes (user_id, code_hash) VALUES ($1, $2)")
            .bind(user_id)
            .bind(hash_token(&crate::util::normalize_code(c)))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(codes)
}

async fn cleanup_backup_codes_if_no_mfa(state: &AppState, user_id: Uuid) -> ApiResult<()> {
    let u = users::by_id(&state.db, user_id).await?;
    if !users::mfa_enabled(state, &u).await? {
        sqlx::query("DELETE FROM backup_codes WHERE user_id = $1")
            .bind(user_id)
            .execute(&state.db)
            .await?;
    }
    Ok(())
}

#[utoipa::path(post, path = "/api/v1/account/mfa/backup-codes", tag = "mfa", responses((status = 200, body = BackupCodes)))]
pub async fn backup_codes(
    State(state): State<AppState>,
    StepUp(auth): StepUp,
) -> ApiResult<Json<BackupCodes>> {
    let u = users::by_id(&state.db, auth.user_id()).await?;
    if !users::mfa_enabled(&state, &u).await? {
        return Err(Error::bad_request("Enable a second factor first"));
    }
    let codes = regenerate_backup_codes(&state, u.id).await?;
    users::security_event(
        &state,
        u.id,
        "backup_codes_regenerated",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        None,
    )
    .await?;
    users::notify(
        &state,
        &u.email,
        "backup codes regenerated",
        "New two-factor backup codes were generated for your account. The previous codes no longer work.",
    )
    .await;
    Ok(Json(BackupCodes { codes }))
}

// ───────────────────────────── WebAuthn ─────────────────────────────

#[utoipa::path(post, path = "/api/v1/account/mfa/webauthn/register/start", tag = "mfa",
    responses((status = 200, body = serde_json::Value)))]
pub async fn webauthn_register_start(
    State(state): State<AppState>,
    StepUp(auth): StepUp,
) -> ApiResult<Json<CreationChallengeResponse>> {
    let webauthn = state
        .webauthn
        .as_ref()
        .ok_or_else(|| Error::feature_disabled("WebAuthn"))?;
    let u = users::by_id(&state.db, auth.user_id()).await?;
    let existing: Vec<_> = load_passkeys(&state, u.id)
        .await?
        .into_iter()
        .map(|(_, k)| k.cred_id().clone())
        .collect();
    let display = u.display_name.clone().unwrap_or_else(|| u.email.clone());
    let (ccr, reg_state) = webauthn
        .start_passkey_registration(u.id, &u.email, &display, Some(existing))
        .map_err(|e| Error::Internal(anyhow::anyhow!("webauthn: {e}")))?;
    codes::set_flow(
        &state,
        P_WEBAUTHN_REG,
        &u.id.to_string(),
        &reg_state,
        SETUP_TTL,
    )
    .await?;
    Ok(Json(ccr))
}

#[utoipa::path(post, path = "/api/v1/account/mfa/webauthn/register/finish", tag = "mfa",
    request_body = WebauthnRegisterFinishRequest, responses((status = 200, body = WebauthnCredentialInfo)))]
pub async fn webauthn_register_finish(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<WebauthnRegisterFinishRequest>,
) -> ApiResult<Json<WebauthnCredentialInfo>> {
    let webauthn = state
        .webauthn
        .as_ref()
        .ok_or_else(|| Error::feature_disabled("WebAuthn"))?;
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(Error::bad_request("Invalid credential name"));
    }
    let u = users::by_id(&state.db, auth.user_id()).await?;
    let reg_state: PasskeyRegistration =
        codes::take_flow(&state, P_WEBAUTHN_REG, &u.id.to_string()).await?;
    let cred: RegisterPublicKeyCredential = serde_json::from_value(req.credential)
        .map_err(|e| Error::bad_request(format!("invalid credential: {e}")))?;
    let passkey = webauthn
        .finish_passkey_registration(&cred, &reg_state)
        .map_err(|e| Error::bad_request(format!("WebAuthn registration rejected: {e}")))?;
    let had_mfa = users::mfa_enabled(&state, &u).await?;
    let id = Uuid::new_v4();
    let (created_at,): (DateTime<Utc>,) = sqlx::query_as(
        "INSERT INTO webauthn_credentials (id, user_id, name, credential) VALUES ($1, $2, $3, $4) RETURNING created_at",
    )
    .bind(id)
    .bind(u.id)
    .bind(name)
    .bind(serde_json::to_value(&passkey)?)
    .fetch_one(&state.db)
    .await?;
    if !had_mfa {
        regenerate_backup_codes(&state, u.id).await?;
    }
    users::security_event(
        &state,
        u.id,
        "webauthn_added",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "name": name })),
    )
    .await?;
    events::publish(&state, Event::AccountUpdated { user_id: u.id }).await?;
    Ok(Json(WebauthnCredentialInfo {
        id,
        name: name.to_string(),
        created_at,
        last_used_at: None,
    }))
}

#[utoipa::path(delete, path = "/api/v1/account/mfa/webauthn/{id}", tag = "mfa",
    params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn webauthn_delete(
    State(state): State<AppState>,
    StepUp(auth): StepUp,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    let row: Option<(String,)> = sqlx::query_as(
        "DELETE FROM webauthn_credentials WHERE id = $1 AND user_id = $2 RETURNING name",
    )
    .bind(id)
    .bind(auth.user_id())
    .fetch_optional(&state.db)
    .await?;
    let Some((name,)) = row else {
        return Err(Error::not_found("Credential"));
    };
    cleanup_backup_codes_if_no_mfa(&state, auth.user_id()).await?;
    users::security_event(
        &state,
        auth.user_id(),
        "webauthn_removed",
        Some(auth.device_id()),
        auth.ip.as_deref(),
        auth.user_agent.as_deref(),
        Some(serde_json::json!({ "name": name })),
    )
    .await?;
    let u = users::by_id(&state.db, auth.user_id()).await?;
    users::notify(
        &state,
        &u.email,
        "security key removed",
        &format!("The security key \"{name}\" was removed from your account."),
    )
    .await;
    events::publish(
        &state,
        Event::AccountUpdated {
            user_id: auth.user_id(),
        },
    )
    .await
    .map(NoContent::from)
}
