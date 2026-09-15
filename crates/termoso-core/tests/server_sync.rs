//! Client core against the real server: OPAQUE account flows, vault keys,
//! entity / history / log sync, conflicts and the realtime loop.
//!
//! Needs PostgreSQL + Redis (see `tests/common`); MinIO and Mailpit unlock the
//! log and email cases. Everything is skipped when they are absent.

mod common;

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use common::{Device, login, register, unique_email};
use serde_json::json;
use termoso_core::account::{self, LoginFlow, LoginStep};
use termoso_core::model::{Host, Identity};
use termoso_core::store::{CommandHistory, EntityFilter, LogMeta};
use termoso_core::sync::{ConflictPolicy, SyncEngine, SyncEvent, SyncOptions};
use termoso_crypto::keys::{SymmetricKey, public_key_from_b64};
use termoso_crypto::sealed::seal_vault_key;
use termoso_proto::auth::MfaCredential;
use termoso_proto::error::codes;
use termoso_proto::vault::{RotateVaultKeyRequest, SealedKeyFor};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple 42";

fn engine(d: &Device, opts: SyncOptions) -> Arc<SyncEngine> {
    SyncEngine::new(d.api.clone(), d.store.clone(), opts)
}

fn host(label: &str) -> Host {
    Host {
        label: label.into(),
        address: format!("{label}.example.net"),
        ..Host::default()
    }
}

fn labels(d: &Device) -> Vec<String> {
    let mut v: Vec<String> = d
        .store
        .list::<Host>(Some(d.personal_vault()))
        .expect("list hosts")
        .into_iter()
        .map(|e| e.data.label)
        .collect();
    v.sort();
    v
}

async fn wait_for<F: FnMut(&SyncEvent) -> bool>(
    rx: &mut tokio::sync::broadcast::Receiver<SyncEvent>,
    mut pred: F,
) -> SyncEvent {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let ev = rx.recv().await.expect("event stream open");
            if pred(&ev) {
                return ev;
            }
        }
    })
    .await
    .expect("expected sync event in time")
}

