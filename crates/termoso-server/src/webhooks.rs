//! Audit webhooks / SIEM delivery. Every audit event becomes one outbox row
//! per matching webhook (see `audit::record`); a worker on any replica claims
//! due rows with `SKIP LOCKED`, posts batches of up to 100 events, retries
//! with exponential backoff (30 s → 6 h, six attempts) and disables a hook
//! after 50 consecutive failures (and emails everyone with `webhooks.manage`).
//! `syslog_tls` hooks speak RFC 5425 (TLS, octet-counted) with one CEF line
//! per event instead of HTTP.

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use std::sync::{Arc, OnceLock};
use termoso_proto::team::{
    CreateWebhookRequest, UpdateWebhookRequest, WEBHOOK_KINDS, Webhook, WebhookDelivery,
    WebhookDeliveryList, WebhookList,
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::audit;
use crate::error::{ApiResult, Error, NoContent};
use crate::extract::{Auth, Json as Body};
use crate::state::AppState;

pub const MAX_PER_TEAM: i64 = 10;
pub const BATCH: i64 = 100;
pub const MAX_ATTEMPTS: i32 = 6;
pub const DISABLE_AFTER: i32 = 50;
const BACKOFF_SECS: [i64; 6] = [30, 60, 300, 1800, 7200, 21600];
const SECRET_PURPOSE: &str = "webhook-secret";
/// RFC 5425 default port.
const SYSLOG_PORT: u16 = 6514;

/// `syslog_tls` hooks take `syslog+tls://host[:port]`, every other kind an
/// http(s) URL.
fn check_url_kind(kind: &str, url: &str) -> ApiResult<()> {
    let syslog = url.starts_with("syslog+tls://");
    match (kind, syslog) {
        ("syslog_tls", false) => Err(Error::bad_request(
            "syslog_tls webhooks need a syslog+tls://host:port URL",
        )),
        ("syslog_tls", true) => Ok(()),
        (_, true) => Err(Error::bad_request(
            "syslog+tls:// URLs are only valid for kind syslog_tls",
        )),
        _ => Ok(()),
    }
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("termoso-server/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client")
    })
}

/// HMAC-SHA256 (RFC 2104) over `msg`.
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    let inner = Sha256::new()
        .chain_update(&ipad)
        .chain_update(msg)
        .finalize();
    Sha256::new()
        .chain_update(&opad)
        .chain_update(inner)
        .finalize()
        .into()
}

/// `X-Termoso-Signature` value: `sha256=HMAC(secret, "<timestamp>.<body>")`.
pub fn signature(secret: &str, timestamp: &str, body: &[u8]) -> String {
    let mut msg = timestamp.as_bytes().to_vec();
    msg.push(b'.');
    msg.extend_from_slice(body);
    let mac = hmac_sha256(secret.as_bytes(), &msg);
    format!("sha256={}", hex(&mac))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1])
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_private(IpAddr::V4(v4)))
        }
    }
}

