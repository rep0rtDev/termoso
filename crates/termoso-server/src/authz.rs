//! Fine-grained team permissions. A member holds exactly one role; a role is a
//! set of `domain.action` keys (see `termoso_proto::team::PERMISSIONS`). The
//! owner always has everything regardless of role contents.

use std::collections::HashSet;

use sqlx::PgExecutor;
use termoso_proto::error::codes;
use termoso_proto::team::{PermissionInfo, RoleRef, TeamRole, PERMISSIONS};
use uuid::Uuid;

use crate::error::{ApiResult, Error};
use crate::state::AppState;

/// A user's standing in a team.
#[derive(Debug, Clone)]
pub struct Membership {
    pub team_id: Uuid,
    pub user_id: Uuid,
    /// Coarse level kept for older clients and for owner checks.
    pub level: TeamRole,
    pub role: RoleRef,
    pub role_source: String,
    permissions: HashSet<String>,
    wildcard: bool,
}

impl Membership {
    /// Whether the member holds `perm`.
    pub fn has(&self, perm: &str) -> bool {
        self.level == TeamRole::Owner || self.wildcard || self.permissions.contains(perm)
    }

    /// Effective permission list for API responses (`*` for the owner).
    pub fn permission_list(&self) -> Vec<String> {
        if self.level == TeamRole::Owner || self.wildcard {
            return vec!["*".to_string()];
        }
        let mut v: Vec<String> = self.permissions.iter().cloned().collect();
        v.sort();
        v
    }

    /// Admin-or-owner, the pre-0.9.0 notion; prefer `has`.
    pub fn is_admin(&self) -> bool {
        self.level.is_admin()
    }

    /// `permission_denied` for `perm`.
    pub fn denied(&self, perm: &str) -> Error {
        denied(perm)
    }
}

/// `403 permission_denied` naming the missing permission.
pub fn denied(perm: &str) -> Error {
    Error::new(
        axum::http::StatusCode::FORBIDDEN,
        codes::PERMISSION_DENIED,
        format!("This needs the `{perm}` permission in the team"),
    )
    .with_details(serde_json::json!({ "permission": perm }))
}

