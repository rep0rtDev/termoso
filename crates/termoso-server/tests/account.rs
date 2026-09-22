//! Account lifecycle beyond plain login: recovery key, password change,
//! recovery-key rotation, email verification, new-device approval, email MFA,
//! email change and account deletion. Email-dependent parts need Mailpit.

mod common;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::*;
use reqwest::{Method, StatusCode};
use termoso_crypto::aead::{self, Aad};
use termoso_crypto::kdf::{Label, derive_key};
use termoso_crypto::keys::{KeyPair, SymmetricKey};
use termoso_crypto::opaque;
use termoso_crypto::recovery::RecoveryKey;
use termoso_proto::account::{ChangeEmailRequest, CodeRequest, SecurityEventList};
use termoso_proto::auth::{
    AuthResponse, DeviceApproveRequest, DeviceApproveResendRequest, DeviceList, MfaCredential,
    MfaMethod, MfaVerifyRequest, PasswordSetupFinishRequest, PasswordSetupStartRequest,
    PasswordSetupStartResponse, RecoveryRotate, RecoveryStartRequest, RecoveryStartResponse,
    Session, WebauthnChallengeRequest,
};
use termoso_proto::team::{CreateTeamRequest, Team};

macro_rules! server_with_mail {
    () => {
        match server().await {
            Some(s) if s.mailpit.is_some() => s,
            _ => return,
        }
    };
}

fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

fn session(resp: AuthResponse) -> Session {
    match resp {
        AuthResponse::Authenticated(s) => s,
        other => panic!("expected a session, got {other:?}"),
    }
}

/// What a client does with the private key it recovered: wrap it for the new
/// password and, optionally, for a new recovery phrase.
struct Rewrapped {
    secret: SymmetricKey,
}

impl Rewrapped {
    fn for_password(&self, export_key: &[u8]) -> String {
        let kek = derive_key(export_key, Label::AccountKek).expect("kek");
        aead::wrap_key(&kek, &Aad::account_private_key(), &self.secret).expect("wrap")
    }

    fn for_recovery(&self, recovery: &RecoveryKey) -> RecoveryRotate {
        let kek = recovery.kek().expect("recovery kek");
        RecoveryRotate {
            recovery_wrapped_private_key: aead::wrap_key(
                &kek,
                &Aad::recovery_private_key(),
                &self.secret,
            )
            .expect("wrap"),
            recovery_verifier: recovery.verifier_b64().expect("verifier"),
        }
    }
}

/// Run the OPAQUE registration half of a password change/recovery and return
/// the finished upload plus the export key (from which the new account KEK is
/// derived).
async fn opaque_new_password(
    s: &TestServer,
    email: &str,
    password: &str,
    token: Option<&str>,
    recovery_token: Option<&str>,
) -> (String, Vec<u8>) {
    let (request, state) = opaque::client_registration_start(password.as_bytes()).expect("start");
    let start: PasswordSetupStartResponse = s
        .json(
            Method::POST,
            "/auth/password/start",
            token,
            Some(&PasswordSetupStartRequest {
                recovery_token: recovery_token.map(String::from),
                opaque_request: request,
            }),
        )
        .await;
    let out = opaque::client_registration_finish(
        state,
        password.as_bytes(),
        &email.to_lowercase(),
        &start.opaque_response,
    )
    .expect("finish");
    (out.upload_b64, out.export_key.to_vec())
}

async fn security_kinds(s: &TestServer, token: &str) -> Vec<String> {
    let list: SecurityEventList = s
        .json(Method::GET, "/account/security-events", Some(token), NOBODY)
        .await;
    list.events.into_iter().map(|e| e.kind).collect()
}