/// Validate a destination: `https://` (plain `http://` and private or
/// loopback addresses only with `TERMOSO_WEBHOOKS_ALLOW_PRIVATE=true`), no
/// credentials in the URL, host must resolve.
pub async fn validate_url(state: &AppState, raw: &str) -> ApiResult<String> {
    let url = url::Url::parse(raw.trim()).map_err(|_| Error::bad_request("Invalid webhook URL"))?;
    let allow_private = state.cfg.webhooks_allow_private;
    match url.scheme() {
        "https" | "syslog+tls" => {}
        "http" if allow_private => {}
        "http" => {
            return Err(Error::bad_request(
                "Webhooks must use https:// (set TERMOSO_WEBHOOKS_ALLOW_PRIVATE for internal http)",
            ));
        }
        _ => {
            return Err(Error::bad_request(
                "Webhook URL must be https:// or syslog+tls://",
            ));
        }
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::bad_request("Webhook URL must not embed credentials"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| Error::bad_request("Webhook URL needs a host"))?
        .trim_matches(['[', ']']);
    if raw.len() > 2048 {
        return Err(Error::bad_request("Webhook URL is too long"));
    }
    if !allow_private {
        let port = url.port().unwrap_or(if url.scheme() == "syslog+tls" {
            SYSLOG_PORT
        } else {
            443
        });
        let addrs: Vec<IpAddr> = match host.parse::<IpAddr>() {
            Ok(ip) => vec![ip],
            Err(_) => tokio::net::lookup_host((host, port))
                .await
                .map_err(|_| Error::bad_request("Webhook host does not resolve"))?
                .map(|a| a.ip())
                .collect(),
        };
        if addrs.is_empty() {
            return Err(Error::bad_request("Webhook host does not resolve"));
        }
        if addrs.into_iter().any(is_private) || host.eq_ignore_ascii_case("localhost") {
            return Err(Error::bad_request(
                "Webhook destinations on private or loopback addresses are not allowed",
            ));
        }
    }
    Ok(url.to_string())
}

fn normalize_actions(actions: Option<Vec<String>>) -> ApiResult<Option<Vec<String>>> {
    let Some(list) = actions else { return Ok(None) };
    let mut out: Vec<String> = Vec::new();
    for a in list {
        let a = a.trim().to_string();
        if a.is_empty() {
            continue;
        }
        if a.len() > 64
            || !a
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
        {
            return Err(Error::bad_request(format!("`{a}` is not an action name")));
        }
        if !out.contains(&a) {
            out.push(a);
        }
    }
    if out.len() > 100 {
        return Err(Error::bad_request("At most 100 actions"));
    }
    Ok((!out.is_empty()).then_some(out))
}

fn check_kind(kind: &str) -> ApiResult<()> {
    if WEBHOOK_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(Error::bad_request(
            "kind must be generic, splunk_hec, elastic, datadog or syslog_tls",
        ))
    }
}

type HookRow = (
    Uuid,
    String,
    String,
    Option<Vec<String>>,
    bool,
    DateTime<Utc>,
    Option<i32>,
    Option<String>,
    Option<DateTime<Utc>>,
    i32,
);

macro_rules! hook_select {
    ($tail:literal) => {
        concat!(
            "SELECT id, url, kind, actions, enabled, created_at, last_status, last_error, last_delivered_at, failures
             FROM team_webhooks ",
            $tail
        )
    };
}

fn to_webhook(r: HookRow) -> Webhook {
    Webhook {
        id: r.0,
        url: r.1,
        kind: r.2,
        actions: r.3,
        enabled: r.4,
        created_at: r.5,
        last_status: r.6,
        last_error: r.7,
        last_delivered_at: r.8,
        failures: r.9,
    }
}

async fn load(state: &AppState, team_id: Uuid, wid: Uuid) -> ApiResult<Webhook> {
    let row: Option<HookRow> = sqlx::query_as(hook_select!("WHERE team_id = $1 AND id = $2"))
        .bind(team_id)
        .bind(wid)
        .fetch_optional(&state.db)
        .await?;
    row.map(to_webhook)
        .ok_or_else(|| Error::not_found("Webhook"))
}

#[utoipa::path(get, path = "/api/v1/teams/{id}/webhooks", tag = "teams", params(("id" = Uuid, Path)),
    responses((status = 200, body = WebhookList)))]
pub async fn list(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<WebhookList>> {
    crate::authz::require(&state, id, auth.user_id(), "webhooks.manage").await?;
    let rows: Vec<HookRow> = sqlx::query_as(hook_select!("WHERE team_id = $1 ORDER BY created_at"))
        .bind(id)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(WebhookList {
        webhooks: rows.into_iter().map(to_webhook).collect(),
    }))
}

#[utoipa::path(post, path = "/api/v1/teams/{id}/webhooks", tag = "teams", params(("id" = Uuid, Path)),
    request_body = CreateWebhookRequest, responses((status = 200, body = Webhook)))]
