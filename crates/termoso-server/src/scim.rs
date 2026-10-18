//! SCIM 2.0 (RFC 7643/7644) provisioning at `/scim/v2`, one tenant per team,
//! authenticated with a team bearer token. Users are team members (or
//! accounts the team provisioned); Groups drive role mappings and vault
//! grants. Deactivating a user revokes sessions, removes them from the team's
//! vaults and, for accounts the team provisioned, disables the account.

use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::audit;
use crate::error::Error;
use crate::state::AppState;
use crate::util::hash_token;

pub const MAX_TOKENS: i64 = 3;
pub const MAX_RESULTS: i64 = 200;
pub const REACTIVATION_DAYS: i64 = 30;
const USER_SCHEMA: &str = "urn:ietf:params:scim:schemas:core:2.0:User";
const GROUP_SCHEMA: &str = "urn:ietf:params:scim:schemas:core:2.0:Group";
const LIST_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:ListResponse";
const PATCH_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:PatchOp";
const ERROR_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:Error";

// ───────────────────────────── errors ─────────────────────────────

#[derive(Debug)]
pub struct ScimError {
    status: StatusCode,
    scim_type: Option<&'static str>,
    detail: String,
}

impl ScimError {
    fn new(status: StatusCode, scim_type: Option<&'static str>, detail: impl Into<String>) -> Self {
        Self {
            status,
            scim_type,
            detail: detail.into(),
        }
    }
    fn invalid(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, Some("invalidValue"), detail)
    }
    fn not_found(what: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, None, format!("{what} not found"))
    }
    fn uniqueness(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, Some("uniqueness"), detail)
    }
    fn mutability(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, Some("mutability"), detail)
    }
}

impl IntoResponse for ScimError {
    fn into_response(self) -> Response {
        let mut body = json!({
            "schemas": [ERROR_SCHEMA],
            "status": self.status.as_u16().to_string(),
            "detail": self.detail,
        });
        if let Some(t) = self.scim_type {
            body["scimType"] = Value::String(t.into());
        }
        (
            self.status,
            [("content-type", "application/scim+json")],
            body.to_string(),
        )
            .into_response()
    }
}

impl From<Error> for ScimError {
    fn from(e: Error) -> Self {
        let resp = e.into_response();
        let status = resp.status();
        Self::new(
            status,
            None,
            status.canonical_reason().unwrap_or("error").to_string(),
        )
    }
}

impl From<sqlx::Error> for ScimError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "scim database error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            None,
            format!("database error: {e}"),
        )
    }
}

impl From<anyhow::Error> for ScimError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = %e, "scim error");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, None, "internal error")
    }
}

type ScimResult<T> = Result<T, ScimError>;

fn scim_json(status: StatusCode, v: Value) -> Response {
    (
        status,
        [("content-type", "application/scim+json")],
        v.to_string(),
    )
        .into_response()
}

// ───────────────────────────── auth ─────────────────────────────

/// A request authenticated with a team SCIM token.
pub struct ScimAuth {
    pub team_id: Uuid,
    pub token_id: Uuid,
}

impl FromRequestParts<AppState> for ScimAuth {
    type Rejection = ScimError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ScimError> {
        let Some(token) = crate::extract::bearer(&parts.headers) else {
            return Err(ScimError::new(
                StatusCode::UNAUTHORIZED,
                None,
                "Bearer token required",
            ));
        };
        let row: Option<(Uuid, Uuid)> = sqlx::query_as(
            "UPDATE scim_tokens SET last_used_at = now()
             WHERE token_hash = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now())
             RETURNING id, team_id",
        )
        .bind(hash_token(token))
        .fetch_optional(&state.db)
        .await?;
        let Some((token_id, team_id)) = row else {
            return Err(ScimError::new(
                StatusCode::UNAUTHORIZED,
                None,
                "Invalid SCIM token",
            ));
        };
        Ok(Self { team_id, token_id })
    }
}

// ───────────────────────────── router ─────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ServiceProviderConfig", get(service_provider_config))
        .route("/ResourceTypes", get(resource_types))
        .route("/Schemas", get(schemas))
        .route("/Users", get(list_users).post(create_user))
        .route(
            "/Users/{id}",
            get(get_user)
                .put(put_user)
                .patch(patch_user)
                .delete(delete_user),
        )
        .route("/Groups", get(list_groups).post(create_group))
        .route(
            "/Groups/{id}",
            get(get_group)
                .put(put_group)
                .patch(patch_group)
                .delete(delete_group),
        )
}

fn base(state: &AppState) -> String {
    format!("{}/scim/v2", state.cfg.public_url.trim_end_matches('/'))
}

async fn service_provider_config(State(state): State<AppState>, _a: ScimAuth) -> Response {
    scim_json(
        StatusCode::OK,
        json!({
            "schemas": ["urn:ietf:params:scim:schemas:core:2.0:ServiceProviderConfig"],
            "documentationUri": "https://docs.termoso.dev/enterprise/scim",
            "patch": { "supported": true },
            "bulk": { "supported": false, "maxOperations": 0, "maxPayloadSize": 0 },
            "filter": { "supported": true, "maxResults": MAX_RESULTS },
            "changePassword": { "supported": false },
            "sort": { "supported": false },
            "etag": { "supported": false },
            "authenticationSchemes": [{
                "type": "oauthbearertoken", "name": "Bearer token",
                "description": "Team SCIM token from Termoso",
            }],
            "meta": { "resourceType": "ServiceProviderConfig", "location": format!("{}/ServiceProviderConfig", base(&state)) }
        }),
    )
}

