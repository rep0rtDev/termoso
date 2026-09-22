//! Step-up re-authentication ("sudo mode") in front of destructive account
//! changes, and the "start over" reset for accounts whose password and
//! recovery phrase are both lost.

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_crypto::opaque;
use termoso_proto::account::{SecurityEventList, UserProfile};
use termoso_proto::auth::{
    AuthResponse, BackupCodes, DeviceList, MfaCredential, MfaVerifyRequest,
    PasswordSetupStartResponse, ReauthFinishRequest, ReauthMethod, ReauthStartRequest,
    ReauthStartResponse, StartOverCancelRequest, StartOverConfirmRequest, StartOverFinishRequest,
    StartOverPasswordStartRequest, StartOverRequest, StartOverRequestResponse, StartOverScheduled,
    StartOverStatus, TotpCodeRequest, TotpSetupResponse,
};
use termoso_proto::error::codes;
use termoso_proto::sync::{EntityChange, PullRequest, PullResponse, PushRequest, PushResponse};
use termoso_proto::vault::VaultList;
use totp_rs::Totp;
use uuid::Uuid;

fn assert_reauth_required(v: &serde_json::Value) {
    assert_eq!(v["code"], codes::REAUTH_REQUIRED, "{v}");
}

#[derive(serde::Deserialize)]
struct AccountResponse {
    user: UserProfile,
}

async fn profile(s: &TestServer, token: &str) -> UserProfile {
    let a: AccountResponse = s.json(Method::GET, "/account", Some(token), NOBODY).await;
    a.user
}

async fn security_kinds(s: &TestServer, token: &str) -> Vec<String> {
    let list: SecurityEventList = s
        .json(Method::GET, "/account/security-events", Some(token), NOBODY)
        .await;
    list.events.into_iter().map(|e| e.kind).collect()
}

/// Password step-up for `token`; returns the server's answer (a session that
/// has MFA on gets `MfaRequired`).
async fn reauth_with_password(
    s: &TestServer,
    token: &str,
    email: &str,
    password: &str,
) -> AuthResponse {
    let (request, state) = opaque::client_login_start(password.as_bytes()).expect("start");
    let start: ReauthStartResponse = s
        .json(
            Method::POST,
            "/auth/reauth/start",
            Some(token),
            Some(&ReauthStartRequest {
                opaque_request: Some(request),
            }),
        )
        .await;
    assert_eq!(start.method, ReauthMethod::Password);
    let finish = opaque::client_login_finish(
        state,
        password.as_bytes(),
        &email.to_lowercase(),
        start.opaque_response.as_deref().expect("opaque response"),
    );
    let finalization = match finish {
        Ok(f) => f.finalization_b64,
        Err(_) => "AAAA".into(),
    };
    s.json(
        Method::POST,
        "/auth/reauth/finish",
        Some(token),
        Some(&ReauthFinishRequest {
            reauth_id: start.reauth_id,
            opaque_finalization: Some(finalization),
            code: None,
        }),
    )
    .await
}

async fn other_device_id(s: &TestServer, u: &User) -> Uuid {
    let list: DeviceList = s
        .json(Method::GET, "/account/devices", Some(u.token()), NOBODY)
        .await;
    list.devices
        .iter()
        .find(|d| !d.current)
        .map(|d| d.id)
        .expect("a second device")
}

