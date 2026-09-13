//! End-to-end API tests against real PostgreSQL + Redis.
//!
//!     docker compose -f deploy/docker-compose.dev.yml up -d --wait
//!     cargo test -p termoso-server --test api
//!
//! Tests are skipped (pass vacuously) when the services are unreachable, unless
//! `TERMOSO_TEST_REQUIRE_SERVICES=1` (as in CI).

mod common;

use std::time::Duration;

use common::*;
use futures::{SinkExt, StreamExt};
use reqwest::{Method, StatusCode};
use termoso_crypto::aead::{self, Aad};
use termoso_crypto::keys::SymmetricKey;
use termoso_crypto::sealed;
use termoso_proto::account::ServerInfo;
use termoso_proto::auth::AuthResponse;
use termoso_proto::sync::{
    EntityChange, EntityDelete, HistoryEntry, HistoryKind, HistoryPullResponse, HistoryPushRequest,
    PullRequest, PullResponse, PushRequest, PushResponse, PushResult,
};
use termoso_proto::team::{AuditEventList, CreateInviteRequest, CreateTeamRequest, Team, TeamRole};
use termoso_proto::vault::{
    CreateVaultRequest, Vault, VaultKind, VaultList, VaultMemberUpsert, VaultRole,
};
use termoso_proto::ws::{ClientMessage, ServerMessage};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

macro_rules! server {
    () => {
        match server().await {
            Some(s) => s,
            None => return,
        }
    };
}

