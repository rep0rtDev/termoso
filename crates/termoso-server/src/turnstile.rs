//! Cloudflare Turnstile verification. Enabled only when both keys are
//! configured; without them every check is a no-op, so self-hosters are
//! not forced to use it.

use std::sync::OnceLock;
use std::time::Duration;

use axum::http::StatusCode;
use serde::Deserialize;

use crate::error::{ApiResult, Error};
use crate::state::AppState;

const VERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

/// Login attempts from one IP that may fail before a captcha is demanded.
pub const LOGIN_FAILURES_BEFORE_CAPTCHA: u64 = 3;
/// Window for counting those failures.
pub const LOGIN_FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .expect("reqwest client")
    })
}

#[derive(Deserialize)]
struct Verdict {
    success: bool,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

/// Whether the server demands captcha tokens at all.
pub fn enabled(state: &AppState) -> bool {
    state.cfg.turnstile.is_some()
}

/// Require a valid Turnstile token when Turnstile is configured. `ip` is
/// forwarded to Cloudflare so the token is bound to the visitor.
pub async fn require(state: &AppState, token: Option<&str>, ip: Option<&str>) -> ApiResult<()> {
    let Some(cfg) = &state.cfg.turnstile else {
        return Ok(());
    };
    let token = token
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| {
            Error::new(
                StatusCode::BAD_REQUEST,
                termoso_proto::error::codes::TURNSTILE_REQUIRED,
                "Please complete the captcha",
            )
        })?;
    let mut form = vec![("secret", cfg.secret_key.as_str()), ("response", token)];
    if let Some(ip) = ip {
        form.push(("remoteip", ip));
    }
    let verdict: Verdict = client()
        .post(VERIFY_URL)
        .form(&form)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| {
            tracing::warn!("turnstile: siteverify unreachable: {e}");
            Error::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "turnstile_unavailable",
                "Captcha service is unavailable, try again later",
            )
        })?
        .json()
        .await
        .map_err(|e| {
            tracing::warn!("turnstile: bad siteverify response: {e}");
            Error::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "turnstile_unavailable",
                "Captcha service is unavailable, try again later",
            )
        })?;
    if verdict.success {
        return Ok(());
    }
    tracing::debug!("turnstile: rejected: {:?}", verdict.error_codes);
    Err(Error::new(
        StatusCode::BAD_REQUEST,
        "turnstile_failed",
        "Captcha check failed, please try again",
    ))
}

fn login_key(ip: &str) -> String {
    format!("turnstile:login:{ip}")
}

/// Count a failed password login from `ip`.
pub async fn note_login_failure(state: &AppState, ip: Option<&str>) {
    if !enabled(state) {
        return;
    }
    if let Some(ip) = ip
        && let Err(e) = state
            .cache
            .incr_window(&login_key(ip), LOGIN_FAILURE_WINDOW)
            .await
    {
        tracing::debug!("turnstile: failure counter: {e}");
    }
}

/// A login from `ip` needs a captcha once enough recent attempts failed.
pub async fn login_needs_captcha(state: &AppState, ip: Option<&str>) -> bool {
    if !enabled(state) {
        return false;
    }
    let Some(ip) = ip else { return false };
    match state.cache.counter(&login_key(ip)).await {
        Ok(n) => n >= LOGIN_FAILURES_BEFORE_CAPTCHA,
        Err(e) => {
            tracing::debug!("turnstile: failure counter: {e}");
            false
        }
    }
}
