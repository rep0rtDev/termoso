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

/// Same as [`public`] but the way a browser asks (`Accept: text/html`),
/// optionally on `host`; redirects are not followed.
async fn browser(
    s: &TestServer,
    path: &str,
    host: Option<&str>,
) -> (StatusCode, reqwest::header::HeaderMap, String) {
    let mut req = s
        .http_no_redirect()
        .get(format!("http://{}{path}", s.addr))
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        );
    if let Some(h) = host {
        req = req.header("Host", h);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    (status, headers, resp.text().await.unwrap())
}

/// Same as [`public`] but addressed to the dedicated SSH ID host
/// (`TERMOSO_SSHID_URL`), the way a reverse proxy would forward it.
async fn dedicated(s: &TestServer, path: &str) -> (StatusCode, String) {
    let resp = s
        .http()
        .get(format!("http://{}{path}", s.addr))
        .header("Host", SSHID_HOST)
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
    assert_eq!(p.url, format!("{SSHID_URL}/{handle}"));
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

    // Own key (generated elsewhere): type from the algorithm token, listed
    // like a FIDO2 key; garbage and duplicates are rejected.
    let own_rsa = fake_key(SshIdKeyType::Rsa, 6);
    let own: SshIdKey = s
        .json(
            Method::POST,
            "/account/sshid/keys",
            Some(u.token()),
            Some(&AddPublicKeyRequest {
                label: "laptop (1Password)".into(),
                public_key: format!("{own_rsa}\n"),
            }),
        )
        .await;
    assert!(own.device_id.is_none() && own.key_type == SshIdKeyType::Rsa);
    assert_eq!(own.public_key, own_rsa.rsplit_once(' ').unwrap().0);
    for bad in ["ssh-dss AAAA", "not a key", ""] {
        let resp = s
            .call(
                Method::POST,
                "/account/sshid/keys",
                Some(u.token()),
                Some(&AddPublicKeyRequest {
                    label: "x".into(),
                    public_key: bad.into(),
                }),
            )
            .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{bad:?}");
    }
    let resp = s
        .call(
            Method::POST,
            "/account/sshid/keys",
            Some(u.token()),
            Some(&AddPublicKeyRequest {
                label: "again".into(),
                public_key: own_rsa.clone(),
            }),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let (st, body) = public(s, &format!("/sshid/{handle}/RSA")).await;
    assert_eq!(st, StatusCode::OK);
    assert!(body.contains(own_rsa.rsplit_once(' ').unwrap().0));
    s.expect_status(
        Method::DELETE,
        &format!("/account/sshid/keys/{}", own.id),
        Some(u.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;

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

    // Dedicated host: the same lists at the root, nothing else.
    let (st, body) = dedicated(s, &format!("/{handle}")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body.trim(), format!("{ed_line} #SSH ID - @{handle}"));
    let (st, body) = dedicated(s, &format!("/{}/ECDSA-SK", handle.to_uppercase())).await;
    assert_eq!(st, StatusCode::OK);
    assert!(body.starts_with(sk.rsplit_once(' ').unwrap().0));
    let (st, body) = dedicated(s, &format!("/{handle}/all")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body.lines().count(), 3);
    let (st, body) = dedicated(s, &format!("/sshid/{handle}/all")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body.lines().count(), 3);
    let (st, body) = dedicated(s, &format!("/{handle}/all?x=1")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body.lines().count(), 3);
    let (st, body) = dedicated(s, "/").await;
    assert_eq!(st, StatusCode::OK);
    assert!(body.contains(&format!("curl -fs {SSHID_URL}/<handle>")));
    let (st, _) = dedicated(s, "/healthz").await;
    assert_eq!(st, StatusCode::OK);
    for closed in [
        "/api/v1/server",
        "/api/v1",
        "/nobody-here-000",
        &format!("/{handle}/dsa"),
        &format!("/{handle}/all/extra"),
        "/index.html",
        "/a",
    ] {
        let (st, _) = dedicated(s, closed).await;
        assert_eq!(st, StatusCode::NOT_FOUND, "{closed}");
    }
    // The cabinet origin is untouched: `/` is the SPA, root handles are not
    // served there.
    let (st, body) = public(s, "/").await;
    assert_eq!(st, StatusCode::OK);
    assert!(!body.contains("Termoso SSH ID"));
    let (_, body) = public(s, &format!("/{handle}")).await;
    assert!(!body.contains(ed_line), "SPA fallback, not the key list");

    // A browser gets a page instead of the raw list; curl above is unchanged.
    let (st, hdrs, body) = browser(s, &format!("/{handle}"), Some(SSHID_HOST)).await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        hdrs["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert!(
        hdrs["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("script-src 'nonce-")
    );
    assert!(body.starts_with("<!doctype html>"));
    assert!(body.contains(&format!("@{handle}")));
    assert!(body.contains(&format!(
        "curl -fs {SSHID_URL}/{handle} &gt;&gt; ~/.ssh/authorized_keys"
    )));
    assert!(body.contains(&format!("{SSHID_URL}/{handle}/ECDSA-SK")));
    assert!(body.contains("SHA256:"));
    assert!(body.contains(&format!(
        "<pre id=\"k-0\">{ed_line} #SSH ID - @{handle}</pre>"
    )));
    let (st, _, body) = browser(s, &format!("/{handle}/rsa"), Some(SSHID_HOST)).await;
    assert_eq!(st, StatusCode::OK);
    assert!(body.contains("aria-current=\"page\""));
    assert!(body.contains("No RSA keys are published"));
    let (st, _, body) = browser(s, "/", Some(SSHID_HOST)).await;
    assert_eq!(st, StatusCode::OK);
    assert!(body.contains("<h1>SSH ID</h1>"));
    assert!(body.contains(&format!("{SSHID_URL}/&lt;handle&gt;")));
    for (path, host) in [
        ("/nobody-here-000", Some(SSHID_HOST)),
        ("/a", Some(SSHID_HOST)),
    ] {
        let (st, hdrs, body) = browser(s, path, host).await;
        assert_eq!(st, StatusCode::NOT_FOUND, "{path}");
        assert!(
            hdrs["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/html"),
            "{path}"
        );
        assert!(body.contains("No such SSH ID"), "{path}");
    }
    let (st, _, body) = browser(s, &format!("/sshid/{handle}/dsa"), None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert!(body.contains("no key type called dsa"));
    // With a dedicated host configured, browsers on the fallback path are
    // sent to the canonical address; the text endpoint there is untouched.
    let (st, hdrs, _) = browser(s, &format!("/sshid/{handle}"), None).await;
    assert_eq!(st, StatusCode::MOVED_PERMANENTLY);
    let (st, hdrs2, _) = browser(s, "/sshid/nobody-here-000", None).await;
    assert_eq!(
        st,
        StatusCode::MOVED_PERMANENTLY,
        "unknown handles redirect too"
    );
    assert_eq!(
        hdrs2["location"],
        format!("{SSHID_URL}/nobody-here-000").as_str()
    );
    assert_eq!(hdrs["location"], format!("{SSHID_URL}/{handle}").as_str());
    let (st, hdrs, _) = browser(
        s,
        &format!("/sshid/{}/ecdsa-sk", handle.to_uppercase()),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        hdrs["location"],
        format!("{SSHID_URL}/{handle}/ECDSA-SK").as_str()
    );
    let (st, hdrs, _) = browser(s, &format!("/sshid/{handle}/all"), None).await;
    assert_eq!(st, StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        hdrs["location"],
        format!("{SSHID_URL}/{handle}/all").as_str()
    );
    let (st, body) = public(s, &format!("/sshid/{handle}")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body.trim(), format!("{ed_line} #SSH ID - @{handle}"));

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
