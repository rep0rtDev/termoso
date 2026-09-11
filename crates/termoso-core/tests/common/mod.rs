//! Boots the real Termoso server (fresh database, Redis, optional MinIO and
//! Mailpit) on an ephemeral port so the client core can be exercised against
//! it end to end. Mirrors the server crate's own harness minus SSO.
//!
//! Skipped (`None`) when PostgreSQL is unreachable, unless
//! `TERMOSO_TEST_REQUIRE_SERVICES` is set.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use sqlx::Connection;
use sqlx::postgres::PgConnection;
use termoso_core::account::{self, LoginFlow, LoginStep, RegisterInput, SignedIn};
use termoso_core::api::ApiClient;
use termoso_core::store::Store;
use termoso_crypto::keys::SymmetricKey;
use termoso_proto::auth::{DeviceInfo, Platform};
use termoso_server::Inner;
use termoso_server::config::{Config, S3Config, SmtpConfig, SmtpSecurity};
use uuid::Uuid;

const DEFAULT_PG: &str = "postgres://termoso:termoso@localhost:5432/termoso";
const DEFAULT_REDIS: &str = "redis://127.0.0.1:6379";
const DEFAULT_S3: &str = "http://127.0.0.1:9000";
const DEFAULT_MAILPIT_SMTP: &str = "127.0.0.1:1025";
const DEFAULT_MAILPIT_API: &str = "http://127.0.0.1:8025";

pub struct TestServer {
    pub addr: SocketAddr,
    pub storage: bool,
    pub mailpit: Option<String>,
}

static SERVER: OnceLock<Option<TestServer>> = OnceLock::new();

fn require_services() -> bool {
    std::env::var("TERMOSO_TEST_REQUIRE_SERVICES").is_ok()
}

async fn tcp_reachable(host_port: &str) -> bool {
    tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::TcpStream::connect(host_port),
    )
    .await
    .is_ok_and(|r| r.is_ok())
}

fn host_port(url: &str) -> String {
    let u = url::Url::parse(url).expect("valid url");
    format!(
        "{}:{}",
        u.host_str().expect("host"),
        u.port_or_known_default().expect("port")
    )
}

async fn optional(name: &str, probe: &str) -> bool {
    if tcp_reachable(probe).await {
        return true;
    }
    if require_services() {
        panic!("{name} unavailable at {probe}");
    }
    eprintln!("{name} unavailable at {probe}: skipping tests that need it");
    false
}

async fn s3_config() -> Option<S3Config> {
    let endpoint = std::env::var("TERMOSO_TEST_S3_ENDPOINT").unwrap_or_else(|_| DEFAULT_S3.into());
    if !optional("MinIO", &host_port(&endpoint)).await {
        return None;
    }
    Some(S3Config {
        bucket: format!("termoso-core-test-{}", Uuid::new_v4().simple()),
        region: None,
        endpoint: Some(endpoint),
        public_endpoint: None,
        access_key: std::env::var("TERMOSO_TEST_S3_ACCESS_KEY")
            .unwrap_or_else(|_| "minioadmin".into()),
        secret_key: std::env::var("TERMOSO_TEST_S3_SECRET_KEY")
            .unwrap_or_else(|_| "minioadmin".into()),
        force_path_style: true,
        presign_secs: 300,
    })
}

async fn smtp_config() -> Option<(SmtpConfig, String)> {
    let smtp =
        std::env::var("TERMOSO_TEST_SMTP_ADDR").unwrap_or_else(|_| DEFAULT_MAILPIT_SMTP.into());
    let api =
        std::env::var("TERMOSO_TEST_MAILPIT_URL").unwrap_or_else(|_| DEFAULT_MAILPIT_API.into());
    if !optional("Mailpit", &smtp).await {
        return None;
    }
    let (host, port) = smtp.rsplit_once(':').expect("host:port");
    Some((
        SmtpConfig {
            host: host.to_string(),
            port: port.parse().expect("smtp port"),
            username: None,
            password: None,
            security: SmtpSecurity::None,
            from: "Termoso Test <no-reply@test.local>".into(),
        },
        api,
    ))
}

