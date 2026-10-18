//! Periodic jobs on a multi-replica deployment: every pass runs under a
//! PostgreSQL session advisory lock keyed by the job name, so exactly one
//! replica does the work while the others skip the tick. Nothing is stored;
//! a crashed holder releases the lock when its connection dies.

use std::time::Duration;

use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};

use crate::state::AppState;

/// Held while a pass runs. Call [`JobLock::release`]; dropping it without
/// releasing closes the underlying connection instead of returning a locked
/// session to the pool.
pub struct JobLock {
    conn: Option<PoolConnection<Postgres>>,
    key: i64,
}

impl JobLock {
    pub async fn release(mut self) {
        if let Some(mut conn) = self.conn.take() {
            let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(self.key)
                .execute(&mut *conn)
                .await;
        }
    }
}

impl Drop for JobLock {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            // Detached connections are closed, which releases session locks.
            drop(conn.detach());
        }
    }
}

/// Stable 64-bit key for `pg_try_advisory_lock`.
pub fn lock_key(name: &str) -> i64 {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(format!("termoso-job:{name}").as_bytes());
    i64::from_be_bytes(d[..8].try_into().expect("8 bytes"))
}

/// `Some(lock)` when this process now holds the job's lock.
pub async fn try_lock(db: &PgPool, name: &str) -> anyhow::Result<Option<JobLock>> {
    let mut conn = db.acquire().await?;
    let key = lock_key(name);
    let (got,): (bool,) = sqlx::query_as("SELECT pg_try_advisory_lock($1)")
        .bind(key)
        .fetch_one(&mut *conn)
        .await?;
    Ok(got.then_some(JobLock {
        conn: Some(conn),
        key,
    }))
}

/// Run `pass` every `every` (±10 % jitter so replicas do not tick in step)
/// on whichever replica wins the lock. Never returns.
pub async fn exclusive<F, Fut>(state: AppState, name: &'static str, every: Duration, pass: F)
where
    F: Fn(AppState) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<u64>>,
{
    loop {
        match try_lock(&state.db, name).await {
            Ok(Some(lock)) => {
                let started = std::time::Instant::now();
                match pass(state.clone()).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!(job = name, count = n, "job pass"),
                    Err(e) => tracing::warn!(job = name, error = %e, "job pass failed"),
                }
                metrics::counter!("termoso_job_runs_total", "job" => name).increment(1);
                metrics::histogram!("termoso_job_seconds", "job" => name)
                    .record(started.elapsed().as_secs_f64());
                lock.release().await;
            }
            Ok(None) => tracing::debug!(job = name, "job held by another replica"),
            Err(e) => tracing::warn!(job = name, error = %e, "job lock failed"),
        }
        let jitter = 0.9 + rand::random::<f64>() * 0.2;
        tokio::time::sleep(every.mul_f64(jitter)).await;
    }
}
