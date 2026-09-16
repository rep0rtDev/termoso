//! "Ask AI" for the phone: the same server proxy the desktop uses, with the
//! same boundary. Only the request text and two labels (OS family, shell)
//! leave the device; the OS label comes from what is saved on the host, never
//! from the terminal. The answer is handed back for the user to paste — it is
//! never typed for them.

use std::sync::Arc;

use termoso_core::model::Host;
use termoso_proto::ai::{AiCommandRequest, AiCommandResponse, AiStatus};

use crate::account::AccountRuntime;
use crate::dto::parse_id;
use crate::error::{MobileError, Result};

/// Whether the server offers suggestions and whether this account opted in.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AiStatusCard {
    /// The server has a provider configured.
    pub available: bool,
    /// This account turned suggestions on.
    pub enabled: bool,
    /// Provider label as configured by the server operator, e.g. `Chutes`.
    pub provider: Option<String>,
    pub model: Option<String>,
    /// Runs in confidential compute (TEE): the operator cannot read the
    /// request; the model still does. Not end-to-end encryption.
    pub confidential: bool,
    pub daily_quota: u32,
    pub used_today: u32,
}

impl From<AiStatus> for AiStatusCard {
    fn from(s: AiStatus) -> Self {
        Self {
            available: s.available,
            enabled: s.enabled,
            provider: s.provider,
            model: s.model,
            confidential: s.confidential,
            daily_quota: s.daily_quota,
            used_today: s.used_today,
        }
    }
}

/// One suggested command. `command` is empty when the request cannot be
/// answered with a shell command (the explanation says why).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AiSuggestionCard {
    pub command: String,
    pub explanation: Option<String>,
    pub remaining_today: u32,
}

impl From<AiCommandResponse> for AiSuggestionCard {
    fn from(r: AiCommandResponse) -> Self {
        Self {
            command: r.command,
            explanation: r.explanation,
            remaining_today: r.remaining_today,
        }
    }
}

/// Where the request is aimed, so the model knows the OS family. Nothing
/// else about the target is sent.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AiTarget {
    /// A saved host: its detected/saved `os_name` is the label (if any).
    Host { host_id: String },
    /// A shell on this device.
    Local,
    /// Nothing known (quick connect, viewer): no OS label.
    Unknown,
}

impl AccountRuntime {
    pub async fn ai_status(self: &Arc<Self>) -> Result<AiStatusCard> {
        Ok(self.api().await?.ai_status().await?.into())
    }

    pub async fn set_ai_enabled(self: &Arc<Self>, enabled: bool) -> Result<AiStatusCard> {
        Ok(self.api().await?.set_ai_enabled(enabled).await?.into())
    }

    pub async fn ai_ask(
        self: &Arc<Self>,
        prompt: String,
        target: AiTarget,
    ) -> Result<AiSuggestionCard> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(MobileError::invalid("describe what the command should do"));
        }
        let os = self.ai_os_label(&target)?;
        Ok(self
            .api()
            .await?
            .ai_command(&AiCommandRequest {
                prompt: prompt.to_string(),
                os,
                shell: None,
            })
            .await?
            .into())
    }

    fn ai_os_label(&self, target: &AiTarget) -> Result<Option<String>> {
        Ok(match target {
            AiTarget::Host { host_id } => self
                .store()
                .require::<Host>(parse_id(host_id)?)
                .ok()
                .and_then(|h| h.data.os_name),
            AiTarget::Local => Some("android".into()),
            AiTarget::Unknown => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_carry_everything_the_ui_shows() {
        let s: AiStatusCard = AiStatus {
            available: true,
            enabled: false,
            provider: Some("Chutes".into()),
            model: Some("GLM".into()),
            confidential: true,
            daily_quota: 50,
            used_today: 3,
        }
        .into();
        assert!(s.available && !s.enabled && s.confidential);
        assert_eq!((s.daily_quota, s.used_today), (50, 3));

        let r: AiSuggestionCard = AiCommandResponse {
            command: "uptime".into(),
            explanation: None,
            remaining_today: 46,
        }
        .into();
        assert_eq!(r.command, "uptime");
        assert_eq!(r.remaining_today, 46);
    }
}