/// A TOTP code different from `used` (waits for the next 30 s step if needed).
async fn fresh_code(totp: &totp_rs::TOTP, used: &str) -> String {
    loop {
        let c = totp.generate_current().unwrap();
        if c != used {
            return c;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

// ───────────────────────────── account ─────────────────────────────

#[tokio::test]
async fn register_login_resume_and_sign_out() {
    let s = server_or_skip!();
    let email = unique_email("acct");
    let (first, phrase) = register(s, &email, PASSWORD).await;

    assert_eq!(phrase.split_whitespace().count(), 24);
    assert!(first.signed_in.expires_at.is_some());
    assert_eq!(first.signed_in.vaults.len(), 1, "personal vault unlocked");
    let acct = first.store.account().unwrap().expect("account persisted");
    assert_eq!(acct.email, email);
    assert!(first.api.token().is_some());
    let personal = first.store.vault(first.personal_vault()).unwrap();
    assert!(personal.unlocked && personal.kind.is_synced());

    // The server only ever saw public material: unwrapping our own key from
    // what it stores requires the OPAQUE export key we never sent.
    let keys = first.api.account().await.unwrap().keys;
    assert_eq!(keys.public_key, acct.public_key);
    assert!(!keys.wrapped_private_key.contains(&phrase));

    // Wrong password never gets past OPAQUE.
    let bad = LoginFlow::start(
        s.api(),
        s.store(),
        &email,
        "definitely not it",
        common::device("bad", &s.store()),
        None,
    )
    .await;
    assert!(
        matches!(&bad, Err(e) if e.is_api_code(codes::INVALID_CREDENTIALS)),
        "{bad:?}"
    );

    // A second device signs in and derives the same account key.
    let second = login(s, &email, PASSWORD).await;
    assert_eq!(
        second.store.account().unwrap().unwrap().public_key,
        acct.public_key
    );
    assert_eq!(second.personal_vault(), first.personal_vault());
    assert_ne!(
        second.signed_in.account.device_id,
        first.signed_in.account.device_id
    );

    // Resume from persisted state with a fresh API client.
    let api2 = s.api();
    let resumed = account::resume(&api2, &second.store)
        .await
        .unwrap()
        .expect("still signed in");
    assert_eq!(resumed.account.user_id, acct.user_id);
    assert!(resumed.expires_at.is_none());
    assert!(api2.token().is_some());
    assert_eq!(api2.devices().await.unwrap().len(), 2);

    // Sign-out revokes the server session and forgets everything local.
    account::sign_out(&api2, &second.store).await.unwrap();
    assert!(second.store.account().unwrap().is_none());
    assert!(api2.token().is_none());
    assert!(
        account::resume(&api2, &second.store)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(first.api.devices().await.unwrap().len(), 1);
    assert!(second.api.devices().await.unwrap_err().is_unauthorized());
}

#[tokio::test]
async fn login_with_totp_second_factor() {
    let s = server_or_skip!();
    let email = unique_email("mfa");
    let (first, _) = register(s, &email, PASSWORD).await;
    let token = first.store.account_secrets().unwrap().token;

    let setup: termoso_proto::auth::TotpSetupResponse = s
        .raw(
            reqwest::Method::POST,
            "/account/mfa/totp/setup",
            &token,
            None,
        )
        .await
        .json()
        .await
        .unwrap();
    let totp = totp_rs::TOTP::from_url(&setup.otpauth_url).unwrap();
    let enrolled = totp.generate_current().unwrap();
    let confirm = s
        .raw(
            reqwest::Method::POST,
            "/account/mfa/totp/confirm",
            &token,
            Some(json!({ "code": enrolled })),
        )
        .await;
    assert!(confirm.status().is_success(), "{}", confirm.status());

    let store = s.store();
    let (mut flow, step) = LoginFlow::start(
        s.api(),
        store.clone(),
        &email,
        PASSWORD,
        common::device("phone", &store),
        None,
    )
    .await
    .unwrap();
    let LoginStep::MfaRequired { methods } = step else {
        panic!("expected MFA challenge, got {step:?}");
    };
    assert!(!methods.is_empty());
    assert!(store.account().unwrap().is_none(), "nothing persisted yet");

    let wrong = flow
        .mfa(MfaCredential::Totp {
            code: "000000".into(),
        })
        .await;
    assert!(
        matches!(&wrong, Err(e) if e.is_api_code(codes::INVALID_MFA)),
        "{wrong:?}"
    );

    let done = flow
        .mfa(MfaCredential::Totp {
            code: fresh_code(&totp, &enrolled).await,
        })
        .await
        .unwrap();
    let LoginStep::Done(signed_in) = done else {
        panic!("expected completion, got {done:?}");
    };
    assert_eq!(signed_in.vaults, first.signed_in.vaults);
    assert_eq!(
        store.account().unwrap().unwrap().public_key,
        first.signed_in.account.public_key
    );
}

#[tokio::test]
async fn new_device_needs_emailed_approval() {
    let s = server_with_mail_or_skip!();
    let email = unique_email("approve");
    let (first, _) = register(s, &email, PASSWORD).await;
    let token = first.store.account_secrets().unwrap().token;
    let code = s.emailed_code(&email, "email verification").await;
    let verified = s
        .raw(
            reqwest::Method::POST,
            "/account/email/verify/confirm",
            &token,
            Some(json!({ "code": code })),
        )
        .await;
    assert!(verified.status().is_success(), "{}", verified.status());

    let store = s.store();
    let (mut flow, step) = LoginFlow::start(
        s.api(),
        store.clone(),
        &email,
        PASSWORD,
        common::device("laptop", &store),
        None,
    )
    .await
    .unwrap();
    let LoginStep::DeviceApprovalRequired { email_hint, .. } = step else {
        panic!("expected device approval, got {step:?}");
    };
    assert!(email_hint.contains('@'));

    let wrong = flow.approve_device("000000").await;
    assert!(wrong.is_err(), "{wrong:?}");
    flow.resend_device_code().await.unwrap();
    let code = s.emailed_code(&email, "new device sign-in").await;
    let done = flow.approve_device(&code).await.unwrap();
    assert!(matches!(done, LoginStep::Done(_)), "{done:?}");
    assert_eq!(
        store
            .vaults()
            .unwrap()
            .iter()
            .filter(|v| v.unlocked)
            .count(),
        2
    );
}

// ───────────────────────────── entities ─────────────────────────────

#[tokio::test]
async fn entities_round_trip_with_tombstones_and_cursors() {
    let s = server_or_skip!();
    let email = unique_email("ent");
    let (a, _) = register(s, &email, PASSWORD).await;
    let b = login(s, &email, PASSWORD).await;
    let vault = a.personal_vault();
    let ea = engine(
        &a,
        SyncOptions {
            batch: 2,
            ..SyncOptions::default()
        },
    );
    let eb = engine(
        &b,
        SyncOptions {
            batch: 2,
            ..SyncOptions::default()
        },
    );

    // Nothing to do on an empty account.
    let r = ea.sync_once().await.unwrap();
    assert_eq!((r.pushed, r.pulled), (0, 0));

    let ids: Vec<Uuid> = ["alpha", "bravo", "charlie", "delta", "echo"]
        .iter()
        .map(|l| a.store.insert(vault, &host(l)).unwrap())
        .collect();
    assert_eq!(a.store.pending_changes().unwrap(), 5);
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r.pushed, 5);
    assert!(r.errors.is_empty());
    assert_eq!(a.store.pending_changes().unwrap(), 0);
    let cursor_a = a.store.vault(vault).unwrap().cursor;
    assert!(cursor_a > 0);

    // B pulls in pages of two and ends on the same cursor.
    let r = eb.sync_once().await.unwrap();
    assert_eq!(r.pulled, 5);
    assert_eq!(labels(&b), ["alpha", "bravo", "charlie", "delta", "echo"]);
    assert_eq!(b.store.vault(vault).unwrap().cursor, cursor_a);
    let got = b.store.get::<Host>(ids[0]).unwrap().unwrap();
    assert_eq!(got.data.address, "alpha.example.net");
    assert!(!got.dirty && got.version >= 1);

    // Update + delete propagate; the tombstone survives locally until pushed.
    let mut renamed = host("alpha");
    renamed.label = "alpha-2".into();
    b.store.update(ids[0], &renamed).unwrap();
    b.store.delete(ids[1]).unwrap();
    assert!(b.store.get::<Host>(ids[1]).unwrap().is_none());
    let deleted_rows = b
        .store
        .rows(&EntityFilter {
            vault_id: Some(vault),
            kind: None,
            include_deleted: true,
        })
        .unwrap();
    assert_eq!(deleted_rows.iter().filter(|r| r.deleted).count(), 1);
    let r = eb.sync_once().await.unwrap();
    assert_eq!(r.pushed, 2);
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r.pulled, 2);
    assert_eq!(labels(&a), ["alpha-2", "charlie", "delta", "echo"]);
    assert!(a.store.get::<Host>(ids[1]).unwrap().is_none());

    // Idle pass changes nothing and cursors stay put.
    let before = a.store.vault(vault).unwrap().cursor;
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r, Default::default());
    assert_eq!(a.store.vault(vault).unwrap().cursor, before);

    // Server ciphertext is opaque: no plaintext label leaks.
    let pull = a
        .api
        .sync_pull(&termoso_proto::sync::PullRequest {
            cursors: Default::default(),
            limit: None,
        })
        .await
        .unwrap();
    assert!(
        pull.entities
            .iter()
            .all(|e| !e.data.contains("example.net"))
    );
}

fn identity(label: &str) -> Identity {
    Identity {
        label: label.into(),
        username: format!("{label}-user"),
        password: Some(format!("{label}-secret")),
        is_visible: true,
        ..Identity::default()
    }
}

fn identity_labels(d: &Device) -> Vec<String> {
    let mut v: Vec<String> = d
        .store
        .list::<Identity>(Some(d.personal_vault()))
        .expect("list identities")
        .into_iter()
        .map(|e| e.data.label)
        .collect();
    v.sort();
    v
}

async fn server_kinds(d: &Device) -> Vec<(String, bool)> {
    let mut v: Vec<(String, bool)> = d
        .api
        .sync_pull(&termoso_proto::sync::PullRequest {
            cursors: [(d.personal_vault(), 0)].into_iter().collect(),
            limit: None,
        })
        .await
        .unwrap()
        .entities
        .into_iter()
        .map(|e| (e.kind, e.deleted))
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn local_only_credentials_stay_off_the_server() {
    let s = server_or_skip!();
    let email = unique_email("cred");
    let (a, _) = register(s, &email, PASSWORD).await;
    let b = login(s, &email, PASSWORD).await;
    let vault = a.personal_vault();
    let local = SyncOptions {
        sync_credentials: false,
        ..SyncOptions::default()
    };

    // A keeps credentials on the device: hosts go up, the identity does not.
    let ea = engine(&a, local.clone());
    a.store.insert(vault, &host("web")).unwrap();
    let kept = a.store.insert(vault, &identity("kept")).unwrap();
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r.pushed, 1);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(server_kinds(&a).await, [("host".to_string(), false)]);
    assert!(a.store.get::<Identity>(kept).unwrap().unwrap().dirty);

    // B (syncing) uploads one; A does not pull it, B does not see A's.
    let eb = engine(&b, SyncOptions::default());
    let shared = b.store.insert(vault, &identity("shared")).unwrap();
    eb.sync_once().await.unwrap();
    eb.sync_once().await.unwrap();
    assert_eq!(identity_labels(&b), ["shared"]);
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r.pulled, 0);
    assert_eq!(identity_labels(&a), ["kept"]);
    // Cursor still advances past the skipped row.
    assert!(a.store.vault(vault).unwrap().cursor >= b.store.vault(vault).unwrap().cursor);

    // Deleting a local-only credential is a no-op for the server.
    let gone = a.store.insert(vault, &identity("gone")).unwrap();
    a.store.delete(gone).unwrap();
    assert_eq!(ea.sync_once().await.unwrap().pushed, 0);
    assert_eq!(a.store.pending_changes().unwrap(), 1);

    // Switching credential sync on: the kept identity goes up, the shared
    // one comes down, both devices converge.
    let ea_on = engine(&a, SyncOptions::default());
    let r = ea_on.resync_credentials().await.unwrap();
    assert_eq!(r.pushed, 1);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(identity_labels(&a), ["kept", "shared"]);
    eb.sync_once().await.unwrap();
    assert_eq!(identity_labels(&b), ["kept", "shared"]);
    assert_eq!(
        server_kinds(&a).await,
        [
            ("host".to_string(), false),
            ("identity".to_string(), false),
            ("identity".to_string(), false),
        ]
    );

    // Switching it off again purges the server copies but keeps A's rows,
    // and B (still syncing) loses them through the tombstones.
    let ea_off = engine(&a, local);
    assert_eq!(ea_off.purge_credentials().await.unwrap(), 2);
    assert_eq!(identity_labels(&a), ["kept", "shared"]);
    assert_eq!(
        server_kinds(&a).await,
        [
            ("host".to_string(), false),
            ("identity".to_string(), true),
            ("identity".to_string(), true),
        ]
    );
    assert!(a.store.get::<Identity>(shared).unwrap().unwrap().dirty);
    eb.sync_once().await.unwrap();
    assert!(identity_labels(&b).is_empty());
    // The purge is idempotent and later passes leave everything as is.
    assert_eq!(ea_off.purge_credentials().await.unwrap(), 2);
    assert_eq!(ea_off.sync_once().await.unwrap().pushed, 0);
    assert_eq!(identity_labels(&a), ["kept", "shared"]);

    // Turning sync back on resurrects both from A.
    let ea_on = engine(&a, SyncOptions::default());
    let r = ea_on.resync_credentials().await.unwrap();
    assert_eq!(r.pushed, 2);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    eb.sync_once().await.unwrap();
    assert_eq!(identity_labels(&b), ["kept", "shared"]);
    assert_eq!(a.store.pending_changes().unwrap(), 0);
}

