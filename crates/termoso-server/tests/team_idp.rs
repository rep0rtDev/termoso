//! Per-team identity provider: permissions, validation, storage without
//! leaking the secret, home-realm discovery and removal.

mod common;

use common::*;
use reqwest::{Method, StatusCode};

#[tokio::test]
async fn team_idp_is_validated_stored_and_discoverable() {
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
    let team_id = team["id"].as_str().unwrap().to_string();
    let invite: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/teams/{team_id}/invites"),
            Some(owner.token()),
            Some(&serde_json::json!({ "email": bob_email, "role": "member", "vault_ids": [] })),
        )
        .await;
    let token = invite["url"].as_str().unwrap().rsplit('/').next().unwrap().to_string();
    let _: serde_json::Value = s
        .json(Method::POST, &format!("/invites/{token}/accept"), Some(bob.token()), NOBODY)
        .await;

    // Members without sso.manage cannot read the IdP settings.
    s.expect_status(
        Method::GET,
        &format!("/teams/{team_id}/idp"),
        Some(bob.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;

    let r: serde_json::Value = s
        .json(Method::GET, &format!("/teams/{team_id}/idp"), Some(owner.token()), NOBODY)
        .await;
    assert!(r["idp"].is_null());
    assert_eq!(r["available"], true);

    // Validation: name, domains and OIDC fields are required.
    s.expect_status(
        Method::PUT,
        &format!("/teams/{team_id}/idp"),
        Some(owner.token()),
        Some(&serde_json::json!({ "domains": ["corp.example"] })),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::PUT,
        &format!("/teams/{team_id}/idp"),
        Some(owner.token()),
        Some(&serde_json::json!({ "name": "Okta", "domains": ["corp.example"], "issuer": "http://idp.example" })),
        StatusCode::BAD_REQUEST,
    )
    .await;
    // An unreachable issuer fails discovery when enabled.
    s.expect_status(
        Method::PUT,
        &format!("/teams/{team_id}/idp"),
        Some(owner.token()),
        Some(&serde_json::json!({
            "name": "Okta", "domains": ["corp.example"],
            "issuer": "https://127.0.0.1:9/", "client_id": "cid", "client_secret": "sec"
        })),
        StatusCode::BAD_REQUEST,
    )
    .await;

    // Disabled settings are stored without contacting the IdP; the secret is not echoed.
    let r: serde_json::Value = s
        .json(
            Method::PUT,
            &format!("/teams/{team_id}/idp"),
            Some(owner.token()),
            Some(&serde_json::json!({
                "name": "Okta", "domains": ["Corp.Example"], "enabled": false,
                "issuer": "https://127.0.0.1:9/", "client_id": "cid", "client_secret": "sec"
            })),
        )
        .await;
    assert_eq!(r["idp"]["name"], "Okta");
    assert_eq!(r["idp"]["domains"][0], "corp.example");
    assert_eq!(r["idp"]["has_client_secret"], true);
    assert_eq!(r["idp"]["enabled"], false);
    assert_eq!(r["idp"]["provider_id"], format!("team-{team_id}"));
    assert!(r["idp"].get("client_secret").is_none());
    assert!(r["idp"]["callback_url"].as_str().unwrap().ends_with("/api/v1/auth/sso/callback"));

    // A disabled IdP is not discoverable.
    let d: serde_json::Value = s
        .json(Method::GET, "/auth/sso/discover?email=ann%40corp.example", None, NOBODY)
        .await;
    assert!(d["provider"].is_null());

    // Another team cannot claim the same domain.
    let other: serde_json::Value = s
        .json(
            Method::POST,
            "/teams",
            Some(bob.token()),
            Some(&serde_json::json!({ "name": "Other" })),
        )
        .await;
    let other_id = other["id"].as_str().unwrap();
    s.expect_status(
        Method::PUT,
        &format!("/teams/{other_id}/idp"),
        Some(bob.token()),
        Some(&serde_json::json!({
            "name": "Dup", "domains": ["corp.example"], "enabled": false,
            "issuer": "https://127.0.0.1:9/", "client_id": "cid", "client_secret": "sec"
        })),
        StatusCode::CONFLICT,
    )
    .await;

    // Policy may point at the team IdP; removing the IdP clears sso_only.
    let _: serde_json::Value = s
        .json(
            Method::PUT,
            &format!("/teams/{team_id}/policies"),
            Some(owner.token()),
            Some(&serde_json::json!({ "sso_only": true, "sso_provider": "team", "sso_domains": ["corp.example"] })),
        )
        .await;
    let r: serde_json::Value = s
        .json(Method::DELETE, &format!("/teams/{team_id}/idp"), Some(owner.token()), NOBODY)
        .await;
    assert!(r["idp"].is_null());
    let p: serde_json::Value = s
        .json(Method::GET, &format!("/teams/{team_id}/policies"), Some(owner.token()), NOBODY)
        .await;
    assert_eq!(p["policies"]["sso_only"], false);
}
