//! Sign-up policy: closed registration, the SSO-only exception and the domain
//! allow-list. Runs in its own binary because it flips server-wide settings
//! that every other registering test would trip over.

mod common;

use common::oidc::{self, Identity};
use common::*;
use reqwest::{Method, StatusCode, redirect::Policy};
use termoso_proto::account::ServerInfo;
use termoso_proto::admin::ServerSettings;
use termoso_proto::auth::{LoginStartRequest, SsoResult, SsoStartResponse};
use url::Url;

const PASSWORD: &str = "pw-policy-1234567890";

/// Browser leg of an SSO round trip for `email`, as the mock IdP would drive it.
async fn sso_sign_in(s: &TestServer, email: &str) -> SsoResult {
    let flow: SsoStartResponse = s
        .json(
            Method::GET,
            &format!("/auth/sso/{SSO_PROVIDER}/start"),
            None,
            NOBODY,
        )
        .await;
    let nonce = Url::parse(&flow.authorization_url)
        .expect("authorization url")
        .query_pairs()
        .find(|(k, _)| k == "nonce")
        .map(|(_, v)| v.into_owned())
        .expect("nonce");
    let who = Identity {
        sub: format!("sub-{}", email.split_once('@').unwrap().0),
        email: email.into(),
        email_verified: true,
        name: None,
        nonce,
    };
    let resp = reqwest::Client::builder()
        .redirect(Policy::none())
        .pool_max_idle_per_host(0)
        .build()
        .unwrap()
        .get(s.url("/auth/sso/callback"))
        .query(&[("state", &flow.flow_id), ("code", &oidc::code_for(&who))])
        .send()
        .await
        .expect("callback");
    assert_eq!(resp.status(), StatusCode::OK);
    s.json(
        Method::GET,
        &format!("/auth/sso/flow/{}", flow.flow_id),
        None,
        NOBODY,
    )
    .await
}

async fn sso_registration_session(s: &TestServer, email: &str) -> String {
    match sso_sign_in(s, email).await {
        SsoResult::RegistrationRequired { sso_session, .. } => sso_session,
        other => panic!("expected registration_required for {email}, got {other:?}"),
    }
}

async fn put_settings(s: &TestServer, admin: &str, settings: &ServerSettings) -> ServerSettings {
    s.json(Method::PUT, "/admin/settings", Some(admin), Some(settings))
        .await
}

async fn server_info(s: &TestServer) -> ServerInfo {
    s.json(Method::GET, "/server/info", None, NOBODY).await
}

/// `sso_session` must be spent: presenting it again is refused outright.
async fn assert_spent(s: &TestServer, email: &str, sso_session: String) {
    let (request, _) = termoso_crypto::opaque::client_login_start(PASSWORD.as_bytes()).unwrap();
    s.expect_status(
        Method::POST,
        "/auth/login/start",
        None,
        Some(&LoginStartRequest {
            email: email.into(),
            opaque_request: request,
            device: device("policy device"),
            sso_session: Some(sso_session),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
}

#[tokio::test]
async fn closed_registration_sso_exception_and_domain_allow_list() {
    let Some(s) = server().await else { return };
    let admin = admin_token(s).await;
    let original: ServerSettings = s
        .json(Method::GET, "/admin/settings", Some(&admin), NOBODY)
        .await;
    assert!(original.registration_open && !original.sso_registration);
    assert!(!server_info(s).await.sso_registration);

    // Registration closed, no SSO exception: nobody gets in, IdP-verified or
    // not, and the SSO session is spent by the attempt.
    put_settings(
        s,
        &admin,
        &ServerSettings {
            registration_open: false,
            ..original.clone()
        },
    )
    .await;
    assert!(!server_info(s).await.registration_open);
    assert!(!server_info(s).await.sso_registration);
    let plain = unique_email("closed-plain");
    let (status, body) = try_register(s, &plain, PASSWORD, None, None)
        .await
        .err()
        .expect("closed registration must refuse");
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("registration_closed"))
    );
    let via_sso = unique_email("closed-sso");
    let sess = sso_registration_session(s, &via_sso).await;
    let (status, body) = try_register(s, &via_sso, PASSWORD, None, Some(sess.clone()))
        .await
        .err()
        .expect("sso without the exception must refuse");
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("registration_closed"))
    );
    assert_spent(s, &via_sso, sess).await;

    // SSO exception on: only IdP-verified identities may sign up.
    put_settings(
        s,
        &admin,
        &ServerSettings {
            registration_open: false,
            sso_registration: true,
            ..original.clone()
        },
    )
    .await;
    let info = server_info(s).await;
    assert!(!info.registration_open && info.sso_registration);
    let (status, _) = try_register(s, &unique_email("still-closed"), PASSWORD, None, None)
        .await
        .err()
        .expect("plain sign-up stays closed");
    assert_eq!(status, StatusCode::FORBIDDEN);
    let sess = sso_registration_session(s, &via_sso).await;
    let user = register_raw(s, &via_sso, PASSWORD, None, Some(sess)).await;
    assert!(user.session.user.email_verified);
    // A made-up session proves nothing.
    let (status, body) = try_register(
        s,
        &unique_email("forged"),
        PASSWORD,
        None,
        Some("not-a-real-sso-session".into()),
    )
    .await
    .err()
    .expect("forged sso session must refuse");
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("token_expired"))
    );

    // Domain allow-list applies to both open and SSO-only sign-up; an SSO
    // session for an outside address is spent by the refused attempt.
    let domain = "policy.example";
    put_settings(
        s,
        &admin,
        &ServerSettings {
            registration_open: true,
            allowed_domains: vec![domain.to_uppercase()],
            ..original.clone()
        },
    )
    .await;
    let (status, body) = try_register(s, &unique_email("outsider"), PASSWORD, None, None)
        .await
        .err()
        .expect("outside domain must refuse");
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("forbidden"))
    );
    let insider = format!("Insider-{}@{domain}", uuid::Uuid::new_v4().simple());
    register_raw(s, &insider, PASSWORD, None, None).await;
    put_settings(
        s,
        &admin,
        &ServerSettings {
            registration_open: false,
            sso_registration: true,
            allowed_domains: vec![domain.into()],
            ..original.clone()
        },
    )
    .await;
    let outsider = unique_email("sso-outsider");
    let sess = sso_registration_session(s, &outsider).await;
    let (status, _) = try_register(s, &outsider, PASSWORD, None, Some(sess.clone()))
        .await
        .err()
        .expect("sso outside domain must refuse");
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_spent(s, &outsider, sess).await;
    let sso_insider = format!("sso-{}@{domain}", uuid::Uuid::new_v4().simple());
    let sess = sso_registration_session(s, &sso_insider).await;
    register_raw(s, &sso_insider, PASSWORD, None, Some(sess)).await;

    put_settings(s, &admin, &original).await;
}
