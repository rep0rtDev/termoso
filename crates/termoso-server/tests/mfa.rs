//! Second factors: TOTP enrolment and login, replay protection, backup codes,
//! WebAuthn boundaries (no authenticator available in CI, so only the parts
//! that do not need one).

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_core::fido2::Fido2Error;
use termoso_core::fido2::ctap::Authenticator;
use termoso_core::fido2::soft::{Config, SoftToken};
use termoso_core::fido2::webauthn::{self, CreationOptions, RequestOptions};
use termoso_proto::auth::WebauthnCredentialInfo;
use termoso_proto::auth::{
    AuthResponse, BackupCodes, MfaCredential, MfaMethod, MfaStatus, MfaVerifyRequest,
    TotpCodeRequest, TotpSetupResponse, WebauthnChallengeRequest, WebauthnRegisterFinishRequest,
};
use totp_rs::{Algorithm, Secret, Totp};
use uuid::Uuid;

/// Authenticator side of TOTP, built from the enrolment response.
fn authenticator(setup: &TotpSetupResponse) -> Totp {
    let from_url = Totp::from_url(&setup.otpauth_url).expect("otpauth url parses");
    let secret = Secret::try_from_base32(&setup.secret).expect("base32 secret");
    assert_eq!(
        from_url.secret().as_bytes(),
        secret.as_bytes(),
        "secret and otpauth url must describe the same key"
    );
    assert_eq!(
        (from_url.algorithm(), from_url.digits(), from_url.step()),
        (Algorithm::SHA1, 6, 30)
    );
    from_url
}

fn code(totp: &Totp) -> String {
    totp.generate_current().to_string()
}

