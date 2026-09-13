//! Multiplayer: relay for shared live terminals.
//!
//! The host creates a session (`POST /live`) with a *join token* derived from
//! the link secret; the server keeps only its hash. Everyone then connects to
//! `GET /live/{id}/ws`: the host without a join token, viewers with it.
//! Binary frames are opaque, end-to-end encrypted payloads the relay forwards
//! host → viewers and (with remote control granted) viewer → host. Presence
//! lives in a Redis hash so several API instances see the same participants,
//! and frames cross instances over one Redis channel ([`BusFrame`]).
//!
//! A team can switch multiplayer off for its members (`teams.multiplayer_enabled`).

use std::time::Duration;

use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use sqlx::AssertSqlSafe;
use termoso_proto::error::codes;
use termoso_proto::live::*;
use tokio::sync::broadcast;
use tokio::time::{interval, timeout};
use uuid::Uuid;

use crate::error::{ApiResult, Error, NoContent};
use crate::extract::{Auth, Json as Body};
use crate::session;
use crate::state::AppState;
use crate::util::hash_token;

/// Links stop working after this even if the host never presses Stop.
const SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Presence hash TTL; refreshed on every change and by the host's pings.
const PRESENCE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const PING_EVERY: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(90);
/// Largest single relayed frame (encrypted terminal output chunk).
const MAX_FRAME: usize = 256 * 1024;
const MAX_ACTIVE_PER_HOST: i64 = 10;

/// Error code when a team of the user disabled multiplayer.
pub const MULTIPLAYER_DISABLED: &str = "multiplayer_disabled";

// ───────────────────────── bus ─────────────────────────

/// What travels between instances on the live channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusKind {
    /// Opaque encrypted payload; `to == None` means every viewer.
    Data,
    /// Presence changed: re-read the hash and tell the socket owner.
    Presence,
    /// Remote control for `to` changed to `data[0] == 1`.
    Control,
    /// Host stopped multiplayer.
    Ended,
}

impl BusKind {
    fn tag(self) -> u8 {
        match self {
            BusKind::Data => 0,
            BusKind::Presence => 1,
            BusKind::Control => 2,
            BusKind::Ended => 3,
        }
    }
    fn from_tag(t: u8) -> Option<Self> {
        Some(match t {
            0 => BusKind::Data,
            1 => BusKind::Presence,
            2 => BusKind::Control,
            3 => BusKind::Ended,
            _ => return None,
        })
    }
}

/// One relayed frame. Binary layout on Redis:
/// `session(16) ‖ from(16) ‖ to(16, nil = broadcast) ‖ kind(1) ‖ data`.
#[derive(Debug, Clone)]
pub struct BusFrame {
    pub session_id: Uuid,
    pub from: Uuid,
    pub to: Option<Uuid>,
    pub kind: BusKind,
    pub data: Bytes,
}

impl BusFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(49 + self.data.len());
        out.extend_from_slice(self.session_id.as_bytes());
        out.extend_from_slice(self.from.as_bytes());
        out.extend_from_slice(self.to.unwrap_or(Uuid::nil()).as_bytes());
        out.push(self.kind.tag());
        out.extend_from_slice(&self.data);
        out
    }

    pub fn decode(raw: &[u8]) -> Option<Self> {
        if raw.len() < 49 {
            return None;
        }
        let uuid = |i: usize| Uuid::from_slice(&raw[i * 16..(i + 1) * 16]).ok();
        let to = uuid(2)?;
        Some(Self {
            session_id: uuid(0)?,
            from: uuid(1)?,
            to: (!to.is_nil()).then_some(to),
            kind: BusKind::from_tag(raw[48])?,
            data: Bytes::copy_from_slice(&raw[49..]),
        })
    }
}

async fn publish(state: &AppState, frame: BusFrame) -> ApiResult<()> {
    state.cache.publish_live(frame.encode()).await
}

