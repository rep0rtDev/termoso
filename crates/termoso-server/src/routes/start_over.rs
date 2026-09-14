//! "Start over": the way back into an account whose password *and* recovery
//! phrase are both lost.
//!
//! The server never holds a key that could decrypt the vault, so there is
//! nothing to restore — the old encrypted data is destroyed and a brand-new
//! key set is installed. Because this is irreversible and only proves control
//! of the mailbox, the reset is scheduled with a waiting period: every signed-in
//! device sees it in the profile, the mailbox receives a cancel link, and the
//! reset can only be completed after the delay. Two-factor authentication,
//! when enabled, is still required.
//!
//! Flow: `request` (email) → `confirm` (code [+ 2FA]) → wait → `password/start`
//! + `finish` (finish token from the email) — or `cancel` at any point.

use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use termoso_crypto::opaque;
use termoso_proto::auth::*;
use uuid::Uuid;

use crate::codes;
use crate::error::{ApiResult, Error, NoContent};
use crate::events::{self, Event};
use crate::extract::{Auth, Client, Json as Body};
use crate::ratelimit;
use crate::routes::auth::{
    build_session, check_totp, consume_backup_code, send_code_email, validate_device, validate_keys,
};
use crate::session;
use crate::state::AppState;
use crate::users::{self, UserRow};
use crate::util::{hash_token, mask_email, normalize_email, random_token};

const P_REQUEST: &str = "start_over";
/// The emailed code lives under its own purpose so it does not clobber the
/// flow record stored under the same token.
const P_CODE: &str = "start_over_code";
const REQUEST_TTL: Duration = Duration::from_secs(900);
/// How long the finish/cancel links stay valid after the delay has passed.
const LINK_GRACE: chrono::Duration = chrono::Duration::days(7);

#[derive(Serialize, Deserialize)]
struct RequestFlow {
    /// `None` for an unknown email: the flow exists so the response looks the
    /// same, but no code was sent and `confirm` can never succeed.
    user_id: Option<Uuid>,
    email: String,
}

fn links(state: &AppState, finish: &str, cancel: &str) -> (String, String) {
    let base = state.cfg.web_url().trim_end_matches('/').to_string();
    (
        format!("{base}/start-over/{finish}"),
        format!("{base}/start-over/cancel/{cancel}"),
    )
}

fn delay(state: &AppState) -> chrono::Duration {
    chrono::Duration::seconds(state.cfg.start_over_delay_secs as i64)
}

async fn by_reset_hash(state: &AppState, column: &str, token: &str) -> ApiResult<UserRow> {
    let id: Option<(Uuid,)> = match column {
        "finish" => {
            sqlx::query_as("SELECT id FROM users WHERE reset_finish_hash = $1")
                .bind(hash_token(token))
                .fetch_optional(&state.db)
                .await?
        }
        _ => {
            sqlx::query_as("SELECT id FROM users WHERE reset_cancel_hash = $1")
                .bind(hash_token(token))
                .fetch_optional(&state.db)
                .await?
        }
    };
    let (id,) = id.ok_or_else(Error::token_expired)?;
    let user = users::by_id(&state.db, id).await?;
    match user.reset_scheduled_for {
        Some(at) if Utc::now() < at + LINK_GRACE => Ok(user),
        _ => Err(Error::token_expired()),
    }
}

// ───────────────────────────── request / confirm ─────────────────────────────

#[utoipa::path(post, path = "/api/v1/auth/start-over/request", tag = "auth",
    request_body = StartOverRequest, responses((status = 200, body = StartOverRequestResponse)))]
pub async fn request(
    State(state): State<AppState>,
    client: Client,
    Body(req): Body<StartOverRequest>,
) -> ApiResult<Json<StartOverRequestResponse>> {
    ratelimit::check_ip(&state, ratelimit::AUTH_IP, client.ip.as_deref()).await?;
    if state.mailer.is_none() {
        return Err(Error::feature_disabled("Email"));
    }
    let email = normalize_email(&req.email).ok_or_else(|| Error::bad_request("Invalid email"))?;
    ratelimit::check(&state, ratelimit::AUTH_ACCOUNT, &email).await?;
    let user = users::by_email(&state.db, &email).await?;
    let user = user.filter(|u| u.email_verified && !u.disabled);
    let flow = RequestFlow {
        user_id: user.as_ref().map(|u| u.id),
        email: email.clone(),
    };
    let request_token = codes::put_flow(&state, P_REQUEST, &flow, REQUEST_TTL).await?;
    if let Some(u) = &user {
        let code = codes::issue_for(&state, P_CODE, &request_token, &u.id, REQUEST_TTL).await?;
        send_code_email(&state, &u.email, "account reset", &code, REQUEST_TTL).await?;
        users::security_event(
            &state,
            u.id,
            "reset_requested",
            None,
            client.ip.as_deref(),
            client.user_agent.as_deref(),
            None,
        )
        .await?;
    }
    Ok(Json(StartOverRequestResponse {
        request_token,
        email_hint: mask_email(&email),
    }))
}

