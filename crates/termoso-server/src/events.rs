//! Internal events fanned out through Redis Pub/Sub to every API instance,
//! then delivered to connected WebSocket clients.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Entities in a vault changed. Delivered to every member of the vault.
    VaultChanged {
        vault_id: Uuid,
        seq: i64,
        device_id: Option<Uuid>,
    },
    /// Membership / keys / list of vaults changed for these users.
    VaultsUpdated {
        user_ids: Vec<Uuid>,
    },
    TeamsUpdated {
        user_ids: Vec<Uuid>,
    },
    /// Who is connected to what changed in a team. Delivered to every member.
    PresenceChanged {
        team_id: Uuid,
    },
    HistoryChanged {
        user_id: Uuid,
        seq: i64,
    },
    LogsChanged {
        user_id: Uuid,
        seq: i64,
    },
    AccountUpdated {
        user_id: Uuid,
    },
    SessionRevoked {
        user_id: Uuid,
        session_id: Uuid,
    },
}

pub async fn publish(state: &AppState, event: Event) -> ApiResult<()> {
    state.cache.publish(&event).await
}

pub async fn vault_changed(
    state: &AppState,
    vault_id: Uuid,
    seq: i64,
    device_id: Option<Uuid>,
) -> ApiResult<()> {
    publish(
        state,
        Event::VaultChanged {
            vault_id,
            seq,
            device_id,
        },
    )
    .await
}
