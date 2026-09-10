//! Shared harness: boots the real server (migrations, Redis, WS fan-out) on an
//! ephemeral port against the services from `deploy/docker-compose.dev.yml`.
//!
//! Every test process gets its own freshly-created database so runs never
//! interfere. Skipped (returns `None`) when `TERMOSO_TEST_DATABASE_URL` is unset
//! and the default local Postgres is unreachable.

#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::postgres::PgConnection;
use sqlx::Connection;
use termoso_crypto::aead::{self, Aad};
use termoso_crypto::kdf::{derive_key, Label};
use termoso_crypto::keys::{KeyPair, SymmetricKey};
use termoso_crypto::opaque;
use termoso_crypto::recovery::RecoveryKey;
use termoso_crypto::sealed;
use termoso_proto::auth::{
    AccountKeysUpload, AuthResponse, DeviceInfo, LoginFinishRequest, LoginStartRequest,
    LoginStartResponse, Platform, RegisterFinishRequest, RegisterStartRequest,
    RegisterStartResponse, Session,
};
use termoso_server::config::Config;
use termoso_server::Inner;
use uuid::Uuid;

const DEFAULT_PG: &str = "postgres://termoso:termoso@localhost:5432/termoso";
const DEFAULT_REDIS: &str = "redis://127.0.0.1:6379";

pub struct TestServer {
    pub addr: SocketAddr,
    pub db_name: String,
    pub database_url: String,
    pub master_key: String,
}

static SERVER: OnceLock<Option<TestServer>> = OnceLock::new();

fn base_pg_url() -> String {
    std::env::var("TERMOSO_TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_PG.into())
}

fn redis_url() -> String {
    std::env::var("TERMOSO_TEST_REDIS_URL").unwrap_or_else(|_| DEFAULT_REDIS.into())
}

fn with_db(url: &str, db: &str) -> String {
    let mut u = url::Url::parse(url).expect("valid postgres url");
    u.set_path(&format!("/{db}"));
    u.to_string()
}

/// Boot (once per test binary) and return the shared server, or `None` when
/// the backing services are not available.
pub async fn server() -> Option<&'static TestServer> {
    if let Some(s) = SERVER.get() {
        return s.as_ref();
    }
    let built = boot().await;
    let _ = SERVER.set(built);
    SERVER.get().expect("set above").as_ref()
}

async fn boot() -> Option<TestServer> {
    let base = base_pg_url();
    let mut admin = match PgConnection::connect(&base).await {
        Ok(c) => c,
        Err(e) => {
            if std::env::var("TERMOSO_TEST_REQUIRE_SERVICES").is_ok() {
                panic!("PostgreSQL unavailable at {base}: {e}");
            }
            eprintln!("skipping integration tests: PostgreSQL unavailable ({e})");
            return None;
        }
    };
    // Databases from earlier runs are dropped lazily here (the harness has no
    // reliable shutdown hook). Anything still in use is simply skipped.
    let stale: Vec<(String,)> =
        sqlx::query_as("SELECT datname FROM pg_database WHERE datname LIKE 'termoso_test_%'")
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

    let db_name = format!("termoso_test_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE \"{db_name}\""
    )))
    .execute(&mut admin)
    .await
    .expect("create test database");
    let database_url = with_db(&base, &db_name);

    let master_key = SymmetricKey::generate().to_b64();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");

    let cfg = Config {
        bind: addr,
        public_url: format!("http://{addr}"),
        database_url: database_url.clone(),
        redis_url: redis_url(),
        redis_prefix: format!("{db_name}:"),
        trust_proxy: true,
        master_key: master_key.clone(),
        admin_emails: "admin@test.local".into(),
        swagger_ui: true,
        ..Config::default()
    };
    cfg.validate().expect("valid test config");
    drop(listener);

    // Each `#[tokio::test]` owns a runtime that is torn down when the test
    // ends, so the shared server must live on its own runtime/thread.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<anyhow::Result<()>>();
    std::thread::Builder::new()
        .name("termoso-test-server".into())
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
        db_name,
        database_url,
        master_key,
    })
}

