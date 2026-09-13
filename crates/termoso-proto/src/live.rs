//! Multiplayer: sharing a live terminal with other signed-in users.
//!
//! The server is a dumb relay. The host generates a random *secret* and puts
//! it in the invitation link; from it both sides derive a *join token* (proves
//! to the server that a viewer holds the link) and a *stream key* (encrypts
//! every terminal frame end-to-end). The server stores only a hash of the join
//! token and never sees terminal content, titles or sizes.
//!
//! Relay protocol on `GET /live/{id}/ws`: the first frame is
//! [`LiveClientMessage::Auth`]; text frames carry control JSON, binary frames
//! carry opaque encrypted payloads that the server forwards host → viewers
//! and (when the viewer has remote control) viewer → host.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::schema;

schema! {
    /// `POST /live`
    pub struct CreateLiveSessionRequest {
        /// Join token derived from the link secret; stored hashed.
        pub join_token: String,
    }
}

schema! {
    /// A live session as the server knows it (metadata only).
    pub struct LiveSession {
        /// Id (part of the invitation link).
        pub id: Uuid,
        /// User hosting the session.
        pub host_user_id: Uuid,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// Links stop working after this even if the host never stops.
        pub expires_at: DateTime<Utc>,
        /// Stopped by the host (or by its connection going away).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ended_at: Option<DateTime<Utc>>,
    }
}

schema! {
    /// `GET /live` — the caller's active sessions.
    pub struct LiveSessionList {
        /// Sessions.
        pub sessions: Vec<LiveSession>,
    }
}

schema! {
    /// Someone connected to a live session.
    pub struct LiveParticipant {
        /// User.
        pub user_id: Uuid,
        /// Email.
        pub email: String,
        /// Display name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        /// The person sharing the terminal.
        pub is_host: bool,
        /// May type into the terminal (always true for the host).
        pub can_write: bool,
    }
}

schema! {
    /// Frames sent by a relay client (text).
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum LiveClientMessage {
        /// Must be the first frame.
        Auth {
            /// Account session token.
            token: String,
            /// Join token from the link; omitted by the host.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            join_token: Option<String>,
        },
        /// Keep-alive.
        Ping,
        /// Host only: grant or revoke remote control.
        Control {
            /// Viewer.
            user_id: Uuid,
            /// Grant (`true`) or revoke.
            enabled: bool,
        },
        /// Host only: an encrypted payload for one viewer (screen catch-up).
        Direct {
            /// Viewer.
            user_id: Uuid,
            /// Base64 (standard) encrypted payload.
            data: String,
        },
    }
}

schema! {
    /// Frames sent by the relay (text; encrypted payloads arrive as binary).
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum LiveServerMessage {
        /// Sent after a successful `Auth`.
        Hello {
            /// Session.
            session_id: Uuid,
            /// Your user id.
            user_id: Uuid,
            /// You are the host.
            is_host: bool,
            /// Everyone currently connected, including you.
            participants: Vec<LiveParticipant>,
        },
        /// Someone joined, left or had remote control changed.
        Participants {
            /// Everyone currently connected.
            participants: Vec<LiveParticipant>,
        },
        /// Your own remote-control state changed.
        Control {
            /// You may type now.
            can_write: bool,
        },
        /// The host stopped multiplayer; the socket closes after this.
        Ended,
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
