//! Per-team identity provider (`/teams/{id}/idp`) and home-realm discovery.

use crate::audit;
use crate::error::{ApiResult, Error};
use crate::extract::{Auth, Json as Body};
use crate::sso::{self, TeamIdpRow};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use termoso_proto::auth::SsoDiscoverResponse;
use termoso_proto::team::{TeamIdp, TeamIdpResponse, UpdateTeamIdpRequest};
use uuid::Uuid;

const KINDS: &[&str] = &["oidc", "saml"];

fn public(state: &AppState, r: &TeamIdpRow) -> TeamIdp {
    let provider_id = sso::team_provider_id(r.team_id);
    let base = format!(
        "{}/api/v1/auth/sso",
        state.cfg.public_url.trim_end_matches('/')
    );
    let callback_url = if r.kind == "saml" {
        format!("{base}/{provider_id}/saml/metadata")
    } else {
        format!("{base}/callback")
    };
    TeamIdp {
        kind: r.kind.clone(),
        name: r.name.clone(),
        issuer: r.issuer.clone(),
        client_id: r.client_id.clone(),
        has_client_secret: r.client_secret.is_some(),
        scopes: r.scopes.clone(),
        saml_metadata: r.saml_metadata.clone(),
        saml_idp_entity_id: r.saml_idp_entity_id.clone(),
        domains: r.domains.clone(),
        enabled: r.enabled,
        jit_join: r.jit_join,
        provider_id,
        callback_url,
        updated_at: r.updated_at,
    }
}

async fn response(state: &AppState, team_id: Uuid) -> ApiResult<Json<TeamIdpResponse>> {
    let row = sso::team_row(state, team_id).await?;
    Ok(Json(TeamIdpResponse {
        idp: row.map(|r| public(state, &r)),
        available: state.cfg.team_idp,
    }))
}

fn ensure_available(state: &AppState) -> ApiResult<()> {
    if state.cfg.team_idp {
        Ok(())
    } else {
        Err(Error::forbidden(
            "Team identity providers are disabled on this server",
        ))
    }
}

fn opt(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[utoipa::path(get, path = "/api/v1/teams/{id}/idp", tag = "teams", params(("id" = Uuid, Path)),
    responses((status = 200, body = TeamIdpResponse)))]
pub async fn get(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TeamIdpResponse>> {
    crate::authz::require(&state, id, auth.user_id(), "sso.manage").await?;
    response(&state, id).await
}

#[utoipa::path(put, path = "/api/v1/teams/{id}/idp", tag = "teams", params(("id" = Uuid, Path)),
    request_body = UpdateTeamIdpRequest, responses((status = 200, body = TeamIdpResponse)))]
