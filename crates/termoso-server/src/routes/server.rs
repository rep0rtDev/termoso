//! Public server information and health probes.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use termoso_proto::account::{ServerFeatures, ServerInfo};

use crate::error::ApiResult;
use crate::state::{AppState, VERSION};

#[utoipa::path(get, path = "/api/v1/server/info", tag = "server",
    responses((status = 200, body = ServerInfo)))]
pub async fn info(State(state): State<AppState>) -> ApiResult<Json<ServerInfo>> {
    let settings = state.settings().await?;
    Ok(Json(ServerInfo {
        name: state.cfg.server_name.clone(),
        version: VERSION.to_string(),
        registration_open: settings.registration_open,
        sso_providers: state.sso.list(),
        features: ServerFeatures {
            session_logs: state.storage.is_some(),
            email: state.mailer.is_some(),
            webauthn: state.webauthn.is_some(),
            teams: settings.users_can_create_teams,
        },
        max_entity_bytes: settings.max_entity_bytes,
        max_log_bytes: settings.max_log_bytes,
        sshid_url: super::sshid::base_url(&state.cfg),
    }))
}

pub async fn healthz() -> &'static str {
    "ok"
}

/// Readiness: database + Redis reachable.
pub async fn readyz(State(state): State<AppState>) -> (StatusCode, &'static str) {
    let db = sqlx::query("SELECT 1").execute(&state.db).await.is_ok();
    let redis = state.cache.ping().await.is_ok();
    if db && redis {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready")
    }
}