async fn resource_types(State(state): State<AppState>, _a: ScimAuth) -> Response {
    let b = base(&state);
    scim_json(
        StatusCode::OK,
        json!({
            "schemas": [LIST_SCHEMA], "totalResults": 2, "startIndex": 1, "itemsPerPage": 2,
            "Resources": [
                { "schemas": ["urn:ietf:params:scim:schemas:core:2.0:ResourceType"], "id": "User", "name": "User",
                  "endpoint": "/Users", "schema": USER_SCHEMA,
                  "meta": { "resourceType": "ResourceType", "location": format!("{b}/ResourceTypes/User") } },
                { "schemas": ["urn:ietf:params:scim:schemas:core:2.0:ResourceType"], "id": "Group", "name": "Group",
                  "endpoint": "/Groups", "schema": GROUP_SCHEMA,
                  "meta": { "resourceType": "ResourceType", "location": format!("{b}/ResourceTypes/Group") } }
            ]
        }),
    )
}

async fn schemas(_a: ScimAuth) -> Response {
    let attr = |name: &str, typ: &str, multi: bool, mutability: &str| json!({ "name": name, "type": typ, "multiValued": multi, "required": false, "mutability": mutability, "returned": "default", "uniqueness": "none" });
    scim_json(
        StatusCode::OK,
        json!({
            "schemas": [LIST_SCHEMA], "totalResults": 2, "startIndex": 1, "itemsPerPage": 2,
            "Resources": [
                { "id": USER_SCHEMA, "name": "User", "description": "Termoso team member",
                  "attributes": [
                      attr("userName", "string", false, "readWrite"), attr("externalId", "string", false, "readWrite"),
                      attr("active", "boolean", false, "readWrite"), attr("displayName", "string", false, "readWrite"),
                      attr("name", "complex", false, "readWrite"), attr("emails", "complex", true, "readWrite"),
                      attr("groups", "complex", true, "readOnly") ] },
                { "id": GROUP_SCHEMA, "name": "Group", "description": "IdP group mapped to a Termoso role and vaults",
                  "attributes": [ attr("displayName", "string", false, "readWrite"), attr("externalId", "string", false, "readWrite"),
                                  attr("members", "complex", true, "readWrite") ] }
            ]
        }),
    )
}

// ───────────────────────────── users ─────────────────────────────

type UserRow = (
    Uuid,
    String,
    Option<String>,
    Option<String>,
    bool,
    bool,
    Option<Uuid>,
    DateTime<Utc>,
    bool,
);

/// Users in scope of a team: members plus accounts the team provisioned.
#[allow(clippy::too_many_arguments)]
async fn user_rows(
    state: &AppState,
    team_id: Uuid,
    id: Option<Uuid>,
    user_name: Option<&str>,
    external_id: Option<&str>,
    active: Option<bool>,
    offset: i64,
    limit: i64,
) -> ScimResult<(Vec<UserRow>, i64)> {
    macro_rules! q {
        ($select:literal, $tail:literal) => {
            concat!(
                $select,
                " FROM users u LEFT JOIN team_members m ON m.team_id = $1 AND m.user_id = u.id
                  WHERE (m.user_id IS NOT NULL OR u.managed_by_team_id = $1)
                    AND ($2::uuid IS NULL OR u.id = $2)
                    AND ($3::text IS NULL OR lower(u.email) = lower($3))
                    AND ($4::text IS NULL OR u.scim_external_id = $4)
                    AND ($5::boolean IS NULL OR (u.scim_active AND NOT u.disabled AND m.user_id IS NOT NULL) = $5) ",
                $tail
            )
        };
    }
    let (total,): (i64,) = sqlx::query_as(q!("SELECT count(*)", ""))
        .bind(team_id)
        .bind(id)
        .bind(user_name)
        .bind(external_id)
        .bind(active)
        .fetch_one(&state.db)
        .await?;
    let rows: Vec<UserRow> = sqlx::query_as(q!(
        "SELECT u.id, u.email, u.display_name, u.scim_external_id, u.scim_active, u.disabled, u.managed_by_team_id, u.created_at, (m.user_id IS NOT NULL)",
        "ORDER BY u.created_at, u.id OFFSET $6 LIMIT $7"
    ))
    .bind(team_id)
    .bind(id)
    .bind(user_name)
    .bind(external_id)
    .bind(active)
    .bind(offset)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok((rows, total))
}

async fn user_groups(state: &AppState, team_id: Uuid, user_id: Uuid) -> ScimResult<Vec<Value>> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT g.id, g.display_name FROM scim_groups g JOIN scim_group_members gm ON gm.group_id = g.id
         WHERE g.team_id = $1 AND gm.user_id = $2 ORDER BY g.display_name",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| json!({ "value": id, "display": name, "$ref": format!("{}/Groups/{id}", base(state)) }))
        .collect())
}

async fn user_resource(state: &AppState, team_id: Uuid, r: &UserRow) -> ScimResult<Value> {
    let (id, email, display_name, external_id, scim_active, disabled, _managed, created_at, member) =
        r;
    let (given, family) = split_name(display_name.as_deref());
    Ok(json!({
        "schemas": [USER_SCHEMA],
        "id": id,
        "externalId": external_id,
        "userName": email,
        "displayName": display_name,
        "name": { "formatted": display_name, "givenName": given, "familyName": family },
        "emails": [{ "value": email, "type": "work", "primary": true }],
        "active": *scim_active && !*disabled && *member,
        "groups": user_groups(state, team_id, *id).await?,
        "meta": {
            "resourceType": "User",
            "created": created_at,
            "location": format!("{}/Users/{id}", base(state)),
        }
    }))
}

fn split_name(n: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(n) = n else { return (None, None) };
    match n.trim().split_once(' ') {
        Some((a, b)) => (Some(a.to_string()), Some(b.to_string())),
        None => (Some(n.to_string()), None),
    }
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub filter: Option<String>,
    #[serde(rename = "startIndex")]
    pub start_index: Option<i64>,
    pub count: Option<i64>,
}

