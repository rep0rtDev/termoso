//! Audit webhooks: CRUD + validation, signed generic delivery through the
//! outbox, delivery history.

mod common;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use common::*;
use reqwest::{Method, StatusCode};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct Received(Arc<Mutex<Vec<(HeaderMap, Bytes)>>>);

async fn sink(State(r): State<Received>, headers: HeaderMap, body: Bytes) -> StatusCode {
    r.0.lock().unwrap().push((headers, body));
    StatusCode::ACCEPTED
}

async fn receiver() -> (String, Received) {
    let received = Received::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/hook", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/hook", post(sink))
        .with_state(received.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, received)
}

#[tokio::test]
async fn webhooks_deliver_signed_batches() {
    let Some(s) = server().await else { return };
    let owner = register(s, &unique_email("owner"), "pw-owner-1234567").await;
    let bob_email = unique_email("bob");
    let bob = register(s, &bob_email, "pw-bob-123456789").await;
    let team: serde_json::Value = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "Ops" })),
        )
        .await;
    let team_id = team["id"].as_str().unwrap().to_string();
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

    let (url, received) = receiver().await;
    // Validation and permissions.
    s.expect_status(
        Method::POST,
        &format!("/teams/{team_id}/webhooks"),
        Some(bob.token()),
        Some(&serde_json::json!({ "url": url, "secret": "s3cret-s3cret" })),
        StatusCode::FORBIDDEN,
    )
    .await;
    for bad in [
        serde_json::json!({ "url": "ftp://example.com/x", "secret": "s3cret-s3cret" }),
        serde_json::json!({ "url": "https://user:pw@example.com/x", "secret": "s3cret-s3cret" }),
        serde_json::json!({ "url": url, "secret": "short" }),
        serde_json::json!({ "url": url, "secret": "s3cret-s3cret", "kind": "carrier_pigeon" }),
        serde_json::json!({ "url": url, "secret": "s3cret-s3cret", "actions": ["bad action"] }),
    ] {
        s.expect_status(
            Method::POST,
            &format!("/teams/{team_id}/webhooks"),
            Some(owner.token()),
            Some(&bad),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    let hook: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/teams/{team_id}/webhooks"),
            Some(owner.token()),
            Some(&serde_json::json!({ "url": url, "secret": "s3cret-s3cret", "actions": ["team.", "webhook.test"] })),
        )
        .await;
    let wid = hook["id"].as_str().unwrap().to_string();
    assert_eq!(hook["kind"], "generic");
    assert_eq!(hook["enabled"], true);
    assert!(hook.get("secret").is_none());

    // Events matching the filter queue up; a rename matches `team.`.
    let _: serde_json::Value = s
        .json(
            Method::PATCH,
            &format!("/teams/{team_id}"),
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "Ops 2" })),
        )
        .await;
    // The test endpoint drains this hook's queue synchronously.
    let d: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/teams/{team_id}/webhooks/{wid}/test"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    let deliveries = d["deliveries"].as_array().unwrap();
    assert!(deliveries.len() >= 2, "{d}");
    assert!(
        deliveries
            .iter()
            .all(|x| x["status"] == "ok" && x["response_status"] == 202),
        "{d}"
    );
    assert!(deliveries.iter().any(|x| x["action"] == "webhook.test"));
    assert!(
        deliveries
            .iter()
            .any(|x| x["action"].as_str().unwrap().starts_with("team."))
    );

    // The receiver saw a signed batch.
    let got = received.0.lock().unwrap().clone();
    assert!(!got.is_empty());
    let (headers, body) = &got[0];
    let ts = headers["x-termoso-timestamp"].to_str().unwrap();
    let sig = headers["x-termoso-signature"].to_str().unwrap();
    assert_eq!(
        sig,
        termoso_server::webhooks::signature("s3cret-s3cret", ts, body)
    );
    let payload: serde_json::Value = serde_json::from_slice(body).unwrap();
    let events = payload["events"].as_array().unwrap();
    assert_eq!(
        headers["x-termoso-event-count"].to_str().unwrap(),
        events.len().to_string()
    );
    assert!(
        events
            .iter()
            .all(|e| e["team_id"] == team_id && e["actor"]["email"] == owner.email)
    );

    // Hook status reflects success; history is readable; delete removes it.
    let list: serde_json::Value = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}/webhooks"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    assert_eq!(list["webhooks"][0]["last_status"], 202);
    assert_eq!(list["webhooks"][0]["failures"], 0);
    let hist: serde_json::Value = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}/webhooks/{wid}/deliveries"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    assert!(!hist["deliveries"].as_array().unwrap().is_empty());
    let upd: serde_json::Value = s
        .json(
            Method::PATCH,
            &format!("/teams/{team_id}/webhooks/{wid}"),
            Some(owner.token()),
            Some(&serde_json::json!({ "enabled": false, "kind": "splunk_hec" })),
        )
        .await;
    assert_eq!(upd["enabled"], false);
    assert_eq!(upd["kind"], "splunk_hec");
    s.expect_status(
        Method::DELETE,
        &format!("/teams/{team_id}/webhooks/{wid}"),
        Some(owner.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::GET,
        &format!("/teams/{team_id}/webhooks/{wid}/deliveries"),
        Some(owner.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;
}
