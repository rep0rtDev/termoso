//! Sync protocol: push local changes, pull remote changes by cursor.
//!
//! Each vault has an independent, monotonically increasing `seq`. A client
//! keeps `since = max(seq)` per vault and pulls everything newer. Pushes use
//! optimistic concurrency on `version`; on mismatch the server returns its copy
//! and the client resolves (default: keep newer `updated_at`, re-push).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::entities::SyncEntity;
use crate::schema;

/// Maximum number of items per push / pull page.
pub const MAX_BATCH: usize = 500;

schema! {
    /// Create or update one entity.
    pub struct EntityChange {
        /// Client-generated UUID.
        pub id: Uuid,
        /// Kind.
        pub kind: String,
        /// Vault.
        pub vault_id: Uuid,
        /// Version this change is based on. `None` → create (must not exist).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub base_version: Option<i64>,
        /// Vault key version used to encrypt `data`.
        pub key_version: i32,
        /// Encrypted payload.
        pub data: String,
        /// Client-side modification time (used only for conflict hints).
        pub updated_at: DateTime<Utc>,
    }
}

schema! {
    /// Delete one entity.
    pub struct EntityDelete {
        /// Id.
        pub id: Uuid,
        /// Version this delete is based on.
        pub base_version: i64,
    }
}

schema! {
    /// `POST /sync/push`
    pub struct PushRequest {
        /// Creates / updates.
        #[serde(default)]
        pub changes: Vec<EntityChange>,
        /// Deletes.
        #[serde(default)]
        pub deletes: Vec<EntityDelete>,
    }
}

schema! {
    /// Per-item outcome.
    #[serde(tag = "status", rename_all = "snake_case")]
    pub enum PushResult {
        /// Applied.
        Ok {
            /// Id.
            id: Uuid,
            /// New version.
            version: i64,
            /// New seq.
            seq: i64,
        },
        /// Version mismatch – server copy attached.
        Conflict {
            /// Id.
            id: Uuid,
            /// Current server state.
            server: SyncEntity,
        },
        /// Rejected.
        Error {
            /// Id.
            id: Uuid,
            /// Error code (`forbidden`, `not_found`, `unknown_kind`, `payload_too_large`, `stale_key_version`).
            code: String,
        },
    }
}

schema! {
    /// Response to `PushRequest`.
    pub struct PushResponse {
        /// One result per input item, same order (changes first, then deletes).
        pub results: Vec<PushResult>,
    }
}

schema! {
    /// `POST /sync/pull`
    pub struct PullRequest {
        /// `vault_id → since seq`. Vaults absent from the map are not pulled.
        /// Use `0` for an initial full pull.
        pub cursors: HashMap<Uuid, i64>,
        /// Max entities to return in total (capped at [`MAX_BATCH`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<u32>,
    }
}

schema! {
    /// Response to `PullRequest`.
    pub struct PullResponse {
        /// Entities newer than the given cursors, ordered by (vault, seq).
        pub entities: Vec<SyncEntity>,
        /// Updated cursors to store.
        pub cursors: HashMap<Uuid, i64>,
        /// More data available – call again with the new cursors.
        pub has_more: bool,
    }
}

// ───────────────────────────── history ─────────────────────────────

schema! {
    /// Kind of history entry.
    #[derive(Copy, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum HistoryKind {
        /// A shell command typed by the user.
        Command,
        /// A connection (host + time + duration).
        Connection,
    }
}

schema! {
    /// Encrypted history record (personal vault key).
    pub struct HistoryEntry {
        /// Client-generated UUID.
        pub id: Uuid,
        /// Kind.
        pub kind: HistoryKind,
        /// Encrypted payload. AAD = `termoso/v1/history/<kind>/<id>`.
        pub data: String,
        /// Personal vault key version.
        pub key_version: i32,
        /// When it happened (plaintext, needed for server-side pruning).
        pub created_at: DateTime<Utc>,
        /// Server sequence (assigned by the server).
        #[serde(default)]
        pub seq: i64,
        /// Tombstone.
        #[serde(default)]
        pub deleted: bool,
    }
}

schema! {
    /// `POST /history/push`
    pub struct HistoryPushRequest {
        /// Entries (idempotent by id).
        pub entries: Vec<HistoryEntry>,
    }
}

schema! {
    /// `GET /history/pull?since=&limit=`
    pub struct HistoryPullResponse {
        /// Entries.
        pub entries: Vec<HistoryEntry>,
        /// New cursor.
        pub since: i64,
        /// More available.
        pub has_more: bool,
    }
}

schema! {
    /// `POST /history/clear`
    pub struct HistoryClearRequest {
        /// Only this kind (all if omitted).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub kind: Option<HistoryKind>,
    }
}
