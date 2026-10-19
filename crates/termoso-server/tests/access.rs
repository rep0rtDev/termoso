//! Just-in-time access requests: ask, approve/deny, revoke, expiry, audit.

mod common;
use common::*;
use reqwest::{Method, StatusCode};
use termoso_crypto::keys::SymmetricKey;
use termoso_crypto::sealed;
use termoso_proto::team::{AccessRequest, AccessRequestList, AuditEventList};
use termoso_proto::vault::{CreateVaultRequest, Vault, VaultMemberUpsert, VaultRole};
use uuid::Uuid;

async fn members_json(s: &TestServer, vault: Uuid, u: &User) -> String {
    let v: serde_json::Value = s
        .json(
            Method::GET,
            &format!("/vaults/{vault}/members"),
            Some(u.token()),
            NOBODY,
        )
        .await;
    v.to_string()
}

async fn ask(s: &TestServer, team: Uuid, u: &User, vault: Uuid, hours: i32) -> AccessRequest {
    s.json(
        Method::POST,
        &format!("/teams/{team}/access-requests"),
        Some(u.token()),
        Some(&serde_json::json!({ "vault_id": vault, "hours": hours, "reason": "incident #42" })),
    )
    .await
}

async fn list(s: &TestServer, team: Uuid, u: &User) -> AccessRequestList {
    s.json(
        Method::GET,
        &format!("/teams/{team}/access-requests"),
        Some(u.token()),
        NOBODY,
    )
    .await
}

#[tokio::test]
async fn jit_access_lifecycle() {
    let Some(s) = server().await else { return };
    let owner = register(s, &unique_email("jit-owner"), "pw-owner-1234567").await;
    let bob_email = unique_email("jit-bob");
    let bob = register(s, &bob_email, "pw-bob-123456789").await;
    let team: serde_json::Value = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "JIT" })),
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
                name: "Prod".into(),
                members: vec![VaultMemberUpsert {
                    user_id: owner.id(),
                    role: VaultRole::Manager,
                    sealed_key: sealed::seal_vault_key(owner.keypair.public(), &key).unwrap(),
                }],
            }),
        )
        .await;
    let path = format!("/teams/{team_id}/access-requests");
    let bob_id = bob.id().to_string();

    // Validation; members of the vault cannot ask.
    for bad in [
        serde_json::json!({ "vault_id": vault.id, "role": "manager" }),
        serde_json::json!({ "vault_id": vault.id, "hours": 0 }),
        serde_json::json!({ "vault_id": vault.id, "hours": 721 }),
    ] {
        s.expect_status(
            Method::POST,
            &path,
            Some(bob.token()),
            Some(&bad),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    s.expect_status(
        Method::POST,
        &path,
        Some(owner.token()),
        Some(&serde_json::json!({ "vault_id": vault.id })),
        StatusCode::CONFLICT,
    )
    .await;

    // Ask, no duplicates, visibility, deny.
    let r1 = ask(s, team_id, &bob, vault.id, 4).await;
    assert_eq!(
        (r1.status.as_str(), r1.role.as_str(), r1.hours),
        ("pending", "viewer", 4)
    );
    s.expect_status(
        Method::POST,
        &path,
        Some(bob.token()),
        Some(&serde_json::json!({ "vault_id": vault.id })),
        StatusCode::CONFLICT,
    )
    .await;
    let mine = list(s, team_id, &bob).await;
    assert!(!mine.can_approve && mine.requests.len() == 1);
    let all = list(s, team_id, &owner).await;
    assert!(all.can_approve && all.requests.len() == 1);
    s.expect_status(
        Method::POST,
        &format!("{path}/{}/approve", r1.id),
        Some(bob.token()),
        Some(&serde_json::json!({})),
        StatusCode::FORBIDDEN,
    )
    .await;
    let denied: AccessRequest = s
        .json(
            Method::POST,
            &format!("{path}/{}/deny", r1.id),
            Some(owner.token()),
            Some(&serde_json::json!({ "note": "not now" })),
        )
        .await;
    assert_eq!(denied.status, "denied");
    assert_eq!(denied.decision_note.as_deref(), Some("not now"));
    assert_eq!(denied.decided_by, Some(owner.id()));

    // Approve with a trimmed window → pending membership with an expiry.
    let r2 = ask(s, team_id, &bob, vault.id, 8).await;
    let approved: AccessRequest = s
        .json(
            Method::POST,
            &format!("{path}/{}/approve", r2.id),
            Some(owner.token()),
            Some(&serde_json::json!({ "hours": 2 })),
        )
        .await;
    assert_eq!((approved.status.as_str(), approved.hours), ("approved", 2));
    let until = approved.expires_at.expect("expiry");
    let left = until - chrono::Utc::now();
    assert!(
        left.num_minutes() > 110 && left.num_minutes() <= 120,
        "{left}"
    );
    assert!(members_json(s, vault.id, &owner).await.contains(&bob_id));
    s.expect_status(
        Method::POST,
        &format!("{path}/{}/approve", r2.id),
        Some(owner.token()),
        Some(&serde_json::json!({})),
        StatusCode::CONFLICT,
    )
    .await;
    // A new request while access is held is refused.
    s.expect_status(
        Method::POST,
        &path,
        Some(bob.token()),
        Some(&serde_json::json!({ "vault_id": vault.id })),
        StatusCode::CONFLICT,
    )
    .await;
    let revoked: AccessRequest = s
        .json(
            Method::POST,
            &format!("{path}/{}/revoke", r2.id),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    assert_eq!(revoked.status, "revoked");
    assert!(!members_json(s, vault.id, &owner).await.contains(&bob_id));

    // Withdraw.
    let r3 = ask(s, team_id, &bob, vault.id, 1).await;
    s.expect_status(
        Method::DELETE,
        &format!("{path}/{}", r3.id),
        Some(owner.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &format!("{path}/{}", r3.id),
        Some(bob.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;

    // Expiry removes the membership and flips the request.
    let r4 = ask(s, team_id, &bob, vault.id, 1).await;
    let _: AccessRequest = s
        .json(
            Method::POST,
            &format!("{path}/{}/approve", r4.id),
            Some(owner.token()),
            Some(&serde_json::json!({})),
        )
        .await;
    assert!(members_json(s, vault.id, &owner).await.contains(&bob_id));
    s.sql(
        "UPDATE vault_members SET expires_at = now() - interval '1 minute' WHERE user_id = $1::uuid",
        &bob_id,
    )
    .await;
    s.sql(
        "UPDATE access_requests SET expires_at = now() - interval '1 minute' WHERE user_id = $1::uuid AND status = 'approved'",
        &bob_id,
    )
    .await;
    let after = list(s, team_id, &owner).await;
    let r4_now = after.requests.iter().find(|r| r.id == r4.id).unwrap();
    assert_eq!(r4_now.status, "expired");
    assert!(!members_json(s, vault.id, &owner).await.contains(&bob_id));
    let statuses: Vec<&str> = after.requests.iter().map(|r| r.status.as_str()).collect();
    assert!(
        statuses.contains(&"cancelled") && statuses.contains(&"denied"),
        "{statuses:?}"
    );

    let events: AuditEventList = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}/audit?limit=50"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    for a in [
        "access.requested",
        "access.approved",
        "access.denied",
        "access.revoked",
        "access.expired",
    ] {
        assert!(events.events.iter().any(|e| e.action == a), "{a}");
    }
}
