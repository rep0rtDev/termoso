//! Second factors: TOTP enrolment and login, replay protection, backup codes,
//! WebAuthn boundaries (no authenticator available in CI, so only the parts
//! that do not need one).

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_proto::auth::{
    AuthResponse, BackupCodes, MfaCredential, MfaMethod, MfaStatus, MfaVerifyRequest,
    TotpCodeRequest, TotpSetupResponse, WebauthnChallengeRequest, WebauthnRegisterFinishRequest,
};
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

/// Authenticator side of TOTP, built from the enrolment response.
fn authenticator(setup: &TotpSetupResponse) -> TOTP {
    let from_url = TOTP::from_url(&setup.otpauth_url).expect("otpauth url parses");
    let bytes = Secret::Encoded(setup.secret.clone())
        .to_bytes()
        .expect("base32 secret");
    assert_eq!(
        from_url.secret, bytes,
        "secret and otpauth url must describe the same key"
    );
    assert_eq!(
        (from_url.algorithm, from_url.digits, from_url.step),
        (Algorithm::SHA1, 6, 30)
    );
    from_url
}

fn code(totp: &TOTP) -> String {
    totp.generate_current().expect("clock")
}

/// Enrol TOTP for `u`; returns the authenticator and the fresh backup codes.
async fn enroll_totp(s: &TestServer, u: &User) -> (TOTP, Vec<String>) {
    let setup: TotpSetupResponse = s
        .json(
            Method::POST,
            "/account/mfa/totp/setup",
            Some(u.token()),
            NOBODY,
        )
        .await;
    let totp = authenticator(&setup);
    let backup: BackupCodes = s
        .json(
            Method::POST,
            "/account/mfa/totp/confirm",
            Some(u.token()),
            Some(&TotpCodeRequest { code: code(&totp) }),
        )
        .await;
    (totp, backup.codes)
}

async fn mfa_status(s: &TestServer, token: &str) -> MfaStatus {
    s.json(Method::GET, "/account/mfa", Some(token), NOBODY)
        .await
}

/// Log in and stop at the MFA prompt.
async fn login_to_mfa(s: &TestServer, u: &User) -> (String, Vec<MfaMethod>) {
    match login(s, &u.email, &u.password).await {
        AuthResponse::MfaRequired { mfa_token, methods } => (mfa_token, methods),
        other => panic!("expected an MFA prompt, got {other:?}"),
    }
}

/// Second step of a login, resolving new-device approval afterwards.
async fn verify(
    s: &TestServer,
    u: &User,
    mfa_token: &str,
    credential: MfaCredential,
) -> Result<AuthResponse, (StatusCode, String)> {
    let resp = s
        .call(
            Method::POST,
            "/auth/mfa/verify",
            None,
            Some(&MfaVerifyRequest {
                mfa_token: mfa_token.into(),
                credential,
            }),
        )
        .await;
    let status = resp.status();
    let text = resp.text().await.expect("body");
    if !status.is_success() {
        return Err((status, text));
    }
    let parsed: AuthResponse = serde_json::from_str(&text).expect("auth response json");
    Ok(approve_device(s, &u.email, parsed).await)
}

fn session_token(resp: AuthResponse) -> String {
    match resp {
        AuthResponse::Authenticated(session) => session.token,
        other => panic!("expected an authenticated session, got {other:?}"),
    }
}

