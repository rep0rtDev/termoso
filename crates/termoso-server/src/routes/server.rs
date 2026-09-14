//! Public server information and health probes.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use termoso_proto::account::{ServerFeatures, ServerInfo};

use crate::error::{ApiResult, Error};
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

#[derive(Serialize)]
struct AssetLink {
    relation: [&'static str; 1],
    target: AssetLinkTarget,
}

#[derive(Serialize)]
struct AssetLinkTarget {
    namespace: &'static str,
    package_name: String,
    sha256_cert_fingerprints: [String; 1],
}

/// Digital Asset Links statement letting the configured Android builds claim
/// this server's https links (`TERMOSO_ANDROID_APP_LINKS`). 404 when unset.
pub async fn assetlinks(State(state): State<AppState>) -> Response {
    let apps = match state.cfg.android_app_links() {
        Ok(apps) if !apps.is_empty() => apps,
        _ => return Error::not_found("assetlinks").into_response(),
    };
    let statements: Vec<AssetLink> = apps
        .into_iter()
        .map(|app| AssetLink {
            relation: ["delegate_permission/common.handle_all_urls"],
            target: AssetLinkTarget {
                namespace: "android_app",
                package_name: app.package,
                sha256_cert_fingerprints: [app.sha256_fingerprint],
            },
        })
        .collect();
    (
        [(axum::http::header::CACHE_CONTROL, "public, max-age=3600")],
        Json(statements),
    )
        .into_response()
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