pub async fn create(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<CreateWebhookRequest>,
) -> ApiResult<Json<Webhook>> {
    crate::authz::require(&state, id, auth.user_id(), "webhooks.manage").await?;
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM team_webhooks WHERE team_id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    if count >= MAX_PER_TEAM {
        return Err(Error::bad_request(format!(
            "At most {MAX_PER_TEAM} webhooks per team"
        )));
    }
    let kind = req.kind.unwrap_or_else(|| "generic".into());
    check_kind(&kind)?;
    let secret = req.secret.trim();
    if secret.len() > 512 || (secret.len() < 8 && kind != "syslog_tls") {
        return Err(Error::bad_request(
            "secret must be 8..=512 characters (syslog_tls: may be empty)",
        ));
    }
    let url = validate_url(&state, &req.url).await?;
    check_url_kind(&kind, &url)?;
    let actions = normalize_actions(req.actions)?;
    let wid = Uuid::new_v4();
    let secret_enc = state.encrypt_secret(SECRET_PURPOSE, secret.as_bytes())?;
    sqlx::query(
        "INSERT INTO team_webhooks (id, team_id, url, secret_enc, kind, actions, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(wid)
    .bind(id)
    .bind(&url)
    .bind(&secret_enc)
    .bind(&kind)
    .bind(&actions)
    .bind(auth.user_id())
    .execute(&state.db)
    .await?;
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "webhook.created").details(
            serde_json::json!({ "webhook_id": wid, "url": url, "kind": kind, "actions": actions }),
        ),
    )
    .await;
    Ok(Json(load(&state, id, wid).await?))
}

#[utoipa::path(patch, path = "/api/v1/teams/{id}/webhooks/{wid}", tag = "teams",
    params(("id" = Uuid, Path), ("wid" = Uuid, Path)),
    request_body = UpdateWebhookRequest, responses((status = 200, body = Webhook)))]
pub async fn update(
    State(state): State<AppState>,
    auth: Auth,
    Path((id, wid)): Path<(Uuid, Uuid)>,
    Body(req): Body<UpdateWebhookRequest>,
) -> ApiResult<Json<Webhook>> {
    crate::authz::require(&state, id, auth.user_id(), "webhooks.manage").await?;
    let cur = load(&state, id, wid).await?;
    let url = match req.url {
        Some(u) => validate_url(&state, &u).await?,
        None => cur.url,
    };
    let kind = req.kind.unwrap_or(cur.kind);
    check_kind(&kind)?;
    let actions = match req.actions {
        Some(a) => normalize_actions(Some(a))?,
        None => cur.actions,
    };
    let secret_enc = match req.secret.as_deref().map(str::trim) {
        Some(s) if s.len() > 512 || (s.len() < 8 && kind != "syslog_tls") => {
            return Err(Error::bad_request("secret must be 8..=512 characters"));
        }
        Some(s) => Some(state.encrypt_secret(SECRET_PURPOSE, s.as_bytes())?),
        None => None,
    };
    let enabled = req.enabled.unwrap_or(cur.enabled);
    check_url_kind(&kind, &url)?;
    sqlx::query(
        "UPDATE team_webhooks SET url = $3, kind = $4, actions = $5, enabled = $6,
             secret_enc = COALESCE($7, secret_enc),
             failures = CASE WHEN $6 AND NOT enabled THEN 0 ELSE failures END,
             last_error = CASE WHEN $6 AND NOT enabled THEN NULL ELSE last_error END
         WHERE team_id = $1 AND id = $2",
    )
    .bind(id)
    .bind(wid)
    .bind(&url)
    .bind(&kind)
    .bind(&actions)
    .bind(enabled)
    .bind(&secret_enc)
    .execute(&state.db)
    .await?;
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "webhook.updated").details(serde_json::json!({
            "webhook_id": wid, "url": url, "kind": kind, "actions": actions, "enabled": enabled,
            "secret_rotated": secret_enc.is_some()
        })),
    )
    .await;
    Ok(Json(load(&state, id, wid).await?))
}

#[utoipa::path(delete, path = "/api/v1/teams/{id}/webhooks/{wid}", tag = "teams",
    params(("id" = Uuid, Path), ("wid" = Uuid, Path)), responses((status = 204)))]
pub async fn delete(
    State(state): State<AppState>,
    auth: Auth,
    Path((id, wid)): Path<(Uuid, Uuid)>,
) -> ApiResult<NoContent> {
    crate::authz::require(&state, id, auth.user_id(), "webhooks.manage").await?;
    let n = sqlx::query("DELETE FROM team_webhooks WHERE team_id = $1 AND id = $2")
        .bind(id)
        .bind(wid)
        .execute(&state.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(Error::not_found("Webhook"));
    }
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "webhook.deleted")
            .details(serde_json::json!({ "webhook_id": wid })),
    )
    .await;
    Ok(NoContent)
}

#[utoipa::path(get, path = "/api/v1/teams/{id}/webhooks/{wid}/deliveries", tag = "teams",
    params(("id" = Uuid, Path), ("wid" = Uuid, Path)), responses((status = 200, body = WebhookDeliveryList)))]
