//! Redis-backed fixed-window rate limits shared across instances.

use std::time::Duration;

use crate::error::{ApiResult, Error};
use crate::state::AppState;

#[derive(Clone, Copy)]
pub struct Limit {
    pub name: &'static str,
    pub max: u64,
    pub window: Duration,
}

/// Login / registration attempts per IP.
pub const AUTH_IP: Limit = Limit {
    name: "auth_ip",
    max: 30,
    window: Duration::from_secs(60),
};
/// Login attempts per account (email).
pub const AUTH_ACCOUNT: Limit = Limit {
    name: "auth_acct",
    max: 10,
    window: Duration::from_secs(300),
};
/// Emails sent per address.
pub const EMAIL: Limit = Limit {
    name: "email",
    max: 5,
    window: Duration::from_secs(3600),
};
/// Guesses of a short code per token (MFA/approval/verification).
pub const CODE_GUESS: Limit = Limit {
    name: "code",
    max: 6,
    window: Duration::from_secs(900),
};
/// General API calls per user.
pub const API_USER: Limit = Limit {
    name: "api",
    max: 1200,
    window: Duration::from_secs(60),
};
/// Sync pushes per user.
pub const SYNC_PUSH: Limit = Limit {
    name: "push",
    max: 120,
    window: Duration::from_secs(60),
};

pub async fn check(state: &AppState, limit: Limit, subject: &str) -> ApiResult<()> {
    let key = format!("rl:{}:{}", limit.name, subject);
    let (count, ttl) = state.cache.incr_window(&key, limit.window).await?;
    if count > limit.max {
        metrics::counter!("termoso_rate_limited_total", "limit" => limit.name).increment(1);
        return Err(Error::rate_limited(ttl.max(1)));
    }
    Ok(())
}

pub async fn check_ip(state: &AppState, limit: Limit, ip: Option<&str>) -> ApiResult<()> {
    check(state, limit, ip.unwrap_or("unknown")).await
}
