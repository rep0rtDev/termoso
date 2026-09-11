//! Serves the built web cabinet (`web/dist`) when `TERMOSO_WEB_DIR` is set.
//!
//! Files are served as-is; any path without a file extension that does not
//! exist on disk falls back to `index.html` so client-side routes deep-link.
//! Hashed Vite assets under `/assets/` are immutable; everything else is
//! revalidated on each load so a deploy takes effect immediately.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::state::AppState;

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

pub fn router(dir: PathBuf) -> Router<AppState> {
    let dir = Arc::new(dir);
    Router::new().fallback(move |req: Request| serve(dir.clone(), req))
}

async fn serve(dir: Arc<PathBuf>, req: Request) -> Response {
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return crate::error::Error::not_found("route").into_response();
    }
    let path = req.uri().path().to_owned();
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
