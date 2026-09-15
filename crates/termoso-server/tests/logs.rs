//! Session logs: metadata in Postgres, encrypted bytes in S3 (MinIO).
//! Skipped when MinIO is unreachable (see `common`).

mod common;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::*;
use reqwest::{Method, StatusCode};
use termoso_proto::admin::ServerSettings;
use termoso_proto::logs::{
    CreateLogRequest, CreateLogResponse, DownloadLogResponse, LogListResponse, SessionLog,
    UpdateLogRequest,
};
use termoso_proto::vault::VaultList;
use uuid::Uuid;

macro_rules! server_with_storage {
    () => {
        match server().await {
            Some(s) if s.storage => s,
            _ => return,
        }
    };
}

async fn personal_vault(s: &TestServer, u: &User) -> Uuid {
    let vaults: VaultList = s
        .json(Method::GET, "/vaults", Some(u.token()), NOBODY)
        .await;
    vaults.vaults[0].id
}

fn meta(label: &str) -> String {
    STANDARD.encode(format!("{{\"encrypted\":\"{label}\"}}"))
}

/// PUT `body` to the presigned URL like a client would. `Content-Length` is
/// left to the HTTP stack (it is part of the signature, so a body of another
/// size fails the signature check instead of stalling the connection).
async fn upload(s: &TestServer, resp: &CreateLogResponse, body: Vec<u8>) -> StatusCode {
    let mut req = s.http().put(&resp.upload_url).body(body);
    for (k, v) in &resp.upload_headers {
        if !k.eq_ignore_ascii_case("content-length") {
            req = req.header(k, v);
        }
    }
    req.send().await.expect("s3 put").status()
}