pub async fn deliveries(
    State(state): State<AppState>,
    auth: Auth,
    Path((id, wid)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<WebhookDeliveryList>> {
    crate::authz::require(&state, id, auth.user_id(), "webhooks.manage").await?;
    load(&state, id, wid).await?;
    Ok(Json(WebhookDeliveryList {
        deliveries: recent_deliveries(&state, wid).await?,
    }))
}

async fn recent_deliveries(state: &AppState, wid: Uuid) -> ApiResult<Vec<WebhookDelivery>> {
    type Row = (
        i64,
        i64,
        String,
        i32,
        String,
        Option<i32>,
        Option<String>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT d.id, d.event_id, e.action, d.attempt, d.status, d.response_status, d.response_body,
                d.next_at, d.delivered_at
         FROM audit_deliveries d JOIN team_audit_events e ON e.id = d.event_id
         WHERE d.webhook_id = $1 ORDER BY d.id DESC LIMIT 100",
    )
    .bind(wid)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| WebhookDelivery {
            id: r.0,
            event_id: r.1,
            action: r.2,
            attempt: r.3,
            status: r.4,
            response_status: r.5,
            response_body: r.6,
            next_at: r.7,
            delivered_at: r.8,
        })
        .collect())
}

/// Send a `webhook.test` event through this hook right away (ignoring its
/// action filter) and return the recent deliveries, the test first.
#[utoipa::path(post, path = "/api/v1/teams/{id}/webhooks/{wid}/test", tag = "teams",
    params(("id" = Uuid, Path), ("wid" = Uuid, Path)), responses((status = 200, body = WebhookDeliveryList)))]
pub async fn test(
    State(state): State<AppState>,
    auth: Auth,
    Path((id, wid)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<WebhookDeliveryList>> {
    crate::authz::require(&state, id, auth.user_id(), "webhooks.manage").await?;
    load(&state, id, wid).await?;
    audit::record(
        &state.db,
        audit::Entry::new(id, &auth, "webhook.test")
            .details(serde_json::json!({ "webhook_id": wid })),
    )
    .await;
    sqlx::query(
        "INSERT INTO audit_deliveries (webhook_id, event_id)
         SELECT $1, e.id FROM team_audit_events e
         WHERE e.team_id = $2 AND e.action = 'webhook.test'
           AND NOT EXISTS (SELECT 1 FROM audit_deliveries d WHERE d.webhook_id = $1 AND d.event_id = e.id)
         ORDER BY e.id DESC LIMIT 1",
    )
    .bind(wid)
    .bind(id)
    .execute(&state.db)
    .await?;
    if let Err(e) = deliver_pass(&state, Some(wid)).await {
        tracing::warn!(error = %e, "webhook test delivery failed");
    }
    Ok(Json(WebhookDeliveryList {
        deliveries: recent_deliveries(&state, wid).await?,
    }))
}

/// Background worker: deliver due rows every few seconds.
pub async fn run(state: AppState) {
    loop {
        match deliver_pass(&state, None).await {
            Ok(0) => {}
            Ok(n) => tracing::debug!(count = n, "webhook deliveries"),
            Err(e) => tracing::warn!(error = %e, "webhook delivery pass failed"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

type EventRow = (
    i64,
    Uuid,
    Option<Uuid>,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    String,
    Option<Uuid>,
    Option<Uuid>,
    Option<String>,
    serde_json::Value,
    DateTime<Utc>,
);

fn event_json(r: &EventRow) -> serde_json::Value {
    serde_json::json!({
        "id": r.0,
        "ts": r.11,
        "team_id": r.1,
        "action": r.6,
        "actor": r.2.map(|id| serde_json::json!({ "user_id": id, "email": r.3, "name": r.4 })),
        "device_id": r.5,
        "vault_id": r.7,
        "target_user": r.8,
        "target_email": r.9,
        "details": r.10,
    })
}

/// Body + headers for one batch in the hook's format.
fn render(
    kind: &str,
    secret: &str,
    events: &[serde_json::Value],
    delivery: Uuid,
) -> (Vec<u8>, Vec<(&'static str, String)>) {
    let ts = Utc::now().timestamp().to_string();
    match kind {
        "splunk_hec" => {
            let body: Vec<u8> = events
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "event": e,
                        "time": e["ts"].as_str().and_then(|t| t.parse::<DateTime<Utc>>().ok()).map(|t| t.timestamp()),
                        "sourcetype": "termoso:audit",
                        "source": "termoso",
                    })
                    .to_string()
                })
                .collect::<Vec<_>>()
                .join("\n")
                .into_bytes();
            (
                body,
                vec![
                    ("Content-Type", "application/json".into()),
                    ("Authorization", format!("Splunk {secret}")),
                ],
            )
        }
        "elastic" => {
            let mut body = String::new();
            for e in events {
                body.push_str("{\"create\":{}}\n");
                body.push_str(
                    &serde_json::json!({ "@timestamp": e["ts"], "termoso": e }).to_string(),
                );
                body.push('\n');
            }
            (
                body.into_bytes(),
                vec![
                    ("Content-Type", "application/x-ndjson".into()),
                    ("Authorization", format!("ApiKey {secret}")),
                ],
            )
        }
        "syslog_tls" => (cef_frames(events), vec![]),
        "datadog" => {
            let body = serde_json::Value::Array(
                events
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "ddsource": "termoso", "service": "termoso-audit",
                            "ddtags": format!("team:{}", e["team_id"].as_str().unwrap_or("")),
                            "message": e["action"], "termoso": e,
                        })
                    })
                    .collect(),
            )
            .to_string()
            .into_bytes();
            (
                body,
                vec![
                    ("Content-Type", "application/json".into()),
                    ("DD-API-KEY", secret.to_string()),
                ],
            )
        }
        _ => {
            let body = serde_json::json!({ "events": events })
                .to_string()
                .into_bytes();
            let sig = signature(secret, &ts, &body);
            (
                body,
                vec![
                    ("Content-Type", "application/json".into()),
                    ("X-Termoso-Event-Count", events.len().to_string()),
                    ("X-Termoso-Delivery", delivery.to_string()),
                    ("X-Termoso-Timestamp", ts),
                    ("X-Termoso-Signature", sig),
                ],
            )
        }
    }
}