/// Load the membership, `404 Team` when the user is not a member.
pub async fn membership<'e, E: PgExecutor<'e>>(
    db: E,
    team_id: Uuid,
    user_id: Uuid,
) -> ApiResult<Membership> {
    let row: Option<(String, Uuid, String, String, Vec<String>, String)> = sqlx::query_as(
        "SELECT m.role, r.id, r.slug, r.name, r.permissions, m.role_source
         FROM team_members m JOIN team_roles r ON r.id = m.role_id
         WHERE m.team_id = $1 AND m.user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    let (level, id, slug, name, perms, source) = row.ok_or_else(|| Error::not_found("Team"))?;
    Ok(Membership {
        team_id,
        user_id,
        level: crate::routes::teams::parse_team_role(&level),
        role: RoleRef { id, slug, name },
        role_source: source,
        wildcard: perms.iter().any(|p| p == "*"),
        permissions: perms.into_iter().collect(),
    })
}

/// Membership that holds `perm`, or `403 permission_denied`.
pub async fn require(
    state: &AppState,
    team_id: Uuid,
    user_id: Uuid,
    perm: &str,
) -> ApiResult<Membership> {
    let m = membership(&state.db, team_id, user_id).await?;
    if !m.has(perm) {
        return Err(denied(perm));
    }
    Ok(m)
}

/// Validate a permission set for a custom/admin role: known keys only, no
/// owner-only keys, deduplicated and sorted.
pub fn normalize_permissions(perms: &[String]) -> ApiResult<Vec<String>> {
    use termoso_proto::team::{is_permission, OWNER_ONLY_PERMISSIONS};
    let mut set = HashSet::new();
    for p in perms {
        let p = p.trim();
        if p == "*" || OWNER_ONLY_PERMISSIONS.contains(&p) {
            return Err(Error::bad_request(format!("`{p}` belongs to the owner only")));
        }
        if !is_permission(p) {
            return Err(Error::bad_request(format!("Unknown permission `{p}`")));
        }
        set.insert(p.to_string());
    }
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    Ok(v)
}

/// Human-readable catalog for the cabinet.
pub fn catalog() -> Vec<PermissionInfo> {
    PERMISSIONS
        .iter()
        .map(|&key| {
            let (title, description, dangerous) = describe(key);
            PermissionInfo {
                key: key.to_string(),
                group: key.split('.').next().unwrap_or("").to_string(),
                title: title.to_string(),
                description: description.to_string(),
                dangerous,
            }
        })
        .collect()
}

fn describe(key: &str) -> (&'static str, &'static str, bool) {
    match key {
        "team.view" => ("View team", "See the team, its name and settings.", false),
        "team.rename" => ("Rename team", "Change the team name.", false),
        "team.delete" => ("Delete team", "Owner only.", true),
        "team.transfer" => ("Transfer ownership", "Owner only.", true),
        "members.view" => ("View members", "See who is in the team.", false),
        "members.invite" => ("Invite members", "Create and revoke invitations.", false),
        "members.remove" => ("Remove members", "Remove people from the team.", true),
        "members.role" => ("Change roles", "Assign roles to members.", true),
        "roles.manage" => ("Manage roles", "Create, edit and delete custom roles.", true),
        "vaults.create" => ("Create vaults", "Create team vaults and folders.", false),
        "vaults.delete" => ("Delete vaults", "Delete team vaults with everything inside.", true),
        "vaults.rename" => ("Rename vaults", "Rename vaults and change their settings.", false),
        "vaults.members" => ("Share vaults", "Grant and revoke access to vaults.", true),
        "vaults.rotate_key" => ("Rotate vault keys", "Re-key a vault after access changes.", false),
        "entities.read" => ("Read vault items", "See hosts, snippets and other items in granted vaults.", false),
        "entities.write" => ("Edit vault items", "Create, change and delete items in granted vaults.", false),
        "secrets.reveal" => ("See secrets", "View and copy passwords and private keys. Without it clients hide them.", true),
        "connect.ssh" => ("Connect (SSH/Mosh/Telnet)", "Open terminal sessions to team hosts.", false),
        "connect.sftp" => ("SFTP", "Transfer files to and from team hosts.", false),
        "connect.port_forward" => ("Port forwarding", "Use team port-forwarding rules.", false),
        "live.start" => ("Start multiplayer", "Share a terminal with teammates.", false),
        "live.join" => ("Join multiplayer", "Watch shared terminals.", false),
        "live.control" => ("Take control", "Type in a shared terminal when handed control.", false),
        "logs.view_own" => ("Own session logs", "See logs of own sessions.", false),
        "logs.view_team" => ("Team session logs", "See logs of teammates' sessions in team vaults.", true),
        "logs.export" => ("Export session logs", "Download session logs.", true),
        "export.vault" => ("Export vaults", "Download a vault as a file.", true),
        "import.vault" => ("Import", "Import hosts and keys into team vaults.", false),
        "audit.view" => ("View audit log", "Read the team audit log.", false),
        "audit.export" => ("Export audit log", "Download the audit log as CSV/JSONL.", false),
        "audit.webhooks" => ("Audit webhooks", "Send audit events to SIEM / webhooks.", true),
        "policies.manage" => ("Manage policies", "MFA, session, key and SSO policies.", true),
        "sso.manage" => ("Manage SSO", "Single sign-on and group → role mappings.", true),
        "scim.manage" => ("Manage SCIM", "Provisioning tokens and groups.", true),
        _ => (key_title(key), "", false),
    }
}

fn key_title(key: &str) -> &'static str {
    Box::leak(key.to_string().into_boxed_str())
}

/// Built-in roles every team gets: `(slug, name, description, permissions, position)`.
pub const BUILTIN_ROLES: &[(&str, &str, &str, &[&str], i32)] = &[
    ("owner", "Owner", "Everything, including deleting the team and transferring ownership.", &["*"], 0),
    ("admin", "Admin", "Manages members, roles, vaults, policies, audit and integrations.", &[
        "team.view", "team.rename", "members.view", "members.invite", "members.remove", "members.role", "roles.manage",
        "vaults.create", "vaults.delete", "vaults.rename", "vaults.members", "vaults.rotate_key",
        "entities.read", "entities.write", "secrets.reveal", "connect.ssh", "connect.sftp", "connect.port_forward",
        "live.start", "live.join", "live.control", "logs.view_own", "logs.view_team", "logs.export",
        "export.vault", "import.vault", "audit.view", "audit.export", "audit.webhooks",
        "policies.manage", "sso.manage", "scim.manage",
    ], 10),
    ("member", "Member", "Works with the vaults they are given access to.", &[
        "team.view", "members.view", "entities.read", "entities.write", "secrets.reveal",
        "connect.ssh", "connect.sftp", "connect.port_forward", "live.start", "live.join", "live.control",
        "logs.view_own", "import.vault", "audit.view",
    ], 20),
    ("viewer", "Viewer", "Connects to hosts but cannot change anything or see secrets.", &[
        "team.view", "members.view", "entities.read", "connect.ssh", "connect.sftp", "connect.port_forward",
        "live.join", "logs.view_own",
    ], 30),
    ("auditor", "Auditor", "Reads the audit log and team session logs; no vault access.", &[
        "team.view", "members.view", "audit.view", "audit.export", "logs.view_team",
    ], 40),
];