#[tokio::test]
async fn sensitive_routes_need_a_fresh_step_up() {
    let Some(s) = server().await else { return };
    let u = register(s, &unique_email("stepup"), "pw-stepup-1234567").await;
    // A second device to revoke.
    let AuthResponse::Authenticated(second) = login(s, &u.email, &u.password).await else {
        panic!("login")
    };
    let victim = other_device_id(s, &u).await;

    // Right after signing in the session is fresh, so nothing extra is asked.
    let devices: DeviceList = s
        .json(Method::GET, "/account/devices", Some(u.token()), NOBODY)
        .await;
    assert_eq!(devices.devices.len(), 2);

    // Once the window has passed, destructive routes refuse with a stable code.
    s.age_step_up(u.token()).await;
    let v = s
        .expect_status(
            Method::DELETE,
            &format!("/account/devices/{victim}"),
            Some(u.token()),
            NOBODY,
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_reauth_required(&v);
    for path in ["/account/mfa/totp/setup", "/account/mfa/backup-codes"] {
        let v = s
            .expect_status(
                Method::POST,
                path,
                Some(u.token()),
                NOBODY,
                StatusCode::FORBIDDEN,
            )
            .await;
        assert_reauth_required(&v);
    }
    let v = s
        .expect_status(
            Method::DELETE,
            "/account",
            Some(u.token()),
            NOBODY,
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_reauth_required(&v);
    // Reads keep working: the session itself is fine.
    let _ = profile(s, u.token()).await;

    // A wrong password does not unlock anything and is recorded.
    let resp = s
        .call(
            Method::POST,
            "/auth/reauth/start",
            Some(u.token()),
            Some(&ReauthStartRequest {
                opaque_request: Some(opaque::client_login_start(b"nope").expect("start").0),
            }),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let start: ReauthStartResponse = resp.json().await.expect("json");
    s.expect_status(
        Method::POST,
        "/auth/reauth/finish",
        Some(u.token()),
        Some(&ReauthFinishRequest {
            reauth_id: start.reauth_id.clone(),
            opaque_finalization: Some("AAAA".into()),
            code: None,
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    // The OPAQUE state is single-use: replaying the flow id fails too.
    s.expect_status(
        Method::POST,
        "/auth/reauth/finish",
        Some(u.token()),
        Some(&ReauthFinishRequest {
            reauth_id: start.reauth_id,
            opaque_finalization: Some("AAAA".into()),
            code: None,
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let v = s
        .expect_status(
            Method::DELETE,
            &format!("/account/devices/{victim}"),
            Some(u.token()),
            NOBODY,
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_reauth_required(&v);

    // The real password re-opens the window for this session only.
    let resp = reauth_with_password(s, u.token(), &u.email, &u.password).await;
    let AuthResponse::Reauthenticated { reauth_expires_at } = resp else {
        panic!("expected Reauthenticated, got {resp:?}")
    };
    let left = reauth_expires_at - chrono::Utc::now();
    assert!(left.num_minutes() >= 4 && left.num_minutes() <= 5, "{left}");

    // A flow started by one session cannot be finished by another.
    let start: ReauthStartResponse = s
        .json(
            Method::POST,
            "/auth/reauth/start",
            Some(u.token()),
            Some(&ReauthStartRequest {
                opaque_request: Some(opaque::client_login_start(b"x").expect("start").0),
            }),
        )
        .await;
    s.expect_status(
        Method::POST,
        "/auth/reauth/finish",
        Some(&second.token),
        Some(&ReauthFinishRequest {
            reauth_id: start.reauth_id,
            opaque_finalization: Some("AAAA".into()),
            code: None,
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;

    s.expect_status(
        Method::DELETE,
        &format!("/account/devices/{victim}"),
        Some(u.token()),
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

    let kinds = security_kinds(s, u.token()).await;
    assert!(kinds.contains(&"reauth_failed".to_string()), "{kinds:?}");
    assert!(kinds.contains(&"reauth".to_string()), "{kinds:?}");
    assert!(kinds.contains(&"device_revoked".to_string()), "{kinds:?}");

    // …and expires again.
    s.age_step_up(u.token()).await;
    let v = s
        .expect_status(
            Method::POST,
            "/account/mfa/totp/setup",
            Some(u.token()),
            NOBODY,
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_reauth_required(&v);
}

#[tokio::test]
async fn step_up_asks_for_the_second_factor_when_enabled() {
    let Some(s) = server().await else { return };
    let u = register(s, &unique_email("stepup-mfa"), "pw-stepup-mfa-1234").await;
    let setup: TotpSetupResponse = s
        .json(
            Method::POST,
            "/account/mfa/totp/setup",
            Some(u.token()),
            NOBODY,
        )
        .await;
    let totp = Totp::from_url(&setup.otpauth_url).expect("otpauth url");
    let code = || totp.generate_current().to_string();
    let _: BackupCodes = s
        .json(
            Method::POST,
            "/account/mfa/totp/confirm",
            Some(u.token()),
            Some(&TotpCodeRequest { code: code() }),
        )
        .await;
    let used = code();
    s.age_step_up(u.token()).await;

    // Password alone is not enough.
    let resp = reauth_with_password(s, u.token(), &u.email, &u.password).await;
    let AuthResponse::MfaRequired { mfa_token, .. } = resp else {
        panic!("expected MfaRequired, got {resp:?}")
    };
    let v = s
        .expect_status(
            Method::POST,
            "/account/mfa/backup-codes",
            Some(u.token()),
            NOBODY,
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_reauth_required(&v);

    // Wrong second factor is refused; a fresh TOTP code completes the step-up.
    s.expect_status(
        Method::POST,
        "/auth/mfa/verify",
        None,
        Some(&MfaVerifyRequest {
            mfa_token: mfa_token.clone(),
            credential: MfaCredential::Totp {
                code: "000000".into(),
            },
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let fresh = loop {
        let c = code();
        if c != used {
            break c;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    };
    let resp: AuthResponse = s
        .json(
            Method::POST,
            "/auth/mfa/verify",
            None,
            Some(&MfaVerifyRequest {
                mfa_token,
                credential: MfaCredential::Totp { code: fresh },
            }),
        )
        .await;
    assert!(
        matches!(resp, AuthResponse::Reauthenticated { .. }),
        "{resp:?}"
    );
    let _: BackupCodes = s
        .json(
            Method::POST,
            "/account/mfa/backup-codes",
            Some(u.token()),
            NOBODY,
        )
        .await;
}

#[tokio::test]
async fn accounts_without_a_password_step_up_by_email() {
    let Some(s) = server().await else { return };
    if s.mailpit.is_none() {
        eprintln!("skipping: needs Mailpit");
        return;
    }
    let u = register(s, &unique_email("stepup-mail"), "pw-stepup-mail-1234").await;
    // SSO-only style account: no OPAQUE record on file.
    s.sql(
        "UPDATE users SET opaque_record = NULL WHERE email = $1",
        &u.email.to_lowercase(),
    )
    .await;
    s.age_step_up(u.token()).await;

    let start: ReauthStartResponse = s
        .json(
            Method::POST,
            "/auth/reauth/start",
            Some(u.token()),
            Some(&ReauthStartRequest {
                opaque_request: None,
            }),
        )
        .await;
    assert_eq!(start.method, ReauthMethod::Email);
    assert!(start.email_hint.is_some());
    let code = s.emailed_code(&u.email, "confirm it's you").await;

    s.expect_status(
        Method::POST,
        "/auth/reauth/finish",
        Some(u.token()),
        Some(&ReauthFinishRequest {
            reauth_id: start.reauth_id.clone(),
            opaque_finalization: None,
            code: Some("000000".into()),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let resp: AuthResponse = s
        .json(
            Method::POST,
            "/auth/reauth/finish",
            Some(u.token()),
            Some(&ReauthFinishRequest {
                reauth_id: start.reauth_id,
                opaque_finalization: None,
                code: Some(code),
            }),
        )
        .await;
    assert!(
        matches!(resp, AuthResponse::Reauthenticated { .. }),
        "{resp:?}"
    );
    let _: TotpSetupResponse = s
        .json(
            Method::POST,
            "/account/mfa/totp/setup",
            Some(u.token()),
            NOBODY,
        )
        .await;
}

#[tokio::test]
async fn account_deletion_needs_step_up_and_the_emailed_code() {
    let Some(s) = server().await else { return };
    if s.mailpit.is_none() {
        eprintln!("skipping: needs Mailpit");
        return;
    }
    let u = register(s, &unique_email("del"), "pw-del-1234567890").await;
    s.age_step_up(u.token()).await;
    let v = s
        .expect_status(
            Method::DELETE,
            "/account",
            Some(u.token()),
            NOBODY,
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_reauth_required(&v);
    reauth_with_password(s, u.token(), &u.email, &u.password).await;
    s.expect_status(
        Method::DELETE,
        "/account",
        Some(u.token()),
        NOBODY,
        StatusCode::ACCEPTED,
    )
    .await;
    let code = s.emailed_code(&u.email, "account deletion").await;
    // The code on its own (window closed again) is not enough either.
    s.age_step_up(u.token()).await;
    let v = s
        .expect_status(
            Method::DELETE,
            "/account",
            Some(u.token()),
            Some(&serde_json::json!({ "code": code })),
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_reauth_required(&v);
    reauth_with_password(s, u.token(), &u.email, &u.password).await;
    s.expect_status(
        Method::DELETE,
        "/account",
        Some(u.token()),
        Some(&serde_json::json!({ "code": code })),
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::GET,
        "/account",
        Some(u.token()),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
}

/// Token of the first link in `body` whose path continues `prefix` with the
/// token itself (so `/start-over/` does not match `/start-over/cancel/`).
fn link_token(body: &str, prefix: &str) -> String {
    body.match_indices(prefix)
        .map(|(i, _)| {
            body[i + prefix.len()..]
                .split_whitespace()
                .next()
                .unwrap_or_default()
        })
        .find(|t| !t.is_empty() && !t.contains('/'))
        .expect("link in email")
        .to_string()
}

async fn personal_vault(s: &TestServer, token: &str) -> Uuid {
    let vaults: VaultList = s.json(Method::GET, "/vaults", Some(token), NOBODY).await;
    let personal: Vec<_> = vaults
        .vaults
        .iter()
        .filter(|v| v.team_id.is_none())
        .collect();
    assert_eq!(personal.len(), 1, "{:?}", vaults.vaults);
    personal[0].id
}

#[tokio::test]
async fn start_over_is_delayed_cancellable_and_destroys_the_old_vault() {
    let Some(s) = server().await else { return };
    if s.mailpit.is_none() {
        eprintln!("skipping: needs Mailpit");
        return;
    }
    let u = register(s, &unique_email("startover"), "pw-startover-12345").await;
    let AuthResponse::Authenticated(second) = login(s, &u.email, &u.password).await else {
        panic!("login")
    };
    let old_vault = personal_vault(s, u.token()).await;
    let host_id = Uuid::new_v4();
    let r: PushResponse = s
        .json(
            Method::POST,
            "/sync/push",
            Some(u.token()),
            Some(&PushRequest {
                changes: vec![EntityChange {
                    id: host_id,
                    kind: "host".into(),
                    vault_id: old_vault,
                    base_version: None,
                    key_version: 1,
                    data: u.encrypt_entity(
                        &u.personal_vault_key,
                        "host",
                        host_id,
                        r#"{"label":"prod"}"#,
                    ),
                    updated_at: chrono::Utc::now(),
                }],
                deletes: vec![],
            }),
        )
        .await;
    assert_eq!(r.results.len(), 1);

    // Unknown address: same shape, no email, code can never match.
    let ghost = unique_email("nobody");
    let r: StartOverRequestResponse = s
        .json(
            Method::POST,
            "/auth/start-over/request",
            None,
            Some(&StartOverRequest {
                email: ghost.clone(),
            }),
        )
        .await;
    assert!(!r.request_token.is_empty());
    s.expect_status(
        Method::POST,
        "/auth/start-over/confirm",
        None,
        Some(&StartOverConfirmRequest {
            request_token: r.request_token,
            code: "000000".into(),
            mfa_code: None,
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(s.email_count(&ghost).await, 0);

    // Real account: code by email, confirm schedules the reset.
    let r: StartOverRequestResponse = s
        .json(
            Method::POST,
            "/auth/start-over/request",
            None,
            Some(&StartOverRequest {
                email: u.email.clone(),
            }),
        )
        .await;
    let code = s.emailed_code(&u.email, "account reset").await;
    let scheduled: StartOverScheduled = s
        .json(
            Method::POST,
            "/auth/start-over/confirm",
            None,
            Some(&StartOverConfirmRequest {
                request_token: r.request_token,
                code,
                mfa_code: None,
            }),
        )
        .await;
    let eta = scheduled.scheduled_for - chrono::Utc::now();
    assert!(eta.num_hours() >= 23, "{eta}");

    // Every signed-in device sees it and can call it off.
    let p = profile(s, &second.token).await;
    assert_eq!(p.reset_scheduled_for, Some(scheduled.scheduled_for));
    let body = s.emailed_body(&u.email, "account reset scheduled").await;
    assert!(body.contains("cannot be recovered"), "{body}");
    let finish = link_token(&body, "/start-over/");
    let cancel = link_token(&body, "/start-over/cancel/");
    assert_ne!(finish, cancel);
    let st: StartOverStatus = s
        .json(
            Method::GET,
            &format!("/auth/start-over/{finish}"),
            None,
            NOBODY,
        )
        .await;
    assert!(!st.ready);

    // Too early: the reset cannot be completed.
    let (req, _) = opaque::client_registration_start(b"new-pw").expect("start");
    s.expect_status(
        Method::POST,
        "/auth/start-over/password/start",
        None,
        Some(&StartOverPasswordStartRequest {
            token: finish.clone(),
            opaque_request: req,
        }),
        StatusCode::CONFLICT,
    )
    .await;

    // Cancel from the mailbox link; tokens die with it, nothing changed.
    s.expect_status(
        Method::POST,
        "/auth/start-over/cancel",
        None,
        Some(&StartOverCancelRequest {
            cancel_token: cancel.clone(),
        }),
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::GET,
        &format!("/auth/start-over/{finish}"),
        None,
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let p = profile(s, u.token()).await;
    assert_eq!(p.reset_scheduled_for, None);
    s.emailed_body(&u.email, "account reset cancelled").await;

    // Again, this time cancelled from a signed-in device.
    s.reset_email_limit(&u.email).await;
    let r: StartOverRequestResponse = s
        .json(
            Method::POST,
            "/auth/start-over/request",
            None,
            Some(&StartOverRequest {
                email: u.email.clone(),
            }),
        )
        .await;
    let code = s.emailed_code(&u.email, "account reset").await;
    let _: StartOverScheduled = s
        .json(
            Method::POST,
            "/auth/start-over/confirm",
            None,
            Some(&StartOverConfirmRequest {
                request_token: r.request_token,
                code,
                mfa_code: None,
            }),
        )
        .await;
    s.expect_status(
        Method::POST,
        "/account/start-over/cancel",
        Some(&second.token),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    let p = profile(s, u.token()).await;
    assert_eq!(p.reset_scheduled_for, None);

    // Third time we let it run: wait out the delay and finish.
    s.reset_email_limit(&u.email).await;
    let r: StartOverRequestResponse = s
        .json(
            Method::POST,
            "/auth/start-over/request",
            None,
            Some(&StartOverRequest {
                email: u.email.clone(),
            }),
        )
        .await;
    let code = s.emailed_code(&u.email, "account reset").await;
    let _: StartOverScheduled = s
        .json(
            Method::POST,
            "/auth/start-over/confirm",
            None,
            Some(&StartOverConfirmRequest {
                request_token: r.request_token,
                code,
                mfa_code: None,
            }),
        )
        .await;
    let body = s.emailed_body(&u.email, "account reset scheduled").await;
    let finish = link_token(&body, "/start-over/");
    s.age_start_over(&u.email).await;
    let st: StartOverStatus = s
        .json(
            Method::GET,
            &format!("/auth/start-over/{finish}"),
            None,
            NOBODY,
        )
        .await;
    assert!(st.ready);

    let new_password = "pw-startover-new-999";
    let (req, state) = opaque::client_registration_start(new_password.as_bytes()).expect("start");
    let start: PasswordSetupStartResponse = s
        .json(
            Method::POST,
            "/auth/start-over/password/start",
            None,
            Some(&StartOverPasswordStartRequest {
                token: finish.clone(),
                opaque_request: req,
            }),
        )
        .await;
    let out = opaque::client_registration_finish(
        state,
        new_password.as_bytes(),
        &u.email.to_lowercase(),
        &start.opaque_response,
    )
    .expect("finish");
    let fresh = FreshKeys::generate();
    let resp: AuthResponse = s
        .json(
            Method::POST,
            "/auth/start-over/finish",
            None,
            Some(&StartOverFinishRequest {
                token: finish.clone(),
                opaque_upload: out.upload_b64,
                keys: fresh.upload_for(&out.export_key),
                device: device("phoenix"),
            }),
        )
        .await;
    let AuthResponse::Authenticated(reborn) = resp else {
        panic!("expected a session, got {resp:?}")
    };

    // Everyone else is out; the finish link is spent.
    for t in [u.token(), second.token.as_str()] {
        s.expect_status(
            Method::GET,
            "/account",
            Some(t),
            NOBODY,
            StatusCode::UNAUTHORIZED,
        )
        .await;
    }
    s.expect_status(
        Method::GET,
        &format!("/auth/start-over/{finish}"),
        None,
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;

    // A brand-new empty personal vault; the old one and its entities are gone.
    let new_vault = personal_vault(s, &reborn.token).await;
    assert_ne!(new_vault, old_vault);
    let pull: PullResponse = s
        .json(
            Method::POST,
            "/sync/pull",
            Some(&reborn.token),
            Some(&PullRequest {
                cursors: Default::default(),
                limit: None,
            }),
        )
        .await;
    assert!(pull.entities.is_empty(), "{:?}", pull.entities);
    let p = profile(s, &reborn.token).await;
    assert_eq!(p.reset_scheduled_for, None);
    let devices: DeviceList = s
        .json(Method::GET, "/account/devices", Some(&reborn.token), NOBODY)
        .await;
    assert_eq!(devices.devices.len(), 1);

    // New password works, the old one does not.
    s.reset_email_limit(&u.email).await;
    assert!(try_login(s, &u.email, &u.password).await.is_err());
    let AuthResponse::Authenticated(_) = login(s, &u.email, new_password).await else {
        panic!("login with the new password")
    };
    s.emailed_body(&u.email, "account reset completed").await;
    let kinds = security_kinds(s, &reborn.token).await;
    for k in [
        "reset_requested",
        "reset_scheduled",
        "reset_cancelled",
        "account_reset",
    ] {
        assert!(kinds.contains(&k.to_string()), "{k} missing in {kinds:?}");
    }
}

#[tokio::test]
async fn start_over_keeps_asking_for_the_second_factor() {
    let Some(s) = server().await else { return };
    if s.mailpit.is_none() {
        eprintln!("skipping: needs Mailpit");
        return;
    }
    let u = register(s, &unique_email("startover-mfa"), "pw-startover-mfa-1").await;
    let setup: TotpSetupResponse = s
        .json(
            Method::POST,
            "/account/mfa/totp/setup",
            Some(u.token()),
            NOBODY,
        )
        .await;
    let totp = Totp::from_url(&setup.otpauth_url).expect("otpauth url");
    let code = || totp.generate_current().to_string();
    let _: BackupCodes = s
        .json(
            Method::POST,
            "/account/mfa/totp/confirm",
            Some(u.token()),
            Some(&TotpCodeRequest { code: code() }),
        )
        .await;
    let used = code();

    let r: StartOverRequestResponse = s
        .json(
            Method::POST,
            "/auth/start-over/request",
            None,
            Some(&StartOverRequest {
                email: u.email.clone(),
            }),
        )
        .await;
    let email_code = s.emailed_code(&u.email, "account reset").await;
    let v = s
        .expect_status(
            Method::POST,
            "/auth/start-over/confirm",
            None,
            Some(&StartOverConfirmRequest {
                request_token: r.request_token.clone(),
                code: email_code,
                mfa_code: None,
            }),
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_eq!(v["code"], codes::MFA_REQUIRED, "{v}");

    // The mailbox code was consumed; a second-factor guess needs a new one.
    let r: StartOverRequestResponse = s
        .json(
            Method::POST,
            "/auth/start-over/request",
            None,
            Some(&StartOverRequest {
                email: u.email.clone(),
            }),
        )
        .await;
    let email_code = s.emailed_code(&u.email, "account reset").await;
    let fresh = loop {
        let c = code();
        if c != used {
            break c;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    };
    let scheduled: StartOverScheduled = s
        .json(
            Method::POST,
            "/auth/start-over/confirm",
            None,
            Some(&StartOverConfirmRequest {
                request_token: r.request_token,
                code: email_code,
                mfa_code: Some(fresh),
            }),
        )
        .await;
    let p = profile(s, u.token()).await;
    assert_eq!(p.reset_scheduled_for, Some(scheduled.scheduled_for));
}