#[utoipa::path(post, path = "/api/v1/auth/start-over/confirm", tag = "auth",
    request_body = StartOverConfirmRequest, responses((status = 200, body = StartOverScheduled)))]
pub async fn confirm(
    State(state): State<AppState>,
    client: Client,
    Body(req): Body<StartOverConfirmRequest>,
) -> ApiResult<Json<StartOverScheduled>> {
    ratelimit::check_ip(&state, ratelimit::AUTH_IP, client.ip.as_deref()).await?;
    let flow: RequestFlow = codes::get_flow(&state, P_REQUEST, &req.request_token).await?;
    let Some(user_id) = flow.user_id else {
        // Unknown account: burn a guess like a real one would.
        ratelimit::check(
            &state,
            ratelimit::CODE_GUESS,
            &format!("{P_CODE}:{}", hash_token(&req.request_token)),
        )
        .await?;
        return Err(Error::invalid_code());
    };
    let user = users::by_id(&state.db, user_id).await?;
    if user.disabled {
        return Err(Error::account_disabled());
    }
    codes::verify::<Uuid>(&state, P_CODE, &req.request_token, &req.code).await?;
    codes::del_flow(&state, P_REQUEST, &req.request_token).await?;
    if users::mfa_enabled(&state, &user).await? {
        let Some(code) = req.mfa_code.as_deref() else {
            return Err(Error::mfa_required());
        };
        // The mailbox code is consumed first so second-factor guesses need a
        // fresh email each time.
        let ok = check_totp(&state, &user, code).await?
            || consume_backup_code(&state, user.id, code).await?;
        if !ok {
            return Err(Error::invalid_mfa());
        }
    }

    let finish = random_token();
    let cancel = random_token();
    // Read the timestamp back so the response matches what every profile
    // fetch will show (Postgres keeps microseconds).
    let (scheduled_for,): (chrono::DateTime<Utc>,) = sqlx::query_as(
        "UPDATE users SET reset_scheduled_for = $2, reset_finish_hash = $3, reset_cancel_hash = $4,
             updated_at = now()
         WHERE id = $1
         RETURNING reset_scheduled_for",
    )
    .bind(user.id)
    .bind(Utc::now() + delay(&state))
    .bind(hash_token(&finish))
    .bind(hash_token(&cancel))
    .fetch_one(&state.db)
    .await?;
    session::invalidate_user_cache(&state, user.id).await?;
    users::security_event(
        &state,
        user.id,
        "reset_scheduled",
        None,
        client.ip.as_deref(),
        client.user_agent.as_deref(),
        Some(serde_json::json!({ "scheduled_for": scheduled_for })),
    )
    .await?;
    events::publish(&state, Event::AccountUpdated { user_id: user.id }).await?;

    let (finish_url, cancel_url) = links(&state, &finish, &cancel);
    let hours = delay(&state).num_hours();
    let text = format!(
        "Someone — hopefully you — asked to reset your {name} account because the password and \
         the recovery phrase are lost.\n\n\
         WHAT THIS DOES: on {when} UTC the reset can be completed. It permanently destroys every \
         host, key, identity, snippet and setting stored in your encrypted vault and signs out every \
         device. {name} does not have a key to decrypt that data, so it cannot be recovered — not \
         by you, not by us.\n\n\
         To go ahead after the waiting period ({hours} h), open:\n    {finish_url}\n\n\
         IF THIS WAS NOT YOU, cancel it now (works from any signed-in device too):\n    {cancel_url}\n\n\
         Nothing changes until the reset is completed.",
        name = state.cfg.server_name,
        when = scheduled_for.format("%Y-%m-%d %H:%M"),
    );
    if let Some(mailer) = &state.mailer {
        mailer
            .send(
                &user.email,
                &format!("{}: account reset scheduled", state.cfg.server_name),
                &text,
            )
            .await
            .map_err(|e| Error::Internal(e.context("sending email")))?;
    }
    Ok(Json(StartOverScheduled {
        scheduled_for,
        email_hint: mask_email(&user.email),
    }))
}

// ───────────────────────────── status / cancel ─────────────────────────────

#[utoipa::path(get, path = "/api/v1/auth/start-over/{token}", tag = "auth",
    params(("token" = String, Path)), responses((status = 200, body = StartOverStatus)))]