/// Boot once per test binary.
pub async fn server() -> Option<&'static TestServer> {
    if let Some(s) = SERVER.get() {
        return s.as_ref();
    }
    let built = boot().await;
    let _ = SERVER.set(built);
    SERVER.get().expect("set above").as_ref()
}

async fn boot() -> Option<TestServer> {
    let base = std::env::var("TERMOSO_TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_PG.into());
    let mut admin = match PgConnection::connect(&base).await {
        Ok(c) => c,
        Err(e) => {
            if require_services() {
                panic!("PostgreSQL unavailable at {base}: {e}");
            }
            eprintln!("skipping server integration tests: PostgreSQL unavailable ({e})");
            return None;
        }
    };
    let stale: Vec<(String,)> =
        sqlx::query_as("SELECT datname FROM pg_database WHERE datname LIKE 'termoso_coretest_%'")
            .fetch_all(&mut admin)
            .await
            .expect("list test databases");
    for (name,) in stale {
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS \"{name}\""
        )))
        .execute(&mut admin)
        .await;
    }
    let db_name = format!("termoso_coretest_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE \"{db_name}\""
    )))
    .execute(&mut admin)
    .await
    .expect("create test database");
    let mut database_url = url::Url::parse(&base).expect("postgres url");
    database_url.set_path(&format!("/{db_name}"));

    let addr = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind")
        .local_addr()
        .expect("addr");
    let s3 = s3_config().await;
    let smtp = smtp_config().await;
    let storage = s3.is_some();
    let mailpit = smtp.as_ref().map(|(_, api)| api.clone());

    let cfg = Config {
        bind: addr,
        public_url: format!("http://{addr}"),
        database_url: database_url.to_string(),
        redis_url: std::env::var("TERMOSO_TEST_REDIS_URL").unwrap_or_else(|_| DEFAULT_REDIS.into()),
        redis_prefix: format!("{db_name}:"),
        trust_proxy: true,
        master_key: SymmetricKey::generate().to_b64(),
        admin_emails: "admin@core.test".into(),
        swagger_ui: false,
        s3,
        smtp: smtp.map(|(cfg, _)| cfg),
        ..Config::default()
    };
    cfg.validate().expect("valid test config");

    // Each #[tokio::test] tears its runtime down; the shared server lives on
    // its own thread/runtime.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<anyhow::Result<()>>();
    std::thread::Builder::new()
        .name("termoso-core-test-server".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("runtime");
            rt.block_on(async move {
                let state = match Inner::build(cfg).await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.into()));
                        return;
                    }
                };
                tokio::spawn(termoso_server::ws::run_fanout(state.clone()));
                let app = termoso_server::app(state);
                let _ = ready_tx.send(Ok(()));
                axum::serve(
                    listener,
                    app.into_make_service_with_connect_info::<SocketAddr>(),
                )
                .await
                .expect("serve");
            });
        })
        .expect("spawn server thread");
    ready_rx
        .recv()
        .expect("server thread died")
        .expect("server state");

    Some(TestServer {
        addr,
        storage,
        mailpit,
    })
}

/// Skip the test (returning early) when the backing services are missing.
#[macro_export]
macro_rules! server_or_skip {
    () => {
        match $crate::common::server().await {
            Some(s) => s,
            None => return,
        }
    };
}

/// Skip unless Mailpit is configured.
#[macro_export]
macro_rules! server_with_mail_or_skip {
    () => {{
        let s = $crate::server_or_skip!();
        if s.mailpit.is_none() {
            eprintln!("skipping: Mailpit not configured");
            return;
        }
        s
    }};
}

/// Skip unless MinIO is configured.
#[macro_export]
macro_rules! server_with_storage_or_skip {
    () => {{
        let s = $crate::server_or_skip!();
        if !s.storage {
            eprintln!("skipping: MinIO not configured");
            return;
        }
        s
    }};
}