/// Create the built-in roles for a new team and set `member` as default.
/// Returns the owner role id.
pub async fn seed_builtin_roles(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    team_id: Uuid,
) -> ApiResult<Uuid> {
    let mut owner = Uuid::nil();
    for (slug, name, desc, perms, pos) in BUILTIN_ROLES {
        let id = Uuid::new_v4();
        let perms: Vec<String> = perms.iter().map(|s| s.to_string()).collect();
        sqlx::query(
            "INSERT INTO team_roles (id, team_id, slug, name, description, permissions, builtin, position)
             VALUES ($1, $2, $3, $4, $5, $6, true, $7)",
        )
        .bind(id).bind(team_id).bind(slug).bind(name).bind(desc).bind(&perms).bind(pos)
        .execute(&mut **tx)
        .await?;
        if *slug == "owner" {
            owner = id;
        }
        if *slug == "member" {
            sqlx::query("UPDATE teams SET default_role_id = $2 WHERE id = $1")
                .bind(team_id).bind(id).execute(&mut **tx).await?;
        }
    }
    Ok(owner)
}

/// Id of a built-in role by slug.
pub async fn builtin_role_id<'e, E: PgExecutor<'e>>(db: E, team_id: Uuid, slug: &str) -> ApiResult<Uuid> {
    let (id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM team_roles WHERE team_id = $1 AND slug = $2 AND builtin")
            .bind(team_id).bind(slug).fetch_one(db).await?;
    Ok(id)
}

/// Role for a coarse level, used by pre-0.9.0 request bodies.
pub async fn role_id_for_level<'e, E: PgExecutor<'e>>(db: E, team_id: Uuid, level: TeamRole) -> ApiResult<Uuid> {
    let slug = match level { TeamRole::Owner => "owner", TeamRole::Admin => "admin", TeamRole::Member => "member" };
    builtin_role_id(db, team_id, slug).await
}

/// Default role for new members (falls back to built-in `member`).
pub async fn default_role_id<'e, E: PgExecutor<'e>>(db: E, team_id: Uuid) -> ApiResult<Uuid> {
    let row: Option<(Option<Uuid>,)> = sqlx::query_as("SELECT default_role_id FROM teams WHERE id = $1")
        .bind(team_id).fetch_optional(db).await?;
    if let Some((Some(id),)) = row { return Ok(id); }
    Err(Error::not_found("Team"))
}

/// `RoleRef` + effective permissions from a joined row.
pub fn role_parts(level: TeamRole, id: Uuid, slug: String, name: String, perms: Vec<String>) -> (RoleRef, Vec<String>) {
    let list = if level == TeamRole::Owner || perms.iter().any(|p| p == "*") {
        vec!["*".to_string()]
    } else {
        let mut v = perms; v.sort(); v
    };
    (RoleRef { id, slug, name }, list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_roles_only_use_known_permissions() {
        for (slug, _, _, perms, _) in BUILTIN_ROLES {
            for p in *perms {
                assert!(*p == "*" || termoso_proto::team::is_permission(p), "{slug}: {p}");
            }
        }
        assert_eq!(catalog().len(), PERMISSIONS.len());
    }

    #[test]
    fn normalize_rejects_owner_only_and_unknown() {
        assert!(normalize_permissions(&["team.delete".into()]).is_err());
        assert!(normalize_permissions(&["*".into()]).is_err());
        assert!(normalize_permissions(&["nope.x".into()]).is_err());
        let v = normalize_permissions(&["vaults.create".into(), " vaults.create ".into(), "audit.view".into()]).unwrap();
        assert_eq!(v, vec!["audit.view", "vaults.create"]);
    }

    #[test]
    fn owner_has_everything() {
        let m = Membership {
            team_id: Uuid::nil(),
            user_id: Uuid::nil(),
            level: TeamRole::Owner,
            role: RoleRef { id: Uuid::nil(), slug: "owner".into(), name: "Owner".into() },
            role_source: "manual".into(),
            permissions: HashSet::new(),
            wildcard: false,
        };
        assert!(m.has("scim.manage"));
        assert_eq!(m.permission_list(), vec!["*"]);
        let v = Membership { level: TeamRole::Member, permissions: ["team.view".to_string()].into_iter().collect(), ..m };
        assert!(v.has("team.view") && !v.has("vaults.create"));
    }
}
