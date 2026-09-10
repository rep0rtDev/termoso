//! Single sign-on. The IdP only proves *who owns the email*; the vault password
//! (OPAQUE) is still required because the server never holds decryption keys.
//!
//! Supported: any OpenID Connect provider (discovery), presets for `google`
//! and `microsoft`, and GitHub (plain OAuth2 + user API). SAML is accepted in
//! the configuration but not implemented yet and is rejected at startup.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::Context;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use termoso_proto::auth::{SsoKind, SsoProvider, SsoResult};

use crate::config::{Config, SsoKindConfig, SsoProviderConfig};
use crate::error::{ApiResult, Error};
use crate::state::AppState;
use crate::util::random_token;

pub const FLOW_TTL: Duration = Duration::from_secs(600);
pub const SESSION_TTL: Duration = Duration::from_secs(900);

type OidcClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

type GitHubClient = oauth2::basic::BasicClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

enum Backend {
    Oidc(Box<OidcClient>),
    GitHub(Box<GitHubClient>),
}

pub struct Provider {
    pub id: String,
    pub name: String,
    allowed_domains: Vec<String>,
    extra_scopes: Vec<String>,
    backend: Backend,
}

#[derive(Default)]
pub struct SsoRegistry {
    providers: BTreeMap<String, Provider>,
    http: Option<reqwest::Client>,
}

#[derive(Serialize, Deserialize)]
struct FlowState {
    provider: String,
    nonce: Option<String>,
    pkce: String,
    /// Where to send the browser after the callback (client deep link / web cabinet URL).
    redirect: Option<String>,
}

/// What a consumed `sso_session` token proves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoSession {
    pub provider: String,
    pub subject: String,
    pub email: String,
    pub display_name: Option<String>,
}

fn flow_key(id: &str) -> String {
    format!("sso_flow:{id}")
}
fn result_key(id: &str) -> String {
    format!("sso_result:{id}")
}
fn session_key(token: &str) -> String {
    format!("sso_sess:{token}")
}

fn http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent(format!("termoso-server/{}", crate::state::VERSION))
        .build()?)
}

impl SsoRegistry {
    pub async fn from_config(cfg: &Config) -> anyhow::Result<Self> {
        if cfg.sso.is_empty() {
            return Ok(Self::default());
        }
        let http = http_client()?;
        let redirect = RedirectUrl::new(format!(
            "{}/api/v1/auth/sso/callback",
            cfg.public_url.trim_end_matches('/')
        ))?;
        let mut providers = BTreeMap::new();
        for (slug, p) in &cfg.sso {
            let slug = slug.to_lowercase();
            let provider = build_provider(&slug, p, &redirect, &http)
                .await
                .with_context(|| format!("SSO provider `{slug}`"))?;
            providers.insert(slug, provider);
        }
        Ok(Self {
            providers,
            http: Some(http),
        })
    }

