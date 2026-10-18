//! Team roles, the permission catalog and group → role mappings.

use axum::Json;
use axum::extract::{Path, Query, State};
use chrono::{DateTime, Utc};
use termoso_proto::team::*;
use uuid::Uuid;

use crate::audit;
use crate::authz;
use crate::error::{ApiResult, Error, NoContent};
use crate::extract::{Auth, Json as Body};
use crate::state::AppState;

const MAX_CUSTOM_ROLES: i64 = 32;

fn slugify(name: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "role".to_string() } else { s }
}

fn validate_name(name: &str) -> ApiResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 40 {
        return Err(Error::bad_request("Role name must be 1–40 characters"));
    }
    Ok(n.to_string())
}

fn validate_description(d: &str) -> ApiResult<String> {
    if d.chars().count() > 200 {
        return Err(Error::bad_request("Description is too long"));
    }
    Ok(d.trim().to_string())
}

type RoleRow = (
    Uuid,
    String,
    String,
    String,
    Vec<String>,
    bool,
    i32,
    i64,
    DateTime<Utc>,
    DateTime<Utc>,
);

fn to_def(r: RoleRow) -> TeamRoleDef {
    let (
        id,
        slug,
        name,
        description,
        permissions,
        builtin,
        position,
        member_count,
        created_at,
        updated_at,
    ) = r;
    TeamRoleDef {
        id,
        slug,
        name,
        description,
        permissions,
        builtin,
        position,
        member_count,
        created_at,
        updated_at,
    }
}

macro_rules! role_select {
    ($tail:literal) => {
        concat!(
            "SELECT r.id, r.slug, r.name, r.description, r.permissions, r.builtin, r.position,
                (SELECT count(*) FROM team_members m WHERE m.role_id = r.id), r.created_at, r.updated_at
             FROM team_roles r ",
            $tail
        )
    };
}

async fn load_role(state: &AppState, team_id: Uuid, role_id: Uuid) -> ApiResult<TeamRoleDef> {
    let row: Option<RoleRow> = sqlx::query_as(role_select!("WHERE r.team_id = $1 AND r.id = $2"))
        .bind(team_id)
        .bind(role_id)
        .fetch_optional(&state.db)
        .await?;
    row.map(to_def).ok_or_else(|| Error::not_found("Role"))
}

#[utoipa::path(get, path = "/api/v1/teams/{id}/permissions", tag = "teams",
    responses((status = 200, body = PermissionCatalog)))]
pub async fn catalog(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<PermissionCatalog>> {
    authz::require(&state, id, auth.user_id(), "team.view").await?;
    Ok(Json(PermissionCatalog {
        permissions: authz::catalog(),
    }))
}

#[utoipa::path(get, path = "/api/v1/teams/{id}/roles", tag = "teams",
    responses((status = 200, body = TeamRoleList)))]
pub async fn list(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<TeamRoleList>> {
    authz::require(&state, id, auth.user_id(), "team.view").await?;
    let rows: Vec<RoleRow> = sqlx::query_as(role_select!(
        "WHERE r.team_id = $1 ORDER BY r.position, r.created_at"
    ))
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let (default_role_id,): (Option<Uuid>,) =
        sqlx::query_as("SELECT default_role_id FROM teams WHERE id = $1")
            .bind(id)
            .fetch_one(&state.db)
            .await?;
    Ok(Json(TeamRoleList {
        roles: rows.into_iter().map(to_def).collect(),
        default_role_id,
    }))
}

#[utoipa::path(post, path = "/api/v1/teams/{id}/roles", tag = "teams",
    request_body = CreateRoleRequest, responses((status = 200, body = TeamRoleDef)))]
pub async fn create(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<CreateRoleRequest>,
) -> ApiResult<Json<TeamRoleDef>> {
    let me = authz::require(&state, id, auth.user_id(), "roles.manage").await?;
    let name = validate_name(&req.name)?;
    let description = validate_description(&req.description)?;
    let (count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM team_roles WHERE team_id = $1 AND NOT builtin")
            .bind(id)
            .fetch_one(&state.db)
            .await?;
    if count >= MAX_CUSTOM_ROLES {
        return Err(Error::bad_request(format!(
            "A team can have at most {MAX_CUSTOM_ROLES} custom roles"
        )));
    }
    let mut perms = req.permissions.clone();
    if let (true, Some(src)) = (perms.is_empty(), req.copy_from) {
        let src = load_role(&state, id, src).await?;
        perms = src.permissions.into_iter().filter(|p| p != "*").collect();
    }
    let perms = authz::normalize_permissions(&perms)?;
    // A non-owner cannot hand out more than they hold themselves.
    if let Some(p) = perms.iter().find(|p| !me.has(p)) {
        return Err(Error::bad_request(format!(
            "You cannot grant `{p}`: you do not hold it"
        )));
    }
    let base = slugify(&name);
    let mut slug = base.clone();
    for n in 2.. {
        let (taken,): (bool,) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM team_roles WHERE team_id = $1 AND slug = $2)",
        )
        .bind(id)
        .bind(&slug)
        .fetch_one(&state.db)
        .await?;
        if !taken {
            break;
        }
        slug = format!("{base}-{n}");
    }
    let role_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO team_roles (id, team_id, slug, name, description, permissions, builtin, position)
         VALUES ($1, $2, $3, $4, $5, $6, false, 100)",
    )
    .bind(role_id).bind(id).bind(&slug).bind(&name).bind(&description).bind(&perms)
    .execute(&state.db)
    .await?;
    audit::record(&state.db, audit::Entry {
        details: serde_json::json!({ "role_id": role_id, "slug": slug, "name": name, "permissions": perms }),
        ..audit::Entry::new(id, &auth, "role.created")
    })
    .await;
    Ok(Json(load_role(&state, id, role_id).await?))
}

