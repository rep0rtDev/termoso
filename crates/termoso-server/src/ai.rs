//! Command suggestions through an OpenAI-compatible chat endpoint.
//!
//! The model gets a fixed system prompt, the OS/shell name and the user's
//! sentence — nothing else. Prompts and completions are never logged; the
//! API key lives only in [`Ai`] and the `Authorization` header.

use std::time::Duration;

use anyhow::Context;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::config::AiConfig;
use crate::error::{ApiResult, Error};

/// Hard cap on the completion; a command plus one explanation line.
const MAX_TOKENS: u32 = 400;

const SYSTEM_PROMPT: &str = "\
You turn a short request into exactly one shell command for the user's terminal.
Rules:
- Reply with two lines only, in this exact format, and nothing else:
COMMAND: <the command>
EXPLANATION: <one short sentence>
- The command must be ready to paste. No markdown, no code fences, no numbering, no alternatives.
- Prefer portable, standard tools for the given OS and shell; use the shell's syntax.
- If the request is destructive (deleting data, rewriting disks, killing everything), still answer, but say so in the explanation and prefer the safer variant (dry-run, interactive flags).
- If the request is not something a shell command can do, or is unclear, reply:
COMMAND:
EXPLANATION: <why, in one sentence>
- Do not add warnings beyond the explanation line. Do not ask questions.";

pub struct Ai {
    cfg: AiConfig,
    http: reqwest::Client,
}

/// A parsed model answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub command: String,
    pub explanation: Option<String>,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [Message<'a>; 2],
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: Option<String>,
}

impl Ai {
    pub fn new(cfg: &AiConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(concat!("termoso-server/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building AI HTTP client")?;
        Ok(Self {
            cfg: cfg.clone(),
            http,
        })
    }

    pub fn cfg(&self) -> &AiConfig {
        &self.cfg
    }

    fn completions_url(&self) -> String {
        format!("{}/chat/completions", self.cfg.url.trim_end_matches('/'))
    }

    /// Ask the model for a command. `os` / `shell` are already validated
    /// short labels; `prompt` is the user's text within `max_prompt_chars`.
    pub async fn suggest(
        &self,
        prompt: &str,
        os: Option<&str>,
        shell: Option<&str>,
    ) -> ApiResult<Suggestion> {
        let user = user_message(prompt, os, shell);
        let body = ChatRequest {
            model: &self.cfg.model,
            messages: [
                Message {
                    role: "system",
                    content: SYSTEM_PROMPT,
                },
                Message {
                    role: "user",
                    content: &user,
                },
            ],
            max_tokens: MAX_TOKENS,
            temperature: 0.2,
            stream: false,
        };
        let resp = self
            .http
            .post(self.completions_url())
            .bearer_auth(&self.cfg.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(error = %redact(&e), "AI provider unreachable");
                Error::ai_unavailable()
            })?;
        let status = resp.status();
        if !status.is_success() {
            // Bodies may echo the prompt; only the status is worth keeping.
            tracing::warn!(status = %status, "AI provider rejected the request");
            return Err(match status {
                StatusCode::TOO_MANY_REQUESTS => Error::ai_busy(),
                _ => Error::ai_unavailable(),
            });
        }
        let parsed: ChatResponse = resp.json().await.map_err(|_| {
            tracing::warn!("AI provider returned an unreadable body");
            Error::ai_unavailable()
        })?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();
        Ok(parse_suggestion(&content))
    }
}

fn user_message(prompt: &str, os: Option<&str>, shell: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str("OS: ");
    out.push_str(os.unwrap_or("unknown"));
    out.push_str("\nShell: ");
    out.push_str(shell.unwrap_or("unknown"));
    out.push_str("\nRequest: ");
    out.push_str(prompt);
    out
}

/// `reqwest::Error`'s `Display` includes the URL, which is fine, but never the
/// request body or headers; keep only the kind of failure anyway.
fn redact(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_request() {
        "request"
    } else {
        "other"
    }
}

