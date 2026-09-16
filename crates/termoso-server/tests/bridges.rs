//! API bridges: restricted sessions that can only sync the vaults sealed to them.

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_crypto::keys::KeyPair;
use termoso_crypto::sealed;
use termoso_proto::auth::DeviceList;
use termoso_proto::bridge::*;
use termoso_proto::sync::{
    EntityChange, PullRequest, PullResponse, PushRequest, PushResponse, PushResult,
};
use termoso_proto::vault::{
    RotateVaultKeyRequest, RotateVaultKeyResponse, SealedKeyFor, VaultList,
};
use uuid::Uuid;

macro_rules! server {
    () => {
        match server().await {
            Some(s) => s,
            None => return,
        }
    };
}

async fn personal_vault(s: &TestServer, u: &User) -> Uuid {
    let vaults: VaultList = s
        .json(Method::GET, "/vaults", Some(u.token()), NOBODY)
        .await;
    vaults.vaults[0].id
}

async fn create_bridge(
    s: &TestServer,
    u: &User,
    vault_id: Uuid,
) -> (KeyPair, CreateBridgeResponse) {
    let kp = KeyPair::generate();
    let sealed_key = sealed::seal_vault_key(kp.public(), &u.personal_vault_key).unwrap();
    let r: CreateBridgeResponse = s
        .json(
            Method::POST,
            "/account/bridges",
            Some(u.token()),
            Some(&CreateBridgeRequest {
                name: "ansible".into(),
                public_key: kp.public_b64(),
                vaults: vec![BridgeVaultKey {
                    vault_id,
                    sealed_key,
                }],
            }),
        )
        .await;
    (kp, r)
}