pub async fn status(
    State(state): State<AppState>,
    client: Client,
    Path(token): Path<String>,
) -> ApiResult<Json<StartOverStatus>> {
    ratelimit::check_ip(&state, ratelimit::AUTH_IP, client.ip.as_deref()).await?;
    let user = by_reset_hash(&state, "finish", &token).await?;
    let scheduled_for = user.reset_scheduled_for.ok_or_else(Error::token_expired)?;
    Ok(Json(StartOverStatus {
        email_hint: mask_email(&user.email),
        email: user.email.clone(),
        scheduled_for,
        ready: Utc::now() >= scheduled_for,
    }))
}

async fn clear_reset(
    state: &AppState,
    user: &UserRow,
    kind: &str,
    client: &Client,
) -> ApiResult<()> {
    sqlx::query(
        "UPDATE users SET reset_scheduled_for = NULL, reset_finish_hash = NULL,
             reset_cancel_hash = NULL, updated_at = now()
         WHERE id = $1",
    )
    .bind(user.id)
    .execute(&state.db)
    .await?;
    session::invalidate_user_cache(state, user.id).await?;
    users::security_event(
        state,
        user.id,
        kind,
        None,
        client.ip.as_deref(),
        client.user_agent.as_deref(),
        None,
    )
    .await?;
    events::publish(state, Event::AccountUpdated { user_id: user.id }).await?;
    Ok(())
}

#[utoipa::path(post, path = "/api/v1/auth/start-over/cancel", tag = "auth",
    request_body = StartOverCancelRequest, responses((status = 204)))]
pub async fn cancel(
    State(state): State<AppState>,
    client: Client,
    Body(req): Body<StartOverCancelRequest>,
) -> ApiResult<NoContent> {
    ratelimit::check_ip(&state, ratelimit::AUTH_IP, client.ip.as_deref()).await?;
    let user = by_reset_hash(&state, "cancel", &req.cancel_token).await?;
    clear_reset(&state, &user, "reset_cancelled", &client).await?;
    users::notify(
        &state,
        &user.email,
        "account reset cancelled",
        "The scheduled reset of your account was cancelled. Nothing was changed.",
    )
    .await;
    Ok(NoContent)
}

/// `POST /account/start-over/cancel` — any signed-in device can call off a
/// pending reset; no step-up needed since cancelling is always safe.
#[utoipa::path(post, path = "/api/v1/account/start-over/cancel", tag = "account", responses((status = 204)))]
pub async fn cancel_authenticated(
    State(state): State<AppState>,
    auth: Auth,
    client: Client,
) -> ApiResult<NoContent> {
    let user = users::by_id(&state.db, auth.user_id()).await?;
    if user.reset_scheduled_for.is_none() {
        return Ok(NoContent);
    }
    clear_reset(&state, &user, "reset_cancelled", &client).await?;
    users::notify(
        &state,
        &user.email,
        "account reset cancelled",
        "The scheduled reset of your account was cancelled from a signed-in device. Nothing was changed.",
    )
    .await;
    Ok(NoContent)
}

// ───────────────────────────── finish ─────────────────────────────

async fn ready_user(state: &AppState, token: &str) -> ApiResult<UserRow> {
    let user = by_reset_hash(state, "finish", token).await?;
    match user.reset_scheduled_for {
        Some(at) if Utc::now() >= at => Ok(user),
        Some(at) => Err(Error::new(
            axum::http::StatusCode::CONFLICT,
            termoso_proto::error::codes::CONFLICT,
            format!(
                "The reset can be completed after {} UTC",
                at.format("%Y-%m-%d %H:%M")
            ),
        )
        .with_details(serde_json::json!({ "scheduled_for": at }))),
        None => Err(Error::token_expired()),
    }
}

#[utoipa::path(post, path = "/api/v1/auth/start-over/password/start", tag = "auth",
    request_body = StartOverPasswordStartRequest, responses((status = 200, body = PasswordSetupStartResponse)))]
pub async fn password_start(
    State(state): State<AppState>,
    client: Client,
    Body(req): Body<StartOverPasswordStartRequest>,
) -> ApiResult<Json<PasswordSetupStartResponse>> {
    ratelimit::check_ip(&state, ratelimit::AUTH_IP, client.ip.as_deref()).await?;
    let user = ready_user(&state, &req.token).await?;
    let opaque_response = state
        .opaque
        .registration_start(&user.email, &req.opaque_request)?;
    Ok(Json(PasswordSetupStartResponse { opaque_response }))
}