/// Claim due deliveries (optionally only for one hook) and send them.
/// Returns how many rows were processed.
pub async fn deliver_pass(state: &AppState, only: Option<Uuid>) -> anyhow::Result<usize> {
    type Claim = (i64, Uuid, i64, i32);
    let claimed: Vec<Claim> = sqlx::query_as(
        "UPDATE audit_deliveries SET status = 'sending' WHERE id IN (
             SELECT id FROM audit_deliveries
             WHERE status = 'pending' AND next_at <= now() AND ($1::uuid IS NULL OR webhook_id = $1)
             ORDER BY id LIMIT 500 FOR UPDATE SKIP LOCKED)
         RETURNING id, webhook_id, event_id, attempt",
    )
    .bind(only)
    .fetch_all(&state.db)
    .await?;
    if claimed.is_empty() {
        return Ok(0);
    }
    let mut by_hook: std::collections::BTreeMap<Uuid, Vec<Claim>> = Default::default();
    for c in claimed {
        by_hook.entry(c.1).or_default().push(c);
    }
    let mut processed = 0;
    for (wid, rows) in by_hook {
        for chunk in rows.chunks(BATCH as usize) {
            processed += chunk.len();
            if let Err(e) = send_batch(state, wid, chunk).await {
                tracing::warn!(error = %e, webhook = %wid, "webhook batch failed");
                // Put the rows back so they are retried later.
                let ids: Vec<i64> = chunk.iter().map(|c| c.0).collect();
                sqlx::query("UPDATE audit_deliveries SET status = 'pending', next_at = now() + interval '60 seconds' WHERE id = ANY($1)")
                    .bind(&ids)
                    .execute(&state.db)
                    .await?;
            }
        }
    }
    Ok(processed)
}