async fn conflicting_edit(policy: ConflictPolicy) -> (Device, Device, Uuid) {
    let s = common::server().await.expect("services");
    let email = unique_email("conf");
    let (a, _) = register(s, &email, PASSWORD).await;
    let b = login(s, &email, PASSWORD).await;
    let vault = a.personal_vault();
    let ea = engine(&a, SyncOptions::default());
    let eb = engine(
        &b,
        SyncOptions {
            conflict: policy,
            ..SyncOptions::default()
        },
    );

    let id = a.store.insert(vault, &host("shared")).unwrap();
    ea.sync_once().await.unwrap();
    eb.sync_once().await.unwrap();

    // B edits first (older), A edits later and wins the race to the server.
    let mut mine = host("shared");
    mine.label = "from-b".into();
    b.store.update(id, &mine).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let mut theirs = host("shared");
    theirs.label = "from-a".into();
    a.store.update(id, &theirs).unwrap();
    ea.sync_once().await.unwrap();

    let r = eb.sync_once().await.unwrap();
    assert_eq!(r.conflicts, 1);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    (a, b, id)
}

#[tokio::test]
async fn conflict_server_wins_takes_remote_row() {
    if common::server().await.is_none() {
        return;
    }
    let (a, b, id) = conflicting_edit(ConflictPolicy::ServerWins).await;
    let row = b.store.get::<Host>(id).unwrap().unwrap();
    assert_eq!(row.data.label, "from-a");
    assert!(!row.dirty);
    assert_eq!(b.store.pending_changes().unwrap(), 0);
    engine(&a, SyncOptions::default())
        .sync_once()
        .await
        .unwrap();
    assert_eq!(
        a.store.get::<Host>(id).unwrap().unwrap().data.label,
        "from-a"
    );
}

