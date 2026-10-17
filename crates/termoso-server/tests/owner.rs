//! The owner role: only the owner grants or revokes admin, and nobody can
//! disable, demote or delete the owner.

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_proto::admin::{AdminUpdateUserRequest, AdminUser};

fn change(is_admin: Option<bool>, disabled: Option<bool>) -> AdminUpdateUserRequest {
    AdminUpdateUserRequest {
        disabled,
        is_admin,
        email_verified: None,
    }
}

#[tokio::test]
async fn only_the_owner_manages_admins() {
    let Some(s) = server().await else { return };
    let owner = admin_token(s).await;
    let a = register(s, &unique_email("owner-a"), "pw-owner-a-1234567").await;
    let b = register(s, &unique_email("owner-b"), "pw-owner-b-1234567").await;

    let me: serde_json::Value = s.json(Method::GET, "/account", Some(&owner), NOBODY).await;
    assert_eq!(me["user"]["is_owner"], true, "{me}");
    let owner_id = me["user"]["id"].as_str().expect("owner id").to_string();
    let mine: serde_json::Value = s.json(Method::GET, "/account", Some(a.token()), NOBODY).await;
    assert_eq!(mine["user"]["is_owner"], false);

    // The owner promotes A; A is an admin but not the owner.
    let u: AdminUser = s
        .json(
            Method::PATCH,
            &format!("/admin/users/{}", a.id()),
            Some(&owner),
            Some(&change(Some(true), None)),
        )
        .await;
    assert!(u.is_admin && !u.is_owner);
    let listed: AdminUser = s
        .json(Method::GET, &format!("/admin/users/{owner_id}"), Some(a.token()), NOBODY)
        .await;
    assert!(listed.is_owner && listed.is_admin);

    // A plain admin cannot touch the admin role of anyone, including granting it.
    s.expect_status(
        Method::PATCH,
        &format!("/admin/users/{}", b.id()),
        Some(a.token()),
        Some(&change(Some(true), None)),
        StatusCode::FORBIDDEN,
    )
    .await;
    // ... but regular admin work still goes through.
    let u: AdminUser = s
        .json(
            Method::PATCH,
            &format!("/admin/users/{}", b.id()),
            Some(a.token()),
            Some(&change(None, Some(true))),
        )
        .await;
    assert!(u.disabled);

    // The owner is untouchable for other admins.
    s.expect_status(
        Method::PATCH,
        &format!("/admin/users/{owner_id}"),
        Some(a.token()),
        Some(&change(None, Some(true))),
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::PATCH,
        &format!("/admin/users/{owner_id}"),
        Some(a.token()),
        Some(&change(Some(false), None)),
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &format!("/admin/users/{owner_id}"),
        Some(a.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::POST,
        &format!("/admin/users/{owner_id}/reset-mfa"),
        Some(a.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;

    // The owner revokes A's admin again; A is back to a regular user.
    let u: AdminUser = s
        .json(
            Method::PATCH,
            &format!("/admin/users/{}", a.id()),
            Some(&owner),
            Some(&change(Some(false), None)),
        )
        .await;
    assert!(!u.is_admin);
    s.expect_status(
        Method::GET,
        &format!("/admin/users/{}", b.id()),
        Some(a.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
}