#[derive(Default)]
struct Filter {
    user_name: Option<String>,
    external_id: Option<String>,
    active: Option<bool>,
    display_name: Option<String>,
    id: Option<Uuid>,
}

/// `attr eq "value"` terms joined with `and`; anything else is rejected.
fn parse_filter(f: Option<&str>) -> ScimResult<Filter> {
    let mut out = Filter::default();
    let Some(f) = f.map(str::trim).filter(|f| !f.is_empty()) else {
        return Ok(out);
    };
    for term in f.split(" and ") {
        let term = term.trim();
        let mut parts = term.splitn(3, ' ');
        let (Some(attr), Some(op), Some(val)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(ScimError::new(
                StatusCode::BAD_REQUEST,
                Some("invalidFilter"),
                format!("unsupported filter `{term}`"),
            ));
        };
        if !op.eq_ignore_ascii_case("eq") {
            return Err(ScimError::new(
                StatusCode::BAD_REQUEST,
                Some("invalidFilter"),
                "only `eq` filters are supported",
            ));
        }
        let val = val.trim().trim_matches('"').to_string();
        let attr = attr.trim().to_ascii_lowercase();
        match attr.as_str() {
            "username" => out.user_name = Some(val),
            "externalid" => out.external_id = Some(val),
            "active" => out.active = Some(val.eq_ignore_ascii_case("true")),
            "displayname" => out.display_name = Some(val),
            "id" => out.id = val.parse().ok(),
            "emails.value" | "emails[type eq \"work\"].value" => out.user_name = Some(val),
            _ => {
                return Err(ScimError::new(
                    StatusCode::BAD_REQUEST,
                    Some("invalidFilter"),
                    format!("unsupported attribute `{attr}`"),
                ));
            }
        }
    }
    Ok(out)
}

fn paging(q: &ListQuery) -> (i64, i64) {
    let start = q.start_index.unwrap_or(1).max(1);
    let count = q.count.unwrap_or(100).clamp(0, MAX_RESULTS);
    (start, count)
}

async fn list_users(
    State(state): State<AppState>,
    a: ScimAuth,
    Query(q): Query<ListQuery>,
) -> ScimResult<Response> {
    let f = parse_filter(q.filter.as_deref())?;
    let (start, count) = paging(&q);
    let (rows, total) = user_rows(
        &state,
        a.team_id,
        f.id,
        f.user_name.as_deref(),
        f.external_id.as_deref(),
        f.active,
        start - 1,
        count,
    )
    .await?;
    let mut resources = Vec::with_capacity(rows.len());
    for r in &rows {
        resources.push(user_resource(&state, a.team_id, r).await?);
    }
    Ok(scim_json(
        StatusCode::OK,
        json!({ "schemas": [LIST_SCHEMA], "totalResults": total, "startIndex": start, "itemsPerPage": resources.len(), "Resources": resources }),
    ))
}

async fn load_user(state: &AppState, team_id: Uuid, id: Uuid) -> ScimResult<UserRow> {
    let (rows, _) = user_rows(state, team_id, Some(id), None, None, None, 0, 1).await?;
    rows.into_iter()
        .next()
        .ok_or_else(|| ScimError::not_found("User"))
}

async fn get_user(
    State(state): State<AppState>,
    a: ScimAuth,
    Path(id): Path<Uuid>,
) -> ScimResult<Response> {
    let r = load_user(&state, a.team_id, id).await?;
    Ok(scim_json(
        StatusCode::OK,
        user_resource(&state, a.team_id, &r).await?,
    ))
}

fn str_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    cur.as_str().map(str::trim).filter(|s| !s.is_empty())
}

/// userName or the primary/work email.
fn email_of(v: &Value) -> Option<String> {
    let from_emails = v.get("emails").and_then(|e| e.as_array()).and_then(|arr| {
        arr.iter()
            .find(|e| e["primary"] == true)
            .or_else(|| arr.iter().find(|e| e["type"] == "work"))
            .or_else(|| arr.first())
            .and_then(|e| e["value"].as_str())
    });
    let raw = str_at(v, &["userName"])
        .filter(|u| u.contains('@'))
        .or(from_emails)?;
    crate::util::normalize_email(raw)
}

fn display_of(v: &Value) -> Option<String> {
    str_at(v, &["displayName"])
        .or_else(|| str_at(v, &["name", "formatted"]))
        .or_else(|| {
            let g = str_at(v, &["name", "givenName"]);
            let f = str_at(v, &["name", "familyName"]);
            match (g, f) {
                (Some(g), Some(f)) => Some(Box::leak(format!("{g} {f}").into_boxed_str()) as &str),
                (Some(g), None) => Some(g),
                (None, Some(f)) => Some(f),
                (None, None) => None,
            }
        })
        .map(|s| s.chars().take(120).collect())
}

