//! OpenAPI document + optional Swagger UI (`TERMOSO_SWAGGER_UI=true`).

use axum::Router;
use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;

struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Termoso API",
        description = "Self-hosted, zero-knowledge sync backend for the Termoso SSH client. \
                       All vault payloads are encrypted client-side; the server only sees opaque blobs.",
        license(name = "AGPL-3.0-or-later"),
    ),
    modifiers(&BearerAuth),
    security(("bearer" = [])),
    paths(
        crate::routes::server::info,
        crate::routes::auth::register_start,
        crate::routes::auth::register_finish,
        crate::routes::auth::login_start,
        crate::routes::auth::login_finish,
        crate::routes::auth::mfa_webauthn_challenge,
        crate::routes::auth::mfa_email_send,
        crate::routes::auth::mfa_verify,
        crate::routes::auth::device_approve,
        crate::routes::auth::device_approve_resend,
        crate::routes::auth::recovery_start,
        crate::routes::auth::password_start,
        crate::routes::auth::password_finish,
        crate::routes::auth::logout,
        crate::routes::auth::sso_providers,
        crate::routes::auth::sso_start,
        crate::routes::auth::sso_poll,
        crate::routes::account::get,
        crate::routes::account::update_profile,
        crate::routes::account::email_verify_send,
        crate::routes::account::email_verify_confirm,
        crate::routes::account::email_change,
        crate::routes::account::email_change_confirm,
        crate::routes::account::get_settings,
        crate::routes::account::put_settings,
        crate::routes::account::devices,
        crate::routes::account::revoke_device,
        crate::routes::account::security_events,
        crate::routes::account::rotate_recovery,
        crate::routes::account::delete_account,
        crate::routes::mfa::status,
        crate::routes::mfa::totp_setup,
        crate::routes::mfa::totp_confirm,
        crate::routes::mfa::totp_disable,
        crate::routes::mfa::backup_codes,
        crate::routes::mfa::webauthn_register_start,
        crate::routes::mfa::webauthn_register_finish,
        crate::routes::mfa::webauthn_delete,
        crate::routes::teams::list,
        crate::routes::teams::create,
        crate::routes::teams::get,
        crate::routes::teams::update,
        crate::routes::teams::delete,
        crate::routes::teams::members,
        crate::routes::teams::update_member,
        crate::routes::teams::remove_member,
        crate::routes::teams::leave,
        crate::routes::teams::invites,
        crate::routes::teams::create_invite,
        crate::routes::teams::delete_invite,
        crate::routes::teams::invite_preview,
        crate::routes::teams::accept_invite,
        crate::routes::teams::pending_keys,
        crate::audit::list,
        crate::routes::vaults::list,
        crate::routes::vaults::get,
        crate::routes::vaults::create_team_vault,
        crate::routes::vaults::update,
        crate::routes::vaults::delete,
        crate::routes::vaults::members,
        crate::routes::vaults::upsert_member,
        crate::routes::vaults::remove_member,
        crate::routes::vaults::rotate_key,
        crate::routes::sync::push,
        crate::routes::sync::pull,
        crate::routes::history::push,
        crate::routes::history::pull,
        crate::routes::history::clear,
        crate::routes::logs::list,
        crate::routes::logs::create,
        crate::routes::logs::update,
        crate::routes::logs::download,
        crate::routes::logs::delete,
        crate::ws::handler,
        crate::live::create,
        crate::live::list,
        crate::live::stop,
        crate::live::ws,
        crate::routes::admin::stats,
        crate::routes::admin::users,
        crate::routes::admin::user,
        crate::routes::admin::update_user,
        crate::routes::admin::delete_user,
        crate::routes::admin::revoke_sessions,
        crate::routes::admin::reset_mfa,
        crate::routes::admin::teams,
        crate::routes::admin::delete_team,
        crate::routes::admin::get_settings,
        crate::routes::admin::put_settings,
        crate::routes::admin::test_email,
    ),
    tags(
        (name = "server", description = "Server capabilities"),
        (name = "auth", description = "OPAQUE registration/login, MFA, device approval, recovery, SSO"),
        (name = "account", description = "Profile, e-mail, settings blob, devices, security events"),
        (name = "mfa", description = "TOTP / WebAuthn / backup codes management"),
        (name = "teams", description = "Teams, roles, invitations"),
        (name = "vaults", description = "Vaults, memberships, sealed keys"),
        (name = "sync", description = "Encrypted entity and history sync"),
        (name = "logs", description = "Encrypted session logs in S3"),
        (name = "realtime", description = "WebSocket notifications"),
        (name = "live", description = "Multiplayer: end-to-end encrypted live terminal relay"),
        (name = "admin", description = "Server administration"),
    )
)]
pub struct ApiDoc;

pub fn swagger<S: Clone + Send + Sync + 'static>() -> Router<S> {
    SwaggerUi::new("/docs")
        .url("/api/openapi.json", ApiDoc::openapi())
        .into()
}
