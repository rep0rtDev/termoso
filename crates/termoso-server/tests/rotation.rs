//! Credential rotation: max-age policy, overdue list, jobs and audit.

mod common;
use common::*;
use reqwest::{Method, StatusCode};
use termoso_crypto::keys::SymmetricKey;
use termoso_crypto::sealed;
use termoso_proto::sync::{EntityChange, PushRequest, PushResponse, PushResult};
use termoso_proto::team::{AuditEventList, RotationJob, RotationJobList, RotationOverview};
use termoso_proto::vault::{CreateVaultRequest, Vault, VaultMemberUpsert, VaultRole};
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

#[tokio::test]
async fn rotation_policy_due_list_and_jobs() {
    let Some(s) = server().await else { return };
    let owner = register(s, &unique_email("rot-owner"), "pw-owner-1234567").await;
    let bob_email = unique_email("rot-bob");
    let bob = register(s, &bob_email, "pw-bob-123456789").await;
    let team: serde_json::Value = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "Rotate" })),
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
                members: vec![
                    upsert(&key, &owner, VaultRole::Manager),
                    upsert(&key, &bob, VaultRole::Editor),
                ],
            }),
        )
        .await;

    // Policy validation and value.
    let policies = format!("/teams/{team_id}/policies");
    for bad in [
        serde_json::json!({ "key_max_age_days": -1 }),
        serde_json::json!({ "password_max_age_days": 9999 }),
    ] {
        s.expect_status(
            Method::PUT,
            &policies,
            Some(owner.token()),
            Some(&bad),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    let p: serde_json::Value = s
        .json(
            Method::PUT,
            &policies,
            Some(owner.token()),
            Some(&serde_json::json!({ "key_max_age_days": 30, "password_max_age_days": 90 })),
        )
        .await;
    assert_eq!(p["policies"]["key_max_age_days"], 30);
    assert_eq!(p["policies"]["password_max_age_days"], 90);

    // An old key (rotated 60 days ago), a fresh key and an old host password.
    let old_key = Uuid::new_v4();
    let fresh_key = Uuid::new_v4();
    let old_host = Uuid::new_v4();
    let change = |id: Uuid, kind: &str, rotated_days_ago: Option<i64>| EntityChange {
        secret_rotated_at: rotated_days_ago.map(|d| chrono::Utc::now() - chrono::Duration::days(d)),
        id,
        kind: kind.into(),
        vault_id: vault.id,
        base_version: None,
        key_version: 1,
        data: owner.encrypt_entity(&key, kind, id, r#"{"label":"x"}"#),
        updated_at: chrono::Utc::now(),
    };
    assert!(push(s, &owner, change(old_key, "ssh_key", Some(60))).await);
    assert!(push(s, &owner, change(fresh_key, "ssh_key", Some(1))).await);
    assert!(push(s, &owner, change(old_host, "host", Some(120))).await);

    // Overview needs rotation.manage.
    let path = format!("/teams/{team_id}/rotation");
    s.expect_status(
        Method::GET,
        &path,
        Some(bob.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    let o: RotationOverview = s
        .json(Method::GET, &path, Some(owner.token()), NOBODY)
        .await;
    let due: Vec<Uuid> = o.due.iter().map(|d| d.entity_id).collect();
    assert!(due.contains(&old_key) && due.contains(&old_host), "{due:?}");
    assert!(!due.contains(&fresh_key));
    let k = o.due.iter().find(|d| d.entity_id == old_key).unwrap();
    assert_eq!(k.kind, "ssh_key");
    assert!(k.age_days >= 59 && k.job_id.is_none(), "{k:?}");

    // Queue jobs: unknown entity rejected, then two jobs; duplicates ignored.
    let jobs_path = format!("/teams/{team_id}/rotation/jobs");
    s.expect_status(
        Method::POST,
        &jobs_path,
        Some(owner.token()),
        Some(&serde_json::json!({ "vault_id": vault.id, "entity_ids": [Uuid::new_v4()] })),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::POST,
        &jobs_path,
        Some(bob.token()),
        Some(&serde_json::json!({ "vault_id": vault.id, "entity_ids": [old_key] })),
        StatusCode::FORBIDDEN,
    )
    .await;
    let created: RotationJobList = s
        .json(
            Method::POST,
            &jobs_path,
            Some(owner.token()),
            Some(&serde_json::json!({
                "vault_id": vault.id, "entity_ids": [old_key, old_host], "note": "quarterly"
            })),
        )
        .await;
    assert_eq!(created.jobs.len(), 2);
    assert!(
        created
            .jobs
            .iter()
            .all(|j| j.status == "pending" && j.note == "quarterly")
    );
    let again: RotationJobList = s
        .json(
            Method::POST,
            &jobs_path,
            Some(owner.token()),
            Some(&serde_json::json!({ "vault_id": vault.id, "entity_ids": [old_key] })),
        )
        .await;
    assert!(again.jobs.is_empty(), "open job is not duplicated");
    let key_job = created
        .jobs
        .iter()
        .find(|j| j.entity_id == old_key)
        .unwrap()
        .id;
    let host_job = created
        .jobs
        .iter()
        .find(|j| j.entity_id == old_host)
        .unwrap()
        .id;

    // Bob (vault editor) sees and executes the jobs.
    let mine: RotationJobList = s
        .json(Method::GET, "/rotation/jobs", Some(bob.token()), NOBODY)
        .await;
    assert_eq!(mine.jobs.len(), 2);
    let running: RotationJob = s
        .json(
            Method::PATCH,
            &format!("/rotation/jobs/{key_job}"),
            Some(bob.token()),
            Some(&serde_json::json!({ "status": "running" })),
        )
        .await;
    assert_eq!(running.status, "running");
    assert_eq!(running.claimed_by, Some(bob.id()));
    // Another member cannot take over a running job.
    s.expect_status(
        Method::PATCH,
        &format!("/rotation/jobs/{key_job}"),
        Some(owner.token()),
        Some(&serde_json::json!({ "status": "running" })),
        StatusCode::CONFLICT,
    )
    .await;
    let done: RotationJob = s
        .json(
            Method::PATCH,
            &format!("/rotation/jobs/{key_job}"),
            Some(bob.token()),
            Some(&serde_json::json!({ "status": "done" })),
        )
        .await;
    assert_eq!(done.status, "done");
    assert!(done.finished_at.is_some());
    s.expect_status(
        Method::PATCH,
        &format!("/rotation/jobs/{key_job}"),
        Some(bob.token()),
        Some(&serde_json::json!({ "status": "failed" })),
        StatusCode::CONFLICT,
    )
    .await;
    // Only rotation.manage cancels; the failed path records the error.
    s.expect_status(
        Method::PATCH,
        &format!("/rotation/jobs/{host_job}"),
        Some(bob.token()),
        Some(&serde_json::json!({ "status": "cancelled" })),
        StatusCode::FORBIDDEN,
    )
    .await;
    let failed: RotationJob = s
        .json(
            Method::PATCH,
            &format!("/rotation/jobs/{host_job}"),
            Some(bob.token()),
            Some(&serde_json::json!({ "status": "failed", "error": "host unreachable" })),
        )
        .await;
    assert_eq!(failed.error.as_deref(), Some("host unreachable"));

    // Done stamps the entity: the key is no longer due, the host still is.
    let o: RotationOverview = s
        .json(Method::GET, &path, Some(owner.token()), NOBODY)
        .await;
    let due: Vec<Uuid> = o.due.iter().map(|d| d.entity_id).collect();
    assert!(
        !due.contains(&old_key) && due.contains(&old_host),
        "{due:?}"
    );
    assert_eq!(o.jobs.len(), 2);

    // Audit trail and cleanup.
    let events: AuditEventList = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}/audit?limit=50"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    for a in [
        "rotation.requested",
        "rotation.completed",
        "rotation.failed",
    ] {
        assert!(events.events.iter().any(|e| e.action == a), "{a}");
    }
    s.expect_status(
        Method::DELETE,
        &format!("/rotation/jobs/{key_job}"),
        Some(bob.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &format!("/rotation/jobs/{key_job}"),
        Some(owner.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
}
