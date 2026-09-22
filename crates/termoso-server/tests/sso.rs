//! Single sign-on against the in-process mock OpenID Connect provider: the
//! browser leg is simulated by calling the callback with a code the mock IdP
//! will turn into a signed ID token.

mod common;

use common::oidc::{self, Identity};
use common::*;
use reqwest::{Method, StatusCode, redirect::Policy};
use termoso_proto::admin::{AdminUpdateUserRequest, AdminUser};
use termoso_proto::auth::{AuthResponse, SsoKind, SsoProvider, SsoResult, SsoStartResponse};
use termoso_server::config::{Config, SsoKindConfig, SsoProviderConfig};
use termoso_server::saml::test_support as saml_support;
use termoso_server::sso::SsoRegistry;
use url::Url;

fn identity(email: &str, nonce: &str) -> Identity {
    Identity {
        sub: format!("sub-{}", email.split_once('@').unwrap().0),
        email: email.into(),
        email_verified: true,
        name: Some("SSO Person".into()),
        nonce: nonce.into(),
    }
}

async fn start(s: &TestServer, provider: &str, redirect: Option<&str>) -> SsoStartResponse {
    let mut path = format!("/auth/sso/{provider}/start");
    if let Some(r) = redirect {
        let q: String = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("redirect", r)
            .finish();
        path = format!("{path}?{q}");
    }
    s.json(Method::GET, &path, None, NOBODY).await
}

