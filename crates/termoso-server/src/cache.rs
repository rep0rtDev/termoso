//! Redis: short-lived state (login handshakes, MFA/approval tokens, codes),
//! session cache, rate limits and cross-instance event fan-out.

use std::time::Duration;

use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::ApiResult;

#[derive(Clone)]
pub struct Cache {
    conn: ConnectionManager,
    client: redis::Client,
    prefix: String,
}

impl Cache {
    /// `prefix` namespaces every key and the event channel so several
    /// deployments can share one Redis.
    pub async fn connect(url: &str, prefix: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)?;
        let conn = client.get_connection_manager().await?;
        Ok(Self {
            conn,
            client,
            prefix: prefix.to_string(),
        })
    }

    pub fn client(&self) -> &redis::Client {
        &self.client
    }

    pub fn events_channel(&self) -> String {
        format!("{}events", self.prefix)
    }

    /// Channel carrying multiplayer relay frames between instances.
    pub fn live_channel(&self) -> String {
        format!("{}live", self.prefix)
    }

    fn key(&self, key: &str) -> String {
        format!("{}{key}", self.prefix)
    }

    pub async fn ping(&self) -> anyhow::Result<()> {
        let mut c = self.conn.clone();
        let _: String = redis::cmd("PING").query_async(&mut c).await?;
        Ok(())
    }

    pub async fn set_json<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> ApiResult<()> {
        let mut c = self.conn.clone();
        let body = serde_json::to_vec(value)?;
        let _: () = c.set_ex(self.key(key), body, ttl.as_secs().max(1)).await?;
        Ok(())
    }

    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> ApiResult<Option<T>> {
        let mut c = self.conn.clone();
        let raw: Option<Vec<u8>> = c.get(self.key(key)).await?;
        Ok(match raw {
            Some(b) => Some(serde_json::from_slice(&b)?),
            None => None,
        })
    }

    /// Atomically read and delete (single-use tokens).
    pub async fn take_json<T: DeserializeOwned>(&self, key: &str) -> ApiResult<Option<T>> {
        let mut c = self.conn.clone();
        let raw: Option<Vec<u8>> = redis::cmd("GETDEL")
            .arg(self.key(key))
            .query_async(&mut c)
            .await?;
        Ok(match raw {
            Some(b) => Some(serde_json::from_slice(&b)?),
            None => None,
        })
    }

    pub async fn del(&self, key: &str) -> ApiResult<()> {
        let mut c = self.conn.clone();
        let _: () = c.del(self.key(key)).await?;
        Ok(())
    }

    /// Current value of a counter created by [`Self::incr_window`] (0 when absent).
    pub async fn counter(&self, key: &str) -> ApiResult<u64> {
        let mut c = self.conn.clone();
        let v: Option<u64> = c.get(self.key(key)).await?;
        Ok(v.unwrap_or(0))
    }

    /// Undo one [`Self::incr_window`] step (e.g. a quota slot the caller
    /// could not use). Never goes below zero.
    pub async fn decr(&self, key: &str) -> ApiResult<()> {
        let mut c = self.conn.clone();
        let key = self.key(key);
        let v: i64 = c.decr(&key, 1i64).await?;
        if v < 0 {
            let _: () = c.del(&key).await?;
        }
        Ok(())
    }

    /// Fixed-window counter. Returns the new count and remaining TTL.
    pub async fn incr_window(&self, key: &str, window: Duration) -> ApiResult<(u64, u64)> {
        let mut c = self.conn.clone();
        let key = self.key(key);
        let (count, ttl): (u64, i64) = redis::pipe()
            .atomic()
            .incr(&key, 1u64)
            .cmd("EXPIRE")
            .arg(&key)
            .arg(window.as_secs().max(1))
            .arg("NX")
            .ignore()
            .ttl(&key)
            .query_async(&mut c)
            .await?;
        Ok((count, ttl.max(0) as u64))
    }

    pub async fn publish<T: Serialize>(&self, event: &T) -> ApiResult<()> {
        let mut c = self.conn.clone();
        let body = serde_json::to_vec(event)?;
        let _: () = c.publish(self.events_channel(), body).await?;
        Ok(())
    }

    /// Publish an opaque frame on the multiplayer channel.
    pub async fn publish_live(&self, frame: Vec<u8>) -> ApiResult<()> {
        let mut c = self.conn.clone();
        let _: () = c.publish(self.live_channel(), frame).await?;
        Ok(())
    }

    /// Set one hash field and refresh the hash TTL; `true` when the field
    /// did not exist before.
    pub async fn hset_json<T: Serialize>(
        &self,
        key: &str,
        field: &str,
        value: &T,
        ttl: Duration,
    ) -> ApiResult<bool> {
        let mut c = self.conn.clone();
        let body = serde_json::to_vec(value)?;
        let k = self.key(key);
        let (added,): (i64,) = redis::pipe()
            .hset(&k, field, body)
            .expire(&k, ttl.as_secs().max(1) as i64)
            .ignore()
            .query_async(&mut c)
            .await?;
        Ok(added > 0)
    }

    pub async fn hget_json<T: DeserializeOwned>(
        &self,
        key: &str,
        field: &str,
    ) -> ApiResult<Option<T>> {
        let mut c = self.conn.clone();
        let raw: Option<Vec<u8>> = c.hget(self.key(key), field).await?;
        Ok(match raw {
            Some(b) => Some(serde_json::from_slice(&b)?),
            None => None,
        })
    }

    /// Remove one hash field; `true` when it existed.
    pub async fn hdel(&self, key: &str, field: &str) -> ApiResult<bool> {
        let mut c = self.conn.clone();
        let n: i64 = c.hdel(self.key(key), field).await?;
        Ok(n > 0)
    }

    pub async fn hgetall_json<T: DeserializeOwned>(&self, key: &str) -> ApiResult<Vec<T>> {
        let mut c = self.conn.clone();
        let raw: Vec<(String, Vec<u8>)> = c.hgetall(self.key(key)).await?;
        raw.into_iter()
            .map(|(_, b)| serde_json::from_slice(&b).map_err(Into::into))
            .collect()
    }
}