/// Extract the command and explanation from a model reply. Tolerates
/// markdown fences, `$ ` prompts and missing labels. Precedence for the
/// command: `COMMAND:` label, then the first fenced block, then the first
/// bare line; the explanation is the `EXPLANATION:` label or the first bare
/// line that is not the command.
pub fn parse_suggestion(text: &str) -> Suggestion {
    let mut labelled: Option<String> = None;
    let mut fenced: Option<String> = None;
    let mut bare: Vec<String> = Vec::new();
    let mut explanation: Option<String> = None;
    let mut in_fence = false;
    let mut block: Vec<String> = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("```") {
            if in_fence && !block.is_empty() && fenced.is_none() {
                fenced = Some(block.join("\n"));
            }
            in_fence = !in_fence;
            block.clear();
            continue;
        }
        if in_fence {
            block.push(strip_prompt(line).to_string());
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = strip_label(line, "COMMAND") {
            if labelled.is_none() {
                labelled = Some(strip_prompt(strip_ticks(rest)).to_string());
            }
        } else if let Some(rest) = strip_label(line, "EXPLANATION") {
            if explanation.is_none() && !rest.is_empty() {
                explanation = Some(rest.to_string());
            }
        } else {
            bare.push(line.to_string());
        }
    }
    if in_fence && !block.is_empty() && fenced.is_none() {
        fenced = Some(block.join("\n"));
    }

    let mut bare = bare.into_iter();
    let command = match (labelled, fenced) {
        (Some(c), _) => c,
        (None, Some(c)) => c,
        (None, None) => bare
            .next()
            .map(|l| strip_prompt(strip_ticks(&l)).to_string())
            .unwrap_or_default(),
    };
    let explanation = explanation
        .or_else(|| bare.find(|l| !l.ends_with(':')))
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty());

    Suggestion {
        command: command.trim().to_string(),
        explanation,
    }
}

fn strip_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let line = line.trim_start_matches(['*', '_', '#', ' ']);
    let (head, rest) = line.split_once(':')?;
    let head = head.trim().trim_matches('*').trim_matches('_');
    if head.eq_ignore_ascii_case(label) {
        Some(rest.trim().trim_matches('*').trim())
    } else {
        None
    }
}

fn strip_ticks(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('`') && s.ends_with('`') {
        s[1..s.len() - 1].trim()
    } else {
        s
    }
}

fn strip_prompt(s: &str) -> &str {
    s.strip_prefix("$ ")
        .or_else(|| s.strip_prefix("# "))
        .unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_labelled_reply() {
        let s = parse_suggestion(
            "COMMAND: du -sh * | sort -h\nEXPLANATION: Sizes of everything here, smallest first.",
        );
        assert_eq!(s.command, "du -sh * | sort -h");
        assert_eq!(
            s.explanation.as_deref(),
            Some("Sizes of everything here, smallest first.")
        );
    }

    #[test]
    fn tolerates_markdown_and_prompt_marker() {
        let s = parse_suggestion(
            "**COMMAND:** `$ ss -tulpn`\n**EXPLANATION:** Lists listening sockets with owning processes.",
        );
        assert_eq!(s.command, "ss -tulpn");
        assert!(s.explanation.is_some());
    }

    #[test]
    fn falls_back_to_fenced_block() {
        let s = parse_suggestion(
            "Here you go:\n```bash\nfind . -name '*.log' -mtime +7 -delete\n```\nDeletes week-old logs.",
        );
        assert_eq!(s.command, "find . -name '*.log' -mtime +7 -delete");
        assert_eq!(s.explanation.as_deref(), Some("Deletes week-old logs."));
    }

    #[test]
    fn empty_command_when_model_declines() {
        let s = parse_suggestion("COMMAND:\nEXPLANATION: That is not something a shell can do.");
        assert_eq!(s.command, "");
        assert_eq!(
            s.explanation.as_deref(),
            Some("That is not something a shell can do.")
        );
    }

    #[test]
    fn bare_line_is_the_command() {
        let s = parse_suggestion("\n  uptime\n");
        assert_eq!(s.command, "uptime");
        assert_eq!(s.explanation, None);
    }

    #[test]
    fn user_message_carries_only_context_and_prompt() {
        let m = user_message("show disk usage", Some("Ubuntu 24.04"), None);
        assert_eq!(
            m,
            "OS: Ubuntu 24.04\nShell: unknown\nRequest: show disk usage"
        );
    }
}