/// Redis → local broadcast bridge for the live channel; runs for the
/// process lifetime and reconnects on failure.
pub async fn run_fanout(state: AppState) {
    loop {
        if let Err(e) = fanout_once(&state).await {
            tracing::warn!(error = %e, "live channel subscription lost; reconnecting");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn fanout_once(state: &AppState) -> anyhow::Result<()> {
    let mut pubsub = state.cache.client().get_async_pubsub().await?;
    pubsub.subscribe(state.cache.live_channel()).await?;
    let mut stream = pubsub.on_message();
    while let Some(msg) = stream.next().await {
        let payload: Vec<u8> = msg.get_payload()?;
        match BusFrame::decode(&payload) {
            Some(f) => {
                let _ = state.live.send(f);
            }
            None => tracing::warn!("malformed frame on live channel"),
        }
    }
    anyhow::bail!("live pubsub stream ended")
}

// ───────────────────────── storage ─────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct Row {
    id: Uuid,
    host_user_id: Uuid,
    join_token_hash: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
}

impl Row {
    fn active(&self) -> bool {
        self.ended_at.is_none() && self.expires_at > Utc::now()
    }
    fn dto(self) -> LiveSession {
        LiveSession {
            id: self.id,
            host_user_id: self.host_user_id,
            created_at: self.created_at,
            expires_at: self.expires_at,
            ended_at: self.ended_at,
        }
    }
}

const COLUMNS: &str = "id, host_user_id, join_token_hash, created_at, expires_at, ended_at";

async fn load(state: &AppState, id: Uuid) -> ApiResult<Row> {
    sqlx::query_as::<_, Row>(AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM live_sessions WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| Error::not_found("Live session"))
}

/// Deny when any team the user belongs to switched multiplayer off.
async fn ensure_allowed(state: &AppState, user_id: Uuid) -> ApiResult<()> {
    let (disabled,): (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM team_members m JOIN teams t ON t.id = m.team_id
         WHERE m.user_id = $1 AND NOT t.multiplayer_enabled)",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if disabled {
        return Err(Error::new(
            StatusCode::FORBIDDEN,
            MULTIPLAYER_DISABLED,
            "Multiplayer is disabled for your team",
        ));
    }
    Ok(())
}

async fn end_session(state: &AppState, row: &Row, by: Uuid) -> ApiResult<()> {
    let updated =
        sqlx::query("UPDATE live_sessions SET ended_at = now() WHERE id = $1 AND ended_at IS NULL")
            .bind(row.id)
            .execute(&state.db)
            .await?
            .rows_affected();
    state.cache.del(&presence_key(row.id)).await?;
    if updated > 0 {
        publish(
            state,
            BusFrame {
                session_id: row.id,
                from: by,
                to: None,
                kind: BusKind::Ended,
                data: Bytes::new(),
            },
        )
        .await?;
    }
    Ok(())
}

// ───────────────────────── presence ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Presence {
    user_id: Uuid,
    email: String,
    display_name: Option<String>,
    is_host: bool,
    can_write: bool,
    /// Socket that owns this entry; a second device of the same user replaces it.
    conn: Uuid,
    joined_at: DateTime<Utc>,
}

impl Presence {
    fn dto(&self) -> LiveParticipant {
        LiveParticipant {
            user_id: self.user_id,
            email: self.email.clone(),
            display_name: self.display_name.clone(),
            is_host: self.is_host,
            can_write: self.can_write,
        }
    }
}

fn presence_key(session_id: Uuid) -> String {
    format!("live:{session_id}:members")
}

async fn participants(state: &AppState, session_id: Uuid) -> ApiResult<Vec<LiveParticipant>> {
    let mut all: Vec<Presence> = state.cache.hgetall_json(&presence_key(session_id)).await?;
    all.sort_by(|a, b| {
        b.is_host
            .cmp(&a.is_host)
            .then(a.joined_at.cmp(&b.joined_at))
    });
    Ok(all.iter().map(Presence::dto).collect())
}

async fn presence_changed(state: &AppState, session_id: Uuid, by: Uuid) -> ApiResult<()> {
    publish(
        state,
        BusFrame {
            session_id,
            from: by,
            to: None,
            kind: BusKind::Presence,
            data: Bytes::new(),
        },
    )
    .await
}

// ───────────────────────── REST ─────────────────────────

/// Start sharing: register the session so viewers holding the link can join.
#[utoipa::path(post, path = "/api/v1/live", tag = "live",
    request_body = CreateLiveSessionRequest,
    responses((status = 201, body = LiveSession), (status = 403, description = "multiplayer_disabled")))]