#[tokio::test]
async fn log_lifecycle_upload_download_delete() {
    let s = server_with_storage!();
    let u = register(s, &unique_email("logs"), "pw-logs-1234567").await;
    let vault_id = personal_vault(s, &u).await;
    let payload: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();

    // Nothing yet.
    let empty: LogListResponse = s.json(Method::GET, "/logs", Some(u.token()), NOBODY).await;
    assert!(empty.logs.is_empty());
    assert_eq!(empty.since, 0);

    let id = Uuid::new_v4();
    let create = CreateLogRequest {
        id,
        vault_id,
        meta: meta("ssh host-1"),
        key_version: 1,
        size_bytes: payload.len() as i64,
    };
    let created: CreateLogResponse = s
        .json(Method::POST, "/logs", Some(u.token()), Some(&create))
        .await;
    assert!(created.expires_in > 0);
    assert!(
        created
            .upload_url
            .contains(&format!("logs/{}/{id}.bin", u.id()))
    );

    // Same id twice is a conflict.
    s.expect_status(
        Method::POST,
        "/logs",
        Some(u.token()),
        Some(&create),
        StatusCode::CONFLICT,
    )
    .await;

    // Pending upload: listed as incomplete, not downloadable.
    let pending: LogListResponse = s.json(Method::GET, "/logs", Some(u.token()), NOBODY).await;
    assert_eq!(pending.logs.len(), 1);
    assert!(!pending.logs[0].completed);
    s.expect_status(
        Method::GET,
        &format!("/logs/{id}/download"),
        Some(u.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;

    // Completing before the object exists is rejected.
    s.expect_status(
        Method::PATCH,
        &format!("/logs/{id}"),
        Some(u.token()),
        Some(&UpdateLogRequest {
            meta: None,
            size_bytes: Some(payload.len() as i64),
            pinned: None,
            note: None,
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;

    // The presigned PUT pins Content-Length: a different size is refused by S3.
    let wrong = upload(s, &created, payload[..payload.len() - 1].to_vec()).await;
    assert!(
        wrong.is_client_error(),
        "S3 accepted an upload of the wrong size: {wrong}"
    );
    assert!(upload(s, &created, payload.clone()).await.is_success());

    // Declared size must match what landed in the bucket.
    s.expect_status(
        Method::PATCH,
        &format!("/logs/{id}"),
        Some(u.token()),
        Some(&UpdateLogRequest {
            meta: None,
            size_bytes: Some(payload.len() as i64 - 1),
            pinned: None,
            note: None,
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let done: SessionLog = s
        .json(
            Method::PATCH,
            &format!("/logs/{id}"),
            Some(u.token()),
            Some(&UpdateLogRequest {
                meta: Some(meta("ssh host-1 (finished)")),
                size_bytes: Some(payload.len() as i64),
                pinned: None,
                note: None,
            }),
        )
        .await;
    assert!(done.completed);
    assert_eq!(done.size_bytes, payload.len() as i64);
    assert_eq!(done.meta, meta("ssh host-1 (finished)"));
    assert!(done.seq > pending.logs[0].seq);

    // Incremental listing: only the changed record since the previous seq.
    let delta: LogListResponse = s
        .json(
            Method::GET,
            &format!("/logs?since={}", pending.since),
            Some(u.token()),
            NOBODY,
        )
        .await;
    assert_eq!(delta.logs.len(), 1);
    assert_eq!(delta.logs[0].seq, done.seq);
    assert!(!delta.has_more);

    // Download round-trips the exact bytes.
    let dl: DownloadLogResponse = s
        .json(
            Method::GET,
            &format!("/logs/{id}/download"),
            Some(u.token()),
            NOBODY,
        )
        .await;
    let got = s.http().get(&dl.download_url).send().await.expect("s3 get");
    assert_eq!(got.status(), StatusCode::OK);
    assert_eq!(got.bytes().await.expect("body").to_vec(), payload);

    // Another user can neither see, complete, download nor delete it.
    let other = register(s, &unique_email("logs-other"), "pw-logs-1234567").await;
    for (m, p) in [
        (Method::PATCH, format!("/logs/{id}")),
        (Method::GET, format!("/logs/{id}/download")),
        (Method::DELETE, format!("/logs/{id}")),
    ] {
        s.expect_status(
            m,
            &p,
            Some(other.token()),
            Some(&UpdateLogRequest {
                meta: None,
                size_bytes: None,
                pinned: None,
                note: None,
            }),
            StatusCode::NOT_FOUND,
        )
        .await;
    }
    let theirs: LogListResponse = s
        .json(Method::GET, "/logs", Some(other.token()), NOBODY)
        .await;
    assert!(theirs.logs.is_empty());

    // Delete: tombstone with zeroed metadata, object gone, idempotent.
    s.expect_status(
        Method::DELETE,
        &format!("/logs/{id}"),
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &format!("/logs/{id}"),
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    let after: LogListResponse = s
        .json(
            Method::GET,
            &format!("/logs?since={}", done.seq),
            Some(u.token()),
            NOBODY,
        )
        .await;
    assert_eq!(after.logs.len(), 1);
    let tomb = &after.logs[0];
    assert!(tomb.deleted);
    assert_eq!(tomb.size_bytes, 0);
    assert_eq!(tomb.meta, "");
    s.expect_status(
        Method::GET,
        &format!("/logs/{id}/download"),
        Some(u.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;
    s.expect_status(
        Method::PATCH,
        &format!("/logs/{id}"),
        Some(u.token()),
        Some(&UpdateLogRequest {
            meta: Some(meta("zombie")),
            size_bytes: None,
            pinned: None,
            note: None,
        }),
        StatusCode::NOT_FOUND,
    )
    .await;
    let gone = s
        .http()
        .get(&dl.download_url)
        .send()
        .await
        .expect("s3 get")
        .status();
    assert_eq!(gone, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn log_creation_validates_input_and_permissions() {
    let s = server_with_storage!();
    let u = register(s, &unique_email("logs-val"), "pw-logs-1234567").await;
    let vault_id = personal_vault(s, &u).await;
    let base = CreateLogRequest {
        id: Uuid::new_v4(),
        vault_id,
        meta: meta("x"),
        key_version: 1,
        size_bytes: 1024,
    };

    // Size bounds.
    for size in [0, -1, (512 * 1024 * 1024) + 1] {
        s.expect_status(
            Method::POST,
            "/logs",
            Some(u.token()),
            Some(&CreateLogRequest {
                size_bytes: size,
                ..base.clone()
            }),
            StatusCode::PAYLOAD_TOO_LARGE,
        )
        .await;
    }
    // Metadata must be base64.
    s.expect_status(
        Method::POST,
        "/logs",
        Some(u.token()),
        Some(&CreateLogRequest {
            meta: "not base64 !!!".into(),
            ..base.clone()
        }),
        StatusCode::PAYLOAD_TOO_LARGE,
    )
    .await;
    // Stale vault key version.
    s.expect_status(
        Method::POST,
        "/logs",
        Some(u.token()),
        Some(&CreateLogRequest {
            key_version: 7,
            ..base.clone()
        }),
        StatusCode::CONFLICT,
    )
    .await;
    // Someone else's vault is indistinguishable from a missing one.
    let other = register(s, &unique_email("logs-val2"), "pw-logs-1234567").await;
    let other_vault = personal_vault(s, &other).await;
    s.expect_status(
        Method::POST,
        "/logs",
        Some(u.token()),
        Some(&CreateLogRequest {
            vault_id: other_vault,
            ..base.clone()
        }),
        StatusCode::NOT_FOUND,
    )
    .await;
    // Unauthenticated.
    s.expect_status(
        Method::POST,
        "/logs",
        None,
        Some(&base),
        StatusCode::UNAUTHORIZED,
    )
    .await;
}

#[tokio::test]
async fn log_quota_counts_live_logs_only() {
    let s = server_with_storage!();
    let admin = admin_token(s).await;
    let original: ServerSettings = s
        .json(Method::GET, "/admin/settings", Some(&admin), NOBODY)
        .await;
    // Generous enough that the other tests in this binary (which run
    // concurrently against the same settings) never hit it.
    let quota = 10_000u64;
    let _: ServerSettings = s
        .json(
            Method::PUT,
            "/admin/settings",
            Some(&admin),
            Some(&ServerSettings {
                log_quota_bytes: quota,
                ..original.clone()
            }),
        )
        .await;

    let u = register(s, &unique_email("logs-quota"), "pw-logs-1234567").await;
    let vault_id = personal_vault(s, &u).await;
    let mk = |size: i64| CreateLogRequest {
        id: Uuid::new_v4(),
        vault_id,
        meta: meta("q"),
        key_version: 1,
        size_bytes: size,
    };
    let first = mk(7000);
    let _: CreateLogResponse = s
        .json(Method::POST, "/logs", Some(u.token()), Some(&first))
        .await;
    // 7000 + 4000 > 10000 even though the first upload never finished: the
    // reservation counts until the record is deleted.
    s.expect_status(
        Method::POST,
        "/logs",
        Some(u.token()),
        Some(&mk(4000)),
        StatusCode::INSUFFICIENT_STORAGE,
    )
    .await;
    let _: CreateLogResponse = s
        .json(Method::POST, "/logs", Some(u.token()), Some(&mk(2500)))
        .await;
    s.expect_status(
        Method::DELETE,
        &format!("/logs/{}", first.id),
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    // Tombstones free their space.
    let _: CreateLogResponse = s
        .json(Method::POST, "/logs", Some(u.token()), Some(&mk(4000)))
        .await;

    // Restore so other tests are unaffected.
    let _: ServerSettings = s
        .json(
            Method::PUT,
            "/admin/settings",
            Some(&admin),
            Some(&original),
        )
        .await;
}

#[tokio::test]
async fn log_incremental_listing_paginates() {
    let s = server_with_storage!();
    let u = register(s, &unique_email("logs-page"), "pw-logs-1234567").await;
    let vault_id = personal_vault(s, &u).await;
    let mut ids = Vec::new();
    for i in 0..5 {
        let req = CreateLogRequest {
            id: Uuid::new_v4(),
            vault_id,
            meta: meta(&format!("p{i}")),
            key_version: 1,
            size_bytes: 100,
        };
        let _: CreateLogResponse = s
            .json(Method::POST, "/logs", Some(u.token()), Some(&req))
            .await;
        ids.push(req.id);
    }
    let page1: LogListResponse = s
        .json(Method::GET, "/logs?limit=2", Some(u.token()), NOBODY)
        .await;
    assert_eq!(page1.logs.len(), 2);
    assert!(page1.has_more);
    let page2: LogListResponse = s
        .json(
            Method::GET,
            &format!("/logs?limit=2&since={}", page1.since),
            Some(u.token()),
            NOBODY,
        )
        .await;
    assert_eq!(page2.logs.len(), 2);
    assert!(page2.has_more);
    let page3: LogListResponse = s
        .json(
            Method::GET,
            &format!("/logs?limit=2&since={}", page2.since),
            Some(u.token()),
            NOBODY,
        )
        .await;
    assert_eq!(page3.logs.len(), 1);
    assert!(!page3.has_more);
    let seen: Vec<Uuid> = page1
        .logs
        .iter()
        .chain(&page2.logs)
        .chain(&page3.logs)
        .map(|l| l.id)
        .collect();
    assert_eq!(seen, ids, "listing is in creation (seq) order");
}
