//! Corporate mode: home team auto-join, settings validation, SSO-only sign-in.

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_proto::account::ServerInfo;
use termoso_proto::admin::ServerSettings;
use termoso_proto::auth::{AuthResponse, LoginStartRequest};
use uuid::Uuid;

#[tokio::test]
async fn home_team_and_sso_only() {
    let Some(s) = server().await else { return };
    let admin = admin_token(s).await;
    let owner = register(s, &unique_email("owner"), "pw-owner-1234567").await;
    let carol_email = unique_email("carol");
    let carol = register(s, &carol_email, "pw-carol-12345678").await;
    let team: serde_json::Value = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "Corp" })),
        )
        .await;
    let team_id: Uuid = team["id"].as_str().unwrap().parse().unwrap();

    let base: ServerSettings = s
        .json(Method::GET, "/admin/settings", Some(&admin), NOBODY)
        .await;
    assert!(base.home_team_id.is_none() && !base.sso_only && base.personal_vaults);

    // Unknown team is refused.
    let mut bad = base.clone();
    bad.home_team_id = Some(Uuid::new_v4());
    s.expect_status(
        Method::PUT,
        "/admin/settings",
        Some(&admin),
        Some(&bad),
        StatusCode::BAD_REQUEST,
    )
    .await;

    let mut corp = base.clone();
    corp.home_team_id = Some(team_id);
    corp.personal_vaults = false;
    let _: ServerSettings = s
        .json(Method::PUT, "/admin/settings", Some(&admin), Some(&corp))
        .await;
    let info: ServerInfo = s.json(Method::GET, "/server/info", None, NOBODY).await;
    let home = info.home_team.expect("home team advertised");
    assert_eq!(home.id, team_id);
    assert_eq!(home.name, "Corp");
    assert!(!info.features.personal_vaults);
    assert!(!info.sso_only);

    // New accounts join on sign-up, existing ones on their next sign-in.
    let bob = register(s, &unique_email("bob"), "pw-bob-123456789").await;
    let teams: serde_json::Value = s
        .json(Method::GET, "/teams", Some(bob.token()), NOBODY)
        .await;
    let names: Vec<&str> = teams
        .as_array()
        .or_else(|| teams["teams"].as_array())
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(names, vec!["Corp"]);
    let teams: serde_json::Value = s
        .json(Method::GET, "/teams", Some(carol.token()), NOBODY)
        .await;
    assert!(
        teams
            .as_array()
            .or_else(|| teams["teams"].as_array())
            .unwrap()
            .is_empty()
    );
    let AuthResponse::Authenticated(carol2) = login(s, &carol_email, "pw-carol-12345678").await
    else {
        panic!("login")
    };
    let teams: serde_json::Value = s
        .json(Method::GET, "/teams", Some(&carol2.token), NOBODY)
        .await;
    assert_eq!(
        teams
            .as_array()
            .or_else(|| teams["teams"].as_array())
            .unwrap()
            .len(),
        1
    );
    let members: serde_json::Value = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}/members"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    let n = members
        .as_array()
        .or_else(|| members["members"].as_array())
        .unwrap()
        .len();
    assert_eq!(n, 3, "owner + bob + carol");

    // SSO-only: password sign-in is refused and points at a provider.
    let mut sso = corp.clone();
    sso.sso_only = true;
    let _: ServerSettings = s
        .json(Method::PUT, "/admin/settings", Some(&admin), Some(&sso))
        .await;
    let info: ServerInfo = s.json(Method::GET, "/server/info", None, NOBODY).await;
    assert!(info.sso_only);
    s.expect_status(
        Method::POST,
        "/auth/login/start",
        None,
        Some(&LoginStartRequest {
            email: carol_email.clone(),
            opaque_request: "AA".into(),
            device: device("corp laptop"),
            sso_session: None,
            turnstile_token: None,
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    // Bootstrap admins keep password access.
    let _ = login(s, "admin@test.local", "pw-admin-1234567").await;

    // Restore the shared server settings.
    let _: ServerSettings = s
        .json(Method::PUT, "/admin/settings", Some(&admin), Some(&base))
        .await;
}
