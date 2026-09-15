//! Team presence: which team-vault hosts each member's devices are connected
//! to right now.
//!
//! Devices report their live connections over the realtime socket; the
//! server keeps them in Redis (one hash per team, one field per device) and
//! fans out a `PresenceChanged` notification so clients re-pull the snapshot.
//! Only routing metadata is stored – vault id, host id, protocol, start time –
//! never what is typed or shown. Nothing survives a restart, and a device
//! that stops reporting is dropped after [`STALE_AFTER`].

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use termoso_proto::team::{PresenceEntry, PresenceSession, TeamPresence};
use uuid::Uuid;

use crate::error::ApiResult;
use crate::events::{self, Event};
use crate::state::AppState;

/// A device that has not reported for this long is treated as gone.
pub const STALE_AFTER: Duration = Duration::from_secs(150);
/// Redis TTL of a team's hash, refreshed on every report.
const HASH_TTL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stored {
    user_id: Uuid,
    device_id: Uuid,
    sessions: Vec<PresenceSession>,
    seen_at: DateTime<Utc>,
}

fn key(team_id: Uuid) -> String {
    format!("presence:{team_id}")
}

fn field(user_id: Uuid, device_id: Uuid) -> String {
    format!("{user_id}:{device_id}")
}

/// One connection's view of what it has reported, so heartbeats that change
/// nothing refresh the TTL without waking every teammate.
pub struct Reporter {
    user_id: Uuid,
    device_id: Uuid,
    reported: HashMap<Uuid, Vec<PresenceSession>>,
}

impl Reporter {
    pub fn new(user_id: Uuid, device_id: Uuid) -> Self {
        Self {
            user_id,
            device_id,
            reported: HashMap::new(),
        }
    }

    /// Store the device's full list of live connections. Sessions in vaults
    /// the user cannot see, in personal vaults, or in teams with presence
    /// turned off are dropped; a hidden user stores nothing.
    pub async fn report(
        &mut self,
        state: &AppState,
        sessions: Vec<PresenceSession>,
    ) -> ApiResult<()> {
        let hidden = user_hidden(state, self.user_id).await?;
        let mut by_team: HashMap<Uuid, Vec<PresenceSession>> = HashMap::new();
        if !hidden && !sessions.is_empty() {
            let vault_ids: Vec<Uuid> = sessions
                .iter()
                .map(|s| s.vault_id)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let teams = team_of_vaults(state, self.user_id, &vault_ids).await?;
            for s in sessions {
                if let Some(team_id) = teams.get(&s.vault_id) {
                    by_team.entry(*team_id).or_default().push(s);
                }
            }
        }
        for sessions in by_team.values_mut() {
            sessions.sort_by(|a, b| a.since.cmp(&b.since).then(a.host_id.cmp(&b.host_id)));
            sessions.dedup();
        }

        let now = Utc::now();
        let previous: HashSet<Uuid> = self.reported.keys().copied().collect();
        for (team_id, sessions) in &by_team {
            let stored = Stored {
                user_id: self.user_id,
                device_id: self.device_id,
                sessions: sessions.clone(),
                seen_at: now,
            };
            let added = state
                .cache
                .hset_json(
                    &key(*team_id),
                    &field(self.user_id, self.device_id),
                    &stored,
                    HASH_TTL,
                )
                .await?;
            // `added` covers records someone else removed behind our back
            // (hide → unhide, team switch off → on): the list is unchanged
            // for us, but teammates have to learn about it again.
            if added || self.reported.get(team_id) != Some(sessions) {
                events::publish(state, Event::PresenceChanged { team_id: *team_id }).await?;
            }
        }
        for team_id in previous.difference(&by_team.keys().copied().collect()) {
            let removed = state
                .cache
                .hdel(&key(*team_id), &field(self.user_id, self.device_id))
                .await?;
            if removed {
                events::publish(state, Event::PresenceChanged { team_id: *team_id }).await?;
            }
        }
        self.reported = by_team;
        Ok(())
    }

    /// Remove everything this connection reported (socket closed).
    pub async fn clear(&mut self, state: &AppState) -> ApiResult<()> {
        self.report(state, Vec::new()).await
    }
}