async fn create_user(
    State(state): State<AppState>,
    a: ScimAuth,
    Json(body): Json<Value>,
) -> ScimResult<Response> {
    let team_id = a.team_id;
    let Some(email) = email_of(&body) else {
        return Err(ScimError::invalid(
            "userName (or emails[].value) must be an email address",
        ));
    };
    if !crate::policy::domain_allowed(
        &crate::policy::load(&state.db, team_id).await?.sso_domains,
        &email,
    ) {
        return Err(ScimError::invalid(
            "email domain is outside the team's SSO domain allow-list",
        ));
    }
    let external_id = str_at(&body, &["externalId"]).map(String::from);
    let display_name = display_of(&body);
    let existing: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT u.id, EXISTS (SELECT 1 FROM team_members m WHERE m.team_id = $2 AND m.user_id = u.id)
         FROM users u WHERE lower(u.email) = $1",
    )
    .bind(&email)
    .bind(team_id)
    .fetch_optional(&state.db)
    .await?;
    let user_id = match existing {
        Some((_, true)) => {
            return Err(ScimError::uniqueness(
                "userName already exists in this team",
            ));
        }
        Some((id, false)) => {
            // An existing account: join the team, keep the account unmanaged.
            sqlx::query(
                "UPDATE users SET scim_external_id = COALESCE($2, scim_external_id) WHERE id = $1",
            )
            .bind(id)
            .bind(&external_id)
            .execute(&state.db)
            .await?;
            id
        }
        None => {
            // Provisioned account: no password yet; the person completes
            // registration through SSO or an invite link and keeps this id.
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO users (id, email, email_verified, display_name, is_admin, managed_by_team_id,
                                    scim_external_id, scim_active, provisioned_at,
                                    public_key, wrapped_private_key, recovery_wrapped_private_key, recovery_verifier_hash)
                 VALUES ($1, $2, true, $3, false, $4, $5, true, now(), '', '', '', '')",
            )
            .bind(id)
            .bind(&email)
            .bind(&display_name)
            .bind(team_id)
            .bind(&external_id)
            .execute(&state.db)
            .await?;
            id
        }
    };
    join_team(&state, team_id, user_id).await?;
    audit::record(
        &state.db,
        audit::Entry::system(team_id, "scim.user_created")
            .user(user_id)
            .details(json!({ "email": email, "external_id": external_id, "token_id": a.token_id })),
    )
    .await;
    let r = load_user(&state, team_id, user_id).await?;
    Ok(scim_json(
        StatusCode::CREATED,
        user_resource(&state, team_id, &r).await?,
    ))
}

/// Add to the team with the default role (`role_source = scim`); no-op if
/// already a member.
async fn join_team(state: &AppState, team_id: Uuid, user_id: Uuid) -> ScimResult<()> {
    let default_role = crate::authz::default_role_id(&state.db, team_id).await?;
    sqlx::query(
        "INSERT INTO team_members (team_id, user_id, role, role_id, role_source)
         VALUES ($1, $2, CASE WHEN (SELECT slug FROM team_roles WHERE id = $3) = 'admin' THEN 'admin' ELSE 'member' END, $3, 'scim')
         ON CONFLICT (team_id, user_id) DO NOTHING",
    )
    .bind(team_id)
    .bind(user_id)
    .bind(default_role)
    .execute(&state.db)
    .await?;
    sync_user_groups(state, team_id, user_id).await?;
    let _ = crate::events::publish(
        state,
        crate::events::Event::TeamsUpdated {
            user_ids: vec![user_id],
        },
    )
    .await;
    Ok(())
}