/// Enrol TOTP for `u`; returns the authenticator and the fresh backup codes.
async fn enroll_totp(s: &TestServer, u: &User) -> (Totp, Vec<String>) {
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
async fn fresh_code(totp: &Totp, used: &str) -> String {
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

const SOFT_PIN: &str = "123456";

fn soft_token() -> (SoftToken, Authenticator<termoso_core::fido2::soft::Direct>) {
    let token = SoftToken::new(Config {
        pin: Some(SOFT_PIN.into()),
        ..Config::default()
    });
    let auth = Authenticator::open(token.direct()).expect("soft token opens");
    (token, auth)
}

async fn registration_options(s: &TestServer, token: &str) -> CreationOptions {
    let ccr: serde_json::Value = s
        .json(
            Method::POST,
            "/account/mfa/webauthn/register/start",
            Some(token),
            NOBODY,
        )
        .await;
    CreationOptions::parse(&ccr).expect("creation options parse")
}

async fn assertion_options(s: &TestServer, mfa_token: &str) -> RequestOptions {
    let rcr: serde_json::Value = s
        .json(
            Method::POST,
            "/auth/mfa/webauthn/challenge",
            None,
            Some(&WebauthnChallengeRequest {
                mfa_token: mfa_token.into(),
            }),
        )
        .await;
    RequestOptions::parse(&rcr).expect("request options parse")
}

/// The whole security-key life cycle the way the mobile client drives it:
/// a CTAP2 authenticator answers the server's WebAuthn challenges through
/// `termoso_core::fido2::webauthn`, never through a browser.
#[tokio::test]
async fn webauthn_security_key_register_login_and_delete() {
    let Some(s) = server().await else { return };
    let u = register(s, &unique_email("skey"), "pw-skey-123456789").await;
    let (token, mut auth) = soft_token();

    // Registration minted for a foreign origin signs the wrong client data
    // and is refused; nothing is stored.
    let options = registration_options(s, u.token()).await;
    assert_eq!(options.rp_id("http://ignored").unwrap(), WEBAUTHN_RP_ID);
    assert!(options.requires_user_verification());
    let foreign = webauthn::register(
        &mut auth,
        &options,
        "https://evil.example",
        "usb",
        Some(SOFT_PIN),
    )
    .expect("token signs whatever client data it is given");
    s.expect_status(
        Method::POST,
        "/account/mfa/webauthn/register/finish",
        Some(u.token()),
        Some(&WebauthnRegisterFinishRequest {
            name: "Evil".into(),
            credential: foreign,
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert!(
        mfa_status(s, u.token())
            .await
            .webauthn_credentials
            .is_empty()
    );

    // The relying party demands user verification, so a token with a PIN
    // refuses to mint without it (typed, before any I/O to the server).
    let options = registration_options(s, u.token()).await;
    assert!(matches!(
        webauthn::register(&mut auth, &options, WEBAUTHN_ORIGIN, "usb", None),
        Err(Fido2Error::PinRequired)
    ));

    // Proper ceremony: registration adds the first second factor, which
    // also mints backup codes.
    let credential =
        webauthn::register(&mut auth, &options, WEBAUTHN_ORIGIN, "usb", Some(SOFT_PIN))
            .expect("register");
    assert_eq!(credential["type"], "public-key");
    assert_eq!(credential["id"], credential["rawId"]);
    assert_eq!(
        credential["response"]["transports"],
        serde_json::json!(["usb"])
    );
    let info: WebauthnCredentialInfo = s
        .json(
            Method::POST,
            "/account/mfa/webauthn/register/finish",
            Some(u.token()),
            Some(&WebauthnRegisterFinishRequest {
                name: "  Soft key  ".into(),
                credential,
            }),
        )
        .await;
    assert_eq!(info.name, "Soft key");
    assert!(info.last_used_at.is_none());
    assert_eq!(token.credential_count(), 2, "one foreign, one real");
    let st = mfa_status(s, u.token()).await;
    assert_eq!(st.webauthn_credentials.len(), 1);
    assert_eq!(st.webauthn_credentials[0].id, info.id);
    assert!(st.backup_codes_remaining > 0);
    assert!(!st.totp_enabled);

    // A second registration must exclude the existing credential so the
    // same token cannot be enrolled twice.
    let again = registration_options(s, u.token()).await;
    let err = webauthn::register(&mut auth, &again, WEBAUTHN_ORIGIN, "usb", Some(SOFT_PIN))
        .expect_err("excluded credential");
    assert_eq!(
        err,
        Fido2Error::Other("this device is already registered".into())
    );

    // Login now stops at MFA and offers the key.
    let (mfa_token, methods) = login_to_mfa(s, &u).await;
    assert!(methods.contains(&MfaMethod::Webauthn));
    let request = assertion_options(s, &mfa_token).await;
    assert_eq!(request.rp_id(), WEBAUTHN_RP_ID);
    assert_eq!(request.allowed_credentials().len(), 1);
    assert!(request.requires_user_verification());

    // No PIN → no user verification → the token refuses before signing.
    assert!(matches!(
        webauthn::assert(&mut auth, &request, WEBAUTHN_ORIGIN, None),
        Err(Fido2Error::PinRequired)
    ));

    // Wrong origin: valid signature over the wrong client data → 401.
    let wrong = webauthn::assert(&mut auth, &request, "https://evil.example", Some(SOFT_PIN))
        .expect("token signs");
    let (st, _) = verify(
        s,
        &u,
        &mfa_token,
        MfaCredential::Webauthn { credential: wrong },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // A different token knows nothing about this relying party.
    let (_, mut stranger) = soft_token();
    assert!(matches!(
        webauthn::assert(&mut stranger, &request, WEBAUTHN_ORIGIN, Some(SOFT_PIN)),
        Err(Fido2Error::WrongDevice)
    ));

    // The failed attempt consumed the challenge; ask for a fresh one and
    // answer it properly.
    let request = assertion_options(s, &mfa_token).await;
    let good =
        webauthn::assert(&mut auth, &request, WEBAUTHN_ORIGIN, Some(SOFT_PIN)).expect("assert");
    assert_eq!(good["type"], "public-key");
    assert!(good["response"]["userHandle"].is_null() || good["response"]["userHandle"].is_string());
    let session = session_token(
        verify(
            s,
            &u,
            &mfa_token,
            MfaCredential::Webauthn {
                credential: good.clone(),
            },
        )
        .await
        .expect("security key signs in"),
    );
    let st = mfa_status(s, &session).await;
    assert!(st.webauthn_credentials[0].last_used_at.is_some());

    // Replaying the assertion against a new challenge fails: the signed
    // client data carries the old challenge.
    let (mfa_token, _) = login_to_mfa(s, &u).await;
    let _fresh = assertion_options(s, &mfa_token).await;
    let (st, _) = verify(
        s,
        &u,
        &mfa_token,
        MfaCredential::Webauthn { credential: good },
    )
    .await
    .unwrap_err();
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Removing the only second factor drops the backup codes with it and
    // makes the next login single-step again.
    s.expect_status(
        Method::DELETE,
        &format!("/account/mfa/webauthn/{}", info.id),
        Some(&session),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    let st = mfa_status(s, &session).await;
    assert!(st.webauthn_credentials.is_empty());
    assert_eq!(st.backup_codes_remaining, 0);
    assert!(matches!(
        login(s, &u.email, &u.password).await,
        AuthResponse::Authenticated(_)
    ));
}

/// Team admins see which members have a second factor; regular members only
/// see their own status.
#[tokio::test]
async fn team_members_expose_mfa_status_to_admins_only() {
    use termoso_proto::team::{
        CreateInviteRequest, CreateTeamRequest, Team, TeamMemberList, TeamRole,
    };

    let Some(s) = server().await else { return };
    let owner = register(s, &unique_email("mfa-owner"), "pw-owner-1234567").await;
    let bob = register(s, &unique_email("mfa-bob"), "pw-bob-123456789").await;
    let carol = register(s, &unique_email("mfa-carol"), "pw-carol-12345678").await;
    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&CreateTeamRequest { name: "Sec".into() }),
        )
        .await;
    for who in [&bob, &carol] {
        let invite: serde_json::Value = s
            .json(
                Method::POST,
                &format!("/teams/{}/invites", team.id),
                Some(owner.token()),
                Some(&CreateInviteRequest {
                    email: who.email.clone(),
                    role: TeamRole::Member,
                    vault_ids: vec![],
                }),
            )
            .await;
        let token = invite["url"].as_str().unwrap().rsplit('/').next().unwrap();
        let _: Team = s
            .json(
                Method::POST,
                &format!("/invites/{token}/accept"),
                Some(who.token()),
                NOBODY,
            )
            .await;
    }
    enroll_totp(s, &bob).await;

    let members = |token: &str| {
        let path = format!("/teams/{}/members", team.id);
        let token = token.to_string();
        async move {
            let list: TeamMemberList = s.json(Method::GET, &path, Some(&token), NOBODY).await;
            let mut v: Vec<(Uuid, Option<bool>)> = list
                .members
                .into_iter()
                .map(|m| (m.user_id, m.mfa_enabled))
                .collect();
            v.sort();
            v
        }
    };
    let mut expected_admin = vec![
        (owner.id(), Some(false)),
        (bob.id(), Some(true)),
        (carol.id(), Some(false)),
    ];
    expected_admin.sort();
    assert_eq!(members(owner.token()).await, expected_admin);

    // A plain member learns only about themself.
    let mut expected_carol = vec![
        (owner.id(), None),
        (bob.id(), None),
        (carol.id(), Some(false)),
    ];
    expected_carol.sort();
    assert_eq!(members(carol.token()).await, expected_carol);

    // Promoting Carol to admin reveals the rest; the security key path counts too.
    s.expect_status(
        Method::PATCH,
        &format!("/teams/{}/members/{}", team.id, carol.id()),
        Some(owner.token()),
        Some(&termoso_proto::team::UpdateTeamMemberRequest {
            role: TeamRole::Admin,
        }),
        StatusCode::NO_CONTENT,
    )
    .await;
    assert_eq!(members(carol.token()).await, expected_admin);
}