#[tokio::test]
async fn conflict_local_wins_rebases_and_pushes() {
    if common::server().await.is_none() {
        return;
    }
    let (a, b, id) = conflicting_edit(ConflictPolicy::LocalWins).await;
    let row = b.store.get::<Host>(id).unwrap().unwrap();
    assert_eq!(row.data.label, "from-b");
    assert!(!row.dirty, "rebased row was pushed in the same pass");
    let ea = engine(&a, SyncOptions::default());
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r.pulled, 1);
    assert_eq!(
        a.store.get::<Host>(id).unwrap().unwrap().data.label,
        "from-b"
    );
}

#[tokio::test]
async fn conflict_newest_wins_prefers_later_edit() {
    if common::server().await.is_none() {
        return;
    }
    // A's edit is newer → B (NewestWins) adopts it.
    let (a, b, id) = conflicting_edit(ConflictPolicy::NewestWins).await;
    assert_eq!(
        b.store.get::<Host>(id).unwrap().unwrap().data.label,
        "from-a"
    );
    assert_eq!(
        a.store.get::<Host>(id).unwrap().unwrap().data.label,
        "from-a"
    );

    // And the other way round: B edits last, so its row survives.
    let eb = engine(&b, SyncOptions::default());
    let ea = engine(&a, SyncOptions::default());
    let mut older = host("shared");
    older.label = "a-again".into();
    a.store.update(id, &older).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let mut newer = host("shared");
    newer.label = "b-again".into();
    b.store.update(id, &newer).unwrap();
    ea.sync_once().await.unwrap();
    let r = eb.sync_once().await.unwrap();
    assert_eq!(r.conflicts, 1);
    ea.sync_once().await.unwrap();
    assert_eq!(
        a.store.get::<Host>(id).unwrap().unwrap().data.label,
        "b-again"
    );
    assert_eq!(
        b.store.get::<Host>(id).unwrap().unwrap().data.label,
        "b-again"
    );
}

