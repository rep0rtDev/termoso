//! Configuration: `TERMOSO_*` environment variables, optionally layered over a
//! TOML file (`TERMOSO_CONFIG=/etc/termoso/config.toml`).
//!
//! Nested keys use `__`: `TERMOSO_S3__BUCKET`, `TERMOSO_SSO__GOOGLE__CLIENT_ID`.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use anyhow::{Context, Result};
use figment::Figment;
use figment::providers::{Env, Format, Toml};
use serde::{Deserialize, Serialize};
use termoso_crypto::keys::SymmetricKey;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// Address to bind the API on.
    pub bind: SocketAddr,
    /// Public base URL of the API (used in emails and OAuth redirects).
    pub public_url: String,
    /// Public URL of the web cabinet (links in emails). Defaults to `public_url`.
    pub web_url: Option<String>,
    /// Human name shown in emails / server info.
    pub server_name: String,
    pub database_url: String,
    pub database_max_connections: u32,
    pub redis_url: String,
    /// Prefix for every Redis key/channel (lets deployments share one Redis).
    pub redis_prefix: String,
    /// Base64, 32 bytes. Encrypts server-side secrets at rest (OPAQUE server
    /// setup, TOTP secrets). Generate: `openssl rand -base64 32`.
    pub master_key: String,
    /// Comma-separated emails that are granted admin on first login/registration.
    pub admin_emails: String,
    /// Allowed CORS origins (comma-separated). Empty = same-origin only.
    pub cors_origins: String,
    /// Trust `X-Forwarded-For` / `X-Real-IP` from the reverse proxy.
    pub trust_proxy: bool,
    pub log: String,
    pub log_format: LogFormat,
    pub s3: Option<S3Config>,
    pub smtp: Option<SmtpConfig>,
    pub webauthn: Option<WebauthnConfig>,
    pub metrics: MetricsConfig,
    pub sso: BTreeMap<String, SsoProviderConfig>,
    /// Serve the Swagger UI at `/api/docs`.
    pub swagger_ui: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct S3Config {
    pub bucket: String,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    /// Endpoint to put into pre-signed URLs handed to clients (e.g. a public
    /// MinIO hostname), when different from `endpoint`.
    pub public_endpoint: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    #[serde(default = "default_true")]
    pub force_path_style: bool,
    #[serde(default = "default_presign_secs")]
    pub presign_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SmtpConfig {
    pub host: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    /// `starttls` (default), `tls` (implicit), `none` (plain, for local dev).
    #[serde(default)]
    pub security: SmtpSecurity,
    /// `From:` header, e.g. `Termoso <no-reply@termoso.com>`.
    pub from: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SmtpSecurity {
    #[default]
    Starttls,
    Tls,
    None,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebauthnConfig {
    /// Relying party id, e.g. `termoso.com`.
    pub rp_id: String,
    /// Allowed origins, comma-separated, e.g. `https://app.termoso.com,tauri://localhost`.
    pub origins: String,
    #[serde(default = "default_rp_name")]
    pub rp_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct MetricsConfig {
    /// Opt-in: expose Prometheus metrics on `bind` (a separate listener, keep it private).
    pub enabled: bool,
    pub bind: SocketAddr,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: "127.0.0.1:9090".parse().expect("valid addr"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsoProviderConfig {
    /// Display name.
    pub name: Option<String>,
    /// `oidc` (default) or `saml`.
    #[serde(default)]
    pub kind: SsoKindConfig,
    /// OIDC issuer URL (discovery). Presets: `google`, `microsoft`, `github`
    /// slugs fill this automatically when omitted.
    pub issuer: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    /// Extra scopes (space separated) in addition to `openid email profile`.
    pub scopes: Option<String>,
    /// SAML IdP metadata URL or XML path.
    pub saml_metadata: Option<String>,
    /// Only allow emails from these domains (comma separated).
    pub allowed_domains: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SsoKindConfig {
    #[default]
    Oidc,
    Saml,
}

fn default_true() -> bool {
    true
}
fn default_presign_secs() -> u64 {
    900
}
fn default_smtp_port() -> u16 {
    587
}
fn default_rp_name() -> String {
    "Termoso".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:8080".parse().expect("valid addr"),
            public_url: "http://localhost:8080".into(),
            web_url: None,
            server_name: "Termoso".into(),
            database_url: "postgres://termoso:termoso@localhost:5432/termoso".into(),
            database_max_connections: 20,
            redis_url: "redis://127.0.0.1:6379".into(),
            redis_prefix: "termoso:".into(),
            master_key: String::new(),
            admin_emails: String::new(),
            cors_origins: String::new(),
            trust_proxy: false,
            log: "info,sqlx=warn,hyper=warn,aws=warn".into(),
            log_format: LogFormat::Text,
            s3: None,
            smtp: None,
            webauthn: None,
            metrics: MetricsConfig::default(),
            sso: BTreeMap::new(),
            swagger_ui: true,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut figment =
            Figment::from(figment::providers::Serialized::defaults(Config::default()));
        if let Ok(path) = std::env::var("TERMOSO_CONFIG") {
            figment = figment.merge(Toml::file(path));
        }
        figment = figment.merge(Env::prefixed("TERMOSO_").split("__").lowercase(true));
        let cfg: Config = figment.extract().context("invalid configuration")?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            !self.master_key.is_empty(),
            "TERMOSO_MASTER_KEY is required (generate with `openssl rand -base64 32`)"
        );
        self.master_key()?;
        url::Url::parse(&self.public_url).context("TERMOSO_PUBLIC_URL must be a URL")?;
        Ok(())
    }

    pub fn master_key(&self) -> Result<SymmetricKey> {
        SymmetricKey::from_b64(self.master_key.trim())
            .map_err(|_| anyhow::anyhow!("TERMOSO_MASTER_KEY must be 32 bytes base64"))
    }

    pub fn web_url(&self) -> &str {
        self.web_url.as_deref().unwrap_or(&self.public_url)
    }

    pub fn admin_emails(&self) -> Vec<String> {
        split_csv(&self.admin_emails)
            .into_iter()
            .map(|s| s.to_lowercase())
            .collect()
    }

    pub fn is_bootstrap_admin(&self, email: &str) -> bool {
        self.admin_emails()
            .iter()
            .any(|a| a == &email.to_lowercase())
    }

    pub fn cors_origins(&self) -> Vec<String> {
        split_csv(&self.cors_origins)
    }
}

pub fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}