pub async fn put(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<UpdateTeamIdpRequest>,
) -> ApiResult<Json<TeamIdpResponse>> {
    crate::authz::require(&state, id, auth.user_id(), "sso.manage").await?;
    ensure_available(&state)?;
    let before = sso::team_row(&state, id).await?;
    let mut next = before.clone().unwrap_or(TeamIdpRow {
        team_id: id,
        kind: "oidc".into(),
        name: String::new(),
        issuer: None,
        client_id: None,
        client_secret: None,
        scopes: None,
        saml_metadata: None,
        saml_idp_entity_id: None,
        domains: Vec::new(),
        enabled: true,
        jit_join: true,
        updated_at: chrono::Utc::now(),
    });
    if let Some(k) = req.kind {
        let k = k.trim().to_lowercase();
        if !KINDS.contains(&k.as_str()) {
            return Err(Error::bad_request("kind must be oidc or saml"));
        }
        next.kind = k;
    }
    if let Some(n) = req.name {
        next.name = n.trim().chars().take(64).collect();
    }
    if let Some(v) = req.issuer {
        next.issuer = opt(Some(v));
    }
    if let Some(v) = req.client_id {
        next.client_id = opt(Some(v));
    }
    if let Some(v) = req.client_secret {
        next.client_secret = opt(Some(v));
    }
    if let Some(v) = req.scopes {
        next.scopes = opt(Some(v));
    }
    if let Some(v) = req.saml_metadata {
        next.saml_metadata = opt(Some(v));
    }
    if let Some(v) = req.saml_idp_entity_id {
        next.saml_idp_entity_id = opt(Some(v));
    }
    if let Some(d) = req.domains {
        next.domains = crate::policy::normalize_domains(&d).map_err(Error::bad_request)?;
    }
    if let Some(on) = req.enabled {
        next.enabled = on;
    }
    if let Some(on) = req.jit_join {
        next.jit_join = on;
    }
    if next.name.is_empty() {
        return Err(Error::bad_request("name is required"));
    }
    if next.domains.is_empty() {
        return Err(Error::bad_request("at least one email domain is required"));
    }
    match next.kind.as_str() {
        "saml" => {
            if next.saml_metadata.is_none() {
                return Err(Error::bad_request("saml_metadata is required for SAML"));
            }
        }
        _ => {
            let issuer = next
                .issuer
                .as_deref()
                .ok_or_else(|| Error::bad_request("issuer is required for OIDC"))?;
            let u =
                url::Url::parse(issuer).map_err(|_| Error::bad_request("issuer must be a URL"))?;
            if u.scheme() != "https"
                && !matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
            {
                return Err(Error::bad_request("issuer must use https"));
            }
            if next.client_id.is_none() || next.client_secret.is_none() {
                return Err(Error::bad_request(
                    "client_id and client_secret are required for OIDC",
                ));
            }
        }
    }
    let taken: Option<(Uuid,)> = sqlx::query_as(
        "SELECT team_id FROM team_idp WHERE team_id <> $1 AND domains && $2 LIMIT 1",
    )
    .bind(id)
    .bind(&next.domains)
    .fetch_optional(&state.db)
    .await?;
    if taken.is_some() {
        return Err(Error::conflict(
            "One of these domains is already routed to another team's identity provider",
        ));
    }
    let provider =
        if next.enabled {
            Some(state.sso.build_team(&state.cfg, &next).await.map_err(|e| {
                Error::bad_request(format!("Identity provider check failed: {e:#}"))
            })?)
        } else {
            None
        };
    let enc = match &next.client_secret {
        Some(s) => Some(state.encrypt_secret("team_idp", s.as_bytes())?),
        None => None,
    };
    sqlx::query(
        "INSERT INTO team_idp (team_id, kind, name, issuer, client_id, client_secret_enc, scopes, saml_metadata,
                               saml_idp_entity_id, domains, enabled, jit_join, updated_by, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, now())
         ON CONFLICT (team_id) DO UPDATE SET kind = EXCLUDED.kind, name = EXCLUDED.name, issuer = EXCLUDED.issuer,
             client_id = EXCLUDED.client_id, client_secret_enc = EXCLUDED.client_secret_enc, scopes = EXCLUDED.scopes,
             saml_metadata = EXCLUDED.saml_metadata, saml_idp_entity_id = EXCLUDED.saml_idp_entity_id,
             domains = EXCLUDED.domains, enabled = EXCLUDED.enabled, jit_join = EXCLUDED.jit_join,
             updated_by = EXCLUDED.updated_by, updated_at = now()",
    )
    .bind(id)
    .bind(&next.kind)
    .bind(&next.name)
    .bind(&next.issuer)
    .bind(&next.client_id)
    .bind(&enc)
    .bind(&next.scopes)
    .bind(&next.saml_metadata)
    .bind(&next.saml_idp_entity_id)
    .bind(&next.domains)
    .bind(next.enabled)
    .bind(next.jit_join)
    .bind(auth.user_id())
    .execute(&state.db)
    .await?;
    state.sso.install_team(id, provider);
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "idp.updated").details(serde_json::json!({
            "kind": next.kind,
            "name": next.name,
            "issuer": next.issuer,
            "domains": next.domains,
            "enabled": next.enabled,
            "jit_join": next.jit_join,
            "secret_changed": req_secret_changed(&before, &next),
            "created": before.is_none(),
        })),
    )
    .await;
    response(&state, id).await
}

