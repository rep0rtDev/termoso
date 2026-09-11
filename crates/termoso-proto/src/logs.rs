//! Session logs: terminal recordings encrypted client-side and stored in object storage.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::schema;

schema! {
    /// Session log metadata.
    pub struct SessionLog {
        /// Client-generated UUID.
        pub id: Uuid,
        /// Vault whose key encrypts `meta` and the object (personal or team).
        pub vault_id: Uuid,
        /// Encrypted metadata (host label, address, start/end time, size…).
        /// AAD = `termoso/v1/log/<id>`.
        pub meta: String,
        /// Vault key version.
        pub key_version: i32,
        /// Encrypted object size in bytes (0 until completed).
        pub size_bytes: i64,
        /// Upload finished.
        pub completed: bool,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// Server sequence for incremental listing.
        pub seq: i64,
        /// Tombstone.
        #[serde(default)]
        pub deleted: bool,
    }
}

schema! {
    /// `POST /logs` – reserve a log and get an upload URL.
    pub struct CreateLogRequest {
        /// Id.
        pub id: Uuid,
        /// Vault.
        pub vault_id: Uuid,
        /// Encrypted metadata.
        pub meta: String,
        /// Key version.
        pub key_version: i32,
        /// Expected size (server enforces the maximum).
        pub size_bytes: i64,
    }
}

schema! {
    /// Response to `CreateLogRequest`.
    pub struct CreateLogResponse {
        /// Pre-signed PUT URL (valid for a limited time).
        pub upload_url: String,
        /// Headers that must be sent with the PUT.
        #[serde(default)]
        pub upload_headers: Vec<(String, String)>,
        /// Seconds until the URL expires.
        pub expires_in: u64,
    }
}

schema! {
    /// `PATCH /logs/{id}` – update metadata (e.g. after the session ends).
    pub struct UpdateLogRequest {
        /// New encrypted metadata.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub meta: Option<String>,
        /// Final size (marks the upload as completed).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub size_bytes: Option<i64>,
    }
}

schema! {
    /// `GET /logs/{id}/download`
    pub struct DownloadLogResponse {
        /// Pre-signed GET URL.
        pub download_url: String,
        /// Seconds until the URL expires.
        pub expires_in: u64,
    }
}

schema! {
    /// `GET /logs?since=&limit=`
    pub struct LogListResponse {
        /// Logs.
        pub logs: Vec<SessionLog>,
        /// New cursor.
        pub since: i64,
        /// More available.
        pub has_more: bool,
    }
}