#[tokio::test]
async fn recovery_phrase_restores_the_account_and_is_single_use() {
    let Some(s) = server().await else { return };
    let mut u = register(s, &unique_email("recovery"), "pw-recovery-123456").await;
    let second = session(login(s, &u.email, &u.password).await);

    // Wrong / malformed verifiers and unknown accounts all look the same.
    let other = RecoveryKey::generate();
    s.expect_status(
        Method::POST,
        "/auth/recovery/start",
        None,
        Some(&RecoveryStartRequest {
            email: u.email.clone(),
            recovery_verifier: other.verifier_b64().unwrap(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/auth/recovery/start",
        None,
        Some(&RecoveryStartRequest {
            email: unique_email("nobody"),
            recovery_verifier: u.recovery.verifier_b64().unwrap(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/auth/recovery/start",
        None,
        Some(&RecoveryStartRequest {
            email: u.email.clone(),
            recovery_verifier: b64(&[1u8; 16]),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/auth/recovery/start",
        None,
        Some(&RecoveryStartRequest {
            email: "not an email".into(),
            recovery_verifier: u.recovery.verifier_b64().unwrap(),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;

    // The phrase round-trips through its 24 words like a user would type it.
    let typed = RecoveryKey::parse(&u.recovery.phrase()).expect("phrase parses");
    let start: RecoveryStartResponse = s
        .json(
            Method::POST,
            "/auth/recovery/start",
            None,
            Some(&RecoveryStartRequest {
                email: u.email.to_uppercase(),
                recovery_verifier: typed.verifier_b64().unwrap(),
            }),
        )
        .await;
    assert_eq!(start.public_key, u.keypair.public_b64());
    let secret = aead::unwrap_key(
        &typed.kek().unwrap(),
        &Aad::recovery_private_key(),
        &start.recovery_wrapped_private_key,
    )
    .expect("recovery KEK unwraps the private key");
    let restored = KeyPair::from_secret_bytes(secret.as_bytes()).unwrap();
    assert_eq!(restored.public_b64(), start.public_key);
    let rewrap = Rewrapped { secret };

    // Recovery needs a device; malformed key material is refused before
    // anything is written.
    let new_password = "pw-recovered-654321";
    let (upload, export) =
        opaque_new_password(s, &u.email, new_password, None, Some(&start.recovery_token)).await;
    s.expect_status(
        Method::POST,
        "/auth/password/finish",
        None,
        Some(&PasswordSetupFinishRequest {
            recovery_token: Some(start.recovery_token.clone()),
            opaque_upload: upload.clone(),
            wrapped_private_key: rewrap.for_password(&export),
            new_recovery: None,
            revoke_other_sessions: true,
            device: None,
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/auth/password/finish",
        None,
        Some(&PasswordSetupFinishRequest {
            recovery_token: Some(start.recovery_token.clone()),
            opaque_upload: upload.clone(),
            wrapped_private_key: "%%not-base64%%".into(),
            new_recovery: None,
            revoke_other_sessions: true,
            device: Some(device("recovered laptop")),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/auth/password/finish",
        None,
        Some(&PasswordSetupFinishRequest {
            recovery_token: Some(start.recovery_token.clone()),
            opaque_upload: upload.clone(),
            wrapped_private_key: rewrap.for_password(&export),
            new_recovery: Some(RecoveryRotate {
                recovery_wrapped_private_key: "AAAA".into(),
                recovery_verifier: b64(&[0u8; 8]),
            }),
            revoke_other_sessions: true,
            device: Some(device("recovered laptop")),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/auth/password/finish",
        None,
        Some(&PasswordSetupFinishRequest {
            recovery_token: Some("bogus".into()),
            opaque_upload: upload.clone(),
            wrapped_private_key: rewrap.for_password(&export),
            new_recovery: None,
            revoke_other_sessions: true,
            device: Some(device("recovered laptop")),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    // Old password still works up to here.
    assert!(try_login(s, &u.email, &u.password).await.is_ok());

    // Recover with a fresh recovery phrase at the same time.
    let new_recovery = RecoveryKey::generate();
    let recovered = session(
        s.json(
            Method::POST,
            "/auth/password/finish",
            None,
            Some(&PasswordSetupFinishRequest {
                recovery_token: Some(start.recovery_token.clone()),
                opaque_upload: upload,
                wrapped_private_key: rewrap.for_password(&export),
                new_recovery: Some(rewrap.for_recovery(&new_recovery)),
                revoke_other_sessions: true,
                device: Some(device("recovered laptop")),
            }),
        )
        .await,
    );
    assert_eq!(recovered.user.id, u.id());
    assert_eq!(recovered.keys.public_key, u.keypair.public_b64());
    let kek = derive_key(&export, Label::AccountKek).unwrap();
    let unwrapped = aead::unwrap_key(
        &kek,
        &Aad::account_private_key(),
        &recovered.keys.wrapped_private_key,
    )
    .expect("new password KEK unwraps the private key");
    assert_eq!(unwrapped.as_bytes(), &u.keypair.secret_bytes());

    // Every pre-existing session is gone; the recovery token is spent.
    for old in [u.token(), second.token.as_str()] {
        s.expect_status(
            Method::GET,
            "/account",
            Some(old),
            NOBODY,
            StatusCode::UNAUTHORIZED,
        )
        .await;
    }
    s.expect_status(
        Method::POST,
        "/auth/password/start",
        None,
        Some(&PasswordSetupStartRequest {
            recovery_token: Some(start.recovery_token),
            opaque_request: "AAAA".into(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let kinds = security_kinds(s, &recovered.token).await;
    assert!(kinds.contains(&"recovery_started".into()), "{kinds:?}");
    assert!(kinds.contains(&"recovery_failed".into()), "{kinds:?}");
    assert!(kinds.contains(&"password_recovered".into()), "{kinds:?}");

    // Old password and old phrase are dead, new ones work.
    assert!(try_login(s, &u.email, &u.password).await.is_err());
    session(login(s, &u.email, new_password).await);
    s.expect_status(
        Method::POST,
        "/auth/recovery/start",
        None,
        Some(&RecoveryStartRequest {
            email: u.email.clone(),
            recovery_verifier: u.recovery.verifier_b64().unwrap(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let again: RecoveryStartResponse = s
        .json(
            Method::POST,
            "/auth/recovery/start",
            None,
            Some(&RecoveryStartRequest {
                email: u.email.clone(),
                recovery_verifier: new_recovery.verifier_b64().unwrap(),
            }),
        )
        .await;
    let secret = aead::unwrap_key(
        &new_recovery.kek().unwrap(),
        &Aad::recovery_private_key(),
        &again.recovery_wrapped_private_key,
    )
    .expect("new recovery KEK unwraps the private key");
    assert_eq!(secret.as_bytes(), &u.keypair.secret_bytes());
    u.password = new_password.into();
    u.recovery = new_recovery;
}

#[tokio::test]
async fn authenticated_password_change_and_recovery_rotation() {
    let Some(s) = server().await else { return };
    let u = register(s, &unique_email("pwchange"), "pw-change-1234567").await;
    let other = session(login(s, &u.email, &u.password).await);
    let secret = SymmetricKey::from_bytes(u.keypair.secret_bytes());
    let rewrap = Rewrapped { secret };

    // Unauthenticated and without a recovery token there is no subject.
    s.expect_status(
        Method::POST,
        "/auth/password/start",
        None,
        Some(&PasswordSetupStartRequest {
            recovery_token: None,
            opaque_request: "AAAA".into(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;

    // Change password. Asking to keep other devices signed in is ignored: a
    // new password always signs everyone else out.
    let pw2 = "pw-change-second-1";
    let (upload, export) = opaque_new_password(s, &u.email, pw2, Some(u.token()), None).await;
    let fresh = session(
        s.json(
            Method::POST,
            "/auth/password/finish",
            Some(u.token()),
            Some(&PasswordSetupFinishRequest {
                recovery_token: None,
                opaque_upload: upload,
                wrapped_private_key: rewrap.for_password(&export),
                new_recovery: None,
                revoke_other_sessions: false,
                device: None,
            }),
        )
        .await,
    );
    assert_eq!(fresh.device_id, u.session.device_id);
    s.expect_status(
        Method::GET,
        "/account",
        Some(u.token()),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    s.expect_status(
        Method::GET,
        "/account",
        Some(&other.token),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let devices: DeviceList = s
        .json(Method::GET, "/account/devices", Some(&fresh.token), NOBODY)
        .await;
    assert_eq!(devices.devices.len(), 1);
    assert!(devices.devices[0].current);
    assert!(try_login(s, &u.email, &u.password).await.is_err());
    session(login(s, &u.email, pw2).await);

    let pw3 = "pw-change-third-12";
    let (upload, export) = opaque_new_password(s, &u.email, pw3, Some(&fresh.token), None).await;
    let fresh2 = session(
        s.json(
            Method::POST,
            "/auth/password/finish",
            Some(&fresh.token),
            Some(&PasswordSetupFinishRequest {
                recovery_token: None,
                opaque_upload: upload,
                wrapped_private_key: rewrap.for_password(&export),
                new_recovery: None,
                revoke_other_sessions: true,
                device: None,
            }),
        )
        .await,
    );
    assert!(try_login(s, &u.email, pw2).await.is_err());

    // Rotate the recovery phrase on its own.
    s.expect_status(
        Method::POST,
        "/account/recovery/rotate",
        Some(&fresh2.token),
        Some(&RecoveryRotate {
            recovery_wrapped_private_key: "AAAA".into(),
            recovery_verifier: b64(&[7u8; 31]),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/account/recovery/rotate",
        Some(&fresh2.token),
        Some(&RecoveryRotate {
            recovery_wrapped_private_key: "*not base64*".into(),
            recovery_verifier: b64(&[7u8; 32]),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let rotated = RecoveryKey::generate();
    s.expect_status(
        Method::POST,
        "/account/recovery/rotate",
        Some(&fresh2.token),
        Some(&rewrap.for_recovery(&rotated)),
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/auth/recovery/start",
        None,
        Some(&RecoveryStartRequest {
            email: u.email.clone(),
            recovery_verifier: u.recovery.verifier_b64().unwrap(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let start: RecoveryStartResponse = s
        .json(
            Method::POST,
            "/auth/recovery/start",
            None,
            Some(&RecoveryStartRequest {
                email: u.email.clone(),
                recovery_verifier: rotated.verifier_b64().unwrap(),
            }),
        )
        .await;
    let secret = aead::unwrap_key(
        &rotated.kek().unwrap(),
        &Aad::recovery_private_key(),
        &start.recovery_wrapped_private_key,
    )
    .expect("rotated phrase unwraps the private key");
    assert_eq!(secret.as_bytes(), &u.keypair.secret_bytes());
    let kinds = security_kinds(s, &fresh2.token).await;
    assert!(kinds.contains(&"password_changed".into()), "{kinds:?}");
    assert!(kinds.contains(&"recovery_key_rotated".into()), "{kinds:?}");
}

#[tokio::test]
async fn email_verification_gates_features() {
    let s = server_with_mail!();
    let email = unique_email("verify");
    let mut u = register_unverified(s, &email, "pw-verify-1234567", None).await;
    assert!(!u.session.user.email_verified);

    // Unverified: no teams, no email MFA, no new-device approval prompts.
    s.expect_status(
        Method::POST,
        "/teams",
        Some(u.token()),
        Some(&CreateTeamRequest {
            name: "Nope".into(),
        }),
        StatusCode::FORBIDDEN,
    )
    .await;
    assert!(matches!(
        login_raw(s, &email, &u.password, device("unverified device"), None)
            .await
            .unwrap(),
        AuthResponse::Authenticated(_)
    ));

    // Wrong code, then the real one (sent at registration); codes are single-use.
    s.expect_status(
        Method::POST,
        "/account/email/verify/confirm",
        Some(u.token()),
        Some(&CodeRequest {
            code: "000000".into(),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    // Re-sending replaces the code.
    s.expect_status(
        Method::POST,
        "/account/email/verify/send",
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    assert!(s.email_count(&email).await >= 2);
    verify_email(s, &mut u).await;
    assert!(u.session.user.email_verified);
    s.expect_status(
        Method::POST,
        "/account/email/verify/confirm",
        Some(u.token()),
        Some(&CodeRequest {
            code: "000000".into(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    // Already verified: sending again is a no-op.
    let before = s.email_count(&email).await;
    s.expect_status(
        Method::POST,
        "/account/email/verify/send",
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    assert_eq!(s.email_count(&email).await, before);

    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(u.token()),
            Some(&CreateTeamRequest {
                name: "Verified".into(),
            }),
        )
        .await;
    assert_eq!(team.name, "Verified");
    let kinds = security_kinds(s, u.token()).await;
    assert!(kinds.contains(&"email_verified".into()), "{kinds:?}");
}

#[tokio::test]
async fn new_devices_need_an_emailed_approval() {
    let s = server_with_mail!();
    let email = unique_email("approve");
    let u = register(s, &email, "pw-approve-1234567").await;

    // Registration device is trusted: no prompt for it.
    let same = login_raw(
        s,
        &email,
        &u.password,
        device_with_id(u.session.device_id),
        None,
    )
    .await
    .unwrap();
    assert!(matches!(same, AuthResponse::Authenticated(_)));

    let AuthResponse::DeviceApprovalRequired {
        approval_token,
        email_hint,
    } = login_raw(s, &email, &u.password, device("new phone"), None)
        .await
        .unwrap()
    else {
        panic!("expected device approval")
    };
    assert_eq!(
        email_hint,
        format!("a***@{}", email.split_once('@').unwrap().1)
    );
    let first_code = s.emailed_code(&email, "new device sign-in").await;

    // Wrong code / bogus token.
    s.expect_status(
        Method::POST,
        "/auth/device/approve",
        None,
        Some(&DeviceApproveRequest {
            approval_token: approval_token.clone(),
            code: "000000".into(),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/auth/device/approve",
        None,
        Some(&DeviceApproveRequest {
            approval_token: "bogus".into(),
            code: first_code.clone(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/auth/device/approve/resend",
        None,
        Some(&DeviceApproveResendRequest {
            approval_token: "bogus".into(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;

    // Resend issues a new code and voids the old one.
    let before = s.email_count(&email).await;
    s.expect_status(
        Method::POST,
        "/auth/device/approve/resend",
        None,
        Some(&DeviceApproveResendRequest {
            approval_token: approval_token.clone(),
        }),
        StatusCode::NO_CONTENT,
    )
    .await;
    let second_code = wait_for_new_code(s, &email, "new device sign-in", before).await;
    assert_ne!(first_code, second_code);
    s.expect_status(
        Method::POST,
        "/auth/device/approve",
        None,
        Some(&DeviceApproveRequest {
            approval_token: approval_token.clone(),
            code: first_code,
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let approved = session(
        s.json(
            Method::POST,
            "/auth/device/approve",
            None,
            Some(&DeviceApproveRequest {
                approval_token: approval_token.clone(),
                code: second_code.clone(),
            }),
        )
        .await,
    );
    // Consumed.
    s.expect_status(
        Method::POST,
        "/auth/device/approve",
        None,
        Some(&DeviceApproveRequest {
            approval_token,
            code: second_code,
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;

    // The approved device is remembered.
    let again = login_raw(
        s,
        &email,
        &u.password,
        device_with_id(approved.device_id),
        None,
    )
    .await
    .unwrap();
    assert!(matches!(again, AuthResponse::Authenticated(_)));
    let kinds = security_kinds(s, &approved.token).await;
    assert!(
        kinds.contains(&"device_approval_requested".into()),
        "{kinds:?}"
    );
    assert!(kinds.contains(&"device_approved".into()), "{kinds:?}");

    // Revoking a device makes it a stranger again.
    s.expect_status(
        Method::DELETE,
        &format!("/account/devices/{}", approved.device_id),
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::GET,
        "/account",
        Some(&approved.token),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
}

/// Poll Mailpit until more than `before` messages exist for `to`, then return
/// the newest code for `purpose`.
async fn wait_for_new_code(s: &TestServer, to: &str, purpose: &str, before: u64) -> String {
    for _ in 0..50 {
        if s.email_count(to).await > before {
            return s.emailed_code(to, purpose).await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("no new `{purpose}` email for {to}");
}

fn device_with_id(id: uuid::Uuid) -> termoso_proto::auth::DeviceInfo {
    let mut d = device("known device");
    d.client_device_id = Some(id);
    d
}

/// One installation keeps a single client device id across sign-outs; a second
/// account signing in from it must get its own device record, not a 500.
#[tokio::test]
async fn shared_installation_gets_a_device_per_account() {
    let s = server_with_mail!();
    let a = register(s, &unique_email("shared-a"), "pw-shared-a-123456").await;
    let b_email = unique_email("shared-b");
    let b = register(s, &b_email, "pw-shared-b-123456").await;

    let resp = login_raw(
        s,
        &b_email,
        &b.password,
        device_with_id(a.session.device_id),
        None,
    )
    .await
    .unwrap();
    assert!(
        matches!(resp, AuthResponse::DeviceApprovalRequired { .. }),
        "{resp:?}"
    );
    let b2 = session(approve_device(s, &b_email, resp).await);
    assert_ne!(b2.device_id, a.session.device_id);

    let a_devices: DeviceList = s
        .json(Method::GET, "/account/devices", Some(a.token()), NOBODY)
        .await;
    assert_eq!(a_devices.devices.len(), 1);
    let b_devices: DeviceList = s
        .json(Method::GET, "/account/devices", Some(&b2.token), NOBODY)
        .await;
    assert_eq!(b_devices.devices.len(), 2);
}

#[tokio::test]
async fn email_is_a_second_factor_once_another_is_enabled() {
    let s = server_with_mail!();
    let email = unique_email("emailmfa");
    let u = register(s, &email, "pw-emailmfa-123456").await;

    // Without MFA there is no flow to send a code for.
    s.expect_status(
        Method::POST,
        "/auth/mfa/email/send",
        None,
        Some(&WebauthnChallengeRequest {
            mfa_token: "bogus".into(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;

    // Enable TOTP so login stops at MFA.
    let setup: termoso_proto::auth::TotpSetupResponse = s
        .json(
            Method::POST,
            "/account/mfa/totp/setup",
            Some(u.token()),
            NOBODY,
        )
        .await;
    let totp = totp_rs::Totp::from_url(&setup.otpauth_url).unwrap();
    s.json::<_, termoso_proto::auth::BackupCodes>(
        Method::POST,
        "/account/mfa/totp/confirm",
        Some(u.token()),
        Some(&termoso_proto::auth::TotpCodeRequest {
            code: totp.generate_current().to_string(),
        }),
    )
    .await;

    let AuthResponse::MfaRequired { mfa_token, methods } =
        login_raw(s, &email, &u.password, device("mail device"), None)
            .await
            .unwrap()
    else {
        panic!("expected MFA")
    };
    assert!(methods.contains(&MfaMethod::Email));

    // A code must be requested first.
    s.expect_status(
        Method::POST,
        "/auth/mfa/verify",
        None,
        Some(&MfaVerifyRequest {
            mfa_token: mfa_token.clone(),
            credential: MfaCredential::Email {
                code: "000000".into(),
            },
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let before = s.email_count(&email).await;
    s.expect_status(
        Method::POST,
        "/auth/mfa/email/send",
        None,
        Some(&WebauthnChallengeRequest {
            mfa_token: mfa_token.clone(),
        }),
        StatusCode::NO_CONTENT,
    )
    .await;
    let code = wait_for_new_code(s, &email, "sign-in verification", before).await;
    s.expect_status(
        Method::POST,
        "/auth/mfa/verify",
        None,
        Some(&MfaVerifyRequest {
            mfa_token: mfa_token.clone(),
            credential: MfaCredential::Email {
                code: "000000".into(),
            },
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let resp: AuthResponse = s
        .json(
            Method::POST,
            "/auth/mfa/verify",
            None,
            Some(&MfaVerifyRequest {
                mfa_token: mfa_token.clone(),
                credential: MfaCredential::Email { code: code.clone() },
            }),
        )
        .await;
    // MFA passed; the device is still new, so approval follows (verified email).
    assert!(matches!(resp, AuthResponse::DeviceApprovalRequired { .. }));
    session(approve_device(s, &email, resp).await);
    // The email code is gone with the flow.
    s.expect_status(
        Method::POST,
        "/auth/mfa/verify",
        None,
        Some(&MfaVerifyRequest {
            mfa_token,
            credential: MfaCredential::Email { code },
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
}

#[tokio::test]
async fn email_change_requires_confirmation_and_password_reset() {
    let s = server_with_mail!();
    let email = unique_email("change");
    let u = register(s, &email, "pw-emailchange-1234").await;
    let taken = register(s, &unique_email("taken"), "pw-taken-12345678").await;

    s.expect_status(
        Method::POST,
        "/account/email/change",
        Some(u.token()),
        Some(&ChangeEmailRequest {
            new_email: taken.email.clone(),
        }),
        StatusCode::CONFLICT,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/account/email/change",
        Some(u.token()),
        Some(&ChangeEmailRequest {
            new_email: "garbage".into(),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;

    let new_email = unique_email("changed");
    s.expect_status(
        Method::POST,
        "/account/email/change",
        Some(u.token()),
        Some(&ChangeEmailRequest {
            new_email: new_email.clone(),
        }),
        StatusCode::NO_CONTENT,
    )
    .await;
    let code = s.emailed_code(&new_email, "email change").await;
    s.expect_status(
        Method::POST,
        "/account/email/change/confirm",
        Some(u.token()),
        Some(&CodeRequest {
            code: "000000".into(),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    s.expect_status(
        Method::POST,
        "/account/email/change/confirm",
        Some(u.token()),
        Some(&CodeRequest { code }),
        StatusCode::NO_CONTENT,
    )
    .await;
    // Old address is told; new address is verified (it just proved receipt).
    assert!(s.email_count(&email).await >= 2);
    let account: serde_json::Value = s
        .json(Method::GET, "/account", Some(u.token()), NOBODY)
        .await;
    assert_eq!(account["user"]["email"], new_email);
    assert_eq!(account["user"]["email_verified"], true);

    // The OPAQUE record was bound to the old address: password login is off
    // until the client re-registers its password while still signed in.
    assert!(try_login(s, &new_email, &u.password).await.is_err());
    let rewrap = Rewrapped {
        secret: SymmetricKey::from_bytes(u.keypair.secret_bytes()),
    };
    let (upload, export) =
        opaque_new_password(s, &new_email, &u.password, Some(u.token()), None).await;
    session(
        s.json(
            Method::POST,
            "/auth/password/finish",
            Some(u.token()),
            Some(&PasswordSetupFinishRequest {
                recovery_token: None,
                opaque_upload: upload,
                wrapped_private_key: rewrap.for_password(&export),
                new_recovery: None,
                revoke_other_sessions: false,
                device: None,
            }),
        )
        .await,
    );
    session(login(s, &new_email, &u.password).await);
    assert!(try_login(s, &email, &u.password).await.is_err());
}

#[tokio::test]
async fn account_deletion_is_confirmed_by_email() {
    let s = server_with_mail!();
    let email = unique_email("delete");
    let u = register(s, &email, "pw-delete-12345678").await;
    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(u.token()),
            Some(&CreateTeamRequest {
                name: "Owned".into(),
            }),
        )
        .await;

    // First call only sends the code.
    s.expect_status(
        Method::DELETE,
        "/account",
        Some(u.token()),
        NOBODY,
        StatusCode::ACCEPTED,
    )
    .await;
    let code = s.emailed_code(&email, "account deletion").await;
    s.expect_status(
        Method::DELETE,
        "/account",
        Some(u.token()),
        Some(&CodeRequest {
            code: "000000".into(),
        }),
        StatusCode::BAD_REQUEST,
    )
    .await;
    // Right code, but the user still owns a team: nothing is deleted and the
    // code is spent, so a new one is needed.
    s.expect_status(
        Method::DELETE,
        "/account",
        Some(u.token()),
        Some(&CodeRequest { code: code.clone() }),
        StatusCode::CONFLICT,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &format!("/teams/{}", team.id),
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        "/account",
        Some(u.token()),
        Some(&CodeRequest { code }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let before = s.email_count(&email).await;
    s.expect_status(
        Method::DELETE,
        "/account",
        Some(u.token()),
        NOBODY,
        StatusCode::ACCEPTED,
    )
    .await;
    let code = wait_for_new_code(s, &email, "account deletion", before).await;
    s.expect_status(
        Method::DELETE,
        "/account",
        Some(u.token()),
        Some(&CodeRequest { code }),
        StatusCode::NO_CONTENT,
    )
    .await;

    // Gone: token dead, login impossible, recovery impossible, email free again.
    s.expect_status(
        Method::GET,
        "/account",
        Some(u.token()),
        NOBODY,
        StatusCode::UNAUTHORIZED,
    )
    .await;
    assert!(try_login(s, &email, &u.password).await.is_err());
    s.expect_status(
        Method::POST,
        "/auth/recovery/start",
        None,
        Some(&RecoveryStartRequest {
            email: email.clone(),
            recovery_verifier: u.recovery.verifier_b64().unwrap(),
        }),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    register(s, &email, "pw-delete-again-1234").await;
}