async fn send_batch(
    state: &AppState,
    wid: Uuid,
    rows: &[(i64, Uuid, i64, i32)],
) -> anyhow::Result<()> {
    let hook: Option<(Uuid, String, Vec<u8>, String, bool, i32)> = sqlx::query_as(
        "SELECT team_id, url, secret_enc, kind, enabled, failures FROM team_webhooks WHERE id = $1",
    )
    .bind(wid)
    .fetch_optional(&state.db)
    .await?;
    let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let Some((team_id, url, secret_enc, kind, enabled, failures)) = hook else {
        sqlx::query("UPDATE audit_deliveries SET status = 'dead' WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&state.db)
            .await?;
        return Ok(());
    };
    if !enabled {
        sqlx::query("UPDATE audit_deliveries SET status = 'dead' WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&state.db)
            .await?;
        return Ok(());
    }
    let secret = String::from_utf8(state.decrypt_secret(SECRET_PURPOSE, &secret_enc)?)?;
    let event_ids: Vec<i64> = rows.iter().map(|r| r.2).collect();
    let events: Vec<EventRow> = sqlx::query_as(
        "SELECT e.id, e.team_id, e.actor_id, a.email, a.display_name, e.device_id, e.action,
                e.vault_id, e.target_user, t.email, e.details, e.created_at
         FROM team_audit_events e
         LEFT JOIN users a ON a.id = e.actor_id
         LEFT JOIN users t ON t.id = e.target_user
         WHERE e.id = ANY($1) ORDER BY e.id",
    )
    .bind(&event_ids)
    .fetch_all(&state.db)
    .await?;
    let payload: Vec<serde_json::Value> = events.iter().map(event_json).collect();
    let delivery = Uuid::new_v4();
    let (body, headers) = render(&kind, &secret, &payload, delivery);
    let (status, snippet, ok) = if kind == "syslog_tls" {
        send_syslog(state, &url, &body).await
    } else {
        let mut req = client().post(&url).body(body);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        match req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16() as i32;
                let ok = resp.status().is_success();
                let text = resp.text().await.unwrap_or_default();
                let snippet: String = text.chars().take(1024).collect();
                (Some(status), snippet, ok)
            }
            Err(e) => (None, e.to_string().chars().take(1024).collect(), false),
        }
    };
    if ok {
        sqlx::query(
            "UPDATE audit_deliveries SET status = 'ok', attempt = attempt + 1, response_status = $2,
                 response_body = $3, delivered_at = now() WHERE id = ANY($1)",
        )
        .bind(&ids)
        .bind(status)
        .bind(&snippet)
        .execute(&state.db)
        .await?;
        sqlx::query(
            "UPDATE team_webhooks SET last_status = $2, last_error = NULL, last_delivered_at = now(), failures = 0
             WHERE id = $1",
        )
        .bind(wid)
        .bind(status)
        .execute(&state.db)
        .await?;
        metrics::counter!("termoso_webhook_deliveries_total", "result" => "ok")
            .increment(ids.len() as u64);
        return Ok(());
    }
    // Failure: schedule retries per row, count against the hook.
    for (id, _, _, attempt) in rows {
        let next_attempt = attempt + 1;
        if next_attempt >= MAX_ATTEMPTS {
            sqlx::query(
                "UPDATE audit_deliveries SET status = 'dead', attempt = $2, response_status = $3, response_body = $4
                 WHERE id = $1",
            )
            .bind(id)
            .bind(next_attempt)
            .bind(status)
            .bind(&snippet)
            .execute(&state.db)
            .await?;
        } else {
            let secs = BACKOFF_SECS[(next_attempt as usize).min(BACKOFF_SECS.len() - 1)];
            sqlx::query(
                "UPDATE audit_deliveries SET status = 'pending', attempt = $2, response_status = $3,
                     response_body = $4, next_at = now() + make_interval(secs => $5) WHERE id = $1",
            )
            .bind(id)
            .bind(next_attempt)
            .bind(status)
            .bind(&snippet)
            .bind(secs as f64)
            .execute(&state.db)
            .await?;
        }
    }
    let err = match status {
        Some(s) => format!("HTTP {s}"),
        None => snippet.clone(),
    };
    let new_failures = failures + 1;
    let disable = new_failures >= DISABLE_AFTER;
    sqlx::query(
        "UPDATE team_webhooks SET last_status = $2, last_error = $3, failures = $4,
             enabled = CASE WHEN $5 THEN false ELSE enabled END WHERE id = $1",
    )
    .bind(wid)
    .bind(status)
    .bind(&err)
    .bind(new_failures)
    .bind(disable)
    .execute(&state.db)
    .await?;
    metrics::counter!("termoso_webhook_deliveries_total", "result" => "failed")
        .increment(ids.len() as u64);
    if disable {
        audit::record(
            &state.db,
            audit::Entry::system(team_id, "webhook.disabled").details(serde_json::json!({
                "webhook_id": wid, "url": url, "failures": new_failures, "last_error": err
            })),
        )
        .await;
        notify_disabled(state, team_id, wid, &url, new_failures, &err).await;
    }
    Ok(())
}