#[tokio::test]
async fn bridge_lifecycle_and_confinement() {
    let s = server!();
    let u = register(s, &unique_email("bridge"), "pw-bridge-1234567").await;
    let vault_id = personal_vault(s, &u).await;
    let (kp, created) = create_bridge(s, &u, vault_id).await;
    assert_eq!(created.bridge.name, "ansible");
    assert_eq!(created.bridge.vaults.len(), 1);
    assert!(created.bridge.last_used_at.is_none());

    // The bridge sees itself and can open the vault key with its private half.
    let me: BridgeSelf = s
        .json(Method::GET, "/bridge/me", Some(&created.token), NOBODY)
        .await;
    assert_eq!(me.id, created.bridge.id);
    assert_eq!(me.user_id, u.id());
    let sealed_key = me.vaults[0].sealed_key.clone().expect("sealed key");
    let key = sealed::open_vault_key(&kp, &sealed_key).unwrap();
    assert_eq!(key.as_bytes(), u.personal_vault_key.as_bytes());

    // A person is not a bridge.
    s.expect_status(
        Method::GET,
        "/bridge/me",
        Some(u.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;

    // Everything outside sync is off limits for the bridge token.
    for path in [
        "/account",
        "/vaults",
        "/teams",
        "/account/bridges",
        "/account/devices",
    ] {
        s.expect_status(
            Method::GET,
            path,
            Some(&created.token),
            NOBODY,
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    s.expect_status(
        Method::DELETE,
        &format!("/account/bridges/{}", created.bridge.id),
        Some(&created.token),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;

    // The bridge pushes an encrypted host through the normal sync path.
    let host_id = Uuid::new_v4();
    let push = PushRequest {
        changes: vec![EntityChange {
            id: host_id,
            kind: "host".into(),
            vault_id,
            base_version: None,
            key_version: 1,
            data: u.encrypt_entity(
                &key,
                "host",
                host_id,
                r#"{"label":"db-1","address":"10.0.0.5","external_id":"i-123"}"#,
            ),
            updated_at: chrono::Utc::now(),
        }],
        deletes: vec![],
    };
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(&created.token),
            Some(&push),
        )
        .await;
    assert!(
        matches!(r.results[0], PushResult::Ok { .. }),
        "{:?}",
        r.results
    );

    // The owner pulls it and it is attributed to the bridge device.
    let pulled: PullResponse = s
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
    let e = pulled
        .entities
        .iter()
        .find(|e| e.id == host_id)
        .expect("host synced");
    assert_eq!(e.updated_by_device, Some(created.bridge.device_id));

    // The bridge shows up on the bridges page, not among devices. Session
    // touches are throttled, so pretend the last one was a while ago.
    s.age_last_used(&created.token).await;
    s.json::<(), BridgeSelf>(Method::GET, "/bridge/me", Some(&created.token), NOBODY)
        .await;
    let list: BridgeList = s
        .json(Method::GET, "/account/bridges", Some(u.token()), NOBODY)
        .await;
    assert_eq!(list.bridges.len(), 1);
    assert!(list.bridges[0].last_used_at.is_some());
    let devices: DeviceList = s
        .json(Method::GET, "/account/devices", Some(u.token()), NOBODY)
        .await;
    assert!(
        devices
            .devices
            .iter()
            .all(|d| d.id != created.bridge.device_id)
    );

    // Revoke: the token dies immediately.
    s.expect_status(
        Method::DELETE,
        &format!("/account/bridges/{}", created.bridge.id),
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::GET,
        "/bridge/me",
        Some(&created.token),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let list: BridgeList = s
        .json(Method::GET, "/account/bridges", Some(u.token()), NOBODY)
        .await;
    assert!(list.bridges.is_empty());
}

#[tokio::test]
async fn bridge_is_confined_to_sealed_vaults_and_rotation_unseals() {
    let s = server!();
    let u = register(s, &unique_email("bridge-scope"), "pw-bridge-1234567").await;
    let vault_id = personal_vault(s, &u).await;
    let other = register(s, &unique_email("bridge-other"), "pw-bridge-1234567").await;
    let other_vault = personal_vault(s, &other).await;

    // A bridge cannot be pointed at a vault the owner cannot write.
    let kp = KeyPair::generate();
    s.expect_status(
        Method::POST,
        "/account/bridges",
        Some(u.token()),
        Some(&CreateBridgeRequest {
            name: "x".into(),
            public_key: kp.public_b64(),
            vaults: vec![BridgeVaultKey {
                vault_id: other_vault,
                sealed_key: sealed::seal_vault_key(kp.public(), &u.personal_vault_key).unwrap(),
            }],
        }),
        StatusCode::NOT_FOUND,
    )
    .await;

    // Create with no vaults at all, then attach the personal vault.
    let r: CreateBridgeResponse = s
        .json(
            Method::POST,
            "/account/bridges",
            Some(u.token()),
            Some(&CreateBridgeRequest {
                name: "cmdb".into(),
                public_key: kp.public_b64(),
                vaults: vec![],
            }),
        )
        .await;
    let bridge_id = r.bridge.id;
    let token = r.token;

    // Out of scope: pull returns nothing, push is forbidden per entity.
    let pulled: PullResponse = s
        .json(
            Method::POST,
            "/sync/pull",
            Some(&token),
            Some(&PullRequest {
                cursors: cursors(&[(vault_id, 0)]),
                limit: None,
            }),
        )
        .await;
    assert!(pulled.entities.is_empty());
    let host_id = Uuid::new_v4();
    let change = |key_version: i32| EntityChange {
        id: host_id,
        kind: "host".into(),
        vault_id,
        base_version: None,
        key_version,
        data: u.encrypt_entity(&u.personal_vault_key, "host", host_id, r#"{"label":"a"}"#),
        updated_at: chrono::Utc::now(),
    };
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(&token),
            Some(&PushRequest {
                changes: vec![change(1)],
                deletes: vec![],
            }),
        )
        .await;
    assert!(
        matches!(&r.results[0], PushResult::Error { code, .. } if code == "forbidden"),
        "{:?}",
        r.results
    );

    let b: Bridge = s
        .json(
            Method::PUT,
            &format!("/account/bridges/{bridge_id}/vaults"),
            Some(u.token()),
            Some(&vec![BridgeVaultKey {
                vault_id,
                sealed_key: sealed::seal_vault_key(kp.public(), &u.personal_vault_key).unwrap(),
            }]),
        )
        .await;
    assert_eq!(b.vaults.len(), 1);
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(&token),
            Some(&PushRequest {
                changes: vec![change(1)],
                deletes: vec![],
            }),
        )
        .await;
    assert!(
        matches!(r.results[0], PushResult::Ok { .. }),
        "{:?}",
        r.results
    );

    // Key rotation drops the bridge's sealed key until the owner re-seals.
    let rot: RotateVaultKeyResponse = s
        .json(
            Method::POST,
            &format!("/vaults/{vault_id}/rotate-key"),
            Some(u.token()),
            Some(&RotateVaultKeyRequest {
                base_key_version: 1,
                members: vec![SealedKeyFor {
                    user_id: u.id(),
                    sealed_key: sealed::seal_vault_key(u.keypair.public(), &u.personal_vault_key)
                        .unwrap(),
                }],
            }),
        )
        .await;
    assert_eq!(rot.key_version, 2);
    let me: BridgeSelf = s
        .json(Method::GET, "/bridge/me", Some(&token), NOBODY)
        .await;
    assert_eq!(me.vaults.len(), 1);
    assert!(me.vaults[0].sealed_key.is_none());
    assert_eq!(me.vaults[0].key_version, 2);
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(&token),
            Some(&PushRequest {
                changes: vec![EntityChange {
                    id: Uuid::new_v4(),
                    ..change(2)
                }],
                deletes: vec![],
            }),
        )
        .await;
    assert!(
        matches!(&r.results[0], PushResult::Error { code, .. } if code == "forbidden"),
        "{:?}",
        r.results
    );

    // Re-seal → back in business.
    s.json::<_, Bridge>(
        Method::PUT,
        &format!("/account/bridges/{bridge_id}/vaults"),
        Some(u.token()),
        Some(&vec![BridgeVaultKey {
            vault_id,
            sealed_key: sealed::seal_vault_key(kp.public(), &u.personal_vault_key).unwrap(),
        }]),
    )
    .await;
    let me: BridgeSelf = s
        .json(Method::GET, "/bridge/me", Some(&token), NOBODY)
        .await;
    assert!(me.vaults[0].sealed_key.is_some());

    // Creating a bridge needs a fresh step-up; a stale session is refused.
    s.age_step_up(u.token()).await;
    s.expect_status(
        Method::POST,
        "/account/bridges",
        Some(u.token()),
        Some(&CreateBridgeRequest {
            name: "late".into(),
            public_key: KeyPair::generate().public_b64(),
            vaults: vec![],
        }),
        StatusCode::FORBIDDEN,
    )
    .await;
}