/// Recompute role (group → role mappings) and vault grants from the user's
/// SCIM groups in this team.
async fn sync_user_groups(state: &AppState, team_id: Uuid, user_id: Uuid) -> ScimResult<()> {
    let groups: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT g.id, g.display_name FROM scim_groups g JOIN scim_group_members gm ON gm.group_id = g.id
         WHERE g.team_id = $1 AND gm.user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    let names: Vec<String> = groups.iter().map(|g| g.1.clone()).collect();
    crate::authz::apply_group_mappings_scoped(&state.db, user_id, &names, Some(team_id), "scim")
        .await?;
    // Vault grants: union over groups, strongest role per vault, pending key.
    sqlx::query(
        "INSERT INTO vault_members (vault_id, user_id, role, sealed_key, key_version, granted_by_group)
         SELECT DISTINCT ON (gv.vault_id) gv.vault_id, $2, gv.vault_role, NULL, v.key_version, gv.group_id
         FROM scim_group_vaults gv
         JOIN scim_group_members gm ON gm.group_id = gv.group_id AND gm.user_id = $2
         JOIN scim_groups g ON g.id = gv.group_id AND g.team_id = $1
         JOIN vaults v ON v.id = gv.vault_id AND v.deleted_at IS NULL
         ORDER BY gv.vault_id, CASE gv.vault_role WHEN 'manager' THEN 0 WHEN 'editor' THEN 1 ELSE 2 END
         ON CONFLICT (vault_id, user_id) DO UPDATE SET granted_by_group = EXCLUDED.granted_by_group,
             role = CASE WHEN vault_members.granted_by_group IS NULL THEN vault_members.role ELSE EXCLUDED.role END",
    )
    .bind(team_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;
    // Grants from groups the user left go away (manual grants stay).
    sqlx::query(
        "DELETE FROM vault_members vm USING vaults v
         WHERE vm.vault_id = v.id AND v.team_id = $1 AND vm.user_id = $2 AND vm.granted_by_group IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM scim_group_vaults gv JOIN scim_group_members gm ON gm.group_id = gv.group_id
                           WHERE gv.vault_id = vm.vault_id AND gm.user_id = $2)",
    )
    .bind(team_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;
    let _ = crate::events::publish(
        state,
        crate::events::Event::VaultsUpdated {
            user_ids: vec![user_id],
        },
    )
    .await;
    Ok(())
}

/// `active=false` / DELETE: sessions out, out of the team and its vaults,
/// provisioned accounts disabled (hard-deleted after 30 days elsewhere).
async fn deprovision(state: &AppState, a: &ScimAuth, user_id: Uuid) -> ScimResult<()> {
    let team_id = a.team_id;
    let role: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT m.role, u.managed_by_team_id FROM users u LEFT JOIN team_members m ON m.team_id = $1 AND m.user_id = u.id WHERE u.id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((role, managed_by)) = role else {
        return Err(ScimError::not_found("User"));
    };
    if role == "owner" {
        audit::record(
            &state.db,
            audit::Entry::system(team_id, "scim.rejected")
                .user(user_id)
                .details(
                    json!({ "reason": "owner cannot be deprovisioned", "token_id": a.token_id }),
                ),
        )
        .await;
        return Err(ScimError::mutability(
            "The team owner cannot be deactivated through SCIM",
        ));
    }
    crate::session::revoke_all(state, user_id, None).await?;
    sqlx::query("DELETE FROM scim_group_members gm USING scim_groups g WHERE g.id = gm.group_id AND g.team_id = $1 AND gm.user_id = $2")
        .bind(team_id)
        .bind(user_id)
        .execute(&state.db)
        .await?;
    sqlx::query(
        "DELETE FROM team_invites WHERE team_id = $1 AND invited_by = $2 AND accepted_at IS NULL",
    )
    .bind(team_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;
    let managed = managed_by == Some(team_id);
    if managed {
        // Keep `managed_by_team_id` so re-activation and the retention delete
        // know whose account this is; `remove` would clear it.
        sqlx::query(
            "UPDATE users SET disabled = true, scim_active = false, deprovisioned_at = now() WHERE id = $1",
        )
        .bind(user_id)
        .execute(&state.db)
        .await?;
        crate::routes::teams::remove(state, team_id, user_id).await?;
        sqlx::query("UPDATE users SET managed_by_team_id = $2 WHERE id = $1")
            .bind(user_id)
            .bind(team_id)
            .execute(&state.db)
            .await?;
    } else {
        crate::routes::teams::remove(state, team_id, user_id).await?;
    }
    audit::record(
        &state.db,
        audit::Entry::system(team_id, "member.removed")
            .user(user_id)
            .details(
                json!({ "source": "scim", "account_disabled": managed, "token_id": a.token_id }),
            ),
    )
    .await;
    Ok(())
}

async fn reactivate(state: &AppState, a: &ScimAuth, user_id: Uuid) -> ScimResult<()> {
    let team_id = a.team_id;
    let row: Option<(Option<Uuid>, Option<DateTime<Utc>>, bool)> = sqlx::query_as(
        "SELECT managed_by_team_id, deprovisioned_at, disabled FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((managed_by, deprovisioned_at, _disabled)) = row else {
        return Err(ScimError::not_found("User"));
    };
    if managed_by == Some(team_id) {
        if let Some(t) = deprovisioned_at
            && Utc::now() - t > chrono::Duration::days(REACTIVATION_DAYS)
        {
            return Err(ScimError::new(
                StatusCode::GONE,
                None,
                "This account was deprovisioned more than 30 days ago; provision a new one",
            ));
        }
        sqlx::query(
            "UPDATE users SET disabled = false, scim_active = true, deprovisioned_at = NULL WHERE id = $1 AND managed_by_team_id = $2",
        )
        .bind(user_id)
        .bind(team_id)
        .execute(&state.db)
        .await?;
    }
    join_team(state, team_id, user_id).await?;
    audit::record(
        &state.db,
        audit::Entry::system(team_id, "scim.user_activated")
            .user(user_id)
            .details(json!({ "token_id": a.token_id })),
    )
    .await;
    Ok(())
}

async fn apply_user_attrs(
    state: &AppState,
    team_id: Uuid,
    user_id: Uuid,
    v: &Value,
    full: bool,
) -> ScimResult<()> {
    let managed: (bool,) =
        sqlx::query_as("SELECT COALESCE(managed_by_team_id = $2, false) FROM users WHERE id = $1")
            .bind(user_id)
            .bind(team_id)
            .fetch_one(&state.db)
            .await?;
    if let Some(ext) = str_at(v, &["externalId"]) {
        sqlx::query("UPDATE users SET scim_external_id = $2 WHERE id = $1")
            .bind(user_id)
            .bind(ext)
            .execute(&state.db)
            .await?;
    }
    if managed.0 {
        if let Some(name) = display_of(v) {
            sqlx::query("UPDATE users SET display_name = $2 WHERE id = $1")
                .bind(user_id)
                .bind(name)
                .execute(&state.db)
                .await?;
        }
        if let Some(email) = email_of(v) {
            let taken: Option<(Uuid,)> =
                sqlx::query_as("SELECT id FROM users WHERE lower(email) = $1 AND id <> $2")
                    .bind(&email)
                    .bind(user_id)
                    .fetch_optional(&state.db)
                    .await?;
            if taken.is_some() {
                return Err(ScimError::uniqueness("userName already in use"));
            }
            sqlx::query("UPDATE users SET email = $2 WHERE id = $1")
                .bind(user_id)
                .bind(&email)
                .execute(&state.db)
                .await?;
        }
    }
    let _ = full;
    Ok(())
}

async fn put_user(
    State(state): State<AppState>,
    a: ScimAuth,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> ScimResult<Response> {
    let r = load_user(&state, a.team_id, id).await?;
    if r.6 != Some(a.team_id)
        && let Some(new_email) = email_of(&body)
        && !new_email.eq_ignore_ascii_case(&r.1)
    {
        return Err(ScimError::mutability(
            "This account is not managed by the team; its email cannot be changed",
        ));
    }
    apply_user_attrs(&state, a.team_id, id, &body, true).await?;
    match body.get("active").and_then(|v| v.as_bool()) {
        Some(false) => deprovision(&state, &a, id).await?,
        Some(true) if !(r.4 && !r.5 && r.8) => reactivate(&state, &a, id).await?,
        _ => {}
    }
    let r = match load_user(&state, a.team_id, id).await {
        Ok(r) => r,
        Err(_) => {
            let mut gone = r.clone();
            gone.4 = false;
            gone.8 = false;
            gone
        }
    };
    Ok(scim_json(
        StatusCode::OK,
        user_resource(&state, a.team_id, &r).await?,
    ))
}

/// PATCH operations → a merged value object plus the `active` flag.
fn patch_to_value(body: &Value) -> ScimResult<(Value, Option<bool>)> {
    if !body["schemas"]
        .as_array()
        .is_some_and(|s| s.iter().any(|x| x == PATCH_SCHEMA))
    {
        return Err(ScimError::new(
            StatusCode::BAD_REQUEST,
            Some("invalidSyntax"),
            "PatchOp schema required",
        ));
    }
    let mut merged = json!({});
    let mut active = None;
    for op in body["Operations"].as_array().unwrap_or(&Vec::new()) {
        let kind = op["op"].as_str().unwrap_or("").to_ascii_lowercase();
        if kind == "remove" {
            continue;
        }
        let value = &op["value"];
        match op["path"].as_str().map(str::trim) {
            None | Some("") => {
                if let Some(obj) = value.as_object() {
                    for (k, v) in obj {
                        if k == "active" {
                            active = v
                                .as_bool()
                                .or_else(|| v.as_str().map(|s| s.eq_ignore_ascii_case("true")));
                        } else {
                            merged[k] = v.clone();
                        }
                    }
                }
            }
            Some("active") => {
                active = value
                    .as_bool()
                    .or_else(|| value.as_str().map(|s| s.eq_ignore_ascii_case("true")))
            }
            Some("userName") => merged["userName"] = value.clone(),
            Some("externalId") => merged["externalId"] = value.clone(),
            Some("displayName") => merged["displayName"] = value.clone(),
            Some(p) if p.starts_with("name.") => merged["name"][&p[5..]] = value.clone(),
            Some(p) if p.starts_with("emails") && p.ends_with(".value") => {
                merged["userName"] = value.clone()
            }
            Some(_) => {}
        }
    }
    Ok((merged, active))
}

async fn patch_user(
    State(state): State<AppState>,
    a: ScimAuth,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> ScimResult<Response> {
    let r = load_user(&state, a.team_id, id).await?;
    let (merged, active) = patch_to_value(&body)?;
    if r.6 != Some(a.team_id)
        && let Some(new_email) = email_of(&merged)
        && !new_email.eq_ignore_ascii_case(&r.1)
    {
        return Err(ScimError::mutability(
            "This account is not managed by the team; its email cannot be changed",
        ));
    }
    apply_user_attrs(&state, a.team_id, id, &merged, false).await?;
    match active {
        Some(false) => deprovision(&state, &a, id).await?,
        Some(true) if !(r.4 && !r.5 && r.8) => reactivate(&state, &a, id).await?,
        _ => {}
    }
    let r = match load_user(&state, a.team_id, id).await {
        Ok(r) => r,
        Err(_) => {
            let mut gone = r.clone();
            gone.4 = false;
            gone.8 = false;
            gone
        }
    };
    Ok(scim_json(
        StatusCode::OK,
        user_resource(&state, a.team_id, &r).await?,
    ))
}

async fn delete_user(
    State(state): State<AppState>,
    a: ScimAuth,
    Path(id): Path<Uuid>,
) -> ScimResult<Response> {
    load_user(&state, a.team_id, id).await?;
    deprovision(&state, &a, id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ───────────────────────────── groups ─────────────────────────────

type GroupRow = (Uuid, String, Option<String>, DateTime<Utc>);

async fn group_resource(state: &AppState, _team_id: Uuid, g: &GroupRow) -> ScimResult<Value> {
    let members: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT u.id, u.email FROM scim_group_members gm JOIN users u ON u.id = gm.user_id WHERE gm.group_id = $1 ORDER BY u.email",
    )
    .bind(g.0)
    .fetch_all(&state.db)
    .await?;
    let b = base(state);
    Ok(json!({
        "schemas": [GROUP_SCHEMA],
        "id": g.0,
        "externalId": g.2,
        "displayName": g.1,
        "members": members.iter().map(|(id, email)| json!({ "value": id, "display": email, "$ref": format!("{b}/Users/{id}") })).collect::<Vec<_>>(),
        "meta": { "resourceType": "Group", "created": g.3, "location": format!("{b}/Groups/{}", g.0) }
    }))
}

async fn load_group(state: &AppState, team_id: Uuid, id: Uuid) -> ScimResult<GroupRow> {
    sqlx::query_as("SELECT id, display_name, external_id, created_at FROM scim_groups WHERE team_id = $1 AND id = $2")
        .bind(team_id)
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ScimError::not_found("Group"))
}

async fn list_groups(
    State(state): State<AppState>,
    a: ScimAuth,
    Query(q): Query<ListQuery>,
) -> ScimResult<Response> {
    let f = parse_filter(q.filter.as_deref())?;
    let (start, count) = paging(&q);
    let (total,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM scim_groups WHERE team_id = $1 AND ($2::text IS NULL OR lower(display_name) = lower($2))
           AND ($3::text IS NULL OR external_id = $3) AND ($4::uuid IS NULL OR id = $4)",
    )
    .bind(a.team_id)
    .bind(&f.display_name)
    .bind(&f.external_id)
    .bind(f.id)
    .fetch_one(&state.db)
    .await?;
    let rows: Vec<GroupRow> = sqlx::query_as(
        "SELECT id, display_name, external_id, created_at FROM scim_groups
         WHERE team_id = $1 AND ($2::text IS NULL OR lower(display_name) = lower($2))
           AND ($3::text IS NULL OR external_id = $3) AND ($4::uuid IS NULL OR id = $4)
         ORDER BY display_name OFFSET $5 LIMIT $6",
    )
    .bind(a.team_id)
    .bind(&f.display_name)
    .bind(&f.external_id)
    .bind(f.id)
    .bind(start - 1)
    .bind(count)
    .fetch_all(&state.db)
    .await?;
    let mut resources = Vec::new();
    for g in &rows {
        resources.push(group_resource(&state, a.team_id, g).await?);
    }
    Ok(scim_json(
        StatusCode::OK,
        json!({ "schemas": [LIST_SCHEMA], "totalResults": total, "startIndex": start, "itemsPerPage": resources.len(), "Resources": resources }),
    ))
}

async fn get_group(
    State(state): State<AppState>,
    a: ScimAuth,
    Path(id): Path<Uuid>,
) -> ScimResult<Response> {
    let g = load_group(&state, a.team_id, id).await?;
    Ok(scim_json(
        StatusCode::OK,
        group_resource(&state, a.team_id, &g).await?,
    ))
}

fn member_ids(v: &Value) -> Vec<Uuid> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["value"].as_str().and_then(|s| s.parse().ok()))
                .collect()
        })
        .unwrap_or_default()
}