/// Query parameters the IdP would receive from the user's browser.
fn auth_params(url: &str) -> std::collections::HashMap<String, String> {
    Url::parse(url)
        .expect("authorization url")
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// Simulate the IdP redirecting the browser back to us.
async fn callback(s: &TestServer, query: &[(&str, &str)]) -> reqwest::Response {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .pool_max_idle_per_host(0)
        .build()
        .unwrap()
        .get(s.url("/auth/sso/callback"))
        .query(query)
        .send()
        .await
        .expect("callback")
}

async fn poll(s: &TestServer, flow_id: &str) -> SsoResult {
    s.json(
        Method::GET,
        &format!("/auth/sso/flow/{flow_id}"),
        None,
        NOBODY,
    )
    .await
}

/// Drive the whole browser leg for `who`, returning the poll result.
async fn sign_in(s: &TestServer, provider: &str, who: &Identity) -> SsoResult {
    let flow = start(s, provider, None).await;
    let p = auth_params(&flow.authorization_url);
    let who = Identity {
        nonce: p["nonce"].clone(),
        ..who.clone()
    };
    let resp = callback(
        s,
        &[("state", &flow.flow_id), ("code", &oidc::code_for(&who))],
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    poll(s, &flow.flow_id).await
}

#[tokio::test]
async fn providers_are_listed_and_flows_start_with_pkce() {
    let Some(s) = server().await else { return };
    let providers: Vec<SsoProvider> = s
        .json(Method::GET, "/auth/sso/providers", None, NOBODY)
        .await;
    let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            SSO_PROVIDER,
            SSO_CORP_PROVIDER,
            SAML_PROVIDER,
            SAML_CORP_PROVIDER
        ]
    );
    let mock = providers.iter().find(|p| p.id == SSO_PROVIDER).unwrap();
    assert_eq!((mock.name.as_str(), mock.kind), ("Mock IdP", SsoKind::Oidc));
    let saml = providers.iter().find(|p| p.id == SAML_PROVIDER).unwrap();
    assert_eq!(
        (saml.name.as_str(), saml.kind),
        ("Mock SAML", SsoKind::Saml)
    );

    s.expect_status(
        Method::GET,
        "/auth/sso/nope/start",
        None,
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;

    let flow = start(s, SSO_PROVIDER, None).await;
    let url = Url::parse(&flow.authorization_url).unwrap();
    assert_eq!(url.path(), "/authorize");
    assert!(
        flow.authorization_url.starts_with(&s.idp.issuer),
        "{}",
        flow.authorization_url
    );
    let p = auth_params(&flow.authorization_url);
    assert_eq!(p["response_type"], "code");
    assert_eq!(p["client_id"], oidc::CLIENT_ID);
    assert_eq!(p["state"], flow.flow_id);
    assert_eq!(p["code_challenge_method"], "S256");
    assert_eq!(p["redirect_uri"], s.url("/auth/sso/callback"));
    assert!(!p["nonce"].is_empty());
    let scopes: Vec<&str> = p["scope"].split(' ').collect();
    for want in ["openid", "email", "profile"] {
        assert!(scopes.contains(&want), "{scopes:?}");
    }
    assert!(matches!(poll(s, &flow.flow_id).await, SsoResult::Pending));

    // Extra configured scopes are requested too.
    let corp = start(s, SSO_CORP_PROVIDER, None).await;
    assert!(auth_params(&corp.authorization_url)["scope"].contains("groups"));

    // Unknown flow.
    s.expect_status(
        Method::GET,
        "/auth/sso/flow/does-not-exist",
        None,
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
}

#[tokio::test]
async fn sso_registers_then_logs_in_and_skips_device_approval() {
    let Some(s) = server().await else { return };
    let email = unique_email("sso");
    let who = identity(&email, "");

    // Unknown email → registration is required; the token is bound to it.
    let SsoResult::RegistrationRequired {
        sso_session,
        email: asserted,
        display_name,
    } = sign_in(s, SSO_PROVIDER, &who).await
    else {
        panic!("expected registration_required")
    };
    assert_eq!(asserted, email);
    assert_eq!(display_name.as_deref(), Some("SSO Person"));

    // The SSO session can't be used for a different address...
    let other = unique_email("intruder");
    let (request, _) =
        termoso_crypto::opaque::client_registration_start(b"pw-x-123456789").unwrap();
    let v = s
        .expect_status(
            Method::POST,
            "/auth/login/start",
            None,
            Some(&termoso_proto::auth::LoginStartRequest {
                email: other,
                opaque_request: request,
                device: device("sso device"),
                sso_session: Some(sso_session.clone()),
            }),
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_eq!(v["code"], "forbidden");
    // ...and the failed attempt consumed it.
    let SsoResult::RegistrationRequired { sso_session, .. } = sign_in(s, SSO_PROVIDER, &who).await
    else {
        panic!("expected registration_required")
    };

    // Register with it: email verified without any code, even with a mailer.
    let password = "pw-sso-1234567890";
    let u = register_raw(s, &email, password, None, Some(sso_session.clone())).await;
    assert!(u.session.user.email_verified);
    if s.mailpit.is_some() {
        assert_eq!(s.email_count(&email).await, 0, "no verification email");
    }
    // Spent.
    let (request, _) =
        termoso_crypto::opaque::client_registration_start(b"pw-x-123456789").unwrap();
    s.expect_status(
        Method::POST,
        "/auth/login/start",
        None,
        Some(&termoso_proto::auth::LoginStartRequest {
            email: email.clone(),
            opaque_request: request,
            device: device("sso device"),
            sso_session: Some(sso_session),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;

    // Second round: the account is known now.
    let SsoResult::LoginRequired {
        sso_session,
        email: asserted,
    } = sign_in(s, SSO_PROVIDER, &who).await
    else {
        panic!("expected login_required")
    };
    assert_eq!(asserted, email);
    // The IdP only vouches for the email; the password is still needed.
    assert!(
        login_raw(
            s,
            &email,
            "wrong-password-12345",
            device("sso laptop"),
            Some(sso_session)
        )
        .await
        .is_err()
    );
    let SsoResult::LoginRequired { sso_session, .. } = sign_in(s, SSO_PROVIDER, &who).await else {
        panic!("expected login_required")
    };
    // A brand-new device signs straight in: the IdP round-trip replaces the
    // emailed approval.
    let resp = login_raw(s, &email, password, device("sso laptop"), Some(sso_session))
        .await
        .unwrap();
    let AuthResponse::Authenticated(sess) = resp else {
        panic!("expected a session, got {resp:?}")
    };
    assert_eq!(sess.user.id, u.id());
    if s.mailpit.is_some() {
        // Sanity: the same new-device login without SSO does ask for approval.
        let plain = login_raw(s, &email, password, device("plain laptop"), None)
            .await
            .unwrap();
        assert!(matches!(plain, AuthResponse::DeviceApprovalRequired { .. }));
    }

    // The identity is linked by subject: even if the IdP now reports a
    // different email for the same subject, it resolves to this account.
    let renamed = Identity {
        email: unique_email("renamed"),
        ..who.clone()
    };
    let SsoResult::LoginRequired {
        email: asserted, ..
    } = sign_in(s, SSO_PROVIDER, &renamed).await
    else {
        panic!("expected login_required")
    };
    assert_eq!(asserted, email);

    // A different subject with this email is still just "an existing account".
    let impostor = Identity {
        sub: "someone-else".into(),
        ..who.clone()
    };
    assert!(matches!(
        sign_in(s, SSO_PROVIDER, &impostor).await,
        SsoResult::LoginRequired { .. }
    ));

    // Disabled accounts are turned away at the IdP door.
    let admin = admin_token(s).await;
    let _: AdminUser = s
        .json(
            Method::PATCH,
            &format!("/admin/users/{}", u.id()),
            Some(&admin),
            Some(&AdminUpdateUserRequest {
                disabled: Some(true),
                is_admin: None,
                email_verified: None,
            }),
        )
        .await;
    let SsoResult::Failed { message } = sign_in(s, SSO_PROVIDER, &who).await else {
        panic!("expected failure")
    };
    assert!(message.contains("disabled"), "{message}");
}

#[tokio::test]
async fn callback_rejects_bad_codes_errors_and_replays() {
    let Some(s) = server().await else { return };

    // Missing / unknown state.
    assert_eq!(
        callback(s, &[("code", "x")]).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        callback(s, &[("state", "nope"), ("code", "x")])
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );

    // IdP-side error.
    let flow = start(s, SSO_PROVIDER, None).await;
    let resp = callback(
        s,
        &[
            ("state", &flow.flow_id),
            ("error", "access_denied"),
            ("error_description", "user said no"),
        ],
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let SsoResult::Failed { message } = poll(s, &flow.flow_id).await else {
        panic!("expected failure")
    };
    assert!(message.contains("access_denied"), "{message}");
    // The flow is consumed: a second callback (replay) is refused.
    assert_eq!(
        callback(s, &[("state", &flow.flow_id), ("code", "late")])
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );

    // No code, no error.
    let flow = start(s, SSO_PROVIDER, None).await;
    callback(s, &[("state", &flow.flow_id)]).await;
    let SsoResult::Failed { message } = poll(s, &flow.flow_id).await else {
        panic!("expected failure")
    };
    assert!(message.contains("Missing authorization code"), "{message}");

    // Garbage code: the token exchange fails and the user is told nothing
    // specific.
    let flow = start(s, SSO_PROVIDER, None).await;
    callback(s, &[("state", &flow.flow_id), ("code", "not-a-code")]).await;
    let SsoResult::Failed { message } = poll(s, &flow.flow_id).await else {
        panic!("expected failure")
    };
    assert!(message.contains("Could not verify"), "{message}");

    // Nonce mismatch: a valid, signed ID token for another flow is refused.
    let email = unique_email("nonce");
    let flow = start(s, SSO_PROVIDER, None).await;
    let stale = identity(&email, "some-other-nonce");
    callback(
        s,
        &[("state", &flow.flow_id), ("code", &oidc::code_for(&stale))],
    )
    .await;
    assert!(matches!(
        poll(s, &flow.flow_id).await,
        SsoResult::Failed { .. }
    ));

    // Email not verified at the IdP is not good enough.
    let flow = start(s, SSO_PROVIDER, None).await;
    let unverified = Identity {
        email_verified: false,
        ..identity(&email, &auth_params(&flow.authorization_url)["nonce"])
    };
    callback(
        s,
        &[
            ("state", &flow.flow_id),
            ("code", &oidc::code_for(&unverified)),
        ],
    )
    .await;
    assert!(matches!(
        poll(s, &flow.flow_id).await,
        SsoResult::Failed { .. }
    ));

    // Result of a finished flow is still readable (until the TTL) but the
    // flow itself can't be completed twice.
    let flow = start(s, SSO_PROVIDER, None).await;
    let who = identity(&email, &auth_params(&flow.authorization_url)["nonce"]);
    callback(
        s,
        &[("state", &flow.flow_id), ("code", &oidc::code_for(&who))],
    )
    .await;
    assert!(matches!(
        poll(s, &flow.flow_id).await,
        SsoResult::RegistrationRequired { .. }
    ));
    assert_eq!(
        callback(
            s,
            &[("state", &flow.flow_id), ("code", &oidc::code_for(&who))]
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn allowed_domains_and_redirect_filtering() {
    let Some(s) = server().await else { return };

    // Domain allow-list (case-insensitive, whitespace-tolerant config).
    let outsider = identity(&unique_email("outsider"), "");
    let SsoResult::Failed { .. } = sign_in(s, SSO_CORP_PROVIDER, &outsider).await else {
        panic!("outsider must be refused")
    };
    let insider = identity(
        &format!(
            "Insider-{}@{}",
            uuid::Uuid::new_v4().simple(),
            SSO_CORP_DOMAIN.to_uppercase()
        ),
        "",
    );
    let SsoResult::RegistrationRequired { email, .. } =
        sign_in(s, SSO_CORP_PROVIDER, &insider).await
    else {
        panic!("insider must pass")
    };
    assert_eq!(email, insider.email.to_lowercase(), "email is normalised");
    let second = identity(
        &format!("x-{}@other.example", uuid::Uuid::new_v4().simple()),
        "",
    );
    assert!(matches!(
        sign_in(s, SSO_CORP_PROVIDER, &second).await,
        SsoResult::RegistrationRequired { .. }
    ));
    // The same person is welcome through the unrestricted provider.
    assert!(matches!(
        sign_in(s, SSO_PROVIDER, &outsider).await,
        SsoResult::RegistrationRequired { .. }
    ));

    // Redirect targets: own origin, localhost and the app scheme are honoured
    // with `flow` appended; anything else is dropped and the fallback page is
    // served instead.
    let cases = [
        (format!("http://{}/cabinet/sso", s.addr), true),
        ("http://localhost:5173/sso?x=1".to_string(), true),
        ("http://127.0.0.1:1420/callback".to_string(), true),
        ("termoso://sso".to_string(), true),
        ("https://evil.example/phish".to_string(), false),
        (format!("http://{}.evil.example/cabinet/sso", s.addr), false),
        (format!("http://{}@evil.example/cabinet/sso", s.addr), false),
        ("http://localhost.evil.example/sso".to_string(), false),
        ("javascript:alert(1)".to_string(), false),
        ("//evil.example".to_string(), false),
        ("not a url".to_string(), false),
    ];
    for (target, allowed) in cases {
        let flow = start(s, SSO_PROVIDER, Some(&target)).await;
        let resp = callback(s, &[("state", &flow.flow_id), ("error", "x")]).await;
        if allowed {
            assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{target}");
            let loc = resp.headers()["location"].to_str().unwrap().to_string();
            let sep = if target.contains('?') { '&' } else { '?' };
            assert_eq!(loc, format!("{target}{sep}flow={}", flow.flow_id));
        } else {
            assert_eq!(resp.status(), StatusCode::OK, "{target}");
            assert!(
                resp.headers()["content-type"]
                    .to_str()
                    .unwrap()
                    .starts_with("text/html")
            );
            let body = resp.text().await.unwrap();
            assert!(body.contains("Signed in"), "{body}");
        }
    }
}

#[tokio::test]
async fn registry_rejects_incomplete_providers() {
    let Some(s) = server().await else { return };
    let oidc_ok = SsoProviderConfig {
        name: None,
        kind: SsoKindConfig::Oidc,
        issuer: Some(s.idp.issuer.clone()),
        client_id: Some(oidc::CLIENT_ID.into()),
        client_secret: Some(oidc::CLIENT_SECRET.into()),
        ..SsoProviderConfig::default()
    };
    let build = |slug: &str, p: SsoProviderConfig| {
        let mut cfg = Config {
            public_url: format!("http://{}", s.addr),
            ..Config::default()
        };
        cfg.sso.insert(slug.into(), p);
        async move {
            SsoRegistry::from_config(&cfg)
                .await
                .map(|r| r.list())
                .map_err(|e| format!("{e:#}"))
        }
    };

    let ok = build("acme", oidc_ok.clone())
        .await
        .expect("discovery works");
    assert_eq!(ok[0].name, "acme", "slug is the default name");

    let saml_no_metadata = build(
        "okta",
        SsoProviderConfig {
            kind: SsoKindConfig::Saml,
            ..SsoProviderConfig::default()
        },
    )
    .await
    .expect_err("SAML needs metadata");
    assert!(
        saml_no_metadata.contains("saml_metadata"),
        "{saml_no_metadata}"
    );

    let saml_bad_metadata = build(
        "okta",
        SsoProviderConfig {
            kind: SsoKindConfig::Saml,
            saml_metadata: Some("<md:EntityDescriptor xmlns:md=\"urn:oasis:names:tc:SAML:2.0:metadata\" entityID=\"x\"/>".into()),
            ..SsoProviderConfig::default()
        },
    )
    .await
    .expect_err("metadata without an IdP role");
    assert!(
        saml_bad_metadata.contains("IDPSSODescriptor"),
        "{saml_bad_metadata}"
    );

    let saml_sign_without_key = build(
        "okta",
        SsoProviderConfig {
            kind: SsoKindConfig::Saml,
            saml_metadata: Some(saml_support::idp_metadata_xml(
                SAML_IDP_ENTITY,
                SAML_IDP_SSO_URL,
            )),
            saml_sign_requests: Some(true),
            ..SsoProviderConfig::default()
        },
    )
    .await
    .expect_err("signing needs a key");
    assert!(
        saml_sign_without_key.contains("saml_sp_private_key"),
        "{saml_sign_without_key}"
    );

    let saml_key_mismatch = build(
        "okta",
        SsoProviderConfig {
            kind: SsoKindConfig::Saml,
            saml_metadata: Some(saml_support::idp_metadata_xml(
                SAML_IDP_ENTITY,
                SAML_IDP_SSO_URL,
            )),
            saml_sp_certificate: Some(saml_support::IDP_CERT_PEM.into()),
            saml_sp_private_key: Some(saml_support::SP_KEY_PEM.into()),
            ..SsoProviderConfig::default()
        },
    )
    .await
    .expect_err("cert/key mismatch");
    assert!(
        saml_key_mismatch.contains("does not match"),
        "{saml_key_mismatch}"
    );

    let no_secret = build(
        "acme",
        SsoProviderConfig {
            client_secret: None,
            ..oidc_ok.clone()
        },
    )
    .await
    .expect_err("client_secret is required");
    assert!(no_secret.contains("client_secret"));

    let no_issuer = build(
        "acme",
        SsoProviderConfig {
            issuer: None,
            ..oidc_ok.clone()
        },
    )
    .await
    .expect_err("generic OIDC needs an issuer");
    assert!(no_issuer.contains("issuer"));

    let unreachable = build(
        "acme",
        SsoProviderConfig {
            issuer: Some("http://127.0.0.1:9/".into()),
            ..oidc_ok
        },
    )
    .await
    .expect_err("discovery must fail");
    assert!(unreachable.contains("discovery"));
}

/// Anonymous token lookups (flow polling, the OIDC callback, invite previews)
/// are capped per client IP so a token scan is refused instead of served.
#[tokio::test]
async fn anonymous_token_lookups_are_rate_limited_per_ip() {
    let Some(s) = server().await else { return };
    let ip = "203.0.113.77";
    let paths = [
        "/auth/sso/flow/not-a-flow",
        "/auth/sso/callback?state=not-a-flow&code=x",
        "/auth/sso/saml/post/not-a-flow",
        "/invites/not-an-invite",
    ];
    let mut limited = false;
    for i in 0..300 {
        let r = s
            .http()
            .get(s.url(paths[i % paths.len()]))
            .header("x-forwarded-for", ip)
            .send()
            .await
            .unwrap();
        if r.status() == StatusCode::TOO_MANY_REQUESTS {
            let body: serde_json::Value = r.json().await.unwrap();
            assert_eq!(body["code"], "rate_limited");
            assert!(body["details"]["retry_after"].as_u64().unwrap_or(0) >= 1);
            limited = true;
            break;
        }
        assert!(
            r.status().is_client_error(),
            "unknown tokens must be rejected, got {}",
            r.status()
        );
    }
    assert!(
        limited,
        "300 anonymous lookups from one IP were never limited"
    );

    // Other clients are unaffected.
    let r = s
        .http()
        .get(s.url("/auth/sso/flow/not-a-flow"))
        .header("x-forwarded-for", "203.0.113.78")
        .send()
        .await
        .unwrap();
    assert_ne!(r.status(), StatusCode::TOO_MANY_REQUESTS);
}
