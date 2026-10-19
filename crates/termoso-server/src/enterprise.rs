//! Corporate mode for self-hosted servers: a home team every account joins,
//! SSO-only sign-in and hiding personal vaults. All of it lives in the admin
//! `ServerSettings`, so the operator switches it on from the cabinet.

use termoso_proto::account::HomeTeam;
use termoso_proto::admin::ServerSettings;
use uuid::Uuid;

use crate::audit;
use crate::error::{ApiResult, Error};
use crate::state::AppState;

/// Join the home team (if configured) with its default role. Idempotent.
pub async fn ensure_home_team(state: &AppState, user_id: Uuid) -> ApiResult<()> {
    let settings = state.settings().await?;
    let Some(team_id) = settings.home_team_id else {
        return Ok(());
    };
    // Team policies that need a reachable mailbox (device approval, email
    // step-up) are no-ops for an unverified address, so the home team waits
    // until the address is confirmed (re-run from the confirm endpoint).
    if state.mailer.is_some()
        && !crate::users::by_id(&state.db, user_id)
            .await?
            .email_verified
    {
        return Ok(());
    }
    let Ok(default_role) = crate::authz::default_role_id(&state.db, team_id).await else {
        tracing::warn!(%team_id, "home team has no default role; skipping auto-join");
        return Ok(());
    };
    let joined = sqlx::query(
        "INSERT INTO team_members (team_id, user_id, role, role_id, role_source)
         VALUES ($1, $2, CASE WHEN (SELECT slug FROM team_roles WHERE id = $3) = 'admin' THEN 'admin' ELSE 'member' END, $3, 'server')
         ON CONFLICT (team_id, user_id) DO NOTHING",
    )
    .bind(team_id)
    .bind(user_id)
    .bind(default_role)
    .execute(&state.db)
    .await?
    .rows_affected()
        > 0;
    if joined {
        audit::record(
            &state.db,
            audit::Entry::system(team_id, "member.joined")
                .user(user_id)
                .details(serde_json::json!({ "source": "home_team" })),
        )
        .await;
        let _ = state.events.send(crate::events::Event::TeamsUpdated {
            user_ids: vec![user_id],
        });
    }
    Ok(())
}

pub async fn home_team(state: &AppState, settings: &ServerSettings) -> ApiResult<Option<HomeTeam>> {
    let Some(id) = settings.home_team_id else {
        return Ok(None);
    };
    let row: Option<(String,)> = sqlx::query_as("SELECT name FROM teams WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    Ok(row.map(|(name,)| HomeTeam { id, name }))
}

/// Which SSO provider a user should be sent to on an SSO-only server: the
/// home-realm match for their e-mail domain, else the single server provider.
async fn provider_for(state: &AppState, email: &str) -> Option<String> {
    if let Some(p) = state.sso.discover(email) {
        return Some(p.id.clone());
    }
    let list = state.sso.list();
    list.first().map(|p| p.id.clone())
}

pub async fn sso_required(state: &AppState, email: &str) -> Error {
    let provider = provider_for(state, email).await;
    Error::new(
        axum::http::StatusCode::UNAUTHORIZED,
        termoso_proto::error::codes::SSO_REQUIRED,
        "This server accepts single sign-on only",
    )
    .with_details(serde_json::json!({ "provider": provider }))
}

/// Refuses a password flow for an address whose domain a team has claimed
/// for SSO (see `policy::domain_claim`). Bootstrap admins are exempt.
pub async fn require_team_sso(state: &AppState, sso_verified: bool, email: &str) -> ApiResult<()> {
    if sso_verified || state.is_bootstrap_admin(email) {
        return Ok(());
    }
    let Some((team_id, team_name, provider)) =
        crate::policy::domain_claim(&state.db, email).await?
    else {
        return Ok(());
    };
    // A claim through a provider that is not configured is not enforceable.
    let configured = match provider.as_deref() {
        Some("team") => true,
        Some(p) => state.sso.has(p),
        None => false,
    };
    if !configured || crate::policy::is_team_owner_email(&state.db, team_id, email).await? {
        return Ok(());
    }
    let provider = crate::policy::resolve_provider(team_id, provider);
    Err(Error::new(
        axum::http::StatusCode::UNAUTHORIZED,
        termoso_proto::error::codes::SSO_REQUIRED,
        format!(
            "{team_name} requires signing in through its identity provider for this e-mail domain"
        ),
    )
    .with_details(serde_json::json!({ "provider": provider, "team_name": team_name })))
}

/// Password sign-in is refused on SSO-only servers (bootstrap admins excepted
/// so the operator can never lock themselves out).
pub async fn require_sso(state: &AppState, sso_verified: bool, email: &str) -> ApiResult<()> {
    if sso_verified || state.is_bootstrap_admin(email) {
        return Ok(());
    }
    if state.settings().await?.sso_only {
        return Err(sso_required(state, email).await);
    }
    Ok(())
}

pub async fn validate_settings(state: &AppState, s: &ServerSettings) -> ApiResult<()> {
    if let Some(id) = s.home_team_id {
        let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM teams WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
        if exists.is_none() {
            return Err(Error::bad_request("home_team_id: no such team"));
        }
    }
    if s.sso_only {
        let has_team_idp = match s.home_team_id {
            Some(id) => {
                let r: Option<(bool,)> =
                    sqlx::query_as("SELECT enabled FROM team_idp WHERE team_id = $1")
                        .bind(id)
                        .fetch_optional(&state.db)
                        .await?;
                matches!(r, Some((true,)))
            }
            None => false,
        };
        if state.sso.list().is_empty() && !has_team_idp {
            return Err(Error::bad_request(
                "sso_only needs at least one SSO provider (server config or the home team's IdP)",
            ));
        }
    }
    Ok(())
}
