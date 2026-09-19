//! HTTP routing.

pub mod account;
pub mod admin;
pub mod ai;
pub mod auth;
pub mod bridges;
pub mod history;
pub mod logs;
pub mod mfa;
pub mod server;
pub mod sshid;
pub mod start_over;
pub mod sync;
pub mod teams;
pub mod vaults;
pub mod web;

use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method, header};
use axum::routing::{delete, get, patch, post, put};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// 8 MiB: a full sync push of 500 × 16 KiB entities fits comfortably.
const BODY_LIMIT: usize = 8 * 1024 * 1024;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/server/info", get(server::info))
        // auth
        .route("/auth/register/start", post(auth::register_start))
        .route("/auth/register/finish", post(auth::register_finish))
        .route("/auth/login/start", post(auth::login_start))
        .route("/auth/login/finish", post(auth::login_finish))
        .route("/auth/mfa/verify", post(auth::mfa_verify))
        .route(
            "/auth/mfa/webauthn/challenge",
            post(auth::mfa_webauthn_challenge),
        )
        .route("/auth/mfa/email/send", post(auth::mfa_email_send))
        .route("/auth/device/approve", post(auth::device_approve))
        .route(
            "/auth/device/approve/resend",
            post(auth::device_approve_resend),
        )
        .route("/auth/recovery/start", post(auth::recovery_start))
        .route("/auth/password/start", post(auth::password_start))
        .route("/auth/password/finish", post(auth::password_finish))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/reauth/start", post(auth::reauth_start))
        .route("/auth/reauth/finish", post(auth::reauth_finish))
        .route("/auth/start-over/request", post(start_over::request))
        .route("/auth/start-over/confirm", post(start_over::confirm))
        .route("/auth/start-over/cancel", post(start_over::cancel))
        .route(
            "/auth/start-over/password/start",
            post(start_over::password_start),
        )
        .route("/auth/start-over/finish", post(start_over::finish))
        .route("/auth/start-over/{token}", get(start_over::status))
        .route(
            "/account/start-over/cancel",
            post(start_over::cancel_authenticated),
        )
        .route("/auth/sso/providers", get(auth::sso_providers))
        .route("/auth/sso/callback", get(auth::sso_callback))
        .route("/auth/sso/flow/{flow_id}", get(auth::sso_poll))
        .route("/auth/sso/{provider}/start", get(auth::sso_start))
        // account
        .route(
            "/account",
            get(account::get).delete(account::delete_account),
        )
        .route("/account/profile", patch(account::update_profile))
        .route("/account/presence", put(account::put_presence))
        .route("/account/ai", get(ai::status).put(ai::put_settings))
        .route("/ai/command", post(ai::command))
        .route(
            "/account/avatar",
            put(account::put_avatar)
                .delete(account::delete_avatar)
                .layer(DefaultBodyLimit::max(crate::avatar::MAX_UPLOAD)),
        )
        .route("/users/{id}/avatar", get(account::user_avatar))
        .route(
            "/account/email/verify/send",
            post(account::email_verify_send),
        )
        .route(
            "/account/email/verify/confirm",
            post(account::email_verify_confirm),
        )
        .route("/account/email/change", post(account::email_change))
        .route(
            "/account/email/change/confirm",
            post(account::email_change_confirm),
        )
        .route(
            "/account/settings",
            get(account::get_settings).put(account::put_settings),
        )
        .route("/account/devices", get(account::devices))
        .route("/account/devices/{id}", delete(account::revoke_device))
        .route("/account/bridges", get(bridges::list).post(bridges::create))
        .route("/account/bridges/{id}", delete(bridges::revoke))
        .route("/account/bridges/{id}/vaults", put(bridges::set_vaults))
        .route("/bridge/me", get(bridges::me))
        .route(
            "/account/sshid",
            get(sshid::get).post(sshid::create).delete(sshid::delete),
        )
        .route("/account/sshid/keys/device", put(sshid::put_device_keys))
        .route("/account/sshid/keys/fido2", post(sshid::add_fido2_key))
        .route("/account/sshid/keys/{id}", delete(sshid::remove_key))
        .route("/account/security-events", get(account::security_events))
        .route("/account/recovery/rotate", post(account::rotate_recovery))
        // mfa management
        .route("/account/mfa", get(mfa::status))
        .route("/account/mfa/totp/setup", post(mfa::totp_setup))
        .route("/account/mfa/totp/confirm", post(mfa::totp_confirm))
        .route("/account/mfa/totp", delete(mfa::totp_disable))
        .route("/account/mfa/backup-codes", post(mfa::backup_codes))
        .route(
            "/account/mfa/webauthn/register/start",
            post(mfa::webauthn_register_start),
        )
        .route(
            "/account/mfa/webauthn/register/finish",
            post(mfa::webauthn_register_finish),
        )
        .route("/account/mfa/webauthn/{id}", delete(mfa::webauthn_delete))
        // teams
        .route("/teams", get(teams::list).post(teams::create))
        .route(
            "/teams/{id}",
            get(teams::get).patch(teams::update).delete(teams::delete),
        )
        .route("/teams/{id}/members", get(teams::members))
        .route(
            "/teams/{id}/members/{user_id}",
            patch(teams::update_member).delete(teams::remove_member),
        )
        .route(
            "/teams/{id}/members/{user_id}/account",
            delete(teams::delete_member_account),
        )
        .route("/teams/{id}/leave", post(teams::leave))
        .route(
            "/teams/{id}/invites",
            get(teams::invites).post(teams::create_invite),
        )
        .route(
            "/teams/{id}/invites/{invite_id}",
            delete(teams::delete_invite),
        )
        .route("/teams/{id}/pending-keys", get(teams::pending_keys))
        .route("/teams/{id}/presence", get(teams::get_presence))
        .route("/teams/{id}/audit", get(crate::audit::list))
        .route(
            "/teams/{id}/digest",
            get(crate::digest::get).put(crate::digest::put),
        )
        .route("/teams/{id}/digest/send", post(crate::digest::send_now))
        .route("/teams/{id}/vaults", post(vaults::create_team_vault))
        .route("/invites/{token}", get(teams::invite_preview))
        .route("/invites/{token}/accept", post(teams::accept_invite))
        // vaults
        .route("/vaults", get(vaults::list))
        .route(
            "/vaults/{id}",
            get(vaults::get)
                .patch(vaults::update)
                .delete(vaults::delete),
        )
        .route("/vaults/{id}/members", get(vaults::members))
        .route(
            "/vaults/{id}/members/{user_id}",
            put(vaults::upsert_member).delete(vaults::remove_member),
        )
        .route("/vaults/{id}/rotate-key", post(vaults::rotate_key))
        // sync
        .route("/sync/push", post(sync::push))
        .route("/sync/pull", post(sync::pull))
        .route("/history/push", post(history::push))
        .route("/history/pull", get(history::pull))
        .route("/history/clear", post(history::clear))
        // logs
        .route("/logs", get(logs::list).post(logs::create))
        .route("/logs/{id}", patch(logs::update).delete(logs::delete))
        .route("/logs/{id}/download", get(logs::download))
        .route("/vaults/{id}/logs", get(logs::list_vault))
        // realtime
        .route("/ws", get(crate::ws::handler))
        // multiplayer
        .route("/live", get(crate::live::list).post(crate::live::create))
        .route("/live/{id}/stop", post(crate::live::stop))
        .route("/live/{id}/ws", get(crate::live::ws))
        // admin
        .route("/admin/stats", get(admin::stats))
        .route("/admin/users", get(admin::users))
        .route(
            "/admin/users/{id}",
            get(admin::user)
                .patch(admin::update_user)
                .delete(admin::delete_user),
        )
        .route(
            "/admin/users/{id}/revoke-sessions",
            post(admin::revoke_sessions),
        )
        .route("/admin/users/{id}/reset-mfa", post(admin::reset_mfa))
        .route("/admin/teams", get(admin::teams))
        .route("/admin/teams/{id}", delete(admin::delete_team))
        .route(
            "/admin/settings",
            get(admin::get_settings).put(admin::put_settings),
        )
        .route("/admin/email/test", post(admin::test_email))
        .fallback(async || crate::error::Error::not_found("route"));

    let mut app = Router::new()
        .route("/healthz", get(server::healthz))
        .route("/readyz", get(server::readyz))
        .route("/.well-known/assetlinks.json", get(server::assetlinks))
        .route("/sshid/{handle}", get(sshid::public_default))
        .route("/sshid/{handle}/{type}", get(sshid::public_typed))
        .nest("/api/v1", api);

    if state.cfg.swagger_ui {
        app = app.merge(crate::openapi::swagger());
    }
    if let Some(dir) = state.cfg.web_dir() {
        app = app.merge(web::router(dir, state.cfg.landing_on_cabinet()));
    }

    let cors = cors_layer(&state);
    // Host-based rewrites for the dedicated SSH ID / landing origins have to
    // run before routing, so they wrap the finished router instead of being
    // route layers.
    let sshid_host = axum::middleware::from_fn_with_state(state.clone(), sshid::host_layer);
    let landing_host = axum::middleware::from_fn_with_state(state.clone(), web::landing_host_layer);

    let app = app
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT))
        .layer(
            tower::ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(SetResponseHeaderLayer::overriding(
                    header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    header::REFERRER_POLICY,
                    HeaderValue::from_static("no-referrer"),
                ))
                .layer(cors)
                .layer(CompressionLayer::new())
                .layer(TimeoutLayer::with_status_code(
                    axum::http::StatusCode::REQUEST_TIMEOUT,
                    Duration::from_secs(60),
                )),
        )
        .with_state(state);

    let app = tower::Layer::layer(&landing_host, app);
    Router::new().fallback_service(tower::Layer::layer(&sshid_host, app))
}

/// `host[:port]` the request was addressed to, lowercase.
pub(crate) fn request_host(req: &axum::extract::Request) -> String {
    req.uri()
        .authority()
        .map(|a| a.as_str().to_string())
        .or_else(|| {
            req.headers()
                .get(header::HOST)
                .and_then(|h| h.to_str().ok())
                .map(str::to_string)
        })
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn cors_layer(state: &AppState) -> CorsLayer {
    let origins = state.cfg.cors_origins();
    let allow = if origins.is_empty() {
        // Desktop (tauri://, http://tauri.localhost) and mobile clients do not
        // send a browser Origin the server can pre-validate; the web cabinet is
        // expected to be same-origin unless TERMOSO_CORS_ORIGINS is set.
        AllowOrigin::predicate(|origin: &HeaderValue, _| {
            origin
                .to_str()
                .map(|o| {
                    o.starts_with("tauri://")
                        || o.starts_with("http://tauri.localhost")
                        || o.starts_with("http://localhost")
                })
                .unwrap_or(false)
        })
    } else {
        AllowOrigin::list(origins.iter().filter_map(|o| HeaderValue::from_str(o).ok()))
    };
    CorsLayer::new()
        .allow_origin(allow)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .max_age(Duration::from_secs(3600))
}
