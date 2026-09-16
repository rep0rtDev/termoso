//! OpenAI-compatible chat-completions stand-in. Behaviour is steered by
//! markers inside the user message so parallel tests never share state:
//! `@fail429` → 429, `@fail500` → 500, `@garbage` → non-JSON body,
//! `@markdown` → fenced reply, `@decline` → empty command. Every request
//! is recorded so tests can assert exactly what left the server.

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

pub const API_KEY: &str = "test-ai-key";
pub const MODEL: &str = "mock-model";
pub const PROVIDER: &str = "Mock";
pub const DAILY_QUOTA: u32 = 5;
pub const MAX_PROMPT_CHARS: usize = 200;

pub const DEFAULT_COMMAND: &str = "uptime";
pub const DEFAULT_EXPLANATION: &str = "Shows how long the host has been running.";

#[derive(Debug, Clone)]
pub struct Recorded {
    pub authorization: Option<String>,
    pub body: serde_json::Value,
}

#[derive(Clone, Default)]
pub struct MockLlm {
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl MockLlm {
    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().expect("llm log").clone()
    }

    /// Requests whose user message contains `marker`.
    pub fn requests_containing(&self, marker: &str) -> Vec<Recorded> {
        self.requests()
            .into_iter()
            .filter(|r| user_content(&r.body).contains(marker))
            .collect()
    }
}

pub fn user_content(body: &serde_json::Value) -> String {
    body["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["role"] == "user")
        .filter_map(|m| m["content"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Deserialize)]
struct ChatRequest {
    model: String,
    messages: Vec<serde_json::Value>,
}

async fn completions(
    State(llm): State<MockLlm>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    llm.requests.lock().expect("llm log").push(Recorded {
        authorization: headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string),
        body: parsed.clone(),
    });
    let Ok(req) = serde_json::from_value::<ChatRequest>(parsed) else {
        return (StatusCode::BAD_REQUEST, "bad request").into_response();
    };
    if req.model != MODEL || req.messages.len() != 2 {
        return (StatusCode::BAD_REQUEST, "unexpected request").into_response();
    }
    let content = user_content(&serde_json::json!({ "messages": req.messages }));
    if content.contains("@fail429") {
        return (StatusCode::TOO_MANY_REQUESTS, "slow down").into_response();
    }
    if content.contains("@fail500") {
        return (StatusCode::INTERNAL_SERVER_ERROR, "boom").into_response();
    }
    if content.contains("@garbage") {
        return (StatusCode::OK, "<html>not json</html>").into_response();
    }
    let reply = if content.contains("@markdown") {
        "Here you go:\n```bash\n$ df -h /\n```\nShows free space on the root filesystem."
            .to_string()
    } else if content.contains("@decline") {
        "COMMAND:\nEXPLANATION: That is not something a shell command can do.".to_string()
    } else {
        format!("COMMAND: {DEFAULT_COMMAND}\nEXPLANATION: {DEFAULT_EXPLANATION}")
    };
    Json(serde_json::json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "model": MODEL,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": reply },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 40, "completion_tokens": 20, "total_tokens": 60 }
    }))
    .into_response()
}

pub async fn serve(listener: std::net::TcpListener) -> anyhow::Result<MockLlm> {
    let llm = MockLlm::default();
    let app = Router::new()
        .route("/v1/chat/completions", post(completions))
        .with_state(llm.clone());
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock llm");
    });
    Ok(llm)
}