fn fake_ip() -> String {
    static N: AtomicU32 = AtomicU32::new(1);
    let n = N.fetch_add(1, Ordering::Relaxed);
    format!("10.{}.{}.{}", (n >> 16) & 255, (n >> 8) & 255, n & 255)
}

impl TestServer {
    /// A fresh client per call: reqwest's pool lives on the runtime that
    /// created it, and every `#[tokio::test]` has its own short-lived runtime.
    pub fn http(&self) -> Client {
        Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .expect("client")
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}/api/v1{}", self.addr, path)
    }

    pub fn ws_url(&self) -> String {
        format!("ws://{}/api/v1/ws", self.addr)
    }

    pub async fn call<T: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&T>,
    ) -> reqwest::Response {
        // Each call comes from a unique "client" so per-IP rate limits never
        // trip while the suite hammers the auth endpoints in parallel.
        let mut req = self
            .http()
            .request(method, self.url(path))
            .header("x-forwarded-for", fake_ip());
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        req.send().await.expect("request")
    }

    pub async fn json<T: Serialize, R: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&T>,
    ) -> R {
        let resp = self.call(method.clone(), path, token, body).await;
        let status = resp.status();
        let text = resp.text().await.expect("body");
        assert!(status.is_success(), "{method} {path} -> {status}: {text}");
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{method} {path}: {e}: {text}"))
    }

    pub async fn expect_status<T: Serialize>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<&T>,
        expected: StatusCode,
    ) -> serde_json::Value {
        let resp = self.call(method.clone(), path, token, body).await;
        let status = resp.status();
        let text = resp.text().await.expect("body");
        assert_eq!(status, expected, "{method} {path}: {text}");
        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null)
    }
}

pub const NOBODY: Option<&()> = None;

/// A registered user with its client-side key material.
pub struct User {
    pub email: String,
    pub password: String,
    pub session: Session,
    pub keypair: KeyPair,
    pub personal_vault_key: SymmetricKey,
    pub recovery: RecoveryKey,
}

impl User {
    pub fn token(&self) -> &str {
        &self.session.token
    }

    pub fn id(&self) -> Uuid {
        self.session.user.id
    }

    /// Encrypt an entity payload the way a real client would.
    pub fn encrypt_entity(&self, key: &SymmetricKey, kind: &str, id: Uuid, json: &str) -> String {
        aead::encrypt_str(key, &Aad::entity(kind, &id.to_string()), json).expect("encrypt")
    }
}

pub fn device(name: &str) -> DeviceInfo {
    DeviceInfo {
        name: name.into(),
        platform: Platform::Linux,
        app_version: "test".into(),
        client_device_id: Some(Uuid::new_v4()),
    }
}

pub fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@test.local", Uuid::new_v4().simple())
}

/// Full OPAQUE registration with a freshly generated key hierarchy.
pub async fn register(server: &TestServer, email: &str, password: &str) -> User {
    register_with(server, email, password, None).await
}