#[utoipa::path(patch, path = "/api/v1/teams/{id}/roles/{role_id}", tag = "teams",
    request_body = UpdateRoleRequest, responses((status = 200, body = TeamRoleDef)))]
pub async fn update(
    State(state): State<AppState>,
    auth: Auth,
    Path((id, role_id)): Path<(Uuid, Uuid)>,
    Body(req): Body<UpdateRoleRequest>,
) -> ApiResult<Json<TeamRoleDef>> {
    let me = authz::require(&state, id, auth.user_id(), "roles.manage").await?;
    let cur = load_role(&state, id, role_id).await?;
    if cur.slug == "owner" {
        return Err(Error::bad_request("The owner role cannot be changed"));
    }
    let name = match &req.name {
        Some(n) => validate_name(n)?,
        None => cur.name.clone(),
    };
    let description = match &req.description {
        Some(d) => validate_description(d)?,
        None => cur.description.clone(),
    };
    let perms = match &req.permissions {
        Some(p) => {
            let p = authz::normalize_permissions(p)?;
            if let Some(x) = p.iter().find(|x| !me.has(x)) {
                return Err(Error::bad_request(format!(
                    "You cannot grant `{x}`: you do not hold it"
                )));
            }
            // Never lock the team out of role administration: the editor
            // keeps `roles.manage` on their own role unless they are the owner.
            if me.role.id == role_id
                && me.level != TeamRole::Owner
                && !p.iter().any(|x| x == "roles.manage")
            {
                return Err(Error::bad_request(
                    "You cannot remove `roles.manage` from your own role",
                ));
            }
            p
        }
        None => cur.permissions.clone(),
    };
    let position = req.position.unwrap_or(cur.position);
    sqlx::query(
        "UPDATE team_roles SET name = $3, description = $4, permissions = $5, position = $6, updated_at = now()
         WHERE team_id = $1 AND id = $2",
    )
    .bind(id).bind(role_id).bind(&name).bind(&description).bind(&perms).bind(position)
    .execute(&state.db)
    .await?;
    let added: Vec<&String> = perms
        .iter()
        .filter(|p| !cur.permissions.contains(p))
        .collect();
    let removed: Vec<&String> = cur
        .permissions
        .iter()
        .filter(|p| !perms.contains(p))
        .collect();
    audit::record(
        &state.db,
        audit::Entry {
            details: serde_json::json!({ "role_id": role_id, "slug": cur.slug, "name": name,
            "added": added, "removed": removed }),
            ..audit::Entry::new(id, &auth, "role.updated")
        },
    )
    .await;
    Ok(Json(load_role(&state, id, role_id).await?))
}

#[derive(serde::Deserialize)]
pub struct DeleteParams {
    /// Move members holding the role here; defaults to the team's default role.
    #[serde(default)]
    pub reassign_to: Option<Uuid>,
}

#[utoipa::path(delete, path = "/api/v1/teams/{id}/roles/{role_id}", tag = "teams",
    responses((status = 204)))]
