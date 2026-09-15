//! Realtime notifications over WebSocket.
//!
//! Every API instance subscribes once to the Redis events channel and fans
//! out to its local connections through a `tokio::sync::broadcast`. Clients
//! authenticate with their first frame; each connection knows the user and the
//! set of vaults it may hear about (refreshed on `VaultsUpdated`).

use std::collections::HashSet;
use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tokio::time::{interval, timeout};
use uuid::Uuid;

use termoso_proto::ws::{ClientMessage, ServerMessage};

use crate::events::Event;
use crate::presence;
use crate::session;
use crate::state::AppState;

const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const PING_EVERY: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(90);

/// Redis → local broadcast bridge. Runs for the lifetime of the process and
/// reconnects on failure.
pub async fn run_fanout(state: AppState) {
    loop {
        if let Err(e) = fanout_once(&state).await {
            tracing::warn!(error = %e, "event subscription lost; reconnecting");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn fanout_once(state: &AppState) -> anyhow::Result<()> {
    let mut pubsub = state.cache.client().get_async_pubsub().await?;
    pubsub.subscribe(state.cache.events_channel()).await?;
    let mut stream = pubsub.on_message();
    while let Some(msg) = stream.next().await {
        let payload: Vec<u8> = msg.get_payload()?;
        match serde_json::from_slice::<Event>(&payload) {
            Ok(ev) => {
                let _ = state.events.send(ev);
            }
            Err(e) => tracing::warn!(error = %e, "malformed event on bus"),
        }
    }
    anyhow::bail!("pubsub stream ended")
}

#[utoipa::path(get, path = "/api/v1/ws", tag = "realtime", responses((status = 101)))]
pub async fn handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(16 * 1024)
        .on_upgrade(move |socket| async move {
            if let Err(e) = serve(state, socket).await {
                tracing::debug!(error = %e, "ws closed");
            }
        })
}

async fn send(socket: &mut WebSocket, msg: &ServerMessage) -> anyhow::Result<()> {
    socket
        .send(Message::Text(serde_json::to_string(msg)?.into()))
        .await?;
    Ok(())
}

async fn close_with(socket: &mut WebSocket, code: &str, message: &str) -> anyhow::Result<()> {
    send(
        socket,
        &ServerMessage::Error {
            code: code.into(),
            message: message.into(),
        },
    )
    .await?;
    socket.close().await.ok();
    Ok(())
}

async fn vault_ids(state: &AppState, user_id: Uuid) -> anyhow::Result<HashSet<Uuid>> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT vm.vault_id FROM vault_members vm JOIN vaults v ON v.id = vm.vault_id
         WHERE vm.user_id = $1 AND v.deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

async fn team_ids(state: &AppState, user_id: Uuid) -> anyhow::Result<HashSet<Uuid>> {
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT team_id FROM team_members WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

async fn serve(state: AppState, mut socket: WebSocket) -> anyhow::Result<()> {
    // 1. auth frame
    let first = match timeout(AUTH_TIMEOUT, socket.recv()).await {
        Ok(Some(Ok(Message::Text(t)))) => t,
        _ => return close_with(&mut socket, "auth_required", "First frame must be auth").await,
    };
    let token = match serde_json::from_str::<ClientMessage>(&first) {
        Ok(ClientMessage::Auth { token }) => token,
        _ => return close_with(&mut socket, "auth_required", "First frame must be auth").await,
    };
    let info = match session::validate(&state, &token).await {
        Ok(i) if !i.disabled => i,
        _ => return close_with(&mut socket, "unauthorized", "Invalid session").await,
    };
    let user_id = info.user_id;
    let session_id = info.session_id;
    let mut vaults = vault_ids(&state, user_id).await?;
    let mut teams = team_ids(&state, user_id).await?;
    let mut reporter = presence::Reporter::new(user_id, info.device_id);
    send(
        &mut socket,
        &ServerMessage::Hello {
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            vault_ids: vaults.iter().copied().collect(),
        },
    )
    .await?;
    metrics::gauge!("termoso_ws_connections").increment(1.0);
    let result = pump(
        &state,
        &mut socket,
        user_id,
        session_id,
        &mut vaults,
        &mut teams,
        &mut reporter,
    )
    .await;
    metrics::gauge!("termoso_ws_connections").decrement(1.0);
    if let Err(e) = reporter.clear(&state).await {
        tracing::debug!(error = %e, "could not clear presence on disconnect");
    }
    result
}

async fn pump(
    state: &AppState,
    socket: &mut WebSocket,
    user_id: Uuid,
    session_id: Uuid,
    vaults: &mut HashSet<Uuid>,
    teams: &mut HashSet<Uuid>,
    reporter: &mut presence::Reporter,
) -> anyhow::Result<()> {
    let mut rx = state.events.subscribe();
    let mut ping = interval(PING_EVERY);
    ping.tick().await;
    let mut last_seen = tokio::time::Instant::now();

    loop {
        tokio::select! {
            ev = rx.recv() => {
                let ev = match ev {
                    Ok(ev) => ev,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!(lagged = n, "ws subscriber lagged; asking client to resync");
                        // Cheapest recovery: tell the client everything may have changed.
                        *vaults = vault_ids(state, user_id).await?;
                        send(socket, &ServerMessage::VaultsUpdated).await?;
                        for v in vaults.iter() {
                            send(socket, &ServerMessage::VaultChanged { vault_id: *v, seq: 0, device_id: None }).await?;
                        }
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => anyhow::bail!("event bus closed"),
                };
                let out = match ev {
                    Event::VaultChanged { vault_id, seq, device_id } if vaults.contains(&vault_id) => {
                        Some(ServerMessage::VaultChanged { vault_id, seq, device_id })
                    }
                    Event::VaultChanged { .. } => None,
                    Event::VaultsUpdated { user_ids } if user_ids.contains(&user_id) => {
                        *vaults = vault_ids(state, user_id).await?;
                        Some(ServerMessage::VaultsUpdated)
                    }
                    Event::TeamsUpdated { user_ids } if user_ids.contains(&user_id) => {
                        *teams = team_ids(state, user_id).await?;
                        Some(ServerMessage::TeamsUpdated)
                    }
                    Event::PresenceChanged { team_id } if teams.contains(&team_id) => Some(ServerMessage::PresenceChanged { team_id }),
                    Event::HistoryChanged { user_id: u, seq } if u == user_id => Some(ServerMessage::HistoryChanged { seq }),
                    Event::LogsChanged { user_id: u, seq } if u == user_id => Some(ServerMessage::LogsChanged { seq }),
                    Event::VaultLogsChanged { vault_id, seq } if vaults.contains(&vault_id) => {
                        Some(ServerMessage::VaultLogsChanged { vault_id, seq })
                    }
                    Event::AccountUpdated { user_id: u } if u == user_id => Some(ServerMessage::AccountUpdated),
                    Event::SessionRevoked { user_id: u, session_id: s } if u == user_id && (s == session_id || s.is_nil()) => {
                        send(socket, &ServerMessage::SessionRevoked).await?;
                        socket.close().await.ok();
                        return Ok(());
                    }
                    _ => None,
                };
                if let Some(m) = out {
                    send(socket, &m).await?;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        last_seen = tokio::time::Instant::now();
                        match serde_json::from_str::<ClientMessage>(&t) {
                            Ok(ClientMessage::Ping) => send(socket, &ServerMessage::Pong).await?,
                            Ok(ClientMessage::Presence { sessions }) => {
                                if let Err(e) = reporter.report(state, sessions).await {
                                    tracing::debug!(error = %e, "presence report failed");
                                }
                            }
                            Ok(ClientMessage::Auth { .. }) => {
                                return close_with(socket, "protocol", "Already authenticated").await;
                            }
                            Err(_) => return close_with(socket, "protocol", "Malformed frame").await,
                        }
                    }
                    Some(Ok(Message::Pong(_))) | Some(Ok(Message::Ping(_))) => last_seen = tokio::time::Instant::now(),
                    Some(Ok(Message::Binary(_))) => return close_with(socket, "protocol", "Binary frames not supported").await,
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    Some(Err(e)) => return Err(e.into()),
                }
            }
            _ = ping.tick() => {
                if last_seen.elapsed() > IDLE_TIMEOUT {
                    socket.close().await.ok();
                    return Ok(());
                }
                // Re-validate the session periodically so revocations that
                // happened on another instance without an event still apply.
                if session::is_revoked(state, session_id).await? {
                    send(socket, &ServerMessage::SessionRevoked).await?;
                    socket.close().await.ok();
                    return Ok(());
                }
                socket.send(Message::Ping(Vec::new().into())).await?;
            }
        }
    }
}
