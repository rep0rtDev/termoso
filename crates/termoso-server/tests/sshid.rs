//! SSH ID: handle lifecycle, device keys, FIDO2 keys, public listing and
//! cleanup on logout / device revoke.

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_crypto::encoding::b64;
use termoso_proto::auth::AuthResponse;
use termoso_proto::sshid::*;

macro_rules! server {
    () => {
        match server().await {
            Some(s) => s,
            None => return,
        }
    };
}

/// A syntactically valid `<alg> <base64>` line whose blob starts with the
/// algorithm name, as OpenSSH encodes it. `seed` makes it unique.
fn fake_key(t: SshIdKeyType, seed: u8) -> String {
    let alg = t.wire_name();
    let mut blob = Vec::new();
    blob.extend_from_slice(&(alg.len() as u32).to_be_bytes());
    blob.extend_from_slice(alg.as_bytes());
    blob.extend_from_slice(&32u32.to_be_bytes());
    blob.extend_from_slice(&[seed; 32]);
    format!("{alg} {} comment", b64(&blob))
}

async fn public(s: &TestServer, path: &str) -> (StatusCode, String) {
    let resp = s
        .http()
        .get(format!("http://{}{path}", s.addr))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    (status, resp.text().await.unwrap())
}

#[tokio::test]
async fn handle_lifecycle_and_public_listing() {
    let s = server!();
    let u = register(s, &unique_email("sshid"), "pw-sshid-1234567").await;
    let handle = format!("h{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);

    let none: Option<SshIdProfile> = s
        .json(Method::GET, "/account/sshid", Some(u.token()), NOBODY)
        .await;
    assert!(none.is_none());

    // Bad handles.
    for bad in ["ab", "-abc", "with space", "admin"] {
        let resp = s
            .call(
                Method::POST,
                "/account/sshid",
                Some(u.token()),
                Some(&CreateSshIdRequest { handle: bad.into() }),
            )
            .await;
        assert!(resp.status().is_client_error(), "{bad}");
    }

    let p: SshIdProfile = s
        .json(
            Method::POST,
            "/account/sshid",
            Some(u.token()),
            Some(&CreateSshIdRequest {
                handle: format!(" @{} ", handle.to_uppercase()),
            }),
        )
        .await;
    assert_eq!(p.handle, handle);
    assert!(p.url.ends_with(&format!("/sshid/{handle}")));
    assert!(p.keys.is_empty());

    // Taken by someone else.
    let other = register(s, &unique_email("sshid2"), "pw-sshid-1234567").await;
    s.expect_status(
        Method::POST,
        "/account/sshid",
        Some(other.token()),
        Some(&CreateSshIdRequest {
            handle: handle.clone(),
        }),
        StatusCode::CONFLICT,
    )
    .await;

    // Publish device keys.
    let ed = fake_key(SshIdKeyType::Ed25519, 1);
    let ec = fake_key(SshIdKeyType::Ecdsa, 2);
    let p: SshIdProfile = s
        .json(
            Method::PUT,
            "/account/sshid/keys/device",
            Some(u.token()),
            Some(&PutDeviceKeysRequest {
                keys: vec![
                    DeviceKeyUpload {
                        key_type: SshIdKeyType::Ed25519,
                        public_key: ed.clone(),
                    },
                    DeviceKeyUpload {
                        key_type: SshIdKeyType::Ecdsa,
                        public_key: ec.clone(),
                    },
                ],
            }),
        )
        .await;
    assert_eq!(p.keys.len(), 2);
    assert!(
        p.keys
            .iter()
            .all(|k| k.current_device && k.device_id.is_some())
    );
    assert!(p.keys.iter().all(|k| !k.public_key.contains("comment")));

    // Wrong algorithm in the line, hardware type via the device route.
    let resp = s
        .call(
            Method::PUT,
            "/account/sshid/keys/device",
            Some(u.token()),
            Some(&PutDeviceKeysRequest {
                keys: vec![DeviceKeyUpload {
                    key_type: SshIdKeyType::Rsa,
                    public_key: ed.clone(),
                }],
            }),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = s
        .call(
            Method::PUT,
            "/account/sshid/keys/device",
            Some(u.token()),
            Some(&PutDeviceKeysRequest {
                keys: vec![DeviceKeyUpload {
                    key_type: SshIdKeyType::EcdsaSk,
                    public_key: fake_key(SshIdKeyType::EcdsaSk, 3),
                }],
            }),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // FIDO2 key.
    let sk = fake_key(SshIdKeyType::EcdsaSk, 4);
    let k: SshIdKey = s
        .json(
            Method::POST,
            "/account/sshid/keys/fido2",
            Some(u.token()),
            Some(&AddFido2KeyRequest {
                label: "YubiKey".into(),
                key_type: SshIdKeyType::EcdsaSk,
                public_key: sk.clone(),
            }),
        )
        .await;
    assert!(k.device_id.is_none() && k.label == "YubiKey");
    let resp = s
        .call(
            Method::POST,
            "/account/sshid/keys/fido2",
            Some(u.token()),
            Some(&AddFido2KeyRequest {
                label: "soft".into(),
                key_type: SshIdKeyType::Ed25519,
                public_key: fake_key(SshIdKeyType::Ed25519, 5),
            }),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Public listing: default = ED25519 only; typed; all.
    let (st, body) = public(s, &format!("/sshid/{handle}")).await;
    assert_eq!(st, StatusCode::OK);
    let ed_line = ed.rsplit_once(' ').unwrap().0;
    assert_eq!(body.trim(), format!("{ed_line} #SSH ID - @{handle}"));
    let (_, body) = public(s, &format!("/sshid/{}/ecdsa-sk", handle.to_uppercase())).await;
    assert_eq!(body.lines().count(), 1);
    assert!(body.starts_with(sk.rsplit_once(' ').unwrap().0));
    let (_, body) = public(s, &format!("/sshid/{handle}/all")).await;
    assert_eq!(body.lines().count(), 3);
    let (st, _) = public(s, &format!("/sshid/{handle}/dsa")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = public(s, "/sshid/nobody-here-000").await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // Replacing the set drops types not listed.
    let p: SshIdProfile = s
        .json(
            Method::PUT,
            "/account/sshid/keys/device",
            Some(u.token()),
            Some(&PutDeviceKeysRequest {
                keys: vec![DeviceKeyUpload {
                    key_type: SshIdKeyType::Ed25519,
                    public_key: ed.clone(),
                }],
            }),
        )
        .await;
    assert_eq!(p.keys.len(), 2); // ed25519 + fido2

    // Remove the FIDO2 key.
    let resp = s
        .call(
            Method::DELETE,
            &format!("/account/sshid/keys/{}", k.id),
            Some(u.token()),
            NOBODY,
        )
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let (_, body) = public(s, &format!("/sshid/{handle}/all")).await;
    assert_eq!(body.lines().count(), 1);

    // Delete SSH ID: handle and everything under it gone.
    let resp = s
        .call(Method::DELETE, "/account/sshid", Some(u.token()), NOBODY)
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let (st, _) = public(s, &format!("/sshid/{handle}")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let resp = s
        .call(
            Method::PUT,
            "/account/sshid/keys/device",
            Some(u.token()),
            Some(&PutDeviceKeysRequest { keys: vec![] }),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn device_keys_follow_sessions() {
    let s = server!();
    let u = register(s, &unique_email("sshid-dev"), "pw-sshid-1234567").await;
    let handle = format!("d{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
    let _: SshIdProfile = s
        .json(
            Method::POST,
            "/account/sshid",
            Some(u.token()),
            Some(&CreateSshIdRequest {
                handle: handle.clone(),
            }),
        )
        .await;
    let _: SshIdProfile = s
        .json(
            Method::PUT,
            "/account/sshid/keys/device",
            Some(u.token()),
            Some(&PutDeviceKeysRequest {
                keys: vec![DeviceKeyUpload {
                    key_type: SshIdKeyType::Ed25519,
                    public_key: fake_key(SshIdKeyType::Ed25519, 10),
                }],
            }),
        )
        .await;

    // Second device publishes its own key.
    let AuthResponse::Authenticated(second) = login(s, &u.email, &u.password).await else {
        panic!("login");
    };
    let p: SshIdProfile = s
        .json(
            Method::PUT,
            "/account/sshid/keys/device",
            Some(&second.token),
            Some(&PutDeviceKeysRequest {
                keys: vec![DeviceKeyUpload {
                    key_type: SshIdKeyType::Ed25519,
                    public_key: fake_key(SshIdKeyType::Ed25519, 11),
                }],
            }),
        )
        .await;
    assert_eq!(p.keys.len(), 2);
    assert_eq!(p.keys.iter().filter(|k| k.current_device).count(), 1);
    let (_, body) = public(s, &format!("/sshid/{handle}")).await;
    assert_eq!(body.lines().count(), 2);

    // Revoking the second device from the first drops its key at once.
    let resp = s
        .call(
            Method::DELETE,
            &format!("/account/devices/{}", second.device_id),
            Some(u.token()),
            NOBODY,
        )
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let (_, body) = public(s, &format!("/sshid/{handle}")).await;
    assert_eq!(body.lines().count(), 1);

    // Logging out the first device leaves the handle empty.
    let resp = s
        .call(Method::POST, "/auth/logout", Some(u.token()), NOBODY)
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let (st, body) = public(s, &format!("/sshid/{handle}")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body.trim(), "");
}