pub async fn create(
    State(state): State<AppState>,
    auth: Auth,
    Body(req): Body<CreateLiveSessionRequest>,
) -> ApiResult<(StatusCode, Json<LiveSession>)> {
    let user_id = auth.user_id();
    ensure_allowed(&state, user_id).await?;
    if req.join_token.len() < 32 || req.join_token.len() > 256 {
        return Err(Error::bad_request("Invalid join token"));
    }
    let (active,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM live_sessions WHERE host_user_id = $1 AND ended_at IS NULL AND expires_at > now()",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if active >= MAX_ACTIVE_PER_HOST {
        return Err(Error::conflict("Too many active multiplayer sessions"));
    }
    let id = Uuid::new_v4();
    let expires_at = Utc::now() + chrono::Duration::from_std(SESSION_TTL).unwrap_or_default();
    let row = sqlx::query_as::<_, Row>(AssertSqlSafe(format!(
        "INSERT INTO live_sessions (id, host_user_id, join_token_hash, expires_at)
         VALUES ($1, $2, $3, $4) RETURNING {COLUMNS}"
    )))
    .bind(id)
    .bind(user_id)
    .bind(hash_token(&req.join_token))
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;
    metrics::counter!("termoso_live_sessions_total").increment(1);
    Ok((StatusCode::CREATED, Json(row.dto())))
}

/// The caller's active sessions (as host).
#[utoipa::path(get, path = "/api/v1/live", tag = "live", responses((status = 200, body = LiveSessionList)))]
pub async fn list(State(state): State<AppState>, auth: Auth) -> ApiResult<Json<LiveSessionList>> {
    let rows = sqlx::query_as::<_, Row>(AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM live_sessions
         WHERE host_user_id = $1 AND ended_at IS NULL AND expires_at > now()
         ORDER BY created_at DESC"
    )))
    .bind(auth.user_id())
    .fetch_all(&state.db)
    .await?;
    Ok(Json(LiveSessionList {
        sessions: rows.into_iter().map(Row::dto).collect(),
    }))
}

/// Stop sharing: viewers are disconnected and the link stops working.
#[utoipa::path(post, path = "/api/v1/live/{id}/stop", tag = "live",
    params(("id" = Uuid, Path)), responses((status = 204)))]
pub async fn stop(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<NoContent> {
    let row = load(&state, id).await?;
    if row.host_user_id != auth.user_id() {
        return Err(Error::forbidden("Only the host can stop multiplayer"));
    }
    end_session(&state, &row, auth.user_id()).await?;
    Ok(NoContent)
}

// ───────────────────────── relay socket ─────────────────────────

/// Relay socket. First frame: `{"type":"auth","token":…,"join_token":…}`.
#[utoipa::path(get, path = "/api/v1/live/{id}/ws", tag = "live",
    params(("id" = Uuid, Path)), responses((status = 101)))]
pub async fn ws(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    ws.max_message_size(MAX_FRAME)
        .on_upgrade(move |socket| async move {
            if let Err(e) = serve(state, id, socket).await {
                tracing::debug!(error = %e, "live ws closed");
            }
        })
        .into_response()
}

async fn send(socket: &mut WebSocket, msg: &LiveServerMessage) -> anyhow::Result<()> {
    socket
        .send(Message::Text(serde_json::to_string(msg)?.into()))
        .await?;
    Ok(())
}

async fn close_with(socket: &mut WebSocket, code: &str, message: &str) -> anyhow::Result<()> {
    send(
        socket,
        &LiveServerMessage::Error {
            code: code.into(),
            message: message.into(),
        },
    )
    .await?;
    socket.close().await.ok();
    Ok(())
}

struct Conn {
    id: Uuid,
    session_id: Uuid,
    user_id: Uuid,
    host_user_id: Uuid,
    is_host: bool,
    can_write: bool,
}

