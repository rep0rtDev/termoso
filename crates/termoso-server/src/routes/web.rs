//! Serves the built web cabinet (`web/dist`) when `TERMOSO_WEB_DIR` is set.
//!
//! Files are served as-is; any path without a file extension that does not
//! exist on disk falls back to `index.html` so client-side routes deep-link.
//! Hashed Vite assets under `/assets/` are immutable; everything else is
//! revalidated on each load so a deploy takes effect immediately.
//!
//! `/` is the landing page unless `TERMOSO_LANDING=false` or the landing has
//! its own origin (`TERMOSO_LANDING_URL`), in which case `/` on the cabinet
//! origin redirects to `/login`. On the landing origin only `/`, the static
//! files and `/api/v1/server/info` are served; everything else redirects to
//! the same path on the cabinet so deep links keep working.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::state::AppState;

/// Request extension marking requests addressed to `TERMOSO_LANDING_URL`.
#[derive(Debug, Clone, Copy)]
pub struct LandingHost;

const IMMUTABLE: HeaderValue = HeaderValue::from_static("public, max-age=31536000, immutable");
const REVALIDATE: HeaderValue = HeaderValue::from_static("no-cache");
/// The cabinet is a same-origin SPA: scripts, styles, fonts and API calls all
/// come from this server. `wasm-unsafe-eval` is needed to instantiate the
/// crypto module; Emotion (MUI) injects styles at runtime.
const CSP: HeaderValue = HeaderValue::from_static(
    "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; \
     img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; worker-src 'self'; \
     object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
);

pub fn validate_dir(dir: &Path) -> Result<()> {
    let index = dir.join("index.html");
    anyhow::ensure!(
        index.is_file(),
        "TERMOSO_WEB_DIR={} does not contain index.html (build the cabinet with `npm run build` in web/)",
        dir.display()
    );
    Ok(())
}

pub fn router<S: Clone + Send + Sync + 'static>(dir: PathBuf, landing_on_root: bool) -> Router<S> {
    let dir = Arc::new(dir);
    Router::new().fallback(move |req: Request| serve(dir.clone(), landing_on_root, req))
}

/// Rewrites requests addressed to the dedicated landing origin: `/` and the
/// static files are served (tagged with [`LandingHost`] so `/` stays the
/// landing and `/server/info` reports it), health probes and `/server/info`
/// pass through, anything else goes to the cabinet at `web_url`.
pub async fn landing_host_layer(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let Some(want) = state.landing_host.as_deref() else {
        return next.run(req).await;
    };
    if super::request_host(&req) != want {
        return next.run(req).await;
    }
    let path = req.uri().path();
    let served_here =
        matches!(path, "/" | "/healthz" | "/readyz" | "/api/v1/server/info") || !is_route(path);
    if !served_here {
        let target = match req.uri().query() {
            Some(q) => format!("{}{path}?{q}", state.cfg.web_url().trim_end_matches('/')),
            None => format!("{}{path}", state.cfg.web_url().trim_end_matches('/')),
        };
        return Redirect::temporary(&target).into_response();
    }
    req.extensions_mut().insert(LandingHost);
    next.run(req).await
}

async fn serve(dir: Arc<PathBuf>, landing_on_root: bool, req: Request) -> Response {
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return crate::error::Error::not_found("route").into_response();
    }
    let path = req.uri().path().to_owned();
    if path == "/" && !landing_on_root && req.extensions().get::<LandingHost>().is_none() {
        let mut res = Redirect::temporary("/login").into_response();
        res.headers_mut().insert(header::CACHE_CONTROL, REVALIDATE);
        return res;
    }
    let mut res = match ServeDir::new(dir.as_path()).oneshot(req).await {
        Ok(r) => r.map(Body::new),
        Err(never) => match never {},
    };
    if res.status() == StatusCode::NOT_FOUND && is_route(&path) {
        let index = Request::get("/index.html").body(Body::empty());
        res = match index {
            Ok(index) => match ServeFile::new(dir.join("index.html")).oneshot(index).await {
                Ok(r) => r.map(Body::new),
                Err(never) => match never {},
            },
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
    }

    let headers = res.headers_mut();
    let html = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("text/html"));
    let cache = if path.starts_with("/assets/") {
        IMMUTABLE
    } else {
        REVALIDATE
    };
    headers.insert(header::CACHE_CONTROL, cache);
    if html {
        headers.insert(header::CONTENT_SECURITY_POLICY, CSP);
        headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    }
    res
}

/// Client-side routes look like `/team/123` — no extension in the last segment.
fn is_route(path: &str) -> bool {
    !path.starts_with("/assets/") && !path.rsplit('/').next().unwrap_or("").contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termoso-web-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), "<!doctype html><div id=root></div>").unwrap();
        std::fs::write(dir.join("assets/app-1.js"), "export {};").unwrap();
        dir
    }

    async fn get(landing_on_root: bool, path: &str, landing_host: bool) -> Response {
        let mut req = Request::get(path).body(Body::empty()).unwrap();
        if landing_host {
            req.extensions_mut().insert(LandingHost);
        }
        router::<()>(dist(), landing_on_root)
            .oneshot(req)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn root_is_the_landing_by_default() {
        let res = get(true, "/", false).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
    }

    #[tokio::test]
    async fn root_redirects_to_login_when_landing_is_off() {
        let res = get(false, "/", false).await;
        assert_eq!(res.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(res.headers()[header::LOCATION], "/login");
        assert_eq!(res.headers()[header::CACHE_CONTROL], "no-cache");

        // Only the root changes: routes, assets and the fallback are untouched.
        let res = get(false, "/login", false).await;
        assert_eq!(res.status(), StatusCode::OK);
        let res = get(false, "/assets/app-1.js", false).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CACHE_CONTROL], IMMUTABLE);
    }

    #[tokio::test]
    async fn landing_host_requests_get_the_landing_even_when_root_is_the_cabinet() {
        let res = get(false, "/", true).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
    }

    #[test]
    fn routes_are_extensionless() {
        assert!(is_route("/"));
        assert!(is_route("/team/1"));
        assert!(!is_route("/favicon.svg"));
        assert!(!is_route("/assets/x"));
    }
}
