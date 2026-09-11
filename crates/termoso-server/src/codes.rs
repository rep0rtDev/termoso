//! Short-lived one-time codes (email verification, device approval, email MFA)
//! and opaque flow tokens, stored in Redis with constant-time verification and
//! a bounded number of guesses.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{ApiResult, Error};
use crate::ratelimit;
use crate::state::AppState;
use crate::util::{hash_token, normalize_code, numeric_code, random_token};

#[derive(Serialize, Deserialize)]
struct Stored<T> {
    code_hash: String,
    payload: T,
}

fn key(purpose: &str, token: &str) -> String {
    format!("code:{purpose}:{token}")
}

/// Create a 6-digit code bound to `payload`. Returns `(token, code)`.
pub async fn issue<T: Serialize>(
    state: &AppState,
    purpose: &str,
    payload: &T,
    ttl: Duration,
) -> ApiResult<(String, String)> {
    let token = random_token();
    let code = issue_for(state, purpose, &token, payload, ttl).await?;
    Ok((token, code))
}

/// Create a 6-digit code under a caller-chosen token (e.g. a user id).
pub async fn issue_for<T: Serialize>(
    state: &AppState,
    purpose: &str,
    token: &str,
    payload: &T,
    ttl: Duration,
) -> ApiResult<String> {
    let code = numeric_code(6);
    state
        .cache
        .set_json(
            &key(purpose, token),
            &Stored {
                code_hash: hash_token(&code),
                payload,
            },
            ttl,
        )
        .await?;
    Ok(code)
}

/// Re-issue a fresh code for an existing token, keeping the payload.
pub async fn reissue<T: Serialize + DeserializeOwned>(
    state: &AppState,
    purpose: &str,
    token: &str,
    ttl: Duration,
) -> ApiResult<(T, String)> {
    let k = key(purpose, token);
    let stored: Stored<T> = state
        .cache
        .get_json(&k)
        .await?
        .ok_or_else(Error::token_expired)?;
    let code = numeric_code(6);
    state
        .cache
        .set_json(
            &k,
            &Stored {
                code_hash: hash_token(&code),
                payload: &stored.payload,
            },
            ttl,
        )
        .await?;
    Ok((stored.payload, code))
}

/// Peek at the payload without consuming it.
pub async fn peek<T: DeserializeOwned>(
    state: &AppState,
    purpose: &str,
    token: &str,
) -> ApiResult<T> {
    let stored: Stored<T> = state
        .cache
        .get_json(&key(purpose, token))
        .await?
        .ok_or_else(Error::token_expired)?;
    Ok(stored.payload)
}

/// Verify a user-supplied code. On success the token is consumed.
pub async fn verify<T: DeserializeOwned>(
    state: &AppState,
    purpose: &str,
    token: &str,
    code: &str,
) -> ApiResult<T> {
    ratelimit::check(
        state,
        ratelimit::CODE_GUESS,
        &format!("{purpose}:{}", hash_token(token)),
    )
    .await?;
    let k = key(purpose, token);
    let stored: Stored<T> = state
        .cache
        .get_json(&k)
        .await?
        .ok_or_else(Error::token_expired)?;
    if !crate::util::constant_time_eq(&stored.code_hash, &hash_token(&normalize_code(code))) {
        return Err(Error::invalid_code());
    }
    state.cache.del(&k).await?;
    Ok(stored.payload)
}

/// Opaque single-use flow token (no code) — e.g. OPAQUE login state.
pub async fn put_flow<T: Serialize>(
    state: &AppState,
    purpose: &str,
    payload: &T,
    ttl: Duration,
) -> ApiResult<String> {
    let token = random_token();
    state
        .cache
        .set_json(&key(purpose, &token), payload, ttl)
        .await?;
    Ok(token)
}

pub async fn take_flow<T: DeserializeOwned>(
    state: &AppState,
    purpose: &str,
    token: &str,
) -> ApiResult<T> {
    state
        .cache
        .take_json(&key(purpose, token))
        .await?
        .ok_or_else(Error::token_expired)
}

pub async fn get_flow<T: DeserializeOwned>(
    state: &AppState,
    purpose: &str,
    token: &str,
) -> ApiResult<T> {
    state
        .cache
        .get_json(&key(purpose, token))
        .await?
        .ok_or_else(Error::token_expired)
}

pub async fn set_flow<T: Serialize>(
    state: &AppState,
    purpose: &str,
    token: &str,
    payload: &T,
    ttl: Duration,
) -> ApiResult<()> {
    state
        .cache
        .set_json(&key(purpose, token), payload, ttl)
        .await
}

pub async fn del_flow(state: &AppState, purpose: &str, token: &str) -> ApiResult<()> {
    state.cache.del(&key(purpose, token)).await
}
