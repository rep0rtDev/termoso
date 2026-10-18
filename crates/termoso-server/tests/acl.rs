//! Group/host access rules in team vaults: permissions, validation,
//! per-user resolution (member beats role) and server-side write denial.

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_crypto::keys::SymmetricKey;
use termoso_crypto::sealed;
use termoso_proto::sync::{EntityChange, EntityDelete, PushRequest, PushResponse, PushResult};
use termoso_proto::vault::{CreateVaultRequest, Vault, VaultList, VaultMemberUpsert, VaultRole};
use uuid::Uuid;

fn upsert(key: &SymmetricKey, u: &User, role: VaultRole) -> VaultMemberUpsert {
    VaultMemberUpsert {
        user_id: u.id(),
        role,
        sealed_key: sealed::seal_vault_key(u.keypair.public(), key).unwrap(),
    }
}

async fn push(s: &TestServer, u: &User, change: EntityChange) -> bool {
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![change],
                deletes: vec![],
            }),
        )
        .await;
    matches!(r.results[0], PushResult::Ok { .. })
}

async fn delete(s: &TestServer, u: &User, id: Uuid, base_version: i64) -> bool {
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![],
                deletes: vec![EntityDelete { id, base_version }],
            }),
        )
        .await;
    matches!(r.results[0], PushResult::Ok { .. })
}