#[tokio::test]
async fn health_and_server_info() {
    let s = server!();
    let r = s
        .http()
        .get(format!("http://{}/healthz", s.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let r = s
        .http()
        .get(format!("http://{}/readyz", s.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK, "{}", r.text().await.unwrap());

    let info: ServerInfo = s.json(Method::GET, "/server/info", None, NOBODY).await;
    assert!(info.registration_open);
    assert_eq!(info.version, termoso_server::VERSION);

    let r = s
        .http()
        .get(format!("http://{}/api/openapi.json", s.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let doc: serde_json::Value = r.json().await.unwrap();
    assert!(doc["paths"]["/api/v1/sync/push"].is_object());
}

#[tokio::test]
async fn web_cabinet_is_served_with_spa_fallback() {
    let s = server!();
    let get = |path: &str| s.http().get(format!("http://{}{path}", s.addr)).send();

    let r = get("/").await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert!(
        r.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(r.headers()["cache-control"], "no-cache");
    assert_eq!(r.headers()["x-frame-options"], "DENY");
    assert!(
        r.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("'wasm-unsafe-eval'")
    );
    assert!(r.text().await.unwrap().contains("<div id=root>"));

    // Client-side routes deep-link to index.html.
    let r = get("/team/00000000-0000-0000-0000-000000000000")
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert!(r.text().await.unwrap().contains("<div id=root>"));

    // Hashed assets are immutable; missing assets are real 404s.
    let r = get("/assets/app-abc123.js").await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(
        r.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
    assert!(r.headers().get("content-security-policy").is_none());
    let r = get("/assets/missing-000.js").await.unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
    let r = get("/favicon.svg").await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers()["cache-control"], "no-cache");
    let r = get("/robots.txt").await.unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);

    // The API keeps JSON 404s and is never shadowed by the SPA.
    let r = get("/api/v1/no-such-route").await.unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["code"], "not_found");
    let r = s
        .http()
        .post(format!("http://{}/team", s.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn register_login_and_sessions() {
    let s = server!();
    let email = unique_email("alice");
    let alice = register(s, &email, "correct horse battery staple").await;
    assert_eq!(alice.session.user.email, email.to_lowercase());
    assert!(!alice.session.user.is_admin);
    assert_eq!(alice.session.keys.public_key, alice.keypair.public_b64());

    // Personal vault exists and the sealed key opens with our keypair.
    let vaults: VaultList = s
        .json(Method::GET, "/vaults", Some(alice.token()), NOBODY)
        .await;
    assert_eq!(vaults.vaults.len(), 1);
    let personal = &vaults.vaults[0];
    assert_eq!(personal.kind, VaultKind::Personal);
    let opened =
        sealed::open_vault_key(&alice.keypair, personal.sealed_key.as_ref().unwrap()).unwrap();
    assert_eq!(opened.as_bytes(), alice.personal_vault_key.as_bytes());

    // Duplicate registration is rejected.
    let (req, _) = termoso_crypto::opaque::client_registration_start(b"whatever").expect("start");
    s.expect_status(
        Method::POST,
        "/auth/register/start",
        None,
        Some(&termoso_proto::auth::RegisterStartRequest {
            email: email.clone(),
            opaque_request: req,
        }),
        StatusCode::CONFLICT,
    )
    .await;

    // Login from a new device with the right password.
    let AuthResponse::Authenticated(second) =
        login(s, &email, "correct horse battery staple").await
    else {
        panic!("expected session")
    };
    assert_ne!(second.token, alice.session.token);
    assert_ne!(second.device_id, alice.session.device_id);
    assert_eq!(
        second.keys.wrapped_private_key,
        alice.session.keys.wrapped_private_key
    );

    // Wrong password fails client-side and the server never yields a session.
    assert!(try_login(s, &email, "wrong password").await.is_err());

    // Two devices visible; revoking the second one kills its token.
    let me: serde_json::Value = s
        .json(Method::GET, "/account", Some(alice.token()), NOBODY)
        .await;
    assert_eq!(me["user"]["id"], alice.id().to_string());
    let devices: serde_json::Value = s
        .json(Method::GET, "/account/devices", Some(alice.token()), NOBODY)
        .await;
    assert_eq!(devices["devices"].as_array().unwrap().len(), 2);
    s.expect_status(
        Method::DELETE,
        &format!("/account/devices/{}", second.device_id),
        Some(alice.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::GET,
        "/account",
        Some(&second.token),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;

    // Logout invalidates our own token.
    s.expect_status(
        Method::POST,
        "/auth/logout",
        Some(alice.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::GET,
        "/account",
        Some(alice.token()),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
}

#[tokio::test]
async fn unauthenticated_requests_are_rejected() {
    let s = server!();
    s.expect_status(
        Method::GET,
        "/vaults",
        None,
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    s.expect_status(
        Method::GET,
        "/vaults",
        Some("not-a-token"),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    s.expect_status(
        Method::GET,
        "/admin/stats",
        None,
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
}

#[tokio::test]
async fn sync_push_pull_conflicts_and_tombstones() {
    let s = server!();
    let u = register(s, &unique_email("sync"), "pw-sync-1234567").await;
    let vaults: VaultList = s
        .json(Method::GET, "/vaults", Some(u.token()), NOBODY)
        .await;
    let vault_id = vaults.vaults[0].id;
    let key = &u.personal_vault_key;

    let host_id = Uuid::new_v4();
    let change = |base: Option<i64>, label: &str| EntityChange {
        id: host_id,
        kind: "host".into(),
        vault_id,
        base_version: base,
        key_version: 1,
        data: u.encrypt_entity(key, "host", host_id, &format!(r#"{{"label":"{label}"}}"#)),
        updated_at: chrono::Utc::now(),
    };

    // Create.
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![change(None, "one")],
                deletes: vec![],
            }),
        )
        .await;
    let PushResult::Ok {
        version: v1,
        seq: s1,
        ..
    } = &r.results[0]
    else {
        panic!("{:?}", r.results)
    };
    assert_eq!(*v1, 1);

    // Creating again with base_version=None conflicts.
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![change(None, "dup")],
                deletes: vec![],
            }),
        )
        .await;
    assert!(
        matches!(r.results[0], PushResult::Conflict { .. }),
        "{:?}",
        r.results
    );

    // Update on the right base version.
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![change(Some(1), "two")],
                deletes: vec![],
            }),
        )
        .await;
    let PushResult::Ok {
        version: v2,
        seq: s2,
        ..
    } = &r.results[0]
    else {
        panic!("{:?}", r.results)
    };
    assert_eq!(*v2, 2);
    assert!(s2 > s1);

    // Stale update conflicts and returns the server copy we can decrypt.
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![change(Some(1), "stale")],
                deletes: vec![],
            }),
        )
        .await;
    let PushResult::Conflict { server, .. } = &r.results[0] else {
        panic!("{:?}", r.results)
    };
    assert_eq!(server.version, 2);
    let plain = aead::decrypt_str(
        key,
        &Aad::entity("host", &host_id.to_string()),
        &server.data,
    )
    .unwrap();
    assert_eq!(plain, r#"{"label":"two"}"#);

    // Unknown kind and unknown vault are per-item errors, not request failures.
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![
                    EntityChange {
                        kind: "spaceship".into(),
                        ..change(None, "x")
                    },
                    EntityChange {
                        id: Uuid::new_v4(),
                        vault_id: Uuid::new_v4(),
                        ..change(None, "x")
                    },
                ],
                deletes: vec![],
            }),
        )
        .await;
    assert!(matches!(&r.results[0], PushResult::Error { code, .. } if code == "unknown_kind"));
    assert!(matches!(&r.results[1], PushResult::Error { code, .. } if code == "forbidden"));

    // Pull from 0 returns the live entity; pull from the last seq is empty.
    let p: PullResponse = s
        .json(
            Method::POST,
            "/sync/pull",
            Some(u.token()),
            Some(&PullRequest {
                cursors: cursors(&[(vault_id, 0)]),
                limit: None,
            }),
        )
        .await;
    assert_eq!(p.entities.len(), 1);
    assert_eq!(p.entities[0].version, 2);
    assert!(!p.has_more);
    assert_eq!(p.cursors[&vault_id], *s2);
    let p: PullResponse = s
        .json(
            Method::POST,
            "/sync/pull",
            Some(u.token()),
            Some(&PullRequest {
                cursors: p.cursors,
                limit: None,
            }),
        )
        .await;
    assert!(p.entities.is_empty());

    // Delete → tombstone shows up in the next pull.
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![],
                deletes: vec![EntityDelete {
                    id: host_id,
                    base_version: 2,
                }],
            }),
        )
        .await;
    assert!(
        matches!(r.results[0], PushResult::Ok { .. }),
        "{:?}",
        r.results
    );
    let p: PullResponse = s
        .json(
            Method::POST,
            "/sync/pull",
            Some(u.token()),
            Some(&PullRequest {
                cursors: cursors(&[(vault_id, *s2)]),
                limit: None,
            }),
        )
        .await;
    assert_eq!(p.entities.len(), 1);
    assert!(p.entities[0].deleted);
    assert!(p.entities[0].data.is_empty());

    // Pagination: many small entities, limit 3.
    let ids: Vec<Uuid> = (0..7).map(|_| Uuid::new_v4()).collect();
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: ids
                    .iter()
                    .map(|id| EntityChange {
                        id: *id,
                        kind: "snippet".into(),
                        vault_id,
                        base_version: None,
                        key_version: 1,
                        data: u.encrypt_entity(key, "snippet", *id, "{}"),
                        updated_at: chrono::Utc::now(),
                    })
                    .collect(),
                deletes: vec![],
            }),
        )
        .await;
    assert!(r.results.iter().all(|x| matches!(x, PushResult::Ok { .. })));
    let mut seen = 0;
    let mut cur = cursors(&[(vault_id, 0)]);
    let mut pages = 0;
    loop {
        let p: PullResponse = s
            .json(
                Method::POST,
                "/sync/pull",
                Some(u.token()),
                Some(&PullRequest {
                    cursors: cur,
                    limit: Some(3),
                }),
            )
            .await;
        seen += p.entities.len();
        cur = p.cursors;
        pages += 1;
        if !p.has_more {
            break;
        }
        assert!(pages < 10, "pagination did not converge");
    }
    assert_eq!(seen, 8, "7 snippets + 1 tombstone");
}

