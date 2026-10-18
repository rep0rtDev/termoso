//! SCIM 2.0: token auth, Users create/filter/deactivate/delete, Groups → roles.

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_proto::auth::AuthResponse;

struct Scim {
    base: String,
    secret: String,
    http: reqwest::Client,
}

impl Scim {
    async fn send(
        &self,
        m: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut req = self
            .http
            .request(m, format!("{}{path}", self.base))
            .bearer_auth(&self.secret);
        if let Some(b) = body {
            req = req
                .header("content-type", "application/scim+json")
                .body(b.to_string());
        }
        let resp = req.send().await.unwrap();
        let status = resp.status();
        let text = resp.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or(serde_json::Value::Null),
        )
    }
}

#[tokio::test]
async fn scim_provisions_users_and_groups() {
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
    let roles: serde_json::Value = s
        .json(
            Method::GET,
            &format!("/teams/{team_id}/roles"),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    let admin = roles["roles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["slug"] == "admin")
        .unwrap()["id"]
        .clone();
    let _: serde_json::Value = s
        .json(
            Method::PUT,
            &format!("/teams/{team_id}/role-mappings"),
            Some(owner.token()),
            Some(&serde_json::json!({ "mappings": [{ "group_name": "Ops-Admins", "role_id": admin, "priority": 10 }] })),
        )
        .await;
    // Tokens need scim.tokens; bob (not a member) is refused.
    s.expect_status(
        Method::POST,
        &format!("/teams/{team_id}/scim/tokens"),
        Some(bob.token()),
        Some(&serde_json::json!({ "name": "Okta" })),
        StatusCode::NOT_FOUND,
    )
    .await;
    let created: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/teams/{team_id}/scim/tokens"),
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "Okta", "expires_in_days": 30 })),
        )
        .await;
    let secret = created["secret"].as_str().unwrap().to_string();
    assert!(secret.starts_with("tsk_"));
    let scim = Scim {
        base: format!("http://{}/scim/v2", s.addr),
        secret,
        http: reqwest::Client::new(),
    };

    // Discovery; no / wrong token → 401 in SCIM error format.
    let (st, _) = Scim {
        secret: "nope".into(),
        ..Scim {
            base: scim.base.clone(),
            secret: String::new(),
            http: reqwest::Client::new(),
        }
    }
    .send(Method::GET, "/ServiceProviderConfig", None)
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, spc) = scim.send(Method::GET, "/ServiceProviderConfig", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(spc["patch"]["supported"], true);

    // Provision a brand-new account and attach an existing one.
    let new_email = unique_email("scim-new");
    let (st, u) = scim
        .send(
            Method::POST,
            "/Users",
            Some(serde_json::json!({
                "schemas": ["urn:ietf:params:scim:schemas:core:2.0:User"],
                "userName": new_email, "externalId": "okta-1",
                "name": { "givenName": "New", "familyName": "Person" },
                "emails": [{ "value": new_email, "type": "work", "primary": true }], "active": true
            })),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{u}");
    let new_id = u["id"].as_str().unwrap().to_string();
    assert_eq!(u["name"]["givenName"], "New");
    let (st, dup) = scim
        .send(
            Method::POST,
            "/Users",
            Some(serde_json::json!({ "userName": new_email })),
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(dup["scimType"], "uniqueness");
    let (st, b) = scim
        .send(
            Method::POST,
            "/Users",
            Some(serde_json::json!({ "userName": bob_email, "externalId": "okta-2" })),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{b}");
    assert_eq!(b["id"].as_str().unwrap(), bob.id().to_string());
    let (_, list) = scim
        .send(
            Method::GET,
            &format!("/Users?filter=userName eq \"{bob_email}\""),
            None,
        )
        .await;
    assert_eq!(list["totalResults"], 1);
    assert_eq!(list["Resources"][0]["externalId"], "okta-2");
    let (_, list) = scim
        .send(Method::GET, "/Users?startIndex=1&count=10", None)
        .await;
    assert_eq!(list["totalResults"], 3); // owner + two provisioned
    let teams: serde_json::Value = s
        .json(Method::GET, "/teams", Some(bob.token()), NOBODY)
        .await;
    assert!(
        teams["teams"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == team_id)
    );

    // Group membership drives the role mapping.
    let (st, g) = scim
        .send(
            Method::POST,
            "/Groups",
            Some(serde_json::json!({ "displayName": "Ops-Admins", "members": [{ "value": bob.id() }] })),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{g}");
    let gid = g["id"].as_str().unwrap().to_string();
    let teams: serde_json::Value = s
        .json(Method::GET, "/teams", Some(bob.token()), NOBODY)
        .await;
    let t = teams["teams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == team_id)
        .unwrap();
    assert_eq!(t["my_role_ref"]["slug"], "admin");
    // Leaving the group (Okta-style PATCH) drops bob to the default role.
    let (st, _) = scim
        .send(
            Method::PATCH,
            &format!("/Groups/{gid}"),
            Some(serde_json::json!({
                "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
                "Operations": [{ "op": "remove", "path": format!("members[value eq \"{}\"]", bob.id()) }]
            })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let teams: serde_json::Value = s
        .json(Method::GET, "/teams", Some(bob.token()), NOBODY)
        .await;
    let t = teams["teams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == team_id)
        .unwrap();
    assert_eq!(t["my_role_ref"]["slug"], "member");

    // Owner cannot be deactivated; bob (unmanaged) leaves the team but keeps his account.
    let (st, e) = scim
        .send(
            Method::PATCH,
            &format!("/Users/{}", owner.id()),
            Some(serde_json::json!({ "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"], "Operations": [{ "op": "replace", "value": { "active": false } }] })),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(e["scimType"], "mutability");
    let (st, _) = scim
        .send(
            Method::PATCH,
            &format!("/Users/{}", bob.id()),
            Some(serde_json::json!({ "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"], "Operations": [{ "op": "Replace", "path": "active", "value": false }] })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    // Deprovisioning revoked Bob's sessions; the account itself is untouched.
    let st = s
        .call(Method::GET, "/teams", Some(bob.token()), NOBODY)
        .await
        .status();
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    // Revoking the sessions also dropped device trust, so a fresh login may
    // ask for device approval; the account itself must still work.
    let resp = login_raw(s, &bob_email, "pw-bob-123456789", device("bob"), None)
        .await
        .expect("unmanaged account still works");
    let bob2 = match approve_device(s, &bob_email, resp).await {
        AuthResponse::Authenticated(sess) => sess,
        other => panic!("expected a session, got {other:?}"),
    };
    let teams: serde_json::Value = s
        .json(Method::GET, "/teams", Some(&bob2.token), NOBODY)
        .await;
    assert!(
        !teams["teams"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == team_id)
    );
    // The provisioned account is disabled on DELETE and comes back with active=true.
    let (st, _) = scim
        .send(Method::DELETE, &format!("/Users/{new_id}"), None)
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (_, u) = scim
        .send(Method::GET, &format!("/Users/{new_id}"), None)
        .await;
    assert_eq!(u["active"], false);
    let (st, u) = scim
        .send(
            Method::PUT,
            &format!("/Users/{new_id}"),
            Some(serde_json::json!({ "userName": new_email, "active": true })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{u}");
    assert_eq!(u["active"], true);
    // Token revocation shuts the door.
    s.expect_status(
        Method::DELETE,
        &format!(
            "/teams/{team_id}/scim/tokens/{}",
            created["token"]["id"].as_str().unwrap()
        ),
        Some(owner.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    let (st, _) = scim.send(Method::GET, "/Users", None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}