#[tokio::test]
async fn entity_acl_is_resolved_and_enforced() {
    let Some(s) = server().await else { return };
    let owner = register(s, &unique_email("owner"), "pw-owner-1234567").await;
    let bob_email = unique_email("bob");
    let bob = register(s, &bob_email, "pw-bob-123456789").await;
    let team: serde_json::Value = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "Corp" })),
        )
        .await;
    let team_id: Uuid = team["id"].as_str().unwrap().parse().unwrap();
    let invite: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/teams/{team_id}/invites"),
            Some(owner.token()),
            Some(&serde_json::json!({ "email": bob_email, "role": "member", "vault_ids": [] })),
        )
        .await;
    let token = invite["url"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    let _: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/invites/{token}/accept"),
            Some(bob.token()),
            NOBODY,
        )
        .await;

    let key = SymmetricKey::generate();
    let vault: Vault = s
        .json(
            Method::POST,
            &format!("/teams/{team_id}/vaults"),
            Some(owner.token()),
            Some(&CreateVaultRequest {
                name: "Ops".into(),
                members: vec![
                    upsert(&key, &owner, VaultRole::Manager),
                    upsert(&key, &bob, VaultRole::Editor),
                ],
            }),
        )
        .await;
    let host_id = Uuid::new_v4();
    let change = |u: &User, base: Option<i64>, label: &str| EntityChange {
        id: host_id,
        kind: "host".into(),
        vault_id: vault.id,
        base_version: base,
        key_version: 1,
        data: u.encrypt_entity(&key, "host", host_id, &format!(r#"{{"label":"{label}"}}"#)),
        updated_at: chrono::Utc::now(),
    };
    assert!(push(s, &owner, change(&owner, None, "v1")).await);
    // Editor without rules may write.
    assert!(push(s, &bob, change(&bob, Some(1), "v2")).await);

    let acl_path = format!("/teams/{team_id}/vaults/{}/acl", vault.id);
    // acl.manage is required to read or change rules.
    s.expect_status(
        Method::GET,
        &acl_path,
        Some(bob.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;

    // Owners/admins cannot be restricted.
    s.expect_status(
        Method::PUT,
        &acl_path,
        Some(owner.token()),
        Some(&serde_json::json!({
            "entity_id": host_id, "entity_kind": "host",
            "principal_kind": "member", "user_id": owner.id(), "mode": "connect"
        })),
        StatusCode::BAD_REQUEST,
    )
    .await;
    // Personal vaults carry no rules.
    let personal: VaultList = s
        .json(Method::GET, "/vaults", Some(bob.token()), NOBODY)
        .await;
    s.expect_status(
        Method::PUT,
        &format!("/teams/{team_id}/vaults/{}/acl", personal.vaults[0].id),
        Some(owner.token()),
        Some(&serde_json::json!({
            "entity_id": host_id, "entity_kind": "host",
            "principal_kind": "member", "user_id": bob.id(), "mode": "connect"
        })),
        StatusCode::BAD_REQUEST,
    )
    .await;

    // Role rule (edit) + member rule (connect): the member rule wins.
    let roles: serde_json::Value = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}/roles"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    let roles = roles
        .get("roles")
        .and_then(|r| r.as_array())
        .cloned()
        .or_else(|| roles.as_array().cloned())
        .expect("roles");
    let member_role = roles
        .iter()
        .find(|r| r["slug"] == "member")
        .expect("member role")["id"]
        .clone();
    let _: serde_json::Value = s
        .json(
            Method::PUT,
            &acl_path,
            Some(owner.token()),
            Some(&serde_json::json!({
                "entity_id": host_id, "entity_kind": "host",
                "principal_kind": "role", "role_id": member_role, "mode": "edit"
            })),
        )
        .await;
    let r: serde_json::Value = s
        .json(
            Method::PUT,
            &acl_path,
            Some(owner.token()),
            Some(&serde_json::json!({
                "entity_id": host_id, "entity_kind": "host",
                "principal_kind": "member", "user_id": bob.id(), "mode": "connect"
            })),
        )
        .await;
    let rules = r["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 2);
    let member_rule = rules
        .iter()
        .find(|x| x["principal_kind"] == "member")
        .unwrap();
    assert_eq!(member_rule["principal_name"], bob_email);

    let me: serde_json::Value = s
        .json(Method::GET, "/acl/me", Some(bob.token()), NOBODY)
        .await;
    assert_eq!(me["rules"].as_array().unwrap().len(), 1);
    assert_eq!(me["rules"][0]["mode"], "connect");
    assert_eq!(me["rules"][0]["entity_id"], host_id.to_string());
    // Owner sees no rules for themselves.
    let me_owner: serde_json::Value = s
        .json(Method::GET, "/acl/me", Some(owner.token()), NOBODY)
        .await;
    assert!(me_owner["rules"].as_array().unwrap().is_empty());

    // connect → bob's write is refused, the owner's goes through.
    assert!(!push(s, &bob, change(&bob, Some(2), "v3")).await);
    assert!(push(s, &owner, change(&owner, Some(2), "v3")).await);
    assert!(!delete(s, &bob, host_id, 3).await);

    // Upsert replaces the mode in place (same principal → one rule).
    let r: serde_json::Value = s
        .json(
            Method::PUT,
            &acl_path,
            Some(owner.token()),
            Some(&serde_json::json!({
                "entity_id": host_id, "entity_kind": "host",
                "principal_kind": "member", "user_id": bob.id(), "mode": "read"
            })),
        )
        .await;
    assert_eq!(r["rules"].as_array().unwrap().len(), 2);
    assert!(!push(s, &bob, change(&bob, Some(3), "v4")).await);

    // Remove the member rule → the role rule (edit) applies again.
    let rule_id = member_rule["id"].as_str().unwrap();
    let r: serde_json::Value = s
        .json(
            Method::DELETE,
            &format!("{acl_path}/{rule_id}"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    assert_eq!(r["rules"].as_array().unwrap().len(), 1);
    let me: serde_json::Value = s
        .json(Method::GET, "/acl/me", Some(bob.token()), NOBODY)
        .await;
    assert_eq!(me["rules"][0]["mode"], "edit");
    assert!(push(s, &bob, change(&bob, Some(3), "v4")).await);

    // Validation.
    s.expect_status(
        Method::PUT,
        &acl_path,
        Some(owner.token()),
        Some(&serde_json::json!({
            "entity_id": host_id, "entity_kind": "folder",
            "principal_kind": "member", "user_id": bob.id(), "mode": "read"
        })),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::PUT,
        &acl_path,
        Some(owner.token()),
        Some(&serde_json::json!({
            "entity_id": host_id, "entity_kind": "host",
            "principal_kind": "group", "mode": "read"
        })),
        StatusCode::BAD_REQUEST,
    )
    .await;
}