async fn serve(state: AppState, session_id: Uuid, mut socket: WebSocket) -> anyhow::Result<()> {
    let first = match timeout(AUTH_TIMEOUT, socket.recv()).await {
        Ok(Some(Ok(Message::Text(t)))) => t,
        _ => return close_with(&mut socket, "auth_required", "First frame must be auth").await,
    };
    let (token, join_token) = match serde_json::from_str::<LiveClientMessage>(&first) {
        Ok(LiveClientMessage::Auth { token, join_token }) => (token, join_token),
        _ => return close_with(&mut socket, "auth_required", "First frame must be auth").await,
    };
    let info = match session::validate(&state, &token).await {
        Ok(i) if !i.disabled => i,
        _ => return close_with(&mut socket, codes::UNAUTHORIZED, "Invalid session").await,
    };
    let row = match load(&state, session_id).await {
        Ok(r) if r.active() => r,
        Ok(_) => return close_with(&mut socket, "ended", "Multiplayer has ended").await,
        Err(_) => return close_with(&mut socket, codes::NOT_FOUND, "Unknown session").await,
    };
    let is_host = row.host_user_id == info.user_id;
    if !is_host {
        let ok = join_token
            .as_deref()
            .map(|t| {
                use subtle::ConstantTimeEq;
                hash_token(t)
                    .as_bytes()
                    .ct_eq(row.join_token_hash.as_bytes())
                    .into()
            })
            .unwrap_or(false);
        if !ok {
            return close_with(&mut socket, codes::FORBIDDEN, "Invalid link").await;
        }
    }
    if let Err(e) = ensure_allowed(&state, info.user_id).await {
        tracing::debug!(error = %e, "live join denied");
        return close_with(
            &mut socket,
            MULTIPLAYER_DISABLED,
            "Multiplayer is disabled for your team",
        )
        .await;
    }
    let user = crate::users::by_id(&state.db, info.user_id).await?;

    let conn = Conn {
        id: Uuid::new_v4(),
        session_id,
        user_id: info.user_id,
        host_user_id: row.host_user_id,
        is_host,
        can_write: is_host,
    };
    let key = presence_key(session_id);
    state
        .cache
        .hset_json(
            &key,
            &conn.user_id.to_string(),
            &Presence {
                user_id: conn.user_id,
                email: user.email,
                display_name: user.display_name,
                is_host,
                can_write: is_host,
                conn: conn.id,
                joined_at: Utc::now(),
            },
            PRESENCE_TTL,
        )
        .await?;
    // Subscribe before announcing so we don't miss our own presence frame.
    let rx = state.live.subscribe();
    presence_changed(&state, session_id, conn.user_id).await?;
    send(
        &mut socket,
        &LiveServerMessage::Hello {
            session_id,
            user_id: conn.user_id,
            is_host,
            participants: participants(&state, session_id).await?,
        },
    )
    .await?;
    metrics::gauge!("termoso_live_connections").increment(1.0);
    let result = pump(&state, &mut socket, &conn, rx).await;
    metrics::gauge!("termoso_live_connections").decrement(1.0);

    // Leave: drop our presence entry unless a newer connection of the same user replaced it.
    let mine: Option<Presence> = state
        .cache
        .hget_json(&key, &conn.user_id.to_string())
        .await?;
    if mine.is_some_and(|p| p.conn == conn.id) {
        state.cache.hdel(&key, &conn.user_id.to_string()).await?;
        if is_host {
            // The terminal is gone with the host; end the session for everyone.
            if let Ok(row) = load(&state, session_id).await {
                end_session(&state, &row, conn.user_id).await?;
            }
        } else {
            presence_changed(&state, session_id, conn.user_id).await?;
        }
    }
    result
}

