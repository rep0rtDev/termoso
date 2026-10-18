//! Periodic jobs take a PostgreSQL advisory lock so only one replica runs a
//! pass at a time.

mod common;

use common::*;
use termoso_server::jobs;

#[tokio::test]
async fn job_lock_is_exclusive_and_released() {
    let Some(s) = server().await else { return };
    let db = sqlx::PgPool::connect(&s.database_url).await.unwrap();
    let first = jobs::try_lock(&db, "test-job").await.unwrap();
    assert!(first.is_some(), "first holder gets the lock");
    assert!(
        jobs::try_lock(&db, "test-job").await.unwrap().is_none(),
        "second holder is refused while the first runs"
    );
    assert!(
        jobs::try_lock(&db, "other-job").await.unwrap().is_some(),
        "locks are per job name"
    );
    first.unwrap().release().await;
    let again = jobs::try_lock(&db, "test-job").await.unwrap();
    assert!(again.is_some(), "released lock can be taken again");
    // Dropping without release closes the connection, which also frees it.
    drop(again);
    for _ in 0..20 {
        if jobs::try_lock(&db, "test-job").await.unwrap().is_some() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("lock not released after drop");
}