impl TestServer {
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}/api/v1{path}", self.addr)
    }

    /// Fresh API client for a device. Every device gets its own fake source IP
    /// so per-IP login rate limits never trip across a parallel test run.
    pub fn api(&self) -> Arc<ApiClient> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-forwarded-for", fake_ip().parse().expect("header value"));
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .pool_max_idle_per_host(0)
            .build()
            .expect("http client");
        Arc::new(ApiClient::with_http(&self.base_url(), http).expect("api client"))
    }

    pub fn store(&self) -> Arc<Store> {
        Arc::new(Store::open_in_memory(SymmetricKey::generate()).expect("store"))
    }

    /// Raw authenticated request for endpoints the core does not wrap.
    pub async fn raw(
        &self,
        method: reqwest::Method,
        path: &str,
        token: &str,
        body: Option<serde_json::Value>,
    ) -> reqwest::Response {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .expect("client");
        let mut req = client
            .request(method, self.url(path))
            .header("x-forwarded-for", fake_ip())
            .bearer_auth(token);
        if let Some(b) = body {
            req = req.json(&b);
        }
        req.send().await.expect("request")
    }

    /// Latest code Mailpit received for `to` in an email whose subject
    /// mentions `purpose`.
    pub async fn emailed_code(&self, to: &str, purpose: &str) -> String {
        let api = self.mailpit.as_ref().expect("mailpit configured");
        let client = reqwest::Client::new();
        for _ in 0..50 {
            let v: serde_json::Value = client
                .get(format!("{api}/api/v1/search"))
                .query(&[("query", format!("to:{to}")), ("limit", "50".into())])
                .send()
                .await
                .expect("mailpit search")
                .json()
                .await
                .expect("mailpit json");
            let found = v["messages"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| m["Subject"].as_str())
                .find(|s| s.contains(purpose))
                .and_then(|s| s.rsplit_once("code ").map(|(_, c)| c.trim().to_string()));
            if let Some(code) = found {
                return code;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("no `{purpose}` email for {to} arrived");
    }
}

fn fake_ip() -> String {
    static N: AtomicU32 = AtomicU32::new(1);
    let n = N.fetch_add(1, Ordering::Relaxed);
    format!("10.{}.{}.{}", (n >> 16) & 255, (n >> 8) & 255, n & 255)
}

pub fn unique_email(tag: &str) -> String {
    format!("{tag}-{}@core.test", Uuid::new_v4().simple())
}

pub fn device(name: &str, store: &Store) -> DeviceInfo {
    account::device_info(store, name, Platform::Linux, "0.0.0-test").expect("device info")
}

/// A signed-in device: its API client and store.
pub struct Device {
    pub api: Arc<ApiClient>,
    pub store: Arc<Store>,
    pub signed_in: SignedIn,
}

impl Device {
    pub fn personal_vault(&self) -> Uuid {
        self.signed_in.vaults[0]
    }
}

/// Register a new account from a fresh device.
pub async fn register(server: &TestServer, email: &str, password: &str) -> (Device, String) {
    let api = server.api();
    let store = server.store();
    let reg = account::register(
        api.clone(),
        store.clone(),
        RegisterInput {
            email: email.into(),
            password: password.into(),
            display_name: Some("Core Test".into()),
            device: device("first device", &store),
            invite_token: None,
            sso_session: None,
        },
    )
    .await
    .expect("register");
    let phrase = reg.recovery_phrase.to_string();
    (
        Device {
            api,
            store,
            signed_in: reg.signed_in,
        },
        phrase,
    )
}

/// Sign in from a brand-new device, following any MFA / approval challenge.
pub async fn login(server: &TestServer, email: &str, password: &str) -> Device {
    let api = server.api();
    let store = server.store();
    let (mut flow, mut step) = LoginFlow::start(
        api.clone(),
        store.clone(),
        email,
        password,
        device("another device", &store),
        None,
    )
    .await
    .expect("login start");
    loop {
        match step {
            LoginStep::Done(signed_in) => {
                return Device {
                    api,
                    store,
                    signed_in,
                };
            }
            LoginStep::DeviceApprovalRequired { .. } => {
                let code = server.emailed_code(email, "new device sign-in").await;
                step = flow.approve_device(&code).await.expect("approve device");
            }
            LoginStep::MfaRequired { methods } => panic!("unexpected MFA challenge: {methods:?}"),
        }
    }
}