pub async fn delete(
    State(state): State<AppState>,
    auth: Auth,
    Path((id, role_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<DeleteParams>,
) -> ApiResult<NoContent> {
    let me = authz::require(&state, id, auth.user_id(), "roles.manage").await?;
    let cur = load_role(&state, id, role_id).await?;
    if cur.builtin {
        return Err(Error::bad_request("Built-in roles cannot be deleted"));
    }
    if me.role.id == role_id {
        return Err(Error::bad_request("You cannot delete your own role"));
    }
    let target = match q.reassign_to {
        Some(t) => t,
        None => authz::default_role_id(&state.db, id).await?,
    };
    if target == role_id {
        return Err(Error::bad_request(
            "Pick a different role to move members to",
        ));
    }
    load_role(&state, id, target).await?;
    let mut tx = state.db.begin().await?;
    let moved = sqlx::query(
        "UPDATE team_members SET role_id = $3,
             role = CASE WHEN (SELECT slug FROM team_roles WHERE id = $3) = 'admin' THEN 'admin' ELSE 'member' END
         WHERE team_id = $1 AND role_id = $2",
    )
        .bind(id).bind(role_id).bind(target)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    sqlx::query("UPDATE teams SET default_role_id = $3 WHERE id = $1 AND default_role_id = $2")
        .bind(id)
        .bind(role_id)
        .bind(target)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM team_roles WHERE team_id = $1 AND id = $2")
        .bind(id)
        .bind(role_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        audit::Entry {
            details: serde_json::json!({ "role_id": role_id, "slug": cur.slug, "name": cur.name,
            "moved_members": moved, "reassigned_to": target }),
            ..audit::Entry::new(id, &auth, "role.deleted")
        },
    )
    .await;
    Ok(NoContent)
}

#[utoipa::path(put, path = "/api/v1/teams/{id}/default-role", tag = "teams",
    request_body = SetDefaultRoleRequest, responses((status = 204)))]
pub async fn set_default(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<SetDefaultRoleRequest>,
) -> ApiResult<NoContent> {
    authz::require(&state, id, auth.user_id(), "roles.manage").await?;
    let role = load_role(&state, id, req.role_id).await?;
    if role.slug == "owner" {
        return Err(Error::bad_request("The owner role cannot be the default"));
    }
    sqlx::query("UPDATE teams SET default_role_id = $2 WHERE id = $1")
        .bind(id)
        .bind(req.role_id)
        .execute(&state.db)
        .await?;
    audit::record(
        &state.db,
        audit::Entry {
            details: serde_json::json!({ "role_id": req.role_id, "slug": role.slug }),
            ..audit::Entry::new(id, &auth, "role.default_changed")
        },
    )
    .await;
    Ok(NoContent)
}

#[utoipa::path(get, path = "/api/v1/teams/{id}/role-mappings", tag = "teams",
    responses((status = 200, body = RoleMappingList)))]
pub async fn mappings(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<RoleMappingList>> {
    authz::require(&state, id, auth.user_id(), "sso.manage").await?;
    let rows: Vec<(String, Uuid, i32)> = sqlx::query_as(
        "SELECT group_name, role_id, priority FROM team_role_mappings WHERE team_id = $1
         ORDER BY priority, group_name",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(RoleMappingList {
        mappings: rows
            .into_iter()
            .map(|(group_name, role_id, priority)| RoleMapping {
                group_name,
                role_id,
                priority,
            })
            .collect(),
    }))
}

#[utoipa::path(put, path = "/api/v1/teams/{id}/role-mappings", tag = "teams",
    request_body = RoleMappingList, responses((status = 200, body = RoleMappingList)))]
pub async fn set_mappings(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<RoleMappingList>,
) -> ApiResult<Json<RoleMappingList>> {
    authz::require(&state, id, auth.user_id(), "sso.manage").await?;
    if req.mappings.len() > 200 {
        return Err(Error::bad_request("At most 200 mappings"));
    }
    let mut seen = std::collections::HashSet::new();
    for m in &req.mappings {
        let g = m.group_name.trim();
        if g.is_empty() || g.chars().count() > 200 {
            return Err(Error::bad_request("Invalid group name"));
        }
        if !seen.insert(g.to_lowercase()) {
            return Err(Error::bad_request(format!("Group `{g}` is listed twice")));
        }
        let role = load_role(&state, id, m.role_id).await?;
        if role.slug == "owner" {
            return Err(Error::bad_request("A group cannot map to the owner role"));
        }
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM team_role_mappings WHERE team_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for m in &req.mappings {
        sqlx::query(
            "INSERT INTO team_role_mappings (team_id, group_name, role_id, priority) VALUES ($1, $2, $3, $4)",
        )
        .bind(id).bind(m.group_name.trim()).bind(m.role_id).bind(m.priority)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    audit::record(
        &state.db,
        audit::Entry {
            details: serde_json::json!({ "count": req.mappings.len() }),
            ..audit::Entry::new(id, &auth, "role_mapping.updated")
        },
    )
    .await;
    mappings(State(state), auth, Path(id)).await
}

/// Resolve the role for a user given their IdP/SCIM group names: the mapping
/// with the lowest priority wins; `None` when nothing matches.
pub async fn role_for_groups<'e, E: sqlx::PgExecutor<'e>>(
    db: E,
    team_id: Uuid,
    groups: &[String],
) -> ApiResult<Option<Uuid>> {
    if groups.is_empty() {
        return Ok(None);
    }
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT role_id FROM team_role_mappings
         WHERE team_id = $1 AND lower(group_name) = ANY($2)
         ORDER BY priority, group_name LIMIT 1",
    )
    .bind(team_id)
    .bind(groups.iter().map(|g| g.to_lowercase()).collect::<Vec<_>>())
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(r,)| r))
}
