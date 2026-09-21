//! Questions the core needs answered by a human (unknown host key, password,
//! keyboard-interactive). Rust emits a `prompt` event and parks the connection
//! future until the UI calls `prompt_answer` (or the session is cancelled).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use termoso_core::hostkey::{HostKeyDecision, HostKeyPrompt, HostKeyVerdict};
use termoso_core::ssh::{InteractivePrompt, InteractiveQuestion};
use tokio::sync::oneshot;
use uuid::Uuid;
use zeroize::Zeroizing;

pub const PROMPT_EVENT: &str = "prompt";
pub const PROMPT_CLOSED_EVENT: &str = "prompt-closed";

/// One prompt shown to the user.
#[derive(Debug, Clone, Serialize)]
pub struct Prompt {
    pub id: Uuid,
    /// Session the prompt belongs to (so the UI can render it in that tab).
    pub session_id: Uuid,
    /// `user@host:port` or a label.
    pub target: String,
    #[serde(flatten)]
    pub request: PromptRequest,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PromptRequest {
    HostKey {
        verdict: HostKeyVerdict,
    },
    /// A TLS server (WebDAV) presented a certificate the public roots do not
    /// vouch for; answered like a host key (reject / once / pin).
    Certificate {
        host: String,
        fingerprint: String,
    },
    /// The host has no username configured: ask which account to log in
    /// as. Answered with [`PromptAnswer::Secret`] (`remember` saves it on
    /// the host's identity).
    Username {
        host: String,
        retry: bool,
    },
    Password {
        username: String,
        retry: bool,
    },
    Passphrase {
        key_label: String,
    },
    /// FIDO2 security key wants its client PIN. `retries` is what the token
    /// reported after a wrong PIN, when it did.
    Pin {
        key_label: String,
        retry: bool,
        retries: Option<i32>,
    },
    Interactive {
        name: String,
        instructions: String,
        questions: Vec<Question>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Question {
    pub prompt: String,
    pub echo: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PromptAnswer {
    HostKey {
        decision: HostKeyDecision,
    },
    Secret {
        value: Zeroizing<String>,
        #[serde(default)]
        remember: bool,
    },
    Interactive {
        answers: Vec<Zeroizing<String>>,
    },
    Cancel,
}

#[derive(Debug, Clone, Serialize)]
struct PromptClosed {
    id: Uuid,
    session_id: Uuid,
}

struct Waiter {
    session_id: Uuid,
    tx: oneshot::Sender<PromptAnswer>,
}

/// Registry of open prompts.
#[derive(Default)]
pub struct PromptBroker {
    waiting: Mutex<HashMap<Uuid, Waiter>>,
}

impl PromptBroker {
    /// Show `request` for `session_id` and wait for the answer. `None` when
    /// the prompt was cancelled (session closed, UI dismissed it).
    pub async fn ask<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        session_id: Uuid,
        target: String,
        request: PromptRequest,
    ) -> Option<PromptAnswer> {
        let id = Uuid::new_v4();
        let (tx, rx) = oneshot::channel();
        self.waiting
            .lock()
            .expect("prompt registry poisoned")
            .insert(id, Waiter { session_id, tx });
        let prompt = Prompt {
            id,
            session_id,
            target,
            request,
        };
        if let Err(e) = app.emit(PROMPT_EVENT, &prompt) {
            tracing::warn!(error = %e, "failed to emit prompt");
            self.waiting
                .lock()
                .expect("prompt registry poisoned")
                .remove(&id);
            return None;
        }
        let answer = rx.await.ok();
        let _ = app.emit(PROMPT_CLOSED_EVENT, PromptClosed { id, session_id });
        match answer {
            Some(PromptAnswer::Cancel) | None => None,
            Some(a) => Some(a),
        }
    }

    /// Deliver the UI's answer. Unknown ids are ignored (prompt already gone).
    pub fn answer(&self, id: Uuid, answer: PromptAnswer) -> bool {
        let waiter = self
            .waiting
            .lock()
            .expect("prompt registry poisoned")
            .remove(&id);
        match waiter {
            Some(w) => w.tx.send(answer).is_ok(),
            None => false,
        }
    }

    /// Drop every prompt of a session (its connection attempt is over).
    pub fn cancel_session(&self, session_id: Uuid) {
        let mut waiting = self.waiting.lock().expect("prompt registry poisoned");
        waiting.retain(|_, w| w.session_id != session_id);
    }
}

/// [`HostKeyPrompt`] that forwards to the UI.
pub struct UiHostKeyPrompt<R: Runtime> {
    pub app: AppHandle<R>,
    pub session_id: Uuid,
    pub target: String,
}

impl<R: Runtime> HostKeyPrompt for UiHostKeyPrompt<R> {
    fn decide(
        &self,
        verdict: HostKeyVerdict,
    ) -> Pin<Box<dyn Future<Output = HostKeyDecision> + Send + '_>> {
        Box::pin(async move {
            let broker = self.app.state::<crate::state::AppState>();
            let answer = broker
                .prompts
                .ask(
                    &self.app,
                    self.session_id,
                    self.target.clone(),
                    PromptRequest::HostKey { verdict },
                )
                .await;
            match answer {
                Some(PromptAnswer::HostKey { decision }) => decision,
                _ => HostKeyDecision::Reject,
            }
        })
    }
}

/// [`InteractivePrompt`] that forwards keyboard-interactive rounds to the UI.
pub struct UiInteractivePrompt<R: Runtime> {
    pub app: AppHandle<R>,
    pub session_id: Uuid,
    pub target: String,
}

impl<R: Runtime> InteractivePrompt for UiInteractivePrompt<R> {
    fn respond(
        &self,
        name: String,
        instructions: String,
        prompts: Vec<InteractiveQuestion>,
    ) -> Pin<Box<dyn Future<Output = Option<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            let state = self.app.state::<crate::state::AppState>();
            let questions = prompts
                .into_iter()
                .map(|q| Question {
                    prompt: q.prompt,
                    echo: q.echo,
                })
                .collect();
            let answer = state
                .prompts
                .ask(
                    &self.app,
                    self.session_id,
                    self.target.clone(),
                    PromptRequest::Interactive {
                        name,
                        instructions,
                        questions,
                    },
                )
                .await;
            match answer {
                Some(PromptAnswer::Interactive { answers }) => {
                    Some(answers.into_iter().map(|a| a.to_string()).collect())
                }
                _ => None,
            }
        })
    }
}