#[tokio::test]
async fn vault_key_rotation_reencrypts_and_recovers_stale_rows() {
    let s = server_or_skip!();
    let email = unique_email("rot");
    let (a, _) = register(s, &email, PASSWORD).await;
    let b = login(s, &email, PASSWORD).await;
    let vault = a.personal_vault();
    let ea = engine(&a, SyncOptions::default());
    let eb = engine(&b, SyncOptions::default());
    let id = a.store.insert(vault, &host("before")).unwrap();
    ea.sync_once().await.unwrap();
    eb.sync_once().await.unwrap();

    // A rotates the personal vault key (sealed only for itself) and pushes.
    let new_key = SymmetricKey::generate();
    let me = a.store.account().unwrap().unwrap();
    let recipient = public_key_from_b64(&me.public_key).unwrap();
    let sealed = seal_vault_key(&recipient, &new_key).unwrap();
    let rotated = a
        .api
        .rotate_vault_key(
            vault,
            &RotateVaultKeyRequest {
                base_key_version: a.store.vault(vault).unwrap().key_version,
                members: vec![SealedKeyFor {
                    user_id: me.user_id,
                    sealed_key: sealed,
                }],
            },
        )
        .await
        .unwrap();
    let unlocked = account::refresh_vaults(&a.api, &a.store).await.unwrap();
    assert_eq!(unlocked, vec![vault]);
    assert_eq!(
        a.store.vault(vault).unwrap().key_version,
        rotated.key_version
    );
    assert_eq!(
        a.store.pending_changes().unwrap(),
        1,
        "row re-encrypted and dirty"
    );
    assert_eq!(
        a.store.get::<Host>(id).unwrap().unwrap().data.label,
        "before"
    );
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r.pushed, 1);

    // B is still on the old key: its stale push is rejected, the key refresh
    // kicks in and the following pass succeeds with the new ciphertext.
    let mut edit = host("before");
    edit.label = "after".into();
    b.store.update(id, &edit).unwrap();
    let r = eb.sync_once().await.unwrap();
    assert!(
        r.errors.iter().any(|(_, code)| code == "stale_key_version") || r.conflicts == 1,
        "{r:?}"
    );
    assert_eq!(
        b.store.vault(vault).unwrap().key_version,
        rotated.key_version
    );
    let r = eb.sync_once().await.unwrap();
    assert!(r.errors.is_empty(), "{r:?}");
    ea.sync_once().await.unwrap();
    let final_a = a.store.get::<Host>(id).unwrap().unwrap().data.label;
    let final_b = b.store.get::<Host>(id).unwrap().unwrap().data.label;
    assert_eq!(final_a, final_b);
}

