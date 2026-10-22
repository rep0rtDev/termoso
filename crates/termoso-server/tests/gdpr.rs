//! Team data tooling: residency status, full export, scheduled deletion.

mod common;
use common::*;
use reqwest::{Method, StatusCode};
use termoso_crypto::keys::SymmetricKey;
use termoso_crypto::sealed;
use termoso_proto::sync::{EntityChange, PushRequest, PushResponse, PushResult};
use termoso_proto::team::{AuditEventList, TeamDataStatus};
use termoso_proto::vault::{CreateVaultRequest, Vault, VaultMemberUpsert, VaultRole};
use uuid::Uuid;

#[tokio::test]
async fn export_and_scheduled_deletion() {
    let Some(s) = server().await else { return };
    let owner = register(s, &unique_email("gdpr-owner"), "pw-owner-1234567").await;
    let bob_email = unique_email("gdpr-bob");
    let bob = register(s, &bob_email, "pw-bob-123456789").await;
    let team: serde_json::Value = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "Residency" })),
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
    let host = Uuid::new_v4();
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(owner.token()),
            Some(&PushRequest {
                changes: vec![EntityChange {
                    secret_rotated_at: None,
                    id: host,
                    kind: "host".into(),
                    vault_id: vault.id,
                    base_version: None,
                    key_version: 1,
                    data: owner.encrypt_entity(&key, "host", host, r#"{"label":"db"}"#),
                    updated_at: chrono::Utc::now(),
                }],
                deletes: vec![],
            }),
        )
        .await;
    assert!(matches!(r.results[0], PushResult::Ok { .. }));

    // Status is visible to any member.
    let st: TeamDataStatus = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}/data"),
            Some(bob.token()),
            NOBODY,
        )
        .await;
    assert!(st.deletion_scheduled_at.is_none());
    assert_eq!(st.deletion_grace_days, 7);

    // Export: team.export only; the document carries ciphertext, not plaintext.
    s.expect_status(
        Method::GET,
        &format!("/teams/{team_id}/export"),
        Some(bob.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    let resp = s
        .http()
        .get(s.url(&format!("/teams/{team_id}/export")))
        .bearer_auth(owner.token())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    let doc: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(doc["format"], "termoso-team-export/1");
    assert_eq!(doc["team"]["name"], "Residency");
    assert_eq!(doc["members"].as_array().unwrap().len(), 2);
    assert_eq!(doc["vaults"].as_array().unwrap().len(), 1);
    let ents = doc["entities"].as_array().unwrap();
    assert_eq!(ents.len(), 1);
    assert_eq!(ents[0]["kind"], "host");
    assert!(
        !ents[0]["data"]
            .as_str()
            .unwrap()
            .contains(r#""label":"db""#),
        "ciphertext only"
    );
    assert!(doc["policies"]["mfa"].is_string());
    assert!(doc["roles"].as_array().unwrap().len() >= 3);

    // Scheduling: owner only, name must match, once.
    let del = format!("/teams/{team_id}/deletion");
    s.expect_status(
        Method::POST,
        &del,
        Some(bob.token()),
        Some(&serde_json::json!({ "confirm_name": "Residency" })),
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::POST,
        &del,
        Some(owner.token()),
        Some(&serde_json::json!({ "confirm_name": "residency" })),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let scheduled: TeamDataStatus = s
        .json(
            Method::POST,
            &del,
            Some(owner.token()),
            Some(&serde_json::json!({ "confirm_name": "Residency" })),
        )
        .await;
    let at = scheduled.deletion_scheduled_at.expect("scheduled");
    let days = (at - chrono::Utc::now()).num_hours();
    assert!((7 * 24 - 1..=7 * 24).contains(&days), "{days}");
    assert_eq!(scheduled.deletion_requested_by, Some(owner.id()));
    s.expect_status(
        Method::POST,
        &del,
        Some(owner.token()),
        Some(&serde_json::json!({ "confirm_name": "Residency" })),
        StatusCode::CONFLICT,
    )
    .await;
    // Still there during the grace period; cancel is owner-only.
    let _: serde_json::Value = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}"),
            Some(bob.token()),
            NOBODY,
        )
        .await;
    s.expect_status(
        Method::DELETE,
        &del,
        Some(bob.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &del,
        Some(owner.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &del,
        Some(owner.token()),
        NOBODY,
        StatusCode::CONFLICT,
    )
    .await;
    let events: AuditEventList = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}/audit?limit=50"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    for a in [
        "team.exported",
        "team.deletion_scheduled",
        "team.deletion_cancelled",
    ] {
        assert!(events.events.iter().any(|e| e.action == a), "{a}");
    }

    // Grace period over → purged.
    let _: TeamDataStatus = s
        .json(
            Method::POST,
            &del,
            Some(owner.token()),
            Some(&serde_json::json!({ "confirm_name": "Residency" })),
        )
        .await;
    s.sql(
        "UPDATE teams SET deletion_scheduled_at = now() - interval '1 minute' WHERE id = $1::uuid",
        &team_id.to_string(),
    )
    .await;
    s.expect_status(
        Method::GET,
        &format!("/teams/{team_id}/data"),
        Some(owner.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;
    s.expect_status(
        Method::GET,
        &format!("/teams/{team_id}"),
        Some(owner.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;
}
