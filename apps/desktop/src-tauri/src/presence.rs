//! Team presence: tell teammates which team-vault hosts this device is on.
//!
//! Only routing metadata leaves the machine (vault, host, protocol, start
//! time); hosts in the local or personal vault are never reported.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use tauri::{AppHandle, Manager, Runtime};
use termoso_core::model::{Host, PfRule};
use termoso_core::store::LocalVaultKind;
use termoso_proto::account::UserProfile;
use termoso_proto::team::{PresenceSession, TeamPresence};
use uuid::Uuid;

use crate::error::Result;
use crate::sessions::SessionState;
use crate::state::AppState;

/// Recompute this device's team-vault connections and hand them to the sync
/// engine, which publishes them over the account WebSocket. Cheap; called
/// after every session, SFTP or tunnel open/close and when the engine starts.
pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tokio::spawn(async move {
        let Some(engine) = crate::account::engine(&app).await else {
            return;
        };
        let state = app.state::<AppState>();
        engine.set_presence(collect(&state));
    });
}

/// Team-vault sessions open on this device, across terminals (SSH / Mosh /
/// Telnet), SFTP browsers and running port-forwarding tunnels.
pub(crate) fn collect(state: &AppState) -> Vec<PresenceSession> {
    let team_vaults: HashSet<Uuid> = state
        .store
        .vaults()
        .unwrap_or_default()
        .into_iter()
        .filter(|v| v.kind == LocalVaultKind::Team)
        .map(|v| v.id)
        .collect();
    if team_vaults.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut push = |host_id: Uuid, protocol: &str, since: DateTime<Utc>| {
        let Ok(host) = state.store.require::<Host>(host_id) else {
            return;
        };
        if team_vaults.contains(&host.vault_id) {
            out.push(PresenceSession {
                vault_id: host.vault_id,
                host_id,
                protocol: protocol.to_string(),
                since,
            });
        }
    };
    for s in state.sessions.list() {
        if let (Some(host_id), SessionState::Connected) = (s.host_id, s.state) {
            push(host_id, &s.protocol, s.started_at);
        }
    }
    for s in state.sftp.list() {
        if let Some(host_id) = s.host_id {
            push(host_id, "sftp", s.started_at);
        }
    }
    for (rule_id, since) in state.forwards.running_since() {
        if let Ok(rule) = state.store.require::<PfRule>(rule_id) {
            push(rule.data.host_id, "forward", since);
        }
    }
    out
}

/// Who is connected to the team's hosts right now (as the server sees it).
pub async fn team<R: Runtime>(app: &AppHandle<R>, team_id: Uuid) -> Result<TeamPresence> {
    let api = crate::account::api(app).await?;
    Ok(api.team_presence(team_id).await?)
}

/// Server-side profile, including whether this account hides its presence.
pub async fn profile<R: Runtime>(app: &AppHandle<R>) -> Result<UserProfile> {
    let api = crate::account::api(app).await?;
    Ok(api.account().await?.user)
}

/// Hide (or show again) this account from teammates' presence views.
pub async fn set_hidden<R: Runtime>(app: &AppHandle<R>, hidden: bool) -> Result<UserProfile> {
    let api = crate::account::api(app).await?;
    Ok(api.set_presence_hidden(hidden).await?)
}
