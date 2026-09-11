//! WebSocket protocol (`/api/v1/ws`), JSON text frames.
//!
//! The client authenticates with the first frame. The server then pushes
//! lightweight *notifications* – never payloads – and the client pulls via
//! REST. This keeps the WebSocket path trivial to scale (Redis pub/sub fan-out).

use uuid::Uuid;

use crate::schema;

schema! {
    /// Frames sent by the client.
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum ClientMessage {
        /// Must be the first frame.
        Auth {
            /// Session token.
            token: String,
        },
        /// Keep-alive.
        Ping,
    }
}

schema! {
    /// Frames sent by the server.
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum ServerMessage {
        /// Sent after successful `Auth`.
        Hello {
            /// Server version.
            server_version: String,
            /// Vaults this connection is subscribed to.
            vault_ids: Vec<Uuid>,
        },
        /// Entities in a vault changed; pull with your cursor.
        VaultChanged {
            /// Vault.
            vault_id: Uuid,
            /// Latest seq.
            seq: i64,
            /// Device that made the change (skip if it is you).
            #[serde(default, skip_serializing_if = "Option::is_none")]
            device_id: Option<Uuid>,
        },
        /// History entries changed.
        HistoryChanged {
            /// Latest seq.
            seq: i64,
        },
        /// Session logs changed.
        LogsChanged {
            /// Latest seq.
            seq: i64,
        },
        /// Vault list / membership / keys changed – reload `GET /vaults`.
        VaultsUpdated,
        /// Team list / membership changed – reload `GET /teams`.
        TeamsUpdated,
        /// Account keys or settings blob changed.
        AccountUpdated,
        /// This device's session was revoked; the socket closes after this.
        SessionRevoked,
        /// Reply to `Ping`.
        Pong,
        /// Protocol error; the socket closes after this.
        Error {
            /// Code.
            code: String,
            /// Message.
            message: String,
        },
    }
}
