//! `backup` → change data → `restore` round trip through real pg_dump /
//! pg_restore. Skipped unless `TERMOSO_TEST_PG_DUMP` points at a pg_dump
//! matching the test server's major version (`TERMOSO_TEST_PG_RESTORE`
//! likewise; both default to the plain binary names when the variable is
//! set to `1`).

mod common;

use common::*;
use termoso_server::backup::{self, Ctx, RestoreOptions};

fn tool(var: &str, default: &str) -> Option<String> {
    match std::env::var(var) {
        Ok(v) if v == "1" => Some(default.to_string()),
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

#[tokio::test]
async fn backup_and_restore_round_trip() {
    let Some(pg_dump) = tool("TERMOSO_TEST_PG_DUMP", "pg_dump") else {
        eprintln!("TERMOSO_TEST_PG_DUMP unset; skipping");
        return;
    };
    let pg_restore = tool("TERMOSO_TEST_PG_RESTORE", "pg_restore").unwrap_or("pg_restore".into());
    let Some(s) = server().await else { return };
    let alice_email = unique_email("alice");
    let _alice = register(s, &alice_email, "pw-alice-12345678").await;

    let ctx = Ctx {
        database_url: s.database_url.clone(),
        master_key_fingerprint: backup::fingerprint(&s.master_key),
        pg_dump,
        pg_restore,
        age: "age".into(),
    };
    let dir = std::env::temp_dir().join(format!("termoso-backup-test-{}", uuid::Uuid::new_v4()));
    let path = backup::backup(&ctx, &dir, None).await.expect("backup");
    let manifest = backup::read_manifest(&path).await.unwrap();
    assert!(manifest.counts.users >= 1, "{manifest:?}");
    assert_eq!(manifest.schema_version, backup::embedded_schema_version());
    assert!(manifest.bytes > 0);
    backup::check(&ctx, &path, false)
        .await
        .expect("check passes");

    // A wrong key is refused unless forced.
    let other = Ctx {
        database_url: ctx.database_url.clone(),
        master_key_fingerprint: "00".repeat(8),
        pg_dump: ctx.pg_dump.clone(),
        pg_restore: ctx.pg_restore.clone(),
        age: ctx.age.clone(),
    };
    let err = backup::check(&other, &path, false).await.unwrap_err();
    assert!(err.to_string().contains("master key"), "{err}");

    // Data written after the backup disappears on restore.
    let bob_email = unique_email("bob");
    let _bob = register(s, &bob_email, "pw-bob-1234567890").await;
    let report = backup::restore(
        &ctx,
        &path,
        RestoreOptions {
            age_identity: None,
            force_key: false,
        },
    )
    .await
    .expect("restore");
    assert!(report.counts_match, "{report:?}");
    assert_eq!(report.counts_after.users, manifest.counts.users);
    assert_eq!(report.schema_after, backup::embedded_schema_version());

    // Alice still logs in; Bob never existed.
    login(s, &alice_email, "pw-alice-12345678").await;
    assert!(
        try_login(s, &bob_email, "pw-bob-1234567890").await.is_err(),
        "bob was created after the backup"
    );
    let _ = std::fs::remove_dir_all(dir);
}
