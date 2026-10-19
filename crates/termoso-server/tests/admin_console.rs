//! Server admin console: team cards, member management, user filters, jobs.

mod common;
use common::*;
use reqwest::{Method, StatusCode};
use termoso_proto::admin::{AdminJobList, AdminTeamDetail, AdminTeamList, AdminUserList};
use uuid::Uuid;

#[tokio::test]
async fn admin_sees_teams_and_manages_members() {
    let Some(s) = server().await else { return };
    let admin = register(s, "admin@test.local", "pw-admin-1234567").await;
    let owner = register(s, &unique_email("ac-owner"), "pw-owner-1234567").await;
    let bob = register(s, &unique_email("ac-bob"), "pw-bob-123456789").await;
    let team: serde_json::Value = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&serde_json::json!({ "name": "Console" })),
        )
        .await;
    let team_id: Uuid = team["id"].as_str().unwrap().parse().unwrap();

    s.expect_status(
        Method::GET,
        "/admin/jobs",
        Some(owner.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;

    let users: AdminUserList = s
        .json(
            Method::GET,
            &format!("/admin/users?team={team_id}&sort=email"),
            Some(admin.token()),
            NOBODY,
        )
        .await;
    assert_eq!(users.total, 1);
    assert_eq!(users.users[0].id, owner.id());
    assert_eq!(users.users[0].source, "password");
    assert_eq!(users.users[0].teams.len(), 1);
    assert_eq!(users.users[0].teams[0].role, "owner");

    let teams: AdminTeamList = s
        .json(
            Method::GET,
            "/admin/teams?q=console",
            Some(admin.token()),
            NOBODY,
        )
        .await;
    let t = teams.teams.iter().find(|t| t.id == team_id).expect("team");
    assert_eq!(
        (t.member_count, t.vault_count, t.idp, t.scim, t.webhooks),
        (1, 0, false, false, 0)
    );
    assert!(t.deletion_scheduled_at.is_none() && !t.is_home);

    let detail: AdminTeamDetail = s
        .json(
            Method::GET,
            &format!("/admin/teams/{team_id}"),
            Some(admin.token()),
            NOBODY,
        )
        .await;
    assert_eq!(detail.members.len(), 1);
    assert_eq!(detail.members[0].level, "owner");
    assert_eq!(detail.policies.mfa, "off");
    assert!(detail.audit.is_array());

    let detail: AdminTeamDetail = s
        .json(
            Method::POST,
            &format!("/admin/teams/{team_id}/members"),
            Some(admin.token()),
            Some(&serde_json::json!({ "user_id": bob.id() })),
        )
        .await;
    assert_eq!(detail.members.len(), 2);
    let added = detail
        .members
        .iter()
        .find(|m| m.user_id == bob.id())
        .unwrap();
    assert_eq!(
        (added.level.as_str(), added.role_source.as_str()),
        ("member", "admin")
    );
    s.expect_status(
        Method::POST,
        &format!("/admin/teams/{team_id}/members"),
        Some(admin.token()),
        Some(&serde_json::json!({ "user_id": bob.id() })),
        StatusCode::CONFLICT,
    )
    .await;
    s.expect_status(
        Method::POST,
        &format!("/admin/teams/{team_id}/members"),
        Some(admin.token()),
        Some(&serde_json::json!({ "user_id": Uuid::new_v4() })),
        StatusCode::NOT_FOUND,
    )
    .await;
    let users: AdminUserList = s
        .json(
            Method::GET,
            &format!("/admin/users?team={team_id}"),
            Some(admin.token()),
            NOBODY,
        )
        .await;
    assert_eq!(users.total, 2);

    s.expect_status(
        Method::DELETE,
        &format!("/admin/teams/{team_id}/members/{}", owner.id()),
        Some(admin.token()),
        NOBODY,
        StatusCode::CONFLICT,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &format!("/admin/teams/{team_id}/members/{}", bob.id()),
        Some(admin.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    let detail: AdminTeamDetail = s
        .json(
            Method::GET,
            &format!("/admin/teams/{team_id}"),
            Some(admin.token()),
            NOBODY,
        )
        .await;
    assert_eq!(detail.members.len(), 1);
    assert!(
        detail
            .audit
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "member.removed")
    );

    let _: AdminJobList = s
        .json(Method::GET, "/admin/jobs", Some(admin.token()), NOBODY)
        .await;
}

#[tokio::test]
async fn admin_sees_effective_config() {
    use termoso_proto::admin::{AdminConfig, ServerSettings};
    let Some(s) = server().await else { return };
    let admin_tok = admin_token(s).await;
    let user = register(s, &unique_email("cfg-user"), "pw-user-12345678").await;

    s.expect_status(
        Method::GET,
        "/admin/config",
        Some(user.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;

    let cfg: AdminConfig = s
        .json(
            Method::GET,
            "/admin/config",
            Some(admin_tok.as_str()),
            NOBODY,
        )
        .await;
    assert_eq!(cfg.server.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(cfg.database.status, "ok");
    assert_eq!(cfg.redis.status, "ok");
    assert!(
        !cfg.database.endpoint.contains('@'),
        "credentials leaked: {}",
        cfg.database.endpoint
    );
    assert!(!cfg.redis.endpoint.contains('@'));
    assert_eq!(cfg.settings.limits.max_entity_bytes_max, 4 * 1024 * 1024);
    assert_eq!(cfg.settings.defaults.session_ttl_days, 90);

    let mut settings: ServerSettings = s
        .json(
            Method::GET,
            "/admin/settings",
            Some(admin_tok.as_str()),
            NOBODY,
        )
        .await;
    settings.session_ttl_days = 45;
    let _: ServerSettings = s
        .json(
            Method::PUT,
            "/admin/settings",
            Some(admin_tok.as_str()),
            Some(&serde_json::to_value(&settings).unwrap()),
        )
        .await;
    let cfg: AdminConfig = s
        .json(
            Method::GET,
            "/admin/config",
            Some(admin_tok.as_str()),
            NOBODY,
        )
        .await;
    assert!(cfg.settings.updated_at.is_some());
    assert_eq!(cfg.settings.updated_by.as_deref(), Some("admin@test.local"));
}