// ───────────────────────────── history ─────────────────────────────

#[tokio::test]
async fn history_syncs_between_devices() {
    let s = server_or_skip!();
    let email = unique_email("hist");
    let (a, _) = register(s, &email, PASSWORD).await;
    let b = login(s, &email, PASSWORD).await;
    let ea = engine(&a, SyncOptions::default());
    let eb = engine(&b, SyncOptions::default());

    for cmd in ["ls -la", "htop", "ls -la /tmp"] {
        a.store
            .record_command(&CommandHistory {
                host_id: None,
                command: cmd.into(),
            })
            .unwrap();
    }
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r.history, (3, 0));
    assert!(a.store.account().unwrap().unwrap().history_cursor > 0);

    let r = eb.sync_once().await.unwrap();
    assert_eq!(r.history, (0, 3));
    let mut cmds: Vec<String> = b
        .store
        .commands(10)
        .unwrap()
        .into_iter()
        .map(|c| c.data.command)
        .collect();
    cmds.sort();
    assert_eq!(cmds, ["htop", "ls -la", "ls -la /tmp"]);
    assert_eq!(
        b.store.account().unwrap().unwrap().history_cursor,
        a.store.account().unwrap().unwrap().history_cursor
    );

    // Second pass is a no-op on both sides.
    assert_eq!(ea.sync_once().await.unwrap().history, (0, 0));
    assert_eq!(eb.sync_once().await.unwrap().history, (0, 0));
}