#[utoipa::path(post, path = "/api/v1/auth/start-over/finish", tag = "auth",
    request_body = StartOverFinishRequest, responses((status = 200, body = AuthResponse)))]
pub async fn finish(
    State(state): State<AppState>,
    client: Client,
    Body(req): Body<StartOverFinishRequest>,
) -> ApiResult<Json<AuthResponse>> {
    ratelimit::check_ip(&state, ratelimit::AUTH_IP, client.ip.as_deref()).await?;
    let user = ready_user(&state, &req.token).await?;
    validate_keys(&req.keys)?;
    validate_device(&req.device)?;
    let record = opaque::Server::registration_finish(&req.opaque_upload)?;

    // Everyone who shared a vault with this user must re-issue its key to the
    // new public key (and learns that the old one is gone).
    let team_peers: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT vm2.user_id FROM vault_members vm
         JOIN vault_members vm2 ON vm2.vault_id = vm.vault_id
         WHERE vm.user_id = $1 AND vm2.user_id <> $1",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    let log_keys: Vec<(String,)> =
        sqlx::query_as("SELECT object_key FROM session_logs WHERE user_id = $1")
            .bind(user.id)
            .fetch_all(&state.db)
            .await?;

    // Sign everything out first so the cache entries go with the rows.
    session::revoke_all(&state, user.id, None).await?;

    let mut tx = state.db.begin().await?;
    let res = sqlx::query(
        "UPDATE users SET opaque_record = $2, public_key = $3, wrapped_private_key = $4,
             recovery_wrapped_private_key = $5, recovery_verifier_hash = $6,
             key_version = key_version + 1,
             reset_scheduled_for = NULL, reset_finish_hash = NULL, reset_cancel_hash = NULL,
             updated_at = now()
         WHERE id = $1 AND reset_finish_hash = $7",
    )
    .bind(user.id)
    .bind(&record)
    .bind(&req.keys.public_key)
    .bind(&req.keys.wrapped_private_key)
    .bind(&req.keys.recovery_wrapped_private_key)
    .bind(hash_token(&req.keys.recovery_verifier))
    .bind(hash_token(&req.token))
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::token_expired());
    }
    // Old encrypted material: personal vault (entities, logs), history,
    // settings, devices (sessions, device SSH keys), published SSH ID keys,
    // live sessions. Team memberships stay; their vault keys become pending.
    for sql in [
        "DELETE FROM vaults WHERE owner_id = $1 AND kind = 'personal'",
        "DELETE FROM history_entries WHERE user_id = $1",
        "DELETE FROM session_logs WHERE user_id = $1",
        "DELETE FROM user_settings WHERE user_id = $1",
        "DELETE FROM live_sessions WHERE host_user_id = $1",
        "DELETE FROM ssh_id_keys WHERE user_id = $1",
        "DELETE FROM devices WHERE user_id = $1",
        "UPDATE vault_members SET sealed_key = NULL WHERE user_id = $1",
    ] {
        sqlx::query(sql).bind(user.id).execute(&mut *tx).await?;
    }
    users::create_personal_vault(&mut tx, user.id, &req.keys.personal_vault_sealed_key).await?;
    tx.commit().await?;

    if let Some(storage) = &state.storage {
        for (k,) in log_keys {
            if let Err(e) = storage.delete(&k).await {
                tracing::warn!(error = %e, key = %k, "could not delete log object");
            }
        }
    }
    let user = users::by_id(&state.db, user.id).await?;
    let (device_id, _) =
        session::upsert_device(&state.db, user.id, &req.device, client.ip.as_deref()).await?;
    let sess = build_session(&state, &user, device_id).await?;
    users::security_event(
        &state,
        user.id,
        "account_reset",
        Some(device_id),
        client.ip.as_deref(),
        client.user_agent.as_deref(),
        None,
    )
    .await?;
    users::notify(
        &state,
        &user.email,
        "account reset completed",
        "Your account was reset: the previous encrypted vault was destroyed, a new password and \
         recovery phrase are in place, and every device was signed out. Team vaults you belong to \
         need their access re-granted by a team admin.",
    )
    .await;
    let peers: Vec<Uuid> = team_peers.into_iter().map(|(id,)| id).collect();
    if !peers.is_empty() {
        events::publish(
            &state,
            Event::VaultsUpdated {
                user_ids: peers.clone(),
            },
        )
        .await?;
        events::publish(&state, Event::TeamsUpdated { user_ids: peers }).await?;
    }
    events::publish(&state, Event::AccountUpdated { user_id: user.id }).await?;
    metrics::counter!("termoso_account_resets_total").increment(1);
    Ok(Json(AuthResponse::Authenticated(sess)))
}