async fn pump(
    state: &AppState,
    socket: &mut WebSocket,
    conn: &Conn,
    mut rx: broadcast::Receiver<BusFrame>,
) -> anyhow::Result<()> {
    let mut can_write = conn.can_write;
    let mut ping = interval(PING_EVERY);
    ping.tick().await;
    let mut last_seen = tokio::time::Instant::now();
    loop {
        tokio::select! {
            ev = rx.recv() => {
                let frame = match ev {
                    Ok(f) => f,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!(lagged = n, "live subscriber lagged");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if frame.session_id != conn.session_id {
                    continue;
                }
                match frame.kind {
                    BusKind::Data => {
                        let for_me = match frame.to {
                            Some(to) => to == conn.user_id && frame.from != conn.user_id,
                            None => !conn.is_host && frame.from == conn.host_user_id,
                        };
                        if for_me {
                            socket.send(Message::Binary(frame.data)).await?;
                        }
                    }
                    BusKind::Presence => {
                        let participants = participants(state, conn.session_id).await?;
                        if let Some(me) = participants.iter().find(|p| p.user_id == conn.user_id)
                            && me.can_write != can_write
                        {
                            can_write = me.can_write;
                            send(socket, &LiveServerMessage::Control { can_write }).await?;
                        }
                        send(socket, &LiveServerMessage::Participants { participants }).await?;
                    }
                    BusKind::Control => {
                        if frame.to == Some(conn.user_id) {
                            can_write = frame.data.first() == Some(&1);
                            send(socket, &LiveServerMessage::Control { can_write }).await?;
                        }
                    }
                    BusKind::Ended => {
                        send(socket, &LiveServerMessage::Ended).await?;
                        socket.close().await.ok();
                        return Ok(());
                    }
                }
            }
            msg = socket.recv() => {
                let Some(msg) = msg else { break };
                last_seen = tokio::time::Instant::now();
                match msg? {
                    Message::Binary(data) => {
                        if conn.is_host {
                            publish(state, BusFrame {
                                session_id: conn.session_id,
                                from: conn.user_id,
                                to: None,
                                kind: BusKind::Data,
                                data,
                            }).await?;
                        } else if can_write {
                            publish(state, BusFrame {
                                session_id: conn.session_id,
                                from: conn.user_id,
                                to: Some(conn.host_user_id),
                                kind: BusKind::Data,
                                data,
                            }).await?;
                        }
                        // Viewers without control: silently dropped.
                    }
                    Message::Text(t) => match serde_json::from_str::<LiveClientMessage>(&t) {
                        Ok(LiveClientMessage::Ping) => {
                            send(socket, &LiveServerMessage::Pong).await?;
                        }
                        Ok(LiveClientMessage::Control { user_id, enabled }) if conn.is_host => {
                            set_control(state, conn, user_id, enabled).await?;
                        }
                        Ok(LiveClientMessage::Direct { user_id, data }) if conn.is_host => {
                            use base64::Engine;
                            let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) else {
                                continue;
                            };
                            publish(state, BusFrame {
                                session_id: conn.session_id,
                                from: conn.user_id,
                                to: Some(user_id),
                                kind: BusKind::Data,
                                data: Bytes::from(bytes),
                            }).await?;
                        }
                        Ok(LiveClientMessage::Auth { .. }) => {
                            return close_with(socket, "protocol", "Already authenticated").await;
                        }
                        Ok(_) => {} // viewer sent a host-only frame
                        Err(_) => return close_with(socket, "protocol", "Malformed frame").await,
                    },
                    Message::Close(_) => break,
                    Message::Ping(_) | Message::Pong(_) => {}
                }
            }
            _ = ping.tick() => {
                if last_seen.elapsed() > IDLE_TIMEOUT {
                    tracing::debug!("live ws idle; closing");
                    socket.close().await.ok();
                    break;
                }
                socket.send(Message::Ping(Vec::new().into())).await?;
            }
        }
    }
    Ok(())
}

async fn set_control(state: &AppState, host: &Conn, user_id: Uuid, enabled: bool) -> ApiResult<()> {
    let key = presence_key(host.session_id);
    let Some(mut p): Option<Presence> = state.cache.hget_json(&key, &user_id.to_string()).await?
    else {
        return Ok(());
    };
    if p.is_host || p.can_write == enabled {
        return Ok(());
    }
    p.can_write = enabled;
    state
        .cache
        .hset_json(&key, &user_id.to_string(), &p, PRESENCE_TTL)
        .await?;
    presence_changed(state, host.session_id, host.user_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_frame_roundtrip() {
        let f = BusFrame {
            session_id: Uuid::new_v4(),
            from: Uuid::new_v4(),
            to: None,
            kind: BusKind::Data,
            data: Bytes::from_static(b"hello"),
        };
        let d = BusFrame::decode(&f.encode()).unwrap();
        assert_eq!(d.session_id, f.session_id);
        assert_eq!(d.from, f.from);
        assert_eq!(d.to, None);
        assert_eq!(d.kind, BusKind::Data);
        assert_eq!(&d.data[..], b"hello");
        let to = Uuid::new_v4();
        let g = BusFrame {
            to: Some(to),
            kind: BusKind::Control,
            data: Bytes::from_static(&[1]),
            ..f
        };
        let d = BusFrame::decode(&g.encode()).unwrap();
        assert_eq!(d.to, Some(to));
        assert_eq!(d.kind, BusKind::Control);
        assert!(BusFrame::decode(&[0u8; 10]).is_none());
    }
}
