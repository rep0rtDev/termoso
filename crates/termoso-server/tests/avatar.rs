//! Profile pictures: upload is normalised to a tiny WebP, served with an
//! immutable tag, visible to teammates, removable.

mod common;

use common::*;
use image::{ImageFormat, Rgb, RgbImage};
use reqwest::{Method, StatusCode, header};
use std::io::Cursor;
use termoso_proto::account::UserProfile;
use termoso_proto::team::{CreateTeamRequest, Team, TeamMemberList};

#[derive(serde::Deserialize)]
struct AccountResponse {
    user: UserProfile,
}

fn png(w: u32, h: u32, colour: [u8; 3]) -> Vec<u8> {
    let img = RgbImage::from_pixel(w, h, Rgb(colour));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).expect("png");
    out.into_inner()
}

async fn put_avatar(s: &TestServer, token: &str, bytes: Vec<u8>) -> reqwest::Response {
    s.http()
        .put(s.url("/account/avatar"))
        .bearer_auth(token)
        .header("x-forwarded-for", "10.99.0.1")
        .body(bytes)
        .send()
        .await
        .expect("put avatar")
}

async fn get_avatar(
    s: &TestServer,
    token: &str,
    id: uuid::Uuid,
    etag: Option<&str>,
) -> reqwest::Response {
    let mut req = s
        .http()
        .get(s.url(&format!("/users/{id}/avatar")))
        .bearer_auth(token);
    if let Some(e) = etag {
        req = req.header(header::IF_NONE_MATCH, e);
    }
    req.send().await.expect("get avatar")
}

#[tokio::test]
async fn avatar_lifecycle() {
    let Some(s) = server().await else { return };
    let alice = register(s, &unique_email("ava-a"), "pw-alice-123456").await;
    let bob = register(s, &unique_email("ava-b"), "pw-bob-123456").await;

    let account: AccountResponse = s
        .json(Method::GET, "/account", Some(alice.token()), NOBODY)
        .await;
    assert_eq!(account.user.avatar, None);
    assert_eq!(
        get_avatar(s, bob.token(), alice.id(), None).await.status(),
        StatusCode::NOT_FOUND
    );

    // Big non-square photo → small square WebP with a content tag.
    let resp = put_avatar(s, alice.token(), png(1600, 900, [200, 40, 40])).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let profile: UserProfile = resp.json().await.expect("profile");
    let tag = profile.avatar.clone().expect("tag set");
    assert_eq!(tag.len(), 16);

    let resp = get_avatar(s, bob.token(), alice.id(), None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()[header::CONTENT_TYPE], "image/webp");
    let etag = resp.headers()[header::ETAG].to_str().unwrap().to_string();
    assert_eq!(etag, format!("\"{tag}\""));
    // Unpinned URL: must revalidate, so a replaced picture never sticks.
    assert_eq!(resp.headers()[header::CACHE_CONTROL], "private, no-cache");
    let bytes = resp.bytes().await.expect("bytes");

    // Pinned to the current tag: immutable. Pinned to anything else: not.
    let pinned = s
        .http()
        .get(s.url(&format!("/users/{}/avatar?v={tag}", alice.id())))
        .bearer_auth(bob.token())
        .send()
        .await
        .expect("pinned");
    assert_eq!(pinned.status(), StatusCode::OK);
    assert!(
        pinned.headers()[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    let stale = s
        .http()
        .get(s.url(&format!("/users/{}/avatar?v=0123456789abcdef", alice.id())))
        .bearer_auth(bob.token())
        .send()
        .await
        .expect("stale");
    assert_eq!(stale.status(), StatusCode::OK);
    assert_eq!(stale.headers()[header::CACHE_CONTROL], "private, no-cache");
    assert!(
        bytes.len() <= termoso_server::avatar::MAX_STORED,
        "{} bytes",
        bytes.len()
    );
    let img = image::load_from_memory(&bytes).expect("decodes");
    assert_eq!(
        (img.width(), img.height()),
        (termoso_server::avatar::SIDE, termoso_server::avatar::SIDE)
    );

    // Cached clients get a 304 for the tag they already have.
    let resp = get_avatar(s, bob.token(), alice.id(), Some(&etag)).await;
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);

    // Anonymous requests are refused.
    let resp = s
        .http()
        .get(s.url(&format!("/users/{}/avatar", alice.id())))
        .send()
        .await
        .expect("anon");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Teammates see the tag next to the member.
    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(alice.token()),
            Some(&CreateTeamRequest {
                name: "Avatars".into(),
            }),
        )
        .await;
    let members: TeamMemberList = s
        .json(
            Method::GET,
            &format!("/teams/{}/members", team.id),
            Some(alice.token()),
            NOBODY,
        )
        .await;
    assert_eq!(members.members[0].avatar.as_deref(), Some(tag.as_str()));

    // A different picture changes the tag; the old ETag no longer matches.
    let resp = put_avatar(s, alice.token(), png(64, 64, [40, 200, 40])).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let profile: UserProfile = resp.json().await.expect("profile");
    let tag2 = profile.avatar.expect("tag set");
    assert_ne!(tag2, tag);
    let resp = get_avatar(s, bob.token(), alice.id(), Some(&etag)).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Garbage and oversized uploads are rejected without touching the row.
    let resp = put_avatar(s, alice.token(), b"not an image at all".to_vec()).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = put_avatar(
        s,
        alice.token(),
        vec![0u8; termoso_server::avatar::MAX_UPLOAD + 1],
    )
    .await;
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let account: AccountResponse = s
        .json(Method::GET, "/account", Some(alice.token()), NOBODY)
        .await;
    assert_eq!(account.user.avatar.as_deref(), Some(tag2.as_str()));

    // Remove.
    let profile: UserProfile = s
        .json(
            Method::DELETE,
            "/account/avatar",
            Some(alice.token()),
            NOBODY,
        )
        .await;
    assert_eq!(profile.avatar, None);
    assert_eq!(
        get_avatar(s, bob.token(), alice.id(), None).await.status(),
        StatusCode::NOT_FOUND
    );
}