#[tokio::test]
async fn teams_vaults_invites_and_sharing() {
    let s = server!();
    let owner = register(s, &unique_email("owner"), "pw-owner-1234567").await;
    let bob_email = unique_email("bob");
    let bob = register(s, &bob_email, "pw-bob-123456789").await;

    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&CreateTeamRequest { name: "Ops".into() }),
        )
        .await;
    assert_eq!(team.my_role, TeamRole::Owner);
    assert_eq!(team.member_count, 1);

    // Team vault with the owner as manager; key sealed to the owner.
    let vault_key = SymmetricKey::generate();
    let vault: Vault = s
        .json(
            Method::POST,
            &format!("/teams/{}/vaults", team.id),
            Some(owner.token()),
            Some(&CreateVaultRequest {
                name: "Production".into(),
                members: vec![VaultMemberUpsert {
                    user_id: owner.id(),
                    role: VaultRole::Manager,
                    sealed_key: sealed::seal_vault_key(owner.keypair.public(), &vault_key).unwrap(),
                }],
            }),
        )
        .await;
    assert_eq!(vault.kind, VaultKind::Team);
    assert_eq!(vault.team_id, Some(team.id));

    // Bob cannot see or push into the vault yet.
    let bobs: VaultList = s
        .json(Method::GET, "/vaults", Some(bob.token()), NOBODY)
        .await;
    assert!(bobs.vaults.iter().all(|v| v.id != vault.id));
    let host_id = Uuid::new_v4();
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(bob.token()),
            Some(&PushRequest {
                changes: vec![EntityChange {
                    id: host_id,
                    kind: "host".into(),
                    vault_id: vault.id,
                    base_version: None,
                    key_version: 1,
                    data: bob.encrypt_entity(&vault_key, "host", host_id, "{}"),
                    updated_at: chrono::Utc::now(),
                }],
                deletes: vec![],
            }),
        )
        .await;
    assert!(matches!(&r.results[0], PushResult::Error { code, .. } if code == "forbidden"));

    // Invite Bob (member) with access to the vault; a random user can't accept.
    let invite: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/teams/{}/invites", team.id),
            Some(owner.token()),
            Some(&CreateInviteRequest {
                email: bob_email.clone(),
                role: TeamRole::Member,
                vault_ids: vec![vault.id],
            }),
        )
        .await;
    let token = invite["url"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();
    let preview: serde_json::Value = s
        .json(Method::GET, &format!("/invites/{token}"), None, NOBODY)
        .await;
    assert_eq!(preview["team_name"], "Ops");
    assert_eq!(preview["account_exists"], true);
    let mallory = register(s, &unique_email("mallory"), "pw-mallory-12345").await;
    s.expect_status(
        Method::POST,
        &format!("/invites/{token}/accept"),
        Some(mallory.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    let joined: Team = s
        .json(
            Method::POST,
            &format!("/invites/{token}/accept"),
            Some(bob.token()),
            NOBODY,
        )
        .await;
    assert_eq!(joined.my_role, TeamRole::Member);
    assert_eq!(joined.member_count, 2);

    // Bob is a vault member but has no sealed key yet → shows in pending keys.
    let bobs: VaultList = s
        .json(Method::GET, "/vaults", Some(bob.token()), NOBODY)
        .await;
    let bv = bobs
        .vaults
        .iter()
        .find(|v| v.id == vault.id)
        .expect("bob sees vault");
    assert!(bv.sealed_key.is_none());
    let pending: serde_json::Value = s
        .json(
            Method::GET,
            &format!("/teams/{}/pending-keys", team.id),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    let items = pending["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["user_id"], bob.id().to_string());
    assert_eq!(items[0]["public_key"], bob.keypair.public_b64());

    // Owner seals the key for Bob via the member upsert endpoint.
    s.expect_status(
        Method::PUT,
        &format!("/vaults/{}/members/{}", vault.id, bob.id()),
        Some(owner.token()),
        Some(&VaultMemberUpsert {
            user_id: bob.id(),
            role: VaultRole::Editor,
            sealed_key: sealed::seal_vault_key(bob.keypair.public(), &vault_key).unwrap(),
        }),
        StatusCode::NO_CONTENT,
    )
    .await;
    let bobs: VaultList = s
        .json(Method::GET, "/vaults", Some(bob.token()), NOBODY)
        .await;
    let bv = bobs.vaults.iter().find(|v| v.id == vault.id).unwrap();
    let opened = sealed::open_vault_key(&bob.keypair, bv.sealed_key.as_ref().unwrap()).unwrap();
    assert_eq!(opened.as_bytes(), vault_key.as_bytes());
    assert_eq!(bv.my_role, VaultRole::Editor);

    // Now Bob can write, and the owner pulls it.
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(bob.token()),
            Some(&PushRequest {
                changes: vec![EntityChange {
                    id: host_id,
                    kind: "host".into(),
                    vault_id: vault.id,
                    base_version: None,
                    key_version: bv.key_version,
                    data: bob.encrypt_entity(&vault_key, "host", host_id, r#"{"label":"db1"}"#),
                    updated_at: chrono::Utc::now(),
                }],
                deletes: vec![],
            }),
        )
        .await;
    assert!(
        matches!(r.results[0], PushResult::Ok { .. }),
        "{:?}",
        r.results
    );
    let p: PullResponse = s
        .json(
            Method::POST,
            "/sync/pull",
            Some(owner.token()),
            Some(&PullRequest {
                cursors: cursors(&[(vault.id, 0)]),
                limit: None,
            }),
        )
        .await;
    assert_eq!(p.entities.len(), 1);
    assert_eq!(p.entities[0].updated_by_device, Some(bob.session.device_id));

    // Team activity log: every step above left a metadata-only entry.
    let audit_path = format!("/teams/{}/audit", team.id);
    s.expect_status(
        Method::GET,
        &audit_path,
        Some(mallory.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;
    let raw = s
        .call(Method::GET, &audit_path, Some(owner.token()), NOBODY)
        .await
        .text()
        .await
        .unwrap();
    assert!(!raw.contains(&bv.sealed_key.clone().unwrap()));
    assert!(!raw.contains(&p.entities[0].data));
    let log: AuditEventList = serde_json::from_str(&raw).unwrap();
    assert!(log.next_before.is_none());
    let actions: Vec<&str> = log.events.iter().map(|e| e.action.as_str()).collect();
    for expected in [
        "team.created",
        "vault.created",
        "invite.created",
        "invite.accepted",
        "vault.access_granted",
        "entity.created",
    ] {
        assert!(
            actions.contains(&expected),
            "{expected} missing in {actions:?}"
        );
    }
    assert!(
        log.events.windows(2).all(|w| w[0].id > w[1].id),
        "newest first"
    );
    let created = log
        .events
        .iter()
        .find(|e| e.action == "entity.created")
        .unwrap();
    assert_eq!(created.actor_id, Some(bob.id()));
    assert_eq!(created.actor_email.as_deref(), Some(bob_email.as_str()));
    assert_eq!(created.vault_id, Some(vault.id));
    assert_eq!(created.device_id, Some(bob.session.device_id));
    assert_eq!(created.details["kind"], "host");
    assert_eq!(created.details["count"], 1);
    assert_eq!(created.details["ids"][0], host_id.to_string());
    let granted = log
        .events
        .iter()
        .find(|e| e.action == "vault.access_granted")
        .unwrap();
    assert_eq!(granted.actor_id, Some(owner.id()));
    assert_eq!(granted.target_user, Some(bob.id()));
    assert_eq!(granted.target_email.as_deref(), Some(bob_email.as_str()));
    assert_eq!(granted.details["role"], "editor");

    // Members see the log too, but device ids only on their own entries.
    let bobs_log: AuditEventList = s
        .json(Method::GET, &audit_path, Some(bob.token()), NOBODY)
        .await;
    assert_eq!(bobs_log.events.len(), log.events.len());
    for e in &bobs_log.events {
        assert_eq!(e.device_id.is_some(), e.actor_id == Some(bob.id()), "{e:?}");
    }

    // Filters: action prefix, vault, actor; pagination via `before`.
    let only_vault: AuditEventList = s
        .json(
            Method::GET,
            &format!("{audit_path}?action=vault.&vault={}", vault.id),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    assert!(!only_vault.events.is_empty());
    assert!(
        only_vault
            .events
            .iter()
            .all(|e| e.action.starts_with("vault.") && e.vault_id == Some(vault.id))
    );
    let by_bob: AuditEventList = s
        .json(
            Method::GET,
            &format!("{audit_path}?actor={}", bob.id()),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    assert!(by_bob.events.iter().all(|e| e.actor_id == Some(bob.id())));
    assert!(by_bob.events.iter().any(|e| e.action == "invite.accepted"));
    let page1: AuditEventList = s
        .json(
            Method::GET,
            &format!("{audit_path}?limit=2"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    assert_eq!(page1.events.len(), 2);
    let before = page1.next_before.expect("more pages");
    let page2: AuditEventList = s
        .json(
            Method::GET,
            &format!("{audit_path}?limit=2&before={before}"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    assert!(page2.events.iter().all(|e| e.id < before));
    assert_eq!(page1.events[1].id, before);

    // Bob (member) cannot delete the team; owner can, and the vault disappears.
    s.expect_status(
        Method::DELETE,
        &format!("/teams/{}", team.id),
        Some(bob.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &format!("/teams/{}", team.id),
        Some(owner.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    let bobs: VaultList = s
        .json(Method::GET, "/vaults", Some(bob.token()), NOBODY)
        .await;
    assert!(bobs.vaults.iter().all(|v| v.id != vault.id));
}

#[tokio::test]
async fn history_push_pull_clear() {
    let s = server!();
    let u = register(s, &unique_email("hist"), "pw-history-12345").await;
    let entry = |kind: HistoryKind| {
        let id = Uuid::new_v4();
        HistoryEntry {
            id,
            kind,
            data: aead::encrypt_str(
                &u.personal_vault_key,
                &Aad::label(&["termoso/v1/history", "command", &id.to_string()]),
                "ls -la",
            )
            .unwrap(),
            key_version: 1,
            created_at: chrono::Utc::now(),
            seq: 0,
            deleted: false,
        }
    };
    let entries = vec![
        entry(HistoryKind::Command),
        entry(HistoryKind::Command),
        entry(HistoryKind::Connection),
    ];
    let _: serde_json::Value = s
        .json(
            Method::POST,
            "/history/push",
            Some(u.token()),
            Some(&HistoryPushRequest {
                entries: entries.clone(),
            }),
        )
        .await;
    // Idempotent re-push.
    let _: serde_json::Value = s
        .json(
            Method::POST,
            "/history/push",
            Some(u.token()),
            Some(&HistoryPushRequest { entries }),
        )
        .await;
    let p: HistoryPullResponse = s
        .json(
            Method::GET,
            "/history/pull?since=0",
            Some(u.token()),
            NOBODY,
        )
        .await;
    assert_eq!(p.entries.len(), 3);
    assert!(p.since > 0);
    let p2: HistoryPullResponse = s
        .json(
            Method::GET,
            &format!("/history/pull?since={}", p.since),
            Some(u.token()),
            NOBODY,
        )
        .await;
    assert!(p2.entries.is_empty());

    s.expect_status(
        Method::POST,
        "/history/clear",
        Some(u.token()),
        Some(&termoso_proto::sync::HistoryClearRequest {
            kind: Some(HistoryKind::Command),
        }),
        StatusCode::NO_CONTENT,
    )
    .await;
    let p3: HistoryPullResponse = s
        .json(
            Method::GET,
            &format!("/history/pull?since={}", p.since),
            Some(u.token()),
            NOBODY,
        )
        .await;
    let deleted = p3.entries.iter().filter(|e| e.deleted).count();
    assert_eq!(deleted, 2, "two commands tombstoned, connection kept");
}

#[tokio::test]
async fn websocket_notifies_on_push_and_revocation() {
    let s = server!();
    let u = register(s, &unique_email("ws"), "pw-websocket-1234").await;
    let vaults: VaultList = s
        .json(Method::GET, "/vaults", Some(u.token()), NOBODY)
        .await;
    let vault_id = vaults.vaults[0].id;

    let (mut ws, _) = tokio_tungstenite::connect_async(s.ws_url()).await.unwrap();
    ws.send(Message::Text(
        serde_json::to_string(&ClientMessage::Auth {
            token: u.token().into(),
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();

    async fn next(
        ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    ) -> ServerMessage {
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(10), ws.next())
                .await
                .expect("ws timeout")
                .expect("ws closed")
                .expect("ws error");
            match msg {
                Message::Text(t) => return serde_json::from_str(&t).unwrap(),
                Message::Ping(_) | Message::Pong(_) => continue,
                other => panic!("unexpected frame {other:?}"),
            }
        }
    }

    let ServerMessage::Hello { vault_ids, .. } = next(&mut ws).await else {
        panic!("expected hello")
    };
    assert_eq!(vault_ids, vec![vault_id]);

    ws.send(Message::Text(
        serde_json::to_string(&ClientMessage::Ping).unwrap().into(),
    ))
    .await
    .unwrap();
    assert!(matches!(next(&mut ws).await, ServerMessage::Pong));

    // A push from another "device" (same user) arrives as VaultChanged.
    let id = Uuid::new_v4();
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![EntityChange {
                    id,
                    kind: "group".into(),
                    vault_id,
                    base_version: None,
                    key_version: 1,
                    data: u.encrypt_entity(&u.personal_vault_key, "group", id, "{}"),
                    updated_at: chrono::Utc::now(),
                }],
                deletes: vec![],
            }),
        )
        .await;
    let PushResult::Ok { seq, .. } = r.results[0] else {
        panic!()
    };
    match next(&mut ws).await {
        ServerMessage::VaultChanged {
            vault_id: v,
            seq: got,
            ..
        } => {
            assert_eq!(v, vault_id);
            assert_eq!(got, seq);
        }
        other => panic!("expected VaultChanged, got {other:?}"),
    }

    // Logging out this session closes the socket with SessionRevoked.
    s.expect_status(
        Method::POST,
        "/auth/logout",
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    assert!(matches!(next(&mut ws).await, ServerMessage::SessionRevoked));

    // Bad first frame → error and close.
    let (mut ws2, _) = tokio_tungstenite::connect_async(s.ws_url()).await.unwrap();
    ws2.send(Message::Text(
        serde_json::to_string(&ClientMessage::Auth {
            token: "garbage".into(),
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();
    assert!(matches!(next(&mut ws2).await, ServerMessage::Error { .. }));
}

#[tokio::test]
async fn admin_endpoints_require_admin() {
    let s = server!();
    let plain = register(s, &unique_email("plain"), "pw-plain-1234567").await;
    s.expect_status(
        Method::GET,
        "/admin/stats",
        Some(plain.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;

    let admin = register(s, "admin@test.local", "pw-admin-1234567").await;
    assert!(admin.session.user.is_admin);
    let stats: serde_json::Value = s
        .json(Method::GET, "/admin/stats", Some(admin.token()), NOBODY)
        .await;
    assert!(stats["users"].as_i64().unwrap() >= 2);
    let users: serde_json::Value = s
        .json(
            Method::GET,
            &format!("/admin/users?q={}", plain.email),
            Some(admin.token()),
            NOBODY,
        )
        .await;
    assert_eq!(users["users"].as_array().unwrap().len(), 1);

    // Disable the user: all their sessions are revoked and login is refused.
    let updated: serde_json::Value = s
        .json(
            Method::PATCH,
            &format!("/admin/users/{}", plain.id()),
            Some(admin.token()),
            Some(&serde_json::json!({ "disabled": true })),
        )
        .await;
    assert_eq!(updated["disabled"], true, "{updated}");
    s.expect_status(
        Method::GET,
        "/account",
        Some(plain.token()),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let err = common::try_login(s, &plain.email, "pw-plain-1234567")
        .await
        .expect_err("disabled user must not log in");
    assert!(err.contains("account_disabled"), "{err}");

    // Admins cannot disable themselves.
    s.expect_status(
        Method::PATCH,
        &format!("/admin/users/{}", admin.id()),
        Some(admin.token()),
        Some(&serde_json::json!({ "disabled": true })),
        StatusCode::BAD_REQUEST,
    )
    .await;
}