/// Email everyone in the team who can manage webhooks that this one was
/// switched off. Best effort; failures only log.
async fn notify_disabled(
    state: &AppState,
    team_id: Uuid,
    wid: Uuid,
    url: &str,
    failures: i32,
    last_error: &str,
) {
    let Some(mailer) = state.mailer.as_ref() else {
        return;
    };
    let rows: Vec<(String, String)> = match sqlx::query_as(
        "SELECT u.email, t.name
         FROM team_members m
         JOIN team_roles r ON r.id = m.role_id
         JOIN users u ON u.id = m.user_id
         JOIN teams t ON t.id = m.team_id
         WHERE m.team_id = $1 AND u.email_verified AND NOT u.disabled
           AND ('*' = ANY(r.permissions) OR 'webhooks.manage' = ANY(r.permissions))",
    )
    .bind(team_id)
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = %e, "webhook.disabled: could not list recipients");
            return;
        }
    };
    let Some(team_name) = rows.first().map(|r| r.1.clone()) else {
        return;
    };
    // Scheme + host + path only: query strings may carry tokens.
    let shown = url::Url::parse(url)
        .map(|u| {
            format!(
                "{}://{}{}",
                u.scheme(),
                u.host_str().unwrap_or(""),
                u.path()
            )
        })
        .unwrap_or_else(|_| url.to_string());
    let link = format!(
        "{}/teams/{team_id}/webhooks",
        state.cfg.web_url().trim_end_matches('/')
    );
    let email = crate::mail::Email::new(
        "Audit webhook disabled",
        format!("A webhook in \"{team_name}\" was disabled"),
    )
    .paragraph(format!(
        "The audit webhook {shown} failed {failures} deliveries in a row and was switched off by {}. \
         Events that could not be delivered are kept in the delivery history; new events are not queued for it until it is re-enabled.",
        state.cfg.server_name
    ))
    .paragraph(format!("Last error: {}", last_error.chars().take(300).collect::<String>()))
    .button("Open webhooks", &link)
    .note(format!(
        "You receive this because you can manage webhooks in \"{team_name}\". Webhook id: {wid}."
    ));
    for (to, _) in rows {
        if let Err(e) = mailer.send(&to, &email).await {
            tracing::warn!(error = %e, "could not send webhook.disabled email");
        }
    }
}

// ───────────────────────────── syslog / CEF ──────────────────────────────

fn cef_escape_header(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|")
}

fn cef_escape_ext(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('=', "\\=")
        .replace(['\r', '\n'], " ")
}

/// 0..10 per the CEF spec; mapped from the action family.
fn cef_severity(action: &str) -> u8 {
    match action {
        a if a.ends_with(".deleted") || a.ends_with(".removed") || a.ends_with(".revoked") => 5,
        a if a.starts_with("webhook.disabled") || a.starts_with("scim.rejected") => 6,
        a if a.starts_with("member.") || a.starts_with("role.") || a.starts_with("policy.") => 4,
        a if a.starts_with("sso.") || a.starts_with("scim.") || a.starts_with("token.") => 4,
        _ => 2,
    }
}

