//! AI command suggestions: opt-in, quota, what leaves the server, and how
//! provider trouble surfaces.

mod common;

use common::*;
use reqwest::{Method, StatusCode};
use termoso_proto::account::ServerInfo;
use termoso_proto::ai::{AiCommandRequest, AiCommandResponse, AiSettingsRequest, AiStatus};

macro_rules! server {
    () => {
        match server().await {
            Some(s) => s,
            None => return,
        }
    };
}

fn ask(prompt: &str) -> AiCommandRequest {
    AiCommandRequest {
        prompt: prompt.into(),
        os: Some("Ubuntu 24.04".into()),
        shell: Some("bash".into()),
    }
}

async fn enable(s: &TestServer, u: &User) -> AiStatus {
    s.json(
        Method::PUT,
        "/account/ai",
        Some(u.token()),
        Some(&AiSettingsRequest { enabled: true }),
    )
    .await
}

async fn status(s: &TestServer, u: &User) -> AiStatus {
    s.json(Method::GET, "/account/ai", Some(u.token()), NOBODY)
        .await
}

#[tokio::test]
async fn advertised_and_off_until_opted_in() {
    let s = server!();
    let u = register(s, &unique_email("ai-optin"), "correct horse battery").await;

    let info: ServerInfo = s.json(Method::GET, "/server/info", None, NOBODY).await;
    assert!(info.features.ai, "server info advertises the AI feature");

    let st = status(s, &u).await;
    assert!(st.available);
    assert!(!st.enabled, "opt-in is off for a fresh account");
    assert_eq!(st.provider.as_deref(), Some(llm::PROVIDER));
    assert_eq!(st.model.as_deref(), Some(llm::MODEL));
    assert!(st.confidential);
    assert_eq!(st.daily_quota, llm::DAILY_QUOTA);
    assert_eq!(st.used_today, 0);

    let marker = format!("ai-optin-{}", uuid::Uuid::new_v4());
    let err = s
        .expect_status(
            Method::POST,
            "/ai/command",
            Some(u.token()),
            Some(&ask(&marker)),
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_eq!(err["code"], "ai_not_enabled");
    assert!(
        s.llm.requests_containing(&marker).is_empty(),
        "nothing reaches the provider before opt-in"
    );

    let st = enable(s, &u).await;
    assert!(st.enabled);
    let st: AiStatus = s
        .json(
            Method::PUT,
            "/account/ai",
            Some(u.token()),
            Some(&AiSettingsRequest { enabled: false }),
        )
        .await;
    assert!(!st.enabled);
    s.expect_status(
        Method::POST,
        "/ai/command",
        Some(u.token()),
        Some(&ask(&marker)),
        StatusCode::FORBIDDEN,
    )
    .await;

    s.expect_status(
        Method::POST,
        "/ai/command",
        None,
        Some(&ask(&marker)),
        StatusCode::UNAUTHORIZED,
    )
    .await;
}

#[tokio::test]
async fn suggestion_sends_only_prompt_and_context() {
    let s = server!();
    let u = register(s, &unique_email("ai-ask"), "correct horse battery").await;
    enable(s, &u).await;

    let marker = format!("show disk usage {}", uuid::Uuid::new_v4());
    let r: AiCommandResponse = s
        .json(
            Method::POST,
            "/ai/command",
            Some(u.token()),
            Some(&ask(&format!("  {marker}  "))),
        )
        .await;
    assert_eq!(r.command, llm::DEFAULT_COMMAND);
    assert_eq!(r.explanation.as_deref(), Some(llm::DEFAULT_EXPLANATION));
    assert_eq!(r.remaining_today, llm::DAILY_QUOTA - 1);

    let sent = s.llm.requests_containing(&marker);
    assert_eq!(sent.len(), 1, "exactly one upstream call");
    let req = &sent[0];
    assert_eq!(
        req.authorization.as_deref(),
        Some(&*format!("Bearer {}", llm::API_KEY)),
        "the server's key, never the user's session"
    );
    assert_eq!(req.body["model"], llm::MODEL);
    assert_eq!(req.body["stream"], false);
    assert_eq!(
        llm::user_content(&req.body),
        format!("OS: Ubuntu 24.04\nShell: bash\nRequest: {marker}"),
        "only OS, shell and the trimmed request travel"
    );
    let raw = req.body.to_string();
    assert!(!raw.contains(&u.email), "no account identity upstream");
    assert!(
        !raw.contains(&u.session.user.id.to_string()),
        "no user id upstream"
    );
    assert!(!raw.contains(u.token()), "no session token upstream");

    assert_eq!(status(s, &u).await.used_today, 1);

    // Missing context is sent as "unknown", not invented.
    let marker = format!("ctx-{}", uuid::Uuid::new_v4());
    let _: AiCommandResponse = s
        .json(
            Method::POST,
            "/ai/command",
            Some(u.token()),
            Some(&AiCommandRequest {
                prompt: marker.clone(),
                os: None,
                shell: Some("   ".into()),
            }),
        )
        .await;
    let sent = s.llm.requests_containing(&marker);
    assert_eq!(
        llm::user_content(&sent[0].body),
        format!("OS: unknown\nShell: unknown\nRequest: {marker}")
    );
}

#[tokio::test]
async fn provider_reply_shapes_are_normalised() {
    let s = server!();
    let u = register(s, &unique_email("ai-shape"), "correct horse battery").await;
    enable(s, &u).await;

    let r: AiCommandResponse = s
        .json(
            Method::POST,
            "/ai/command",
            Some(u.token()),
            Some(&ask("@markdown free space on root")),
        )
        .await;
    assert_eq!(r.command, "df -h /");
    assert_eq!(
        r.explanation.as_deref(),
        Some("Shows free space on the root filesystem.")
    );

    let r: AiCommandResponse = s
        .json(
            Method::POST,
            "/ai/command",
            Some(u.token()),
            Some(&ask("@decline write me a poem")),
        )
        .await;
    assert_eq!(r.command, "", "the model may decline with an empty command");
    assert_eq!(
        r.explanation.as_deref(),
        Some("That is not something a shell command can do.")
    );
}

#[tokio::test]
async fn input_is_validated_before_anything_is_spent() {
    let s = server!();
    let u = register(s, &unique_email("ai-input"), "correct horse battery").await;
    enable(s, &u).await;

    let bad = |prompt: &str, os: Option<&str>, shell: Option<&str>| AiCommandRequest {
        prompt: prompt.into(),
        os: os.map(str::to_string),
        shell: shell.map(str::to_string),
    };
    let cases = [
        bad("   ", Some("Linux"), Some("bash")),
        bad(&"x".repeat(llm::MAX_PROMPT_CHARS + 1), None, None),
        bad("list files", Some(&"y".repeat(81)), None),
        bad("list files", None, Some("bash\u{0}")),
    ];
    for c in cases {
        let err = s
            .expect_status(
                Method::POST,
                "/ai/command",
                Some(u.token()),
                Some(&c),
                StatusCode::BAD_REQUEST,
            )
            .await;
        assert_eq!(err["code"], "validation_failed", "{c:?}");
    }
    let r = s
        .call(
            Method::POST,
            "/ai/command",
            Some(u.token()),
            Some(&serde_json::json!({ "os": "Linux" })),
        )
        .await;
    assert_eq!(r.status(), StatusCode::BAD_REQUEST, "prompt is required");

    // A prompt at exactly the limit is fine.
    let _: AiCommandResponse = s
        .json(
            Method::POST,
            "/ai/command",
            Some(u.token()),
            Some(&ask(&"z".repeat(llm::MAX_PROMPT_CHARS))),
        )
        .await;
    assert_eq!(
        status(s, &u).await.used_today,
        1,
        "rejected input costs nothing"
    );
}

#[tokio::test]
async fn daily_quota_is_enforced_per_account() {
    let s = server!();
    let u = register(s, &unique_email("ai-quota"), "correct horse battery").await;
    let other = register(s, &unique_email("ai-quota-other"), "correct horse battery").await;
    enable(s, &u).await;
    enable(s, &other).await;

    for i in 0..llm::DAILY_QUOTA {
        let r: AiCommandResponse = s
            .json(
                Method::POST,
                "/ai/command",
                Some(u.token()),
                Some(&ask(&format!("quota {i}"))),
            )
            .await;
        assert_eq!(r.remaining_today, llm::DAILY_QUOTA - 1 - i);
    }
    let marker = format!("quota-over-{}", uuid::Uuid::new_v4());
    let err = s
        .expect_status(
            Method::POST,
            "/ai/command",
            Some(u.token()),
            Some(&ask(&marker)),
            StatusCode::TOO_MANY_REQUESTS,
        )
        .await;
    assert_eq!(err["code"], "ai_quota_exceeded");
    let retry = err["details"]["retry_after"].as_u64().expect("retry_after");
    assert!(
        (1..=86_400).contains(&retry),
        "resets at UTC midnight: {retry}"
    );
    assert!(
        s.llm.requests_containing(&marker).is_empty(),
        "over quota never reaches the provider"
    );
    let st = status(s, &u).await;
    assert_eq!(
        st.used_today,
        llm::DAILY_QUOTA,
        "counter is capped at the quota"
    );

    // The neighbour's quota is untouched.
    let r: AiCommandResponse = s
        .json(
            Method::POST,
            "/ai/command",
            Some(other.token()),
            Some(&ask("other user")),
        )
        .await;
    assert_eq!(r.remaining_today, llm::DAILY_QUOTA - 1);
}

#[tokio::test]
async fn provider_failures_are_safe_and_refunded() {
    let s = server!();
    let u = register(s, &unique_email("ai-fail"), "correct horse battery").await;
    enable(s, &u).await;

    let cases = [
        (
            "@fail500 restart nginx",
            StatusCode::BAD_GATEWAY,
            "ai_unavailable",
        ),
        (
            "@garbage restart nginx",
            StatusCode::BAD_GATEWAY,
            "ai_unavailable",
        ),
        (
            "@fail429 restart nginx",
            StatusCode::SERVICE_UNAVAILABLE,
            "ai_busy",
        ),
    ];
    for (prompt, http, code) in cases {
        let err = s
            .expect_status(
                Method::POST,
                "/ai/command",
                Some(u.token()),
                Some(&ask(prompt)),
                http,
            )
            .await;
        assert_eq!(err["code"], code, "{prompt}");
        let text = err.to_string();
        assert!(
            !text.contains("boom"),
            "provider body is not echoed: {text}"
        );
        assert!(
            !text.contains("html"),
            "provider body is not echoed: {text}"
        );
    }
    assert_eq!(
        status(s, &u).await.used_today,
        0,
        "failed calls give the quota slot back"
    );
}