#[tokio::test]
async fn totp_enrolment_and_login() {
    let Some(s) = server().await else { return };
    let u = register(s, &unique_email("totp"), "pw-totp-12345678").await;

    let before = mfa_status(s, u.token()).await;
    assert!(!before.totp_enabled);
    assert!(before.webauthn_credentials.is_empty());
    assert_eq!(before.backup_codes_remaining, 0);
    assert!(!u.session.user.mfa_enabled);

    // Backup codes need a second factor first.
    s.expect_status(
        Method::POST,
        "/account/mfa/backup-codes",
        Some(u.token()),
        NOBODY,
        StatusCode::BAD_REQUEST,
    )
    .await;

    // Confirming without a pending setup / with a bogus code is rejected.
    s.expect_status(
        Method::POST,
        "/account/mfa/totp/confirm",
        Some(u.token()),
        Some(&TotpCodeRequest {
            code: "000000".into(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let setup: TotpSetupResponse = s
        .json(
            Method::POST,
            "/account/mfa/totp/setup",
            Some(u.token()),
            NOBODY,
        )
        .await;
    let totp = authenticator(&setup);
    let wrong = format!(
        "{:06}",
        (code(&totp).parse::<u32>().unwrap() + 1) % 1_000_000
    );
    s.expect_status(
        Method::POST,
        "/account/mfa/totp/confirm",
        Some(u.token()),
        Some(&TotpCodeRequest { code: wrong }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    // Still not enabled: a failed confirmation must not leak a partial state.
    assert!(!mfa_status(s, u.token()).await.totp_enabled);

    // Restarting setup issues a different secret; the old one is void.
    let setup2: TotpSetupResponse = s
        .json(
            Method::POST,
            "/account/mfa/totp/setup",
            Some(u.token()),
            NOBODY,
        )
        .await;
    assert_ne!(setup.secret, setup2.secret);
    s.expect_status(
        Method::POST,
        "/account/mfa/totp/confirm",
        Some(u.token()),
        Some(&TotpCodeRequest { code: code(&totp) }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let totp = authenticator(&setup2);
    let confirm_code = code(&totp);
    let backup: BackupCodes = s
        .json(
            Method::POST,
            "/account/mfa/totp/confirm",
            Some(u.token()),
            Some(&TotpCodeRequest {
                code: confirm_code.clone(),
            }),
        )
        .await;
    assert_eq!(backup.codes.len(), 10);
    assert!(
        backup
            .codes
            .iter()
            .all(|c| c.len() == 11 && c.as_bytes()[5] == b'-'),
        "backup codes: {:?}",
        backup.codes
    );

    let after = mfa_status(s, u.token()).await;
    assert!(after.totp_enabled);
    assert_eq!(after.backup_codes_remaining, 10);
    let account: serde_json::Value = s
        .json(Method::GET, "/account", Some(u.token()), NOBODY)
        .await;
    assert_eq!(account["user"]["mfa_enabled"], true);

    // Setting up again while enabled is a conflict.
    s.expect_status(
        Method::POST,
        "/account/mfa/totp/setup",
        Some(u.token()),
        NOBODY,
        StatusCode::CONFLICT,
    )
    .await;

    // Login now stops at MFA and advertises TOTP + backup codes.
    let (mfa_token, methods) = login_to_mfa(s, &u).await;
    let mut expected = vec![MfaMethod::Totp, MfaMethod::BackupCode];
    if s.mailpit.is_some() {
        expected.push(MfaMethod::Email);
    }
    assert_eq!(methods, expected);
    assert!(!methods.contains(&MfaMethod::Webauthn));

    // Wrong code, bogus token, wrong shape.
    let (st, _) = verify(
        s,
        &u,
        &mfa_token,
        MfaCredential::Totp {
            code: "000000".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = verify(
        s,
        &u,
        "not-a-real-token",
        MfaCredential::Totp { code: code(&totp) },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = verify(
        s,
        &u,
        &mfa_token,
        MfaCredential::BackupCode {
            code: "aaaaa-bbbbb".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Correct code (not the one already spent on enrolment): a session for
    // the new device.
    let current = fresh_code(&totp, &confirm_code).await;
    let session = session_token(
        verify(
            s,
            &u,
            &mfa_token,
            MfaCredential::Totp {
                code: current.clone(),
            },
        )
        .await
        .expect("totp login"),
    );
    // The flow token is single-use.
    let (st, _) = verify(
        s,
        &u,
        &mfa_token,
        MfaCredential::Totp {
            code: current.clone(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Replay: the same code is refused on a fresh login even though it is
    // still within the validity window.
    let (mfa_token, _) = login_to_mfa(s, &u).await;
    let (st, body) = verify(
        s,
        &u,
        &mfa_token,
        MfaCredential::Totp {
            code: current.clone(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED, "{body}");

    // The new session is fully usable.
    let status_via_new: MfaStatus = mfa_status(s, &session).await;
    assert!(status_via_new.totp_enabled);

    // Disable: wrong code refused, valid code accepted, backup codes wiped.
    s.expect_status(
        Method::DELETE,
        "/account/mfa/totp",
        Some(&session),
        Some(&TotpCodeRequest {
            code: "123456".into(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    // Wait for a fresh step so the code used above for login is not replayed.
    let fresh = fresh_code(&totp, &current).await;
    s.expect_status(
        Method::DELETE,
        "/account/mfa/totp",
        Some(&session),
        Some(&TotpCodeRequest { code: fresh }),
        StatusCode::NO_CONTENT,
    )
    .await;
    let off = mfa_status(s, &session).await;
    assert!(!off.totp_enabled);
    assert_eq!(off.backup_codes_remaining, 0);
    // Idempotent.
    s.expect_status(
        Method::DELETE,
        "/account/mfa/totp",
        Some(&session),
        Some(&TotpCodeRequest {
            code: "000000".into(),
        }),
        StatusCode::NO_CONTENT,
    )
    .await;
    // Plain login again.
    assert!(matches!(
        login(s, &u.email, &u.password).await,
        AuthResponse::Authenticated(_)
    ));
}

/// A TOTP code different from `used` (waits for the next 30 s step if needed).
async fn fresh_code(totp: &TOTP, used: &str) -> String {
    loop {
        let c = code(totp);
        if c != used {
            return c;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

#[tokio::test]
async fn backup_codes_are_single_use_and_regenerable() {
    let Some(s) = server().await else { return };
    let u = register(s, &unique_email("backup"), "pw-backup-1234567").await;
    let (totp, codes) = enroll_totp(s, &u).await;

    // A backup code signs in once...
    let (mfa_token, _) = login_to_mfa(s, &u).await;
    let spaced = format!(" {} ", codes[0].to_uppercase());
    let session = session_token(
        verify(
            s,
            &u,
            &mfa_token,
            MfaCredential::BackupCode { code: spaced },
        )
        .await
        .expect("backup code login (normalised input)"),
    );
    assert_eq!(mfa_status(s, &session).await.backup_codes_remaining, 9);

    // ...and never again.
    let (mfa_token, methods) = login_to_mfa(s, &u).await;
    assert!(methods.contains(&MfaMethod::BackupCode));
    let (st, _) = verify(
        s,
        &u,
        &mfa_token,
        MfaCredential::BackupCode {
            code: codes[0].clone(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Regeneration invalidates the remaining old codes.
    let regenerated: BackupCodes = s
        .json(
            Method::POST,
            "/account/mfa/backup-codes",
            Some(&session),
            NOBODY,
        )
        .await;
    assert_eq!(regenerated.codes.len(), 10);
    assert!(regenerated.codes.iter().all(|c| !codes.contains(c)));
    assert_eq!(mfa_status(s, &session).await.backup_codes_remaining, 10);
    let (st, _) = verify(
        s,
        &u,
        &mfa_token,
        MfaCredential::BackupCode {
            code: codes[1].clone(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    session_token(
        verify(
            s,
            &u,
            &mfa_token,
            MfaCredential::BackupCode {
                code: regenerated.codes[0].clone(),
            },
        )
        .await
        .expect("new backup code"),
    );

    // A backup code can also switch TOTP off (and is consumed doing so).
    s.expect_status(
        Method::DELETE,
        "/account/mfa/totp",
        Some(&session),
        Some(&TotpCodeRequest {
            code: regenerated.codes[0].clone(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        "/account/mfa/totp",
        Some(&session),
        Some(&TotpCodeRequest {
            code: regenerated.codes[1].clone(),
        }),
        StatusCode::NO_CONTENT,
    )
    .await;
    let off = mfa_status(s, &session).await;
    assert!(!off.totp_enabled);
    assert_eq!(off.backup_codes_remaining, 0);
    // No factor left: login goes straight through and the old authenticator is void.
    assert!(matches!(
        login(s, &u.email, &u.password).await,
        AuthResponse::Authenticated(_)
    ));
    drop(totp);
}

#[tokio::test]
async fn mfa_code_guessing_is_rate_limited() {
    let Some(s) = server().await else { return };
    let u = register(s, &unique_email("mfa-rl"), "pw-mfarl-1234567").await;
    let (_totp, _) = enroll_totp(s, &u).await;
    let (mfa_token, _) = login_to_mfa(s, &u).await;

    let mut statuses = Vec::new();
    for _ in 0..8 {
        let (st, _) = verify(
            s,
            &u,
            &mfa_token,
            MfaCredential::Totp {
                code: "000000".into(),
            },
        )
        .await
        .unwrap_err();
        statuses.push(st);
    }
    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "guessing never throttled: {statuses:?}"
    );
    assert_eq!(statuses.last(), Some(&StatusCode::TOO_MANY_REQUESTS));
}

#[tokio::test]
async fn webauthn_boundaries_without_an_authenticator() {
    let Some(s) = server().await else { return };
    let u = register(s, &unique_email("webauthn"), "pw-webauthn-123456").await;

    // Challenge is well-formed and scoped to the configured relying party.
    let ccr: serde_json::Value = s
        .json(
            Method::POST,
            "/account/mfa/webauthn/register/start",
            Some(u.token()),
            NOBODY,
        )
        .await;
    let pk = &ccr["publicKey"];
    assert_eq!(pk["rp"]["id"], WEBAUTHN_RP_ID);
    assert_eq!(pk["user"]["name"], u.email);
    assert!(pk["challenge"].as_str().is_some_and(|c| c.len() >= 16));

    // Bad names are rejected before the credential is even looked at.
    for name in ["", "   ", &"x".repeat(65)] {
        s.expect_status(
            Method::POST,
            "/account/mfa/webauthn/register/finish",
            Some(u.token()),
            Some(&WebauthnRegisterFinishRequest {
                name: name.into(),
                credential: serde_json::json!({}),
            }),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    // A malformed credential consumes the pending registration...
    s.expect_status(
        Method::POST,
        "/account/mfa/webauthn/register/finish",
        Some(u.token()),
        Some(&WebauthnRegisterFinishRequest {
            name: "YubiKey".into(),
            credential: serde_json::json!({ "id": "nope" }),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    // ...so a retry without a new challenge has nothing to finish.
    s.expect_status(
        Method::POST,
        "/account/mfa/webauthn/register/finish",
        Some(u.token()),
        Some(&WebauthnRegisterFinishRequest {
            name: "YubiKey".into(),
            credential: serde_json::json!({ "id": "nope" }),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;

    // A syntactically valid but forged attestation is refused by the
    // ceremony (wrong challenge/origin), never stored.
    s.json::<_, serde_json::Value>(
        Method::POST,
        "/account/mfa/webauthn/register/start",
        Some(u.token()),
        NOBODY,
    )
    .await;
    let forged = serde_json::json!({
        "id": "AAAA",
        "rawId": "AAAA",
        "type": "public-key",
        "response": {
            "attestationObject": "o2NmbXRkbm9uZWdhdHRTdG10oGhhdXRoRGF0YVgA",
            "clientDataJSON": "e30"
        },
        "extensions": {}
    });
    s.expect_status(
        Method::POST,
        "/account/mfa/webauthn/register/finish",
        Some(u.token()),
        Some(&WebauthnRegisterFinishRequest {
            name: "Forged".into(),
            credential: forged,
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let st = mfa_status(s, u.token()).await;
    assert!(st.webauthn_credentials.is_empty());
    assert_eq!(st.backup_codes_remaining, 0);

    // Deleting an unknown / someone else's credential id is a 404.
    s.expect_status(
        Method::DELETE,
        &format!("/account/mfa/webauthn/{}", Uuid::new_v4()),
        Some(u.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;

    // Login-time challenge needs a live MFA flow, and a user without keys
    // cannot be asked for one.
    s.expect_status(
        Method::POST,
        "/auth/mfa/webauthn/challenge",
        None,
        Some(&WebauthnChallengeRequest {
            mfa_token: "bogus".into(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let (_totp, _) = enroll_totp(s, &u).await;
    let (mfa_token, methods) = login_to_mfa(s, &u).await;
    assert!(!methods.contains(&MfaMethod::Webauthn));
    s.expect_status(
        Method::POST,
        "/auth/mfa/webauthn/challenge",
        None,
        Some(&WebauthnChallengeRequest {
            mfa_token: mfa_token.clone(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let (st, _) = verify(
        s,
        &u,
        &mfa_token,
        MfaCredential::Webauthn {
            credential: serde_json::json!({ "id": "nope" }),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}