async fn user_hidden(state: &AppState, user_id: Uuid) -> ApiResult<bool> {
    let row: Option<(bool,)> = sqlx::query_as("SELECT presence_hidden FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    Ok(row.is_none_or(|(h,)| h))
}

/// `vault → team` for the given vaults, restricted to live team vaults the
/// user is a member of whose team has presence turned on.
async fn team_of_vaults(
    state: &AppState,
    user_id: Uuid,
    vault_ids: &[Uuid],
) -> ApiResult<HashMap<Uuid, Uuid>> {
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT v.id, v.team_id
         FROM vaults v
         JOIN vault_members vm ON vm.vault_id = v.id AND vm.user_id = $1
         JOIN teams t ON t.id = v.team_id
         JOIN team_members tm ON tm.team_id = t.id AND tm.user_id = $1
         WHERE v.id = ANY($2) AND v.deleted_at IS NULL AND t.presence_enabled",
    )
    .bind(user_id)
    .bind(vault_ids)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Drop every device of `user_id` from every team it belongs to (the user
/// hid themselves, or left / was removed).
pub async fn clear_user(state: &AppState, user_id: Uuid) -> ApiResult<()> {
    let teams: Vec<(Uuid,)> = sqlx::query_as("SELECT team_id FROM team_members WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;
    for (team_id,) in teams {
        clear_user_in_team(state, team_id, user_id).await?;
    }
    Ok(())
}

/// Forget everything reported in a team (presence was switched off).
pub async fn clear_team(state: &AppState, team_id: Uuid) -> ApiResult<()> {
    state.cache.del(&key(team_id)).await
}

pub async fn clear_user_in_team(state: &AppState, team_id: Uuid, user_id: Uuid) -> ApiResult<()> {
    let entries: Vec<Stored> = state.cache.hgetall_json(&key(team_id)).await?;
    let mut changed = false;
    for e in entries.into_iter().filter(|e| e.user_id == user_id) {
        changed |= state
            .cache
            .hdel(&key(team_id), &field(e.user_id, e.device_id))
            .await?;
    }
    if changed {
        events::publish(state, Event::PresenceChanged { team_id }).await?;
    }
    Ok(())
}

/// What `viewer` may see in `team_id`. The caller has checked membership;
/// `enabled` is the team switch.
pub async fn snapshot(
    state: &AppState,
    team_id: Uuid,
    viewer: Uuid,
    enabled: bool,
) -> ApiResult<TeamPresence> {
    if !enabled {
        return Ok(TeamPresence {
            enabled: false,
            entries: Vec::new(),
        });
    }
    let stale_before = Utc::now() - chrono::Duration::from_std(STALE_AFTER).unwrap_or_default();
    let mut stored: Vec<Stored> = Vec::new();
    let mut expired = false;
    for e in state.cache.hgetall_json::<Stored>(&key(team_id)).await? {
        if e.seen_at < stale_before {
            expired |= state
                .cache
                .hdel(&key(team_id), &field(e.user_id, e.device_id))
                .await?;
        } else {
            stored.push(e);
        }
    }
    if expired {
        events::publish(state, Event::PresenceChanged { team_id }).await?;
    }
    if stored.is_empty() {
        return Ok(TeamPresence {
            enabled: true,
            entries: Vec::new(),
        });
    }

    let visible_vaults: HashSet<Uuid> = sqlx::query_as::<_, (Uuid,)>(
        "SELECT vm.vault_id FROM vault_members vm JOIN vaults v ON v.id = vm.vault_id
         WHERE vm.user_id = $1 AND v.team_id = $2 AND v.deleted_at IS NULL",
    )
    .bind(viewer)
    .bind(team_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id,)| id)
    .collect();

    let user_ids: Vec<Uuid> = stored
        .iter()
        .map(|e| e.user_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    type UserRow = (Uuid, String, Option<String>, Option<String>);
    let users: HashMap<Uuid, (String, Option<String>, Option<String>)> =
        sqlx::query_as::<_, UserRow>(
            "SELECT u.id, u.email, u.display_name, u.avatar_tag FROM users u
         JOIN team_members tm ON tm.user_id = u.id AND tm.team_id = $2
         WHERE u.id = ANY($1) AND NOT u.presence_hidden AND NOT u.disabled",
        )
        .bind(&user_ids)
        .bind(team_id)
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|(id, email, name, avatar)| (id, (email, name, avatar)))
        .collect();
    let device_ids: Vec<Uuid> = stored.iter().map(|e| e.device_id).collect();
    let devices: HashMap<Uuid, (String, String)> = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, name, platform FROM devices WHERE id = ANY($1)",
    )
    .bind(&device_ids)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id, name, platform)| (id, (name, platform)))
    .collect();

    let mut entries: Vec<PresenceEntry> = stored
        .into_iter()
        .filter_map(|e| {
            let (email, display_name, avatar) = users.get(&e.user_id)?.clone();
            let (device_name, platform) = devices.get(&e.device_id)?.clone();
            let sessions: Vec<PresenceSession> = e
                .sessions
                .into_iter()
                .filter(|s| visible_vaults.contains(&s.vault_id))
                .collect();
            if sessions.is_empty() {
                return None;
            }
            Some(PresenceEntry {
                user_id: e.user_id,
                email,
                display_name,
                avatar,
                device_id: e.device_id,
                device_name,
                platform,
                sessions,
                seen_at: e.seen_at,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        a.email
            .cmp(&b.email)
            .then(a.device_name.cmp(&b.device_name))
            .then(a.device_id.cmp(&b.device_id))
    });
    Ok(TeamPresence {
        enabled: true,
        entries,
    })
}