    pub fn list(&self) -> Vec<SsoProvider> {
        self.providers
            .values()
            .map(|p| SsoProvider {
                id: p.id.clone(),
                name: p.name.clone(),
                kind: SsoKind::Oidc,
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    fn get(&self, id: &str) -> ApiResult<&Provider> {
        self.providers
            .get(id)
            .ok_or_else(|| Error::not_found("SSO provider"))
    }
}

async fn build_provider(
    slug: &str,
    p: &SsoProviderConfig,
    redirect: &RedirectUrl,
    http: &reqwest::Client,
) -> anyhow::Result<Provider> {
    if p.kind == SsoKindConfig::Saml {
        anyhow::bail!(
            "SAML providers are not supported yet; use an OIDC bridge (e.g. Keycloak/Dex) for now"
        );
    }
    let client_id = p.client_id.clone().context("client_id is required")?;
    let client_secret = p
        .client_secret
        .clone()
        .context("client_secret is required")?;
    let allowed_domains = p
        .allowed_domains
        .as_deref()
        .map(crate::config::split_csv)
        .unwrap_or_default()
        .into_iter()
        .map(|d| d.to_lowercase())
        .collect();
    let extra_scopes: Vec<String> = p
        .scopes
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(String::from)
        .collect();

    let (name, backend) = if slug == "github" {
        let client = oauth2::basic::BasicClient::new(oauth2::ClientId::new(client_id))
            .set_client_secret(oauth2::ClientSecret::new(client_secret))
            .set_auth_uri(oauth2::AuthUrl::new(
                "https://github.com/login/oauth/authorize".into(),
            )?)
            .set_token_uri(oauth2::TokenUrl::new(
                "https://github.com/login/oauth/access_token".into(),
            )?)
            .set_redirect_uri(oauth2::RedirectUrl::new(redirect.to_string())?);
        ("GitHub".to_string(), Backend::GitHub(Box::new(client)))
    } else {
        let issuer = p
            .issuer
            .clone()
            .or_else(|| match slug {
                "google" => Some("https://accounts.google.com".into()),
                "microsoft" => Some("https://login.microsoftonline.com/common/v2.0".into()),
                _ => None,
            })
            .context("issuer is required for OIDC providers")?;
        let metadata = CoreProviderMetadata::discover_async(IssuerUrl::new(issuer)?, http)
            .await
            .context("OIDC discovery failed")?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
        )
        .set_redirect_uri(redirect.clone());
        let default_name = match slug {
            "google" => "Google",
            "microsoft" => "Microsoft",
            _ => slug,
        };
        (default_name.to_string(), Backend::Oidc(Box::new(client)))
    };
    Ok(Provider {
        id: slug.to_string(),
        name: p.name.clone().unwrap_or(name),
        allowed_domains,
        extra_scopes,
        backend,
    })
}

/// Begin a flow. Returns `(authorization_url, flow_id)`.
pub async fn start(
    state: &AppState,
    provider_id: &str,
    redirect: Option<String>,
) -> ApiResult<(String, String)> {
    let provider = state.sso.get(provider_id)?;
    let flow_id = random_token();
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, nonce) = match &provider.backend {
        Backend::Oidc(client) => {
            let fid = flow_id.clone();
            let mut req = client
                .authorize_url(
                    CoreAuthenticationFlow::AuthorizationCode,
                    move || CsrfToken::new(fid),
                    Nonce::new_random,
                )
                .add_scope(Scope::new("email".into()))
                .add_scope(Scope::new("profile".into()))
                .set_pkce_challenge(pkce_challenge);
            for s in &provider.extra_scopes {
                req = req.add_scope(Scope::new(s.clone()));
            }
            let (url, _csrf, nonce) = req.url();
            (url.to_string(), Some(nonce.secret().clone()))
        }
        Backend::GitHub(client) => {
            let fid = flow_id.clone();
            let mut req = client
                .authorize_url(move || oauth2::CsrfToken::new(fid))
                .add_scope(oauth2::Scope::new("read:user".into()))
                .add_scope(oauth2::Scope::new("user:email".into()))
                .set_pkce_challenge(pkce_challenge);
            for s in &provider.extra_scopes {
                req = req.add_scope(oauth2::Scope::new(s.clone()));
            }
            let (url, _csrf) = req.url();
            (url.to_string(), None)
        }
    };
    let redirect = redirect.filter(|r| is_safe_redirect(state, r));
    state
        .cache
        .set_json(
            &flow_key(&flow_id),
            &FlowState {
                provider: provider_id.to_string(),
                nonce,
                pkce: pkce_verifier.secret().clone(),
                redirect,
            },
            FLOW_TTL,
        )
        .await?;
    state
        .cache
        .set_json(&result_key(&flow_id), &SsoResult::Pending, FLOW_TTL)
        .await?;
    Ok((url, flow_id))
}

/// Only allow redirects to the web cabinet or custom app schemes (desktop/mobile deep links).
fn is_safe_redirect(state: &AppState, r: &str) -> bool {
    if let Ok(u) = url::Url::parse(r) {
        let scheme = u.scheme();
        if scheme == "termoso" {
            return true;
        }
        if scheme == "http" || scheme == "https" {
            let web = state.cfg.web_url().trim_end_matches('/');
            let public = state.cfg.public_url.trim_end_matches('/');
            return r.starts_with(web)
                || r.starts_with(public)
                || r.starts_with("http://localhost")
                || r.starts_with("http://127.0.0.1");
        }
    }
    false
}

/// Handle the IdP redirect. Returns `(redirect target if any, flow_id)`.
pub async fn callback(
    state: &AppState,
    flow_id: &str,
    code: Option<&str>,
    error: Option<&str>,
) -> ApiResult<(Option<String>, String)> {
    let Some(flow) = state
        .cache
        .take_json::<FlowState>(&flow_key(flow_id))
        .await?
    else {
        return Err(Error::token_expired());
    };
    let outcome = match (code, error) {
        (_, Some(err)) => SsoResult::Failed {
            message: format!("Identity provider returned an error: {err}"),
        },
        (Some(code), None) => match exchange(state, &flow, code).await {
            Ok(sess) => finish(state, sess).await?,
            Err(e) => {
                tracing::warn!(error = %e, provider = %flow.provider, "sso exchange failed");
                SsoResult::Failed {
                    message: "Could not verify your identity with the provider".into(),
                }
            }
        },
        (None, None) => SsoResult::Failed {
            message: "Missing authorization code".into(),
        },
    };
    state
        .cache
        .set_json(&result_key(flow_id), &outcome, FLOW_TTL)
        .await?;
    Ok((flow.redirect, flow_id.to_string()))
}

async fn exchange(state: &AppState, flow: &FlowState, code: &str) -> anyhow::Result<SsoSession> {
    let provider = state
        .sso
        .get(&flow.provider)
        .map_err(|_| anyhow::anyhow!("provider vanished"))?;
    let http = state.sso.http.as_ref().context("no http client")?;
    let sess = match &provider.backend {
        Backend::Oidc(client) => {
            let tokens = client
                .exchange_code(AuthorizationCode::new(code.to_string()))?
                .set_pkce_verifier(PkceCodeVerifier::new(flow.pkce.clone()))
                .request_async(http)
                .await?;
            let id_token = tokens.id_token().context("no id_token")?;
            let nonce = Nonce::new(flow.nonce.clone().context("no nonce")?);
            let claims = id_token.claims(&client.id_token_verifier(), &nonce)?;
            let email = claims.email().context("no email claim")?.to_string();
            if claims.email_verified() == Some(false) {
                anyhow::bail!("email not verified at provider");
            }
            let display_name = claims
                .name()
                .and_then(|n| n.get(None))
                .map(|n| n.to_string());
            SsoSession {
                provider: provider.id.clone(),
                subject: claims.subject().to_string(),
                email,
                display_name,
            }
        }
        Backend::GitHub(client) => {
            let tokens = client
                .exchange_code(oauth2::AuthorizationCode::new(code.to_string()))
                .set_pkce_verifier(oauth2::PkceCodeVerifier::new(flow.pkce.clone()))
                .request_async(http)
                .await?;
            let access = tokens.access_token().secret().clone();
            #[derive(Deserialize)]
            struct GhUser {
                id: u64,
                name: Option<String>,
                login: String,
            }
            #[derive(Deserialize)]
            struct GhEmail {
                email: String,
                primary: bool,
                verified: bool,
            }
            let user: GhUser = http
                .get("https://api.github.com/user")
                .bearer_auth(&access)
                .header("accept", "application/vnd.github+json")
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let emails: Vec<GhEmail> = http
                .get("https://api.github.com/user/emails")
                .bearer_auth(&access)
                .header("accept", "application/vnd.github+json")
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let email = emails
                .iter()
                .find(|e| e.primary && e.verified)
                .or_else(|| emails.iter().find(|e| e.verified))
                .map(|e| e.email.clone())
                .context("no verified email on GitHub account")?;
            SsoSession {
                provider: provider.id.clone(),
                subject: user.id.to_string(),
                email,
                display_name: user.name.or(Some(user.login)),
            }
        }
    };
    let email = crate::util::normalize_email(&sess.email).context("invalid email from provider")?;
    if !provider.allowed_domains.is_empty() {
        let domain = email.rsplit('@').next().unwrap_or_default();
        anyhow::ensure!(
            provider.allowed_domains.iter().any(|d| d == domain),
            "email domain not allowed for this provider"
        );
    }
    Ok(SsoSession { email, ..sess })
}

async fn finish(state: &AppState, sess: SsoSession) -> ApiResult<SsoResult> {
    // Prefer the linked identity; fall back to the email.
    let linked: Option<(String,)> =
        sqlx::query_as("SELECT u.email FROM sso_identities i JOIN users u ON u.id = i.user_id WHERE i.provider = $1 AND i.subject = $2")
            .bind(&sess.provider)
            .bind(&sess.subject)
            .fetch_optional(&state.db)
            .await?;
    let email = match linked {
        Some((e,)) => e,
        None => sess.email.clone(),
    };
    let exists: Option<(bool,)> =
        sqlx::query_as("SELECT disabled FROM users WHERE lower(email) = $1")
            .bind(&email)
            .fetch_optional(&state.db)
            .await?;
    let token = random_token();
    let stored = SsoSession {
        email: email.clone(),
        ..sess
    };
    state
        .cache
        .set_json(&session_key(&token), &stored, SESSION_TTL)
        .await?;
    Ok(match exists {
        Some((true,)) => SsoResult::Failed {
            message: "This account is disabled".into(),
        },
        Some((false,)) => SsoResult::LoginRequired {
            sso_session: token,
            email,
        },
        None => SsoResult::RegistrationRequired {
            sso_session: token,
            email,
            display_name: stored.display_name,
        },
    })
}

pub async fn poll(state: &AppState, flow_id: &str) -> ApiResult<SsoResult> {
    state
        .cache
        .get_json::<SsoResult>(&result_key(flow_id))
        .await?
        .ok_or_else(Error::token_expired)
}

/// Consume an `sso_session` token; verifies it matches `email`.
pub async fn consume_session(state: &AppState, token: &str, email: &str) -> ApiResult<SsoSession> {
    let sess = state
        .cache
        .take_json::<SsoSession>(&session_key(token))
        .await?
        .ok_or_else(Error::token_expired)?;
    if sess.email != email {
        return Err(Error::forbidden("SSO session does not match this email"));
    }
    Ok(sess)
}

pub async fn link_identity(
    state: &AppState,
    sess: &SsoSession,
    user_id: uuid::Uuid,
) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO sso_identities (provider, subject, user_id, email) VALUES ($1, $2, $3, $4)
         ON CONFLICT (provider, subject) DO UPDATE SET user_id = EXCLUDED.user_id, email = EXCLUDED.email",
    )
    .bind(&sess.provider)
    .bind(&sess.subject)
    .bind(user_id)
    .bind(&sess.email)
    .execute(&state.db)
    .await?;
    Ok(())
}