/// Replace the member set of a group; members must be in scope of the team
/// (team members or provisioned accounts) and are joined to the team.
async fn set_members(
    state: &AppState,
    a: &ScimAuth,
    group_id: Uuid,
    members: &[Uuid],
    mode: &str,
) -> ScimResult<()> {
    let team_id = a.team_id;
    let before: Vec<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM scim_group_members WHERE group_id = $1")
            .bind(group_id)
            .fetch_all(&state.db)
            .await?;
    let before: Vec<Uuid> = before.into_iter().map(|r| r.0).collect();
    let mut touched: Vec<Uuid> = Vec::new();
    match mode {
        "replace" => {
            sqlx::query("DELETE FROM scim_group_members WHERE group_id = $1")
                .bind(group_id)
                .execute(&state.db)
                .await?;
            touched.extend(before.iter().copied());
        }
        "remove" => {
            sqlx::query("DELETE FROM scim_group_members WHERE group_id = $1 AND user_id = ANY($2)")
                .bind(group_id)
                .bind(members)
                .execute(&state.db)
                .await?;
            touched.extend(members.iter().copied());
        }
        _ => {}
    }
    if mode != "remove" {
        for &uid in members {
            let in_scope: (bool,) = sqlx::query_as(
                "SELECT EXISTS (SELECT 1 FROM users u LEFT JOIN team_members m ON m.team_id = $1 AND m.user_id = u.id
                                WHERE u.id = $2 AND (m.user_id IS NOT NULL OR u.managed_by_team_id = $1))",
            )
            .bind(team_id)
            .bind(uid)
            .fetch_one(&state.db)
            .await?;
            if !in_scope.0 {
                return Err(ScimError::invalid(format!(
                    "member {uid} is not a user of this team"
                )));
            }
            sqlx::query("INSERT INTO scim_group_members (group_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(group_id)
                .bind(uid)
                .execute(&state.db)
                .await?;
            let disabled: (bool,) = sqlx::query_as("SELECT disabled FROM users WHERE id = $1")
                .bind(uid)
                .fetch_one(&state.db)
                .await?;
            if !disabled.0 {
                join_team(state, team_id, uid).await?;
            }
            touched.push(uid);
        }
    }
    touched.sort();
    touched.dedup();
    for uid in touched {
        sync_user_groups(state, team_id, uid).await?;
    }
    sqlx::query("UPDATE scim_groups SET updated_at = now() WHERE id = $1")
        .bind(group_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

async fn create_group(
    State(state): State<AppState>,
    a: ScimAuth,
    Json(body): Json<Value>,
) -> ScimResult<Response> {
    let Some(name) = str_at(&body, &["displayName"]) else {
        return Err(ScimError::invalid("displayName is required"));
    };
    if name.len() > 200 {
        return Err(ScimError::invalid("displayName is too long"));
    }
    let dup: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM scim_groups WHERE team_id = $1 AND lower(display_name) = lower($2)",
    )
    .bind(a.team_id)
    .bind(name)
    .fetch_optional(&state.db)
    .await?;
    if dup.is_some() {
        return Err(ScimError::uniqueness(
            "A group with this displayName exists",
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO scim_groups (id, team_id, external_id, display_name) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(a.team_id)
    .bind(str_at(&body, &["externalId"]))
    .bind(name)
    .execute(&state.db)
    .await?;
    set_members(&state, &a, id, &member_ids(&body["members"]), "replace").await?;
    audit::record(
        &state.db,
        audit::Entry::system(a.team_id, "scim.group_created")
            .details(json!({ "group_id": id, "display_name": name, "token_id": a.token_id })),
    )
    .await;
    let g = load_group(&state, a.team_id, id).await?;
    Ok(scim_json(
        StatusCode::CREATED,
        group_resource(&state, a.team_id, &g).await?,
    ))
}

async fn rename_group(state: &AppState, a: &ScimAuth, id: Uuid, body: &Value) -> ScimResult<()> {
    if let Some(name) = str_at(body, &["displayName"]) {
        let dup: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM scim_groups WHERE team_id = $1 AND lower(display_name) = lower($2) AND id <> $3")
            .bind(a.team_id)
            .bind(name)
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
        if dup.is_some() {
            return Err(ScimError::uniqueness(
                "A group with this displayName exists",
            ));
        }
        sqlx::query("UPDATE scim_groups SET display_name = $3, updated_at = now() WHERE team_id = $1 AND id = $2")
            .bind(a.team_id)
            .bind(id)
            .bind(name)
            .execute(&state.db)
            .await?;
    }
    if let Some(ext) = str_at(body, &["externalId"]) {
        sqlx::query("UPDATE scim_groups SET external_id = $3 WHERE team_id = $1 AND id = $2")
            .bind(a.team_id)
            .bind(id)
            .bind(ext)
            .execute(&state.db)
            .await?;
    }
    Ok(())
}

async fn put_group(
    State(state): State<AppState>,
    a: ScimAuth,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> ScimResult<Response> {
    load_group(&state, a.team_id, id).await?;
    rename_group(&state, &a, id, &body).await?;
    if body.get("members").is_some() {
        set_members(&state, &a, id, &member_ids(&body["members"]), "replace").await?;
    }
    let g = load_group(&state, a.team_id, id).await?;
    Ok(scim_json(
        StatusCode::OK,
        group_resource(&state, a.team_id, &g).await?,
    ))
}

async fn patch_group(
    State(state): State<AppState>,
    a: ScimAuth,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> ScimResult<Response> {
    load_group(&state, a.team_id, id).await?;
    if !body["schemas"]
        .as_array()
        .is_some_and(|s| s.iter().any(|x| x == PATCH_SCHEMA))
    {
        return Err(ScimError::new(
            StatusCode::BAD_REQUEST,
            Some("invalidSyntax"),
            "PatchOp schema required",
        ));
    }
    for op in body["Operations"].as_array().unwrap_or(&Vec::new()) {
        let kind = op["op"].as_str().unwrap_or("").to_ascii_lowercase();
        let path = op["path"].as_str().map(str::trim).unwrap_or("");
        let value = &op["value"];
        if path.is_empty() {
            // Entra-style: value object with displayName / members.
            rename_group(&state, &a, id, value).await?;
            if value.get("members").is_some() {
                let mode = if kind == "add" { "add" } else { "replace" };
                set_members(&state, &a, id, &member_ids(&value["members"]), mode).await?;
            }
        } else if path == "displayName" || path == "externalId" {
            rename_group(&state, &a, id, &json!({ path: value })).await?;
        } else if path == "members" {
            let mode = match kind.as_str() {
                "add" => "add",
                "remove" => "remove",
                _ => "replace",
            };
            set_members(&state, &a, id, &member_ids(value), mode).await?;
        } else if let Some(rest) = path.strip_prefix("members[") {
            // `members[value eq "<id>"]` with op remove (Okta / Entra).
            if kind == "remove"
                && let Some(idx) = rest.find("eq ")
            {
                let v = rest[idx + 3..]
                    .trim()
                    .trim_end_matches(']')
                    .trim_matches('"');
                if let Ok(uid) = v.parse::<Uuid>() {
                    set_members(&state, &a, id, &[uid], "remove").await?;
                }
            }
        }
    }
    let g = load_group(&state, a.team_id, id).await?;
    Ok(scim_json(
        StatusCode::OK,
        group_resource(&state, a.team_id, &g).await?,
    ))
}

async fn delete_group(
    State(state): State<AppState>,
    a: ScimAuth,
    Path(id): Path<Uuid>,
) -> ScimResult<Response> {
    let g = load_group(&state, a.team_id, id).await?;
    let members: Vec<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM scim_group_members WHERE group_id = $1")
            .bind(id)
            .fetch_all(&state.db)
            .await?;
    sqlx::query("DELETE FROM scim_groups WHERE team_id = $1 AND id = $2")
        .bind(a.team_id)
        .bind(id)
        .execute(&state.db)
        .await?;
    for (uid,) in members {
        sync_user_groups(&state, a.team_id, uid).await?;
    }
    audit::record(
        &state.db,
        audit::Entry::system(a.team_id, "scim.group_deleted")
            .details(json!({ "group_id": id, "display_name": g.1, "token_id": a.token_id })),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Re-apply group → role / vault grants for one user (team admin changed grants).
pub async fn resync_user(
    state: &AppState,
    team_id: Uuid,
    user_id: Uuid,
) -> crate::error::ApiResult<()> {
    sync_user_groups(state, team_id, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("scim resync: {}", e.detail).into())
}

/// Every 6 h on one replica: hard-delete provisioned accounts deprovisioned
/// more than 30 days ago.
pub async fn run_retention(state: AppState) {
    crate::jobs::exclusive(
        state,
        "scim-retention",
        std::time::Duration::from_secs(6 * 3600),
        retention_pass,
    )
    .await
}

pub async fn retention_pass(state: AppState) -> anyhow::Result<u64> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM users WHERE managed_by_team_id IS NOT NULL AND disabled AND deprovisioned_at < now() - make_interval(days => $1) LIMIT 100",
    )
    .bind(REACTIVATION_DAYS as i32)
    .fetch_all(&state.db)
    .await?;
    let mut n = 0;
    for (id,) in rows {
        match crate::users::delete(&state, id).await {
            Ok(()) => n += 1,
            Err(e) => tracing::warn!(error = %e, user = %id, "scim retention delete failed"),
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_and_patches() {
        let f = parse_filter(Some(r#"userName eq "a@b.c" and active eq true"#)).unwrap();
        assert_eq!(f.user_name.as_deref(), Some("a@b.c"));
        assert_eq!(f.active, Some(true));
        assert!(parse_filter(Some("userName co \"x\"")).is_err());
        let (merged, active) = patch_to_value(&json!({
            "schemas": [PATCH_SCHEMA],
            "Operations": [
                { "op": "Replace", "path": "active", "value": "False" },
                { "op": "replace", "path": "name.givenName", "value": "Ann" },
                { "op": "add", "value": { "externalId": "ext-1" } }
            ]
        }))
        .unwrap();
        assert_eq!(active, Some(false));
        assert_eq!(merged["name"]["givenName"], "Ann");
        assert_eq!(merged["externalId"], "ext-1");
        assert_eq!(
            email_of(
                &json!({ "userName": "login", "emails": [{ "value": "X@Y.io", "type": "work" }] })
            )
            .as_deref(),
            Some("x@y.io")
        );
    }
}