// ───────────────────────────── session logs ─────────────────────────────

fn log_meta(label: &str) -> LogMeta {
    LogMeta {
        host_id: None,
        label: label.into(),
        target: "root@box".into(),
        protocol: "ssh".into(),
        started_at: Utc::now(),
        ended_at: Some(Utc::now()),
        cols: 120,
        rows: 40,
    }
}

#[tokio::test]
async fn session_logs_upload_download_and_delete() {
    let s = server_with_storage_or_skip!();
    let email = unique_email("logs");
    let (a, _) = register(s, &email, PASSWORD).await;
    let b = login(s, &email, PASSWORD).await;
    let vault = a.personal_vault();
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let ea = engine(
        &a,
        SyncOptions {
            log_dir: Some(dir_a.path().to_path_buf()),
            ..SyncOptions::default()
        },
    );
    let eb = engine(
        &b,
        SyncOptions {
            log_dir: Some(dir_b.path().to_path_buf()),
            ..SyncOptions::default()
        },
    );

    let body = b"$ uname -a\r\nLinux box 6.1\r\n".repeat(50);
    let id = a.store.begin_log(vault, &log_meta("uname")).unwrap();
    // Unfinished recordings are never uploaded.
    assert_eq!(ea.sync_once().await.unwrap().logs, (0, 0));
    a.store
        .finish_log(id, &log_meta("uname"), &body, dir_a.path())
        .unwrap();
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r.logs.0, 1);
    let item = a
        .store
        .logs()
        .unwrap()
        .into_iter()
        .find(|l| l.id == id)
        .unwrap();
    assert!(item.uploaded && item.completed && item.cached);

    // B sees the metadata, downloads and decrypts the body.
    let r = eb.sync_once().await.unwrap();
    assert_eq!(r.logs.1, 1);
    let mine = b.store.logs().unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].meta.label, "uname");
    assert!(!mine[0].cached);
    assert!(b.store.read_log(id).is_err());
    eb.download_log(id).await.unwrap();
    assert_eq!(b.store.read_log(id).unwrap(), body);

    // Server holds only ciphertext + encrypted metadata.
    let listed = a.api.logs(0, 10).await.unwrap();
    assert_eq!(listed.logs.len(), 1);
    assert!(!listed.logs[0].meta.contains("uname"));
    assert_eq!(
        listed.logs[0].size_bytes as usize,
        std::fs::metadata(dir_a.path().join(format!("{id}.tlog")))
            .unwrap()
            .len() as usize
    );

    // Re-running the upload is idempotent.
    assert_eq!(ea.sync_once().await.unwrap().logs, (0, 0));

    // With uploads off, a personal recording stays on the device (a team
    // vault's `session_logging` policy is the only thing that overrides it).
    let kept = a.store.begin_log(vault, &log_meta("local")).unwrap();
    a.store
        .finish_log(kept, &log_meta("local"), b"$ ls\r\n", dir_a.path())
        .unwrap();
    let ea_off = engine(
        &a,
        SyncOptions {
            log_dir: Some(dir_a.path().to_path_buf()),
            upload_logs: false,
            ..SyncOptions::default()
        },
    );
    assert_eq!(ea_off.sync_once().await.unwrap().logs.0, 0);
    assert_eq!(a.api.logs(0, 10).await.unwrap().logs.len(), 1);
    a.store.delete_log(kept).unwrap();

    // Delete on B tombstones locally, then propagates.
    b.store.delete_log(id).unwrap();
    assert!(b.store.logs().unwrap().is_empty());
    eb.sync_once().await.unwrap();
    let r = ea.sync_once().await.unwrap();
    assert_eq!(r.logs.1, 1);
    assert!(a.store.logs().unwrap().is_empty());
    assert!(!dir_a.path().join(format!("{id}.tlog")).exists());
    assert!(
        a.api
            .logs(0, 10)
            .await
            .unwrap()
            .logs
            .iter()
            .all(|l| l.deleted)
    );
}