pub async fn register_with(
    server: &TestServer,
    email: &str,
    password: &str,
    invite_token: Option<String>,
) -> User {
    let (request, state) = opaque::client_registration_start(password.as_bytes()).expect("start");
    let start: RegisterStartResponse = server
        .json(
            Method::POST,
            "/auth/register/start",
            None,
            Some(&RegisterStartRequest {
                email: email.into(),
                opaque_request: request,
            }),
        )
        .await;
    let out = opaque::client_registration_finish(
        state,
        password.as_bytes(),
        &email.to_lowercase(),
        &start.opaque_response,
    )
    .expect("finish");

    let kek = derive_key(&out.export_key, Label::AccountKek).expect("kek");
    let keypair = KeyPair::generate();
    let recovery = RecoveryKey::generate();
    let recovery_kek = recovery.kek().expect("recovery kek");
    let personal_vault_key = SymmetricKey::generate();
    let secret = SymmetricKey::from_bytes(keypair.secret_bytes());

    let keys = AccountKeysUpload {
        public_key: keypair.public_b64(),
        wrapped_private_key: aead::wrap_key(&kek, &Aad::account_private_key(), &secret)
            .expect("wrap"),
        recovery_wrapped_private_key: aead::wrap_key(
            &recovery_kek,
            &Aad::recovery_private_key(),
            &secret,
        )
        .expect("wrap"),
        recovery_verifier: recovery.verifier_b64().expect("verifier"),
        personal_vault_sealed_key: sealed::seal_vault_key(keypair.public(), &personal_vault_key)
            .expect("seal"),
    };

    let resp: AuthResponse = server
        .json(
            Method::POST,
            "/auth/register/finish",
            None,
            Some(&RegisterFinishRequest {
                email: email.into(),
                opaque_upload: out.upload_b64,
                display_name: Some("Test User".into()),
                device: device("registration device"),
                keys,
                invite_token,
                sso_session: None,
            }),
        )
        .await;
    let AuthResponse::Authenticated(session) = resp else {
        panic!("expected direct session after registration, got {resp:?}");
    };
    User {
        email: email.into(),
        password: password.into(),
        session,
        keypair,
        personal_vault_key,
        recovery,
    }
}

/// OPAQUE login from a new device. Returns the raw auth response so tests can
/// assert on MFA / approval branches too.
pub async fn login(server: &TestServer, email: &str, password: &str) -> AuthResponse {
    try_login(server, email, password)
        .await
        .expect("login should succeed")
}

/// Like [`login`] but reports a wrong password as `Err` instead of panicking.
pub async fn try_login(
    server: &TestServer,
    email: &str,
    password: &str,
) -> Result<AuthResponse, String> {
    let (request, state) = opaque::client_login_start(password.as_bytes()).expect("start");
    let resp = server
        .call(
            Method::POST,
            "/auth/login/start",
            None,
            Some(&LoginStartRequest {
                email: email.into(),
                opaque_request: request,
                device: device("second device"),
                sso_session: None,
            }),
        )
        .await;
    let status = resp.status();
    let text = resp.text().await.expect("body");
    assert!(status.is_success(), "login/start -> {status}: {text}");
    let start: LoginStartResponse = serde_json::from_str(&text).expect("json");
    let finish = opaque::client_login_finish(
        state,
        password.as_bytes(),
        &email.to_lowercase(),
        &start.opaque_response,
    );
    let Ok(finish) = finish else {
        // Wrong password: only the client can tell at this point. Still hit
        // finish with garbage so the server-side rejection path is exercised.
        let v: serde_json::Value = server
            .expect_status(
                Method::POST,
                "/auth/login/finish",
                None,
                Some(&LoginFinishRequest {
                    login_id: start.login_id,
                    opaque_finalization: "AAAA".into(),
                }),
                StatusCode::UNAUTHORIZED,
            )
            .await;
        return Err(format!("invalid credentials (server said {v})"));
    };
    let resp = server
        .call(
            Method::POST,
            "/auth/login/finish",
            None,
            Some(&LoginFinishRequest {
                login_id: start.login_id,
                opaque_finalization: finish.finalization_b64,
            }),
        )
        .await;
    let status = resp.status();
    let text = resp.text().await.expect("body");
    if !status.is_success() {
        return Err(format!("login/finish -> {status}: {text}"));
    }
    Ok(serde_json::from_str(&text).expect("json"))
}

/// Cursors helper for `/sync/pull`.
pub fn cursors(pairs: &[(Uuid, i64)]) -> HashMap<Uuid, i64> {
    pairs.iter().copied().collect()
}