fn req_secret_changed(before: &Option<TeamIdpRow>, next: &TeamIdpRow) -> bool {
    before.as_ref().map(|b| b.client_secret.as_deref()) != Some(next.client_secret.as_deref())
}

#[utoipa::path(delete, path = "/api/v1/teams/{id}/idp", tag = "teams", params(("id" = Uuid, Path)),
    responses((status = 200, body = TeamIdpResponse)))]
pub async fn delete(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TeamIdpResponse>> {
    crate::authz::require(&state, id, auth.user_id(), "sso.manage").await?;
    let n = sqlx::query("DELETE FROM team_idp WHERE team_id = $1")
        .bind(id)
        .execute(&state.db)
        .await?
        .rows_affected();
    state.sso.install_team(id, None);
    if n > 0 {
        sqlx::query(
            "UPDATE team_policies SET sso_only = false, sso_provider = NULL WHERE team_id = $1 AND sso_provider = 'team'",
        )
        .bind(id)
        .execute(&state.db)
        .await?;
        audit::record(&state.db, audit::Entry::new(id, &auth, "idp.removed")).await;
    }
    response(&state, id).await
}

/// Re-run discovery / metadata parsing with the stored settings.
#[utoipa::path(post, path = "/api/v1/teams/{id}/idp/test", tag = "teams", params(("id" = Uuid, Path)),
    responses((status = 200, body = TeamIdpResponse)))]
pub async fn test(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TeamIdpResponse>> {
    crate::authz::require(&state, id, auth.user_id(), "sso.manage").await?;
    ensure_available(&state)?;
    let row = sso::team_row(&state, id)
        .await?
        .ok_or_else(|| Error::not_found("Team identity provider"))?;
    let provider = state
        .sso
        .build_team(&state.cfg, &row)
        .await
        .map_err(|e| Error::bad_request(format!("Identity provider check failed: {e:#}")))?;
    if row.enabled {
        state.sso.install_team(id, Some(provider));
    }
    audit::record(&state.db, audit::Entry::new(id, &auth, "idp.tested")).await;
    response(&state, id).await
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct DiscoverQuery {
    pub email: String,
}

/// Home-realm discovery for the sign-in form (unauthenticated).
#[utoipa::path(get, path = "/api/v1/auth/sso/discover", tag = "auth", params(DiscoverQuery),
    responses((status = 200, body = SsoDiscoverResponse)))]
pub async fn discover(
    State(state): State<AppState>,
    Query(q): Query<DiscoverQuery>,
) -> ApiResult<Json<SsoDiscoverResponse>> {
    let email = crate::util::normalize_email(&q.email)
        .ok_or_else(|| Error::bad_request("Invalid email"))?;
    let Some(p) = state.sso.discover(&email) else {
        return Ok(Json(SsoDiscoverResponse {
            provider: None,
            team_name: None,
            required: false,
        }));
    };
    let team_id = p.team.unwrap_or_default();
    let (team_name, required): (Option<String>, bool) = sqlx::query_as(
        "SELECT t.name, COALESCE(pol.sso_only AND pol.sso_provider = 'team' AND $2 = ANY(pol.sso_domains), false)
         FROM teams t LEFT JOIN team_policies pol ON pol.team_id = t.id WHERE t.id = $1",
    )
    .bind(team_id)
    .bind(email.rsplit('@').next().unwrap_or_default())
    .fetch_optional(&state.db)
    .await?
    .unwrap_or((None, false));
    Ok(Json(SsoDiscoverResponse {
        provider: Some(p.public()),
        team_name,
        required,
    }))
}
