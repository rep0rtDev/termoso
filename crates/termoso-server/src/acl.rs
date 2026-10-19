//! Access rules on groups/hosts inside team vaults (`entity_acl`).
//!
//! Entities are client-encrypted, so the server only resolves *who* a rule
//! applies to and hands the result to clients, which apply group → host
//! inheritance and hide/limit entities. The one thing the server enforces
//! itself is that a directly-ruled entity below `edit` cannot be written or
//! deleted by that user. Team owners and admins are never restricted.

use std::collections::HashMap;

use sqlx::PgExecutor;
use uuid::Uuid;

use crate::error::ApiResult;

pub const MODES: &[&str] = &["none", "connect", "read", "edit"];
pub const ENTITY_KINDS: &[&str] = &["group", "host"];
pub const PRINCIPAL_KINDS: &[&str] = &["role", "member", "group"];

pub fn mode_rank(mode: &str) -> u8 {
    MODES.iter().position(|m| *m == mode).unwrap_or(0) as u8
}

fn principal_rank(kind: &str) -> u8 {
    match kind {
        "member" => 3,
        "group" => 2,
        _ => 1,
    }
}

/// Anything below `edit` blocks writes/deletes on the server.
pub fn blocks_write(mode: &str) -> bool {
    mode != "edit"
}

/// A rule resolved for one user.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub vault_id: Uuid,
    pub entity_id: Uuid,
    pub entity_kind: String,
    pub mode: String,
}

const SELECT: &str = "SELECT a.vault_id, a.entity_id, a.entity_kind, a.principal_kind, a.mode
     FROM entity_acl a
     JOIN team_members m ON m.team_id = a.team_id AND m.user_id = $1
    WHERE m.role NOT IN ('owner', 'admin')
      AND ($2::uuid IS NULL OR a.vault_id = $2)
      AND (
            (a.principal_kind = 'member' AND a.user_id = $1)
         OR (a.principal_kind = 'role' AND a.role_id = m.role_id)
         OR (a.principal_kind = 'group' AND EXISTS (
                SELECT 1 FROM scim_group_members gm
                JOIN scim_groups g ON g.id = gm.group_id
               WHERE gm.user_id = $1 AND g.team_id = a.team_id
                 AND lower(g.display_name) = lower(a.group_name)))
      )";

/// Rules that apply to `user_id` (optionally in one vault), one per entity:
/// a member rule beats a group rule beats a role rule; equal specificity →
/// the most permissive mode wins.
pub async fn rules_for_user<'e, E: PgExecutor<'e>>(
    db: E,
    user_id: Uuid,
    vault_id: Option<Uuid>,
) -> ApiResult<Vec<Resolved>> {
    let rows: Vec<(Uuid, Uuid, String, String, String)> = sqlx::query_as(SELECT)
        .bind(user_id)
        .bind(vault_id)
        .fetch_all(db)
        .await?;
    let mut best: HashMap<(Uuid, Uuid), (u8, Resolved)> = HashMap::new();
    for (vault_id, entity_id, entity_kind, principal_kind, mode) in rows {
        let rank = principal_rank(&principal_kind);
        let key = (vault_id, entity_id);
        let replace = match best.get(&key) {
            None => true,
            Some((r, cur)) => rank > *r || (rank == *r && mode_rank(&mode) > mode_rank(&cur.mode)),
        };
        if replace {
            best.insert(
                key,
                (
                    rank,
                    Resolved {
                        vault_id,
                        entity_id,
                        entity_kind,
                        mode,
                    },
                ),
            );
        }
    }
    let mut out: Vec<Resolved> = best.into_values().map(|(_, r)| r).collect();
    out.sort_by_key(|r| (r.vault_id, r.entity_id));
    Ok(out)
}

/// `entity_id → mode` for the user's directly-ruled entities in one vault.
pub async fn direct_modes<'e, E: PgExecutor<'e>>(
    db: E,
    vault_id: Uuid,
    user_id: Uuid,
) -> ApiResult<HashMap<Uuid, String>> {
    Ok(rules_for_user(db, user_id, Some(vault_id))
        .await?
        .into_iter()
        .map(|r| (r.entity_id, r.mode))
        .collect())
}
