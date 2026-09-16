//! AI command suggestions: per-account opt-in, daily quota, one relayed
//! request. See `termoso_proto::ai` for what does (and does not) travel.

use axum::Json;
use axum::extract::State;
use chrono::{Duration as ChronoDuration, Utc};
use termoso_proto::ai::{AiCommandRequest, AiCommandResponse, AiSettingsRequest, AiStatus};
use uuid::Uuid;

use crate::ai::Ai;
use crate::error::{ApiResult, Error};
use crate::extract::{Auth, Json as Body};
use crate::ratelimit;
use crate::state::AppState;

/// Longest OS / shell label a client may attach.
const MAX_CONTEXT_CHARS: usize = 80;

fn quota_key(user_id: Uuid) -> String {
    format!("ai:quota:{user_id}:{}", Utc::now().format("%Y%m%d"))
}

/// Seconds until the next UTC midnight (when the daily counter resets).
fn secs_until_midnight() -> u64 {
    let now = Utc::now();
    let tomorrow = (now + ChronoDuration::days(1))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight")
        .and_utc();
    (tomorrow - now).num_seconds().max(1) as u64
}

async fn is_enabled(state: &AppState, user_id: Uuid) -> ApiResult<bool> {
    let (on,): (bool,) = sqlx::query_as("SELECT ai_enabled FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    Ok(on)
}

async fn status_for(state: &AppState, user_id: Uuid, enabled: bool) -> ApiResult<AiStatus> {
    let Some(ai) = &state.ai else {
        return Ok(AiStatus {
            available: false,
            enabled,
            provider: None,
            model: None,
            confidential: false,
            daily_quota: 0,
            used_today: 0,
        });
    };
    let cfg = ai.cfg();
    let used = state.cache.counter(&quota_key(user_id)).await?;
    Ok(AiStatus {
        available: true,
        enabled,
        provider: Some(cfg.provider.clone()),
        model: Some(cfg.model.clone()),
        confidential: cfg.confidential,
        daily_quota: cfg.daily_quota,
        used_today: used.min(u64::from(cfg.daily_quota)) as u32,
    })
}

#[utoipa::path(get, path = "/api/v1/account/ai", tag = "ai",
    responses((status = 200, body = AiStatus)))]
pub async fn status(State(state): State<AppState>, auth: Auth) -> ApiResult<Json<AiStatus>> {
    let enabled = is_enabled(&state, auth.user_id()).await?;
    Ok(Json(status_for(&state, auth.user_id(), enabled).await?))
}

#[utoipa::path(put, path = "/api/v1/account/ai", tag = "ai",
    request_body = AiSettingsRequest, responses((status = 200, body = AiStatus)))]
pub async fn put_settings(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<AiSettingsRequest>,
) -> ApiResult<Json<AiStatus>> {
    sqlx::query("UPDATE users SET ai_enabled = $2, updated_at = now() WHERE id = $1")
        .bind(auth.user_id())
        .bind(req.enabled)
        .execute(&state.db)
        .await?;
    Ok(Json(status_for(&state, auth.user_id(), req.enabled).await?))
}

fn context_label(value: Option<String>, what: &str) -> ApiResult<Option<String>> {
    let Some(v) = value else {
        return Ok(None);
    };
    let v = v.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if v.chars().count() > MAX_CONTEXT_CHARS || v.chars().any(char::is_control) {
        return Err(Error::bad_request(format!("{what} label is too long")));
    }
    Ok(Some(v.to_string()))
}

#[utoipa::path(post, path = "/api/v1/ai/command", tag = "ai",
    request_body = AiCommandRequest,
    responses(
        (status = 200, body = AiCommandResponse),
        (status = 403, description = "ai_not_enabled: the account has not opted in"),
        (status = 429, description = "ai_quota_exceeded: daily quota reached"),
        (status = 501, description = "feature_disabled: no model configured"),
        (status = 502, description = "ai_unavailable: provider failed"),
    ))]
pub async fn command(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<AiCommandRequest>,
) -> ApiResult<Json<AiCommandResponse>> {
    let ai: &Ai = state
        .ai
        .as_ref()
        .ok_or_else(|| Error::feature_disabled("AI suggestions"))?;
    if !is_enabled(&state, auth.user_id()).await? {
        return Err(Error::ai_not_enabled());
    }

    let prompt = req.prompt.trim();
    if prompt.is_empty() {
        return Err(Error::bad_request("Describe what the command should do"));
    }
    if prompt.chars().count() > ai.cfg().max_prompt_chars {
        return Err(Error::bad_request(format!(
            "Request is too long ({} characters max)",
            ai.cfg().max_prompt_chars
        )));
    }
    let os = context_label(req.os, "OS")?;
    let shell = context_label(req.shell, "Shell")?;

    ratelimit::check(&state, ratelimit::AI_USER, &auth.user_id().to_string()).await?;

    let key = quota_key(auth.user_id());
    let (used, _) = state
        .cache
        .incr_window(&key, std::time::Duration::from_secs(secs_until_midnight()))
        .await?;
    let quota = u64::from(ai.cfg().daily_quota);
    if used > quota {
        state.cache.decr(&key).await?;
        return Err(Error::ai_quota_exceeded(secs_until_midnight()));
    }

    let suggestion = match ai.suggest(prompt, os.as_deref(), shell.as_deref()).await {
        Ok(s) => s,
        Err(e) => {
            // The provider did not answer; give the slot back.
            state.cache.decr(&key).await?;
            return Err(e);
        }
    };
    metrics::counter!("termoso_ai_requests_total").increment(1);

    Ok(Json(AiCommandResponse {
        command: suggestion.command,
        explanation: suggestion.explanation,
        remaining_today: quota.saturating_sub(used) as u32,
    }))
}
