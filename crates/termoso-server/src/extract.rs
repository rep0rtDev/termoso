//! Request extractors: authenticated user, admin, client IP, validated JSON.

use axum::extract::rejection::JsonRejection;
use axum::extract::{
    ConnectInfo, FromRequest, FromRequestParts, OptionalFromRequest, OptionalFromRequestParts,
    Request,
};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use std::net::{IpAddr, SocketAddr};
use uuid::Uuid;

use crate::error::Error;
use crate::session::{self, SessionInfo};
use crate::state::AppState;

/// Authenticated request. Rejects disabled accounts.
#[derive(Debug, Clone)]
pub struct Auth {
    pub session: SessionInfo,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

impl Auth {
    pub fn user_id(&self) -> Uuid {
        self.session.user_id
    }
    pub fn device_id(&self) -> Uuid {
        self.session.device_id
    }
    /// Set when the caller is an API bridge rather than a person.
    pub fn bridge_id(&self) -> Option<Uuid> {
        self.session.bridge_id
    }
    /// Fails unless this session completed a step-up recently.
    pub fn require_step_up(&self) -> Result<(), Error> {
        if self.session.step_up_fresh() {
            Ok(())
        } else {
            Err(Error::reauth_required())
        }
    }
}

/// Routes an API-bridge session may call. Everything else — account, teams,
/// vaults, logs, live sessions — is off limits so a leaked bridge token
/// cannot reach beyond the vaults sealed to it.
fn bridge_allowed(method: &http::Method, path: &str) -> bool {
    let path = path.strip_prefix(termoso_proto::API_PREFIX).unwrap_or(path);
    matches!(
        (method, path),
        (&http::Method::POST, "/sync/push")
            | (&http::Method::POST, "/sync/pull")
            | (&http::Method::GET, "/bridge/me")
    )
}

pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// Client address for rate limits and security events.
///
/// Behind a trusted proxy the *last* `X-Forwarded-For` entry is used: that is
/// the one the proxy itself appended, whereas earlier entries arrive from the
/// client and can be forged. Values that are not IP addresses are ignored.
pub fn client_ip(parts: &Parts, trust_proxy: bool) -> Option<String> {
    if trust_proxy && let Some(ip) = forwarded_ip(&parts.headers) {
        return Some(ip.to_string());
    }
    parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string())
}

fn forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    header("x-forwarded-for")
        .and_then(|v| v.rsplit(',').next())
        .or_else(|| header("x-real-ip"))
        .and_then(parse_ip)
}

fn parse_ip(v: &str) -> Option<IpAddr> {
    let v = v.trim();
    v.trim_matches(['[', ']'])
        .parse()
        .ok()
        .or_else(|| v.parse::<SocketAddr>().ok().map(|s| s.ip()))
}

pub fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(256).collect())
}

/// Unauthenticated request context (IP + UA).
#[derive(Debug, Clone)]
pub struct Client {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

impl FromRequestParts<AppState> for Client {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Error> {
        Ok(Client {
            ip: client_ip(parts, state.cfg.trust_proxy),
            user_agent: user_agent(&parts.headers),
        })
    }
}

impl FromRequestParts<AppState> for Auth {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Error> {
        let token = bearer(&parts.headers).ok_or_else(Error::unauthorized)?;
        let info = session::validate(state, token).await?;
        if info.disabled {
            return Err(Error::account_disabled());
        }
        if info.bridge_id.is_some() && !bridge_allowed(&parts.method, parts.uri.path()) {
            return Err(Error::forbidden(
                "API bridge tokens may only sync and read /bridge/me",
            ));
        }
        let ip = client_ip(parts, state.cfg.trust_proxy);
        session::touch(state, &info, ip.as_deref()).await?;
        Ok(Auth {
            session: info,
            ip,
            user_agent: user_agent(&parts.headers),
        })
    }
}

impl OptionalFromRequestParts<AppState> for Auth {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Option<Self>, Error> {
        if bearer(&parts.headers).is_none() {
            return Ok(None);
        }
        <Auth as FromRequestParts<AppState>>::from_request_parts(parts, state)
            .await
            .map(Some)
    }
}

/// Authenticated request whose session completed a step-up
/// (`POST /auth/reauth/*`) within the last few minutes. Sensitive account
/// mutations take this instead of `Auth` so a leaked bearer token alone cannot
/// lock the owner out.
#[derive(Debug, Clone)]
pub struct StepUp(pub Auth);

impl std::ops::Deref for StepUp {
    type Target = Auth;
    fn deref(&self) -> &Auth {
        &self.0
    }
}

impl FromRequestParts<AppState> for StepUp {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Error> {
        let auth = <Auth as FromRequestParts<AppState>>::from_request_parts(parts, state).await?;
        auth.require_step_up()?;
        Ok(StepUp(auth))
    }
}

/// Authenticated administrator.
#[derive(Debug, Clone)]
pub struct Admin(pub Auth);

impl FromRequestParts<AppState> for Admin {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Error> {
        let auth = <Auth as FromRequestParts<AppState>>::from_request_parts(parts, state).await?;
        if !auth.session.is_admin {
            return Err(Error::forbidden("Administrator access required"));
        }
        Ok(Admin(auth))
    }
}

/// `axum::Json` with our error body on failure.
pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Self, Error> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(v)) => Ok(Json(v)),
            Err(rej) => Err(Error::bad_request(rej.body_text())),
        }
    }
}

/// `Option<Json<T>>`: absent when the request carries no body at all.
impl<T, S> OptionalFromRequest<S> for Json<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Error> {
        let empty = req
            .headers()
            .get(axum::http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "0")
            .unwrap_or(
                req.headers()
                    .get(axum::http::header::CONTENT_TYPE)
                    .is_none(),
            );
        if empty {
            return Ok(None);
        }
        <Json<T> as FromRequest<S>>::from_request(req, state)
            .await
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(headers: &[(&str, &str)]) -> Parts {
        let mut req = http::Request::builder();
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let (mut parts, ()) = req.body(()).unwrap().into_parts();
        parts
            .extensions
            .insert(ConnectInfo::<SocketAddr>("10.0.0.2:4000".parse().unwrap()));
        parts
    }

    #[test]
    fn proxy_headers_are_ignored_unless_trusted() {
        let p = parts(&[("x-forwarded-for", "1.2.3.4")]);
        assert_eq!(client_ip(&p, false).as_deref(), Some("10.0.0.2"));
        assert_eq!(client_ip(&p, true).as_deref(), Some("1.2.3.4"));
    }

    #[test]
    fn last_forwarded_entry_wins() {
        let p = parts(&[("x-forwarded-for", "6.6.6.6, 203.0.113.9")]);
        assert_eq!(client_ip(&p, true).as_deref(), Some("203.0.113.9"));
        let p = parts(&[("x-forwarded-for", "6.6.6.6,[2001:db8::1]:443")]);
        assert_eq!(client_ip(&p, true).as_deref(), Some("2001:db8::1"));
    }

    #[test]
    fn garbage_falls_back_to_the_socket() {
        let p = parts(&[("x-forwarded-for", "unknown"), ("x-real-ip", "nope")]);
        assert_eq!(client_ip(&p, true).as_deref(), Some("10.0.0.2"));
        let p = parts(&[("x-real-ip", "198.51.100.7")]);
        assert_eq!(client_ip(&p, true).as_deref(), Some("198.51.100.7"));
    }
}