// ───────────────────────────── realtime ─────────────────────────────

#[tokio::test]
async fn realtime_loop_reacts_to_peers_and_revocation() {
    let s = server_or_skip!();
    let email = unique_email("ws");
    let (a, _) = register(s, &email, PASSWORD).await;
    let b = login(s, &email, PASSWORD).await;
    let vault = a.personal_vault();
    let ea = engine(
        &a,
        SyncOptions {
            debounce: Duration::from_millis(50),
            ..SyncOptions::default()
        },
    );
    let eb = engine(&b, SyncOptions::default());

    let mut events = ea.subscribe();
    let cancel = CancellationToken::new();
    let loop_task = tokio::spawn(ea.clone().run(cancel.clone()));
    wait_for(&mut events, |e| matches!(e, SyncEvent::Connected)).await;
    wait_for(&mut events, |e| matches!(e, SyncEvent::Finished(_))).await;

    // A peer pushes → A is nudged, pulls, and reports the vault as changed.
    b.store.insert(vault, &host("pushed-by-b")).unwrap();
    eb.sync_once().await.unwrap();
    wait_for(
        &mut events,
        |e| matches!(e, SyncEvent::EntitiesChanged { vault_id } if *vault_id == vault),
    )
    .await;
    assert_eq!(labels(&a), ["pushed-by-b"]);

    // Local edits requested through the engine are pushed too.
    a.store.insert(vault, &host("pushed-by-a")).unwrap();
    ea.request_sync();
    wait_for(
        &mut events,
        |e| matches!(e, SyncEvent::Finished(r) if r.pushed == 1),
    )
    .await;
    eb.sync_once().await.unwrap();
    assert_eq!(labels(&b), ["pushed-by-a", "pushed-by-b"]);

    // Peer history lands as well.
    b.store
        .record_command(&CommandHistory {
            host_id: None,
            command: "whoami".into(),
        })
        .unwrap();
    eb.sync_once().await.unwrap();
    wait_for(&mut events, |e| matches!(e, SyncEvent::HistoryChanged)).await;
    assert_eq!(a.store.commands(5).unwrap()[0].data.command, "whoami");

    // B revokes A's device: the loop stops and tells the UI.
    b.api
        .revoke_device(a.signed_in.account.device_id)
        .await
        .unwrap();
    wait_for(&mut events, |e| matches!(e, SyncEvent::SessionRevoked)).await;
    tokio::time::timeout(Duration::from_secs(5), loop_task)
        .await
        .expect("loop exits after revocation")
        .unwrap();
    assert!(a.api.devices().await.unwrap_err().is_unauthorized());
    assert!(!cancel.is_cancelled());
}

#[tokio::test]
async fn realtime_loop_stops_on_cancel() {
    let s = server_or_skip!();
    let email = unique_email("cancel");
    let (a, _) = register(s, &email, PASSWORD).await;
    let ea = engine(&a, SyncOptions::default());
    let mut events = ea.subscribe();
    let cancel = CancellationToken::new();
    let task = tokio::spawn(ea.clone().run(cancel.clone()));
    wait_for(&mut events, |e| matches!(e, SyncEvent::Connected)).await;
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("loop exits on cancel")
        .unwrap();
    // Session is still valid: cancellation is not a sign-out.
    assert_eq!(a.api.devices().await.unwrap().len(), 1);
    let _ = s;
}
