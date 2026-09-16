//! AI command suggestions: a short natural-language request becomes one
//! shell command the user may then run — or not.
//!
//! Only what the user typed plus the host's OS/shell name ever leaves the
//! client; the terminal buffer, host names and credentials never do. The
//! server relays that to the configured model and returns text; nothing is
//! executed anywhere. The feature is off per account until the user turns it
//! on, and every account has a daily quota.

use crate::schema;

schema! {
    /// `GET /account/ai`
    pub struct AiStatus {
        /// The server has a model configured at all.
        pub available: bool,
        /// This account has opted in.
        pub enabled: bool,
        /// Display name of the provider (e.g. `Chutes`), when configured.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub provider: Option<String>,
        /// Model identifier, when configured.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub model: Option<String>,
        /// The model runs inside a trusted execution environment: the
        /// provider's operators cannot read prompts. This is confidential
        /// compute, not end-to-end encryption — the model itself sees the text.
        #[serde(default)]
        pub confidential: bool,
        /// Requests allowed per account per UTC day.
        pub daily_quota: u32,
        /// Requests already used today.
        pub used_today: u32,
    }
}

schema! {
    /// `PUT /account/ai`
    pub struct AiSettingsRequest {
        /// Opt in (or out) of AI suggestions for this account.
        pub enabled: bool,
    }
}

schema! {
    /// `POST /ai/command`
    pub struct AiCommandRequest {
        /// What the user wants to do, in their own words.
        pub prompt: String,
        /// Operating system of the target host, if known (e.g. `Ubuntu 24.04`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub os: Option<String>,
        /// Shell on the target host, if known (e.g. `bash`, `zsh`, `fish`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub shell: Option<String>,
    }
}

schema! {
    /// `POST /ai/command` response.
    pub struct AiCommandResponse {
        /// The suggested command. Never executed by anything server-side.
        pub command: String,
        /// One-line plain-language explanation, when the model gave one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub explanation: Option<String>,
        /// Requests left today after this one.
        pub remaining_today: u32,
    }
}