/// One RFC 5424 message with a CEF payload per event, octet-counted (RFC 5425).
pub fn cef_frames(events: &[serde_json::Value]) -> Vec<u8> {
    let mut out = Vec::new();
    for e in events {
        let action = e["action"].as_str().unwrap_or("unknown");
        let sev = cef_severity(action);
        // syslog PRI: facility 13 (log audit) × 8 + severity (6 info / 4 warning).
        let pri = 13 * 8 + if sev >= 5 { 4 } else { 6 };
        let ts = e["ts"].as_str().unwrap_or("-");
        let mut ext = Vec::new();
        ext.push(format!(
            "rt={}",
            e["ts"]
                .as_str()
                .and_then(|t| t.parse::<DateTime<Utc>>().ok())
                .map(|t| t.timestamp_millis())
                .unwrap_or(0)
        ));
        ext.push(format!("externalId={}", e["id"]));
        ext.push(format!(
            "cs1Label=team cs1={}",
            cef_escape_ext(e["team_id"].as_str().unwrap_or(""))
        ));
        if let Some(actor) = e["actor"].as_object() {
            if let Some(em) = actor.get("email").and_then(|v| v.as_str()) {
                ext.push(format!("suser={}", cef_escape_ext(em)));
            }
            if let Some(id) = actor.get("user_id").and_then(|v| v.as_str()) {
                ext.push(format!("suid={id}"));
            }
        }
        if let Some(t) = e["target_email"].as_str() {
            ext.push(format!("duser={}", cef_escape_ext(t)));
        }
        if let Some(t) = e["target_user"].as_str() {
            ext.push(format!("duid={t}"));
        }
        if let Some(v) = e["vault_id"].as_str() {
            ext.push(format!("cs2Label=vault cs2={v}"));
        }
        if let Some(d) = e["device_id"].as_str() {
            ext.push(format!("deviceExternalId={d}"));
        }
        if !e["details"].is_null() {
            let d: String = e["details"].to_string().chars().take(4096).collect();
            ext.push(format!("cs3Label=details cs3={}", cef_escape_ext(&d)));
        }
        let cef = format!(
            "CEF:0|Termoso|Termoso|{}|{}|{}|{}|{}",
            env!("CARGO_PKG_VERSION"),
            cef_escape_header(action),
            cef_escape_header(action),
            sev,
            ext.join(" ")
        );
        let msg = format!("<{pri}>1 {ts} termoso termoso-audit - {} - {cef}", e["id"]);
        out.extend_from_slice(format!("{} ", msg.len()).as_bytes());
        out.extend_from_slice(msg.as_bytes());
    }
    out
}

fn tls_connector(state: &AppState) -> anyhow::Result<tokio_rustls::TlsConnector> {
    use rustls_pki_types::pem::PemObject;
    use tokio_rustls::rustls;
    let mut roots = rustls::RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    for c in native.certs {
        let _ = roots.add(c);
    }
    if let Some(path) = state.cfg.webhooks_syslog_ca.as_deref() {
        for c in rustls_pki_types::CertificateDer::pem_file_iter(path)
            .map_err(|e| anyhow::anyhow!("syslog CA {path}: {e}"))?
        {
            let c = c.map_err(|e| anyhow::anyhow!("syslog CA {path}: {e}"))?;
            roots
                .add(c)
                .map_err(|e| anyhow::anyhow!("syslog CA {path}: {e}"))?;
        }
    }
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(cfg)))
}

/// Open a TLS connection, write the frames, half-close, wait for the peer
/// to close. A clean close counts as delivered (syslog has no ack).
async fn send_syslog(state: &AppState, url: &str, body: &[u8]) -> (Option<i32>, String, bool) {
    let attempt = async {
        let u = url::Url::parse(url)?;
        let host = u
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("no host"))?
            .trim_matches(['[', ']'])
            .to_string();
        let port = u.port().unwrap_or(SYSLOG_PORT);
        let connector = tls_connector(state)?;
        let server_name = rustls_pki_types::ServerName::try_from(host.clone())?;
        let tcp = tokio::net::TcpStream::connect((host.as_str(), port)).await?;
        let mut tls = connector.connect(server_name, tcp).await?;
        tls.write_all(body).await?;
        tls.flush().await?;
        tls.shutdown().await?;
        anyhow::Ok(())
    };
    match tokio::time::timeout(std::time::Duration::from_secs(10), attempt).await {
        Ok(Ok(())) => (Some(200), String::new(), true),
        Ok(Err(e)) => (None, e.to_string().chars().take(1024).collect(), false),
        Err(_) => (None, "timed out after 10s".into(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc4231_vector_2() {
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        assert!(signature("Jefe", "1", b"{}").starts_with("sha256="));
    }

    #[test]
    fn private_ranges() {
        for ip in [
            "10.1.2.3",
            "192.168.0.1",
            "172.16.5.5",
            "127.0.0.1",
            "169.254.1.1",
            "100.64.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:10.0.0.1",
        ] {
            assert!(is_private(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700::1111"] {
            assert!(!is_private(ip.parse().unwrap()), "{ip}");
        }
    }
}
