//! A team's first vault is its default: undeletable, renameable, at most one.

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use sqlx::Connection;
use termoso_crypto::keys::SymmetricKey;
use termoso_crypto::sealed;
use termoso_proto::team::{CreateTeamRequest, Team};
use termoso_proto::vault::{
    CreateVaultRequest, UpdateVaultRequest, Vault, VaultList, VaultMemberUpsert, VaultRole,
};

macro_rules! server {
    () => {
        match server().await {
            Some(s) => s,
            None => return,
        }
    };
}

async fn team_vault(s: &TestServer, team: &Team, owner: &User, name: &str) -> Vault {
    let key = SymmetricKey::generate();
    s.json(
        Method::POST,
        &format!("/teams/{}/vaults", team.id),
        Some(owner.token()),
        Some(&CreateVaultRequest {
            name: name.into(),
            members: vec![VaultMemberUpsert {
                user_id: owner.id(),
                role: VaultRole::Manager,
                sealed_key: sealed::seal_vault_key(owner.keypair.public(), &key).unwrap(),
            }],
        }),
    )
    .await
}

#[tokio::test]
async fn first_team_vault_is_default_and_undeletable() {
    let s = server!();
    let owner = register(s, &unique_email("dv-owner"), "pw-owner-1234567").await;
    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&CreateTeamRequest { name: "Ops".into() }),
        )
        .await;

    let first = team_vault(s, &team, &owner, "Ops").await;
    assert!(first.is_default, "first vault of a team is the default");
    let second = team_vault(s, &team, &owner, "Staging").await;
    assert!(!second.is_default, "later vaults are ordinary");

    // list / get carry the flag.
    let list: VaultList = s
        .json(Method::GET, "/vaults", Some(owner.token()), NOBODY)
        .await;
    let defaults: Vec<_> = list
        .vaults
        .iter()
        .filter(|v| v.team_id == Some(team.id) && v.is_default)
        .collect();
    assert_eq!(defaults.len(), 1);
    assert_eq!(defaults[0].id, first.id);
    let got: Vault = s
        .json(
            Method::GET,
            &format!("/vaults/{}", second.id),
            Some(owner.token()),
            NOBODY,
        )
        .await;
    assert!(!got.is_default);

    // The default can be renamed but not deleted; others can be deleted.
    let renamed: Vault = s
        .json(
            Method::PATCH,
            &format!("/vaults/{}", first.id),
            Some(owner.token()),
            Some(&UpdateVaultRequest {
                name: Some("Main".into()),
                session_logging: None,
            }),
        )
        .await;
    assert_eq!(renamed.name, "Main");
    assert!(renamed.is_default);
    s.expect_status(
        Method::DELETE,
        &format!("/vaults/{}", first.id),
        Some(owner.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::DELETE,
        &format!("/vaults/{}", second.id),
        Some(owner.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;

    // The personal vault is never "default".
    assert!(
        list.vaults
            .iter()
            .filter(|v| v.team_id.is_none())
            .all(|v| !v.is_default)
    );
}

#[tokio::test]
async fn backfill_marks_oldest_vault_default() {
    let s = server!();
    let owner = register(s, &unique_email("dv-bf"), "pw-owner-1234567").await;
    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&CreateTeamRequest {
                name: "Legacy".into(),
            }),
        )
        .await;
    let a = team_vault(s, &team, &owner, "A").await;
    let b = team_vault(s, &team, &owner, "B").await;

    // Pretend both predate the flag (as rows did before migration 0014).
    s.sql(
        "UPDATE vaults SET is_default = false WHERE team_id = $1::uuid",
        &team.id.to_string(),
    )
    .await;
    s.sql(
        "UPDATE vaults v SET is_default = true
           FROM (SELECT DISTINCT ON (team_id) id FROM vaults
                 WHERE kind = 'team' AND deleted_at IS NULL AND team_id = $1::uuid
                 ORDER BY team_id, created_at, id) first
          WHERE v.id = first.id",
        &team.id.to_string(),
    )
    .await;

    let list: VaultList = s
        .json(Method::GET, "/vaults", Some(owner.token()), NOBODY)
        .await;
    let find = |id| list.vaults.iter().find(|v| v.id == id).unwrap();
    assert!(find(a.id).is_default, "oldest becomes the default");
    assert!(!find(b.id).is_default);

    // The unique index keeps it at one per team.
    let mut conn = sqlx::PgConnection::connect(&s.database_url).await.unwrap();
    let dup = sqlx::query("UPDATE vaults SET is_default = true WHERE id = $1")
        .bind(b.id)
        .execute(&mut conn)
        .await;
    assert!(dup.is_err(), "second default per team is rejected");
}
