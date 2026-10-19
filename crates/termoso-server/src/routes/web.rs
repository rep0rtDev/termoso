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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use tower::ServiceExt;
use tower_http::services::ServeDir;

use crate::state::AppState;

/// Request extension marking requests addressed to `TERMOSO_LANDING_URL`.
#[derive(Debug, Clone, Copy)]
pub struct LandingHost;

/// Request extension marking requests addressed to `TERMOSO_DOCS_URL`.
#[derive(Debug, Clone, Copy)]
pub struct DocsHost;

const IMMUTABLE: HeaderValue = HeaderValue::from_static("public, max-age=31536000, immutable");
const REVALIDATE: HeaderValue = HeaderValue::from_static("no-cache");
/// The cabinet is a same-origin SPA: scripts, styles, fonts and API calls all
/// come from this server. `wasm-unsafe-eval` is needed to instantiate the
/// crypto module; Emotion (MUI) injects styles at runtime. With Turnstile
/// configured the widget script and its challenge iframe come from Cloudflare.
const CSP: HeaderValue = HeaderValue::from_static(
    "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; \
     img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; worker-src 'self'; \
     frame-src 'none'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
);
const CSP_TURNSTILE: HeaderValue = HeaderValue::from_static(
    "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' https://challenges.cloudflare.com; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; \
     connect-src 'self'; worker-src 'self'; frame-src https://challenges.cloudflare.com; \
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

/// `<title>` / description of a public page, from `dist/seo.json`.
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
pub struct PageMeta {
    pub title: String,
    pub description: String,
}

/// Public origins the metadata links to. `canonical` URLs and the sitemap use
/// the dedicated landing / docs origins when configured, the cabinet otherwise.
#[derive(Debug, Clone, Default)]
pub struct Origins {
    pub web: String,
    pub landing: Option<String>,
    pub docs: Option<String>,
}

impl Origins {
    fn landing(&self) -> &str {
        self.landing.as_deref().unwrap_or(&self.web)
    }
    fn docs(&self) -> &str {
        self.docs.as_deref().unwrap_or_else(|| self.landing())
    }
    fn canonical(&self, path: &str) -> String {
        let base = if is_docs(path) {
            self.docs()
        } else {
            self.landing()
        };
        format!("{}{}", base.trim_end_matches('/'), path)
    }
}

/// Per-route `index.html` for public pages (landing, docs), rendered once
/// when the router is built so a request is a map lookup. Cabinet routes
/// share one shell with `noindex`: they are behind a login and have nothing
/// to show a crawler.
#[derive(Debug, Default)]
pub struct Seo {
    pages: HashMap<String, PageMeta>,
    origins: Origins,
    shells: HashMap<String, String>,
    cabinet_shell: String,
}

impl Seo {
    pub fn load(dir: &Path, origins: Origins) -> Self {
        let pages = std::fs::read(dir.join("seo.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self::new(pages, origins)
    }

    fn new(pages: HashMap<String, PageMeta>, origins: Origins) -> Self {
        Self {
            pages,
            origins,
            shells: HashMap::new(),
            cabinet_shell: String::new(),
        }
    }

    /// Renders the SPA shell for every known public route plus the cabinet.
    fn prerender(&mut self, index: &str) {
        self.shells = self
            .pages
            .keys()
            .map(|route| (route.clone(), self.render(index, route, true)))
            .collect();
        self.cabinet_shell = self.render(index, "/", false);
    }

    /// The shell to serve for `path`: its prerendered public page when it has
    /// one and the origin exposes it, the `noindex` cabinet shell otherwise.
    fn shell(&self, path: &str, public: bool) -> &str {
        public
            .then(|| self.shells.get(Self::key(path)))
            .flatten()
            .unwrap_or(&self.cabinet_shell)
    }

    fn key(path: &str) -> &str {
        let k = path.trim_end_matches('/');
        if k.is_empty() { "/" } else { k }
    }

    /// Public, indexable routes in sitemap order (landing first, then docs).
    fn public_routes(&self) -> Vec<&str> {
        let mut routes: Vec<&str> = self.pages.keys().map(String::as_str).collect();
        routes.sort_by_key(|r| (*r != "/", *r));
        routes
    }

    /// `<head>` tags for `path`. Public pages get title, description,
    /// canonical and Open Graph / Twitter cards; everything else is `noindex`.
    fn head(&self, path: &str, public: bool) -> String {
        let meta = if public {
            self.pages.get(Self::key(path))
        } else {
            None
        };
        let Some(meta) = meta else {
            return "<meta name=\"robots\" content=\"noindex, nofollow\" />\n    <title>Termoso</title>".to_string();
        };
        let canonical = self.origins.canonical(Self::key(path));
        let image = format!("{}/og.png", self.origins.landing().trim_end_matches('/'));
        let title = escape(&meta.title);
        let description = escape(&meta.description);
        let canonical = escape(&canonical);
        format!(
            "<title>{title}</title>\n    \
             <meta name=\"description\" content=\"{description}\" />\n    \
             <link rel=\"canonical\" href=\"{canonical}\" />\n    \
             <meta property=\"og:type\" content=\"website\" />\n    \
             <meta property=\"og:site_name\" content=\"Termoso\" />\n    \
             <meta property=\"og:title\" content=\"{title}\" />\n    \
             <meta property=\"og:description\" content=\"{description}\" />\n    \
             <meta property=\"og:url\" content=\"{canonical}\" />\n    \
             <meta property=\"og:image\" content=\"{image}\" />\n    \
             <meta name=\"twitter:card\" content=\"summary_large_image\" />\n    \
             <meta name=\"twitter:title\" content=\"{title}\" />\n    \
             <meta name=\"twitter:description\" content=\"{description}\" />\n    \
             <meta name=\"twitter:image\" content=\"{image}\" />"
        )
    }

    /// Replaces the static `<title>` of the SPA shell with [`Seo::head`].
    fn render(&self, index: &str, path: &str, public: bool) -> String {
        let head = self.head(path, public);
        match index.find("<title>").zip(index.find("</title>")) {
            Some((start, end)) => {
                format!(
                    "{}{}{}",
                    &index[..start],
                    head,
                    &index[end + "</title>".len()..]
                )
            }
            None if index.contains("</head>") => {
                index.replacen("</head>", &format!("{head}\n  </head>"), 1)
            }
            None => format!("{head}\n{index}"),
        }
    }

    fn robots(&self, public: bool, docs_here: bool) -> String {
        if public {
            let sitemap = format!(
                "{}/sitemap.xml",
                self.origins.landing().trim_end_matches('/')
            );
            format!("User-agent: *\nAllow: /\nSitemap: {sitemap}\n")
        } else if docs_here {
            "User-agent: *\nDisallow: /\nAllow: /docs\n".to_string()
        } else {
            "User-agent: *\nDisallow: /\n".to_string()
        }
    }

    fn sitemap(&self) -> String {
        let mut out = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
        );
        for route in self.public_routes() {
            let loc = self.origins.canonical(route);
            out.push_str(&format!("  <url><loc>{}</loc></url>\n", escape(&loc)));
        }
        out.push_str("</urlset>\n");
        out
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn router<S: Clone + Send + Sync + 'static>(
    dir: PathBuf,
    landing_on_root: bool,
    docs: bool,
    turnstile: bool,
    mut seo: Seo,
) -> Router<S> {
    let index = std::fs::read_to_string(dir.join("index.html")).unwrap_or_default();
    seo.prerender(&index);
    let dir = Arc::new(dir);
    let seo = Arc::new(seo);
    Router::new().fallback(move |req: Request| {
        serve(
            dir.clone(),
            seo.clone(),
            landing_on_root,
            docs,
            turnstile,
            req,
        )
    })
}

/// Rewrites requests addressed to the dedicated landing or docs origins.
/// Landing origin: `/`, the static files, health probes and `/server/info`
/// are served (tagged with [`LandingHost`] so `/` stays the landing and
/// `/server/info` reports it); `/docs/*` is served too unless the docs have
/// their own origin. Docs origin: `/docs/*`, the static files and the probes
/// are served, `/` goes to `/docs`. Any other cabinet route redirects to the
/// same path under `web_url`; the rest of the API answers 404 on these
/// origins so clients cannot be pointed at the marketing domain.
pub async fn landing_host_layer(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let host = super::request_host(&req);
    let path = req.uri().path().to_owned();
    let web_url = state.cfg.web_url().trim_end_matches('/').to_string();
    let redirect = |base: &str, req: &Request| {
        let target = match req.uri().query() {
            Some(q) => format!("{base}{path}?{q}"),
            None => format!("{base}{path}"),
        };
        Redirect::temporary(&target).into_response()
    };
    let passthrough = matches!(
        path.as_str(),
        "/healthz" | "/readyz" | "/api/v1/server/info"
    ) || (!is_route(&path) && !is_api(&path));
    let api_not_here = || crate::error::Error::not_found("route").into_response();
    if state.docs_host.as_deref() == Some(host.as_str()) {
        if path == "/" {
            return Redirect::temporary("/docs").into_response();
        }
        if !(passthrough || is_docs(&path)) {
            if is_api(&path) {
                return api_not_here();
            }
            if is_cabinet(&path) {
                return redirect(&web_url, &req);
            }
        }
        req.extensions_mut().insert(DocsHost);
        return next.run(req).await;
    }
    if state.landing_host.as_deref() == Some(host.as_str()) {
        if is_docs(&path)
            && let Some(docs_url) = state.cfg.docs_url()
        {
            return redirect(docs_url, &req);
        } else if !(is_landing(&path) || passthrough || is_docs(&path)) {
            if is_api(&path) {
                return api_not_here();
            }
            if is_cabinet(&path) {
                return redirect(&web_url, &req);
            }
        }
        req.extensions_mut().insert(LandingHost);
        return next.run(req).await;
    }
    if is_docs(&path)
        && let Some(docs_url) = state.cfg.docs_url()
    {
        return redirect(docs_url, &req);
    }
    next.run(req).await
}

async fn serve(
    dir: Arc<PathBuf>,
    seo: Arc<Seo>,
    landing_on_root: bool,
    docs: bool,
    turnstile: bool,
    req: Request,
) -> Response {
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return crate::error::Error::not_found("route").into_response();
    }
    let path = req.uri().path().to_owned();
    if is_docs(&path) && !docs {
        return crate::error::Error::not_found("route").into_response();
    }
    let landing_host = req.extensions().get::<LandingHost>().is_some();
    let docs_host = req.extensions().get::<DocsHost>().is_some();
    if path == "/" && !landing_on_root && !landing_host {
        let mut res = Redirect::temporary("/login").into_response();
        res.headers_mut().insert(header::CACHE_CONTROL, REVALIDATE);
        return res;
    }
    // The landing and the docs are the public site; the rest is the cabinet.
    let public = (path == "/" && (landing_on_root || landing_host)) || (docs && is_docs(&path));
    if path == "/robots.txt" {
        // Crawlers read robots.txt per origin: the public site (landing or
        // docs origin) allows everything, the cabinet only its docs, if any.
        let docs_here = docs && seo.origins.docs.is_none();
        return text(
            "text/plain; charset=utf-8",
            seo.robots(landing_on_root || landing_host || docs_host, docs_here),
        );
    }
    if path == "/sitemap.xml" && (landing_on_root || landing_host) {
        return text("application/xml; charset=utf-8", seo.sitemap());
    }
    // `/` is `index.html` on disk; serve the rendered shell instead of the file.
    let mut res = if path == "/" {
        Html(seo.shell(&path, public).to_owned()).into_response()
    } else {
        match ServeDir::new(dir.as_path()).oneshot(req).await {
            Ok(r) => r.map(Body::new),
            Err(never) => match never {},
        }
    };
    if res.status() == StatusCode::NOT_FOUND && is_route(&path) {
        res = Html(seo.shell(&path, public).to_owned()).into_response();
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
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            if turnstile { CSP_TURNSTILE } else { CSP },
        );
        headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    }
    res
}

/// Client-side routes look like `/team/123` — no extension in the last segment.
/// The documentation is part of the web cabinet bundle but belongs to the
/// public site, so the landing origin serves it instead of redirecting.
/// Public pages that live on the landing origin.
fn is_landing(path: &str) -> bool {
    path == "/" || path == "/sponsor"
}

fn is_docs(path: &str) -> bool {
    path == "/docs" || path.starts_with("/docs/")
}

/// Cabinet routes (`web/src/router.tsx`) that the public origins send to the
/// cabinet origin, so links in mail and invites keep working on any host.
/// Anything else that is not a public page is a 404 of the public site and
/// stays on the origin it was requested from.
fn is_cabinet(path: &str) -> bool {
    const ROOTS: &[&str] = &[
        "login",
        "signup",
        "forgot-password",
        "start-over",
        "invite",
        "join",
        "sso",
        "account",
        "confirm",
        "security",
        "devices",
        "bridges",
        "delete-account",
        "team",
        "vaults",
        "admin",
        "sshid",
    ];
    let root = path.trim_start_matches('/').split('/').next().unwrap_or("");
    ROOTS.contains(&root)
}

fn is_api(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

fn is_route(path: &str) -> bool {
    !path.starts_with("/assets/") && !path.rsplit('/').next().unwrap_or("").contains('.')
}

fn text(content_type: &'static str, body: String) -> Response {
    let mut res = (StatusCode::OK, body).into_response();
    let headers = res.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(header::CACHE_CONTROL, REVALIDATE);
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    fn dist() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termoso-web-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(
            dir.join("index.html"),
            "<!doctype html><html><head><title>Termoso</title></head><body><div id=root></div></body></html>",
        )
        .unwrap();
        std::fs::write(dir.join("assets/app-1.js"), "export {};").unwrap();
        dir
    }

    async fn get(landing_on_root: bool, path: &str, landing_host: bool) -> Response {
        get_docs(landing_on_root, true, path, landing_host).await
    }

    async fn get_docs(
        landing_on_root: bool,
        docs: bool,
        path: &str,
        landing_host: bool,
    ) -> Response {
        let mut req = Request::get(path).body(Body::empty()).unwrap();
        if landing_host {
            req.extensions_mut().insert(LandingHost);
        }
        if let Some(rest) = path.strip_prefix("/docs-host") {
            // `/docs-host/x` stands for `/x` asked on the dedicated docs origin.
            *req.uri_mut() = rest.parse().unwrap();
            req.extensions_mut().insert(DocsHost);
        }
        router::<()>(dist(), landing_on_root, docs, false, seo())
            .oneshot(req)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn csp_opens_cloudflare_only_with_turnstile() {
        let csp = |turnstile: bool| async move {
            let req = Request::get("/login").body(Body::empty()).unwrap();
            let res = router::<()>(dist(), false, false, turnstile, seo())
                .oneshot(req)
                .await
                .unwrap();
            res.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .to_owned()
        };
        let plain = csp(false).await;
        assert!(plain.contains("frame-src 'none'"));
        assert!(!plain.contains("challenges.cloudflare.com"));
        let with = csp(true).await;
        assert!(
            with.contains("script-src 'self' 'wasm-unsafe-eval' https://challenges.cloudflare.com")
        );
        assert!(with.contains("frame-src https://challenges.cloudflare.com"));
        assert!(with.contains("frame-ancestors 'none'"));
    }

    fn seo() -> Seo {
        let mut pages = HashMap::new();
        pages.insert(
            "/".to_string(),
            PageMeta {
                title: "Termoso — SSH client".into(),
                description: "Native & \"private\"".into(),
            },
        );
        pages.insert(
            "/docs".to_string(),
            PageMeta {
                title: "Termoso docs".into(),
                description: "All the docs".into(),
            },
        );
        pages.insert(
            "/docs/quick-start".to_string(),
            PageMeta {
                title: "Quick start · Termoso docs".into(),
                description: "Install and connect".into(),
            },
        );
        Seo::new(
            pages,
            Origins {
                web: "https://app.example.com".into(),
                landing: Some("https://example.com".into()),
                docs: Some("https://docs.example.com".into()),
            },
        )
    }

    async fn body(res: Response) -> String {
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn public_pages_get_metadata_and_the_cabinet_gets_noindex() {
        let html = body(get(true, "/docs/quick-start", false).await).await;
        assert!(
            html.contains("<title>Quick start · Termoso docs</title>"),
            "{html}"
        );
        assert!(html.contains(r#"<meta name="description" content="Install and connect" />"#));
        assert!(html.contains(
            r#"<link rel="canonical" href="https://docs.example.com/docs/quick-start" />"#
        ));
        assert!(
            html.contains(r#"<meta property="og:image" content="https://example.com/og.png" />"#)
        );
        assert!(!html.contains("noindex"));

        // Trailing slashes resolve to the same page; the landing escapes quotes.
        let html = body(get(true, "/docs/", false).await).await;
        assert!(html.contains("<title>Termoso docs</title>"), "{html}");
        let html = body(get(false, "/", true).await).await;
        assert!(
            html.contains("<title>Termoso — SSH client</title>"),
            "{html}"
        );
        assert!(
            html.contains(r#"content="Native &amp; &quot;private&quot;""#),
            "{html}"
        );
        assert!(
            html.contains(r#"<link rel="canonical" href="https://example.com/" />"#),
            "{html}"
        );

        // Cabinet routes and unknown docs paths: generic title, noindex.
        for path in ["/login", "/team/1", "/docs/nope"] {
            let html = body(get(true, path, false).await).await;
            assert!(html.contains("<title>Termoso</title>"), "{path}: {html}");
            assert!(
                html.contains(r#"<meta name="robots" content="noindex, nofollow" />"#),
                "{path}"
            );
        }
        // Not a shell: the asset is served verbatim.
        let js = body(get(true, "/assets/app-1.js", false).await).await;
        assert_eq!(js, "export {};");
    }

    #[tokio::test]
    async fn robots_and_sitemap_follow_the_origin() {
        let res = get(false, "/robots.txt", false).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(body(res).await.contains("Disallow: /"));

        for (path, host) in [("/robots.txt", true), ("/docs-host/robots.txt", false)] {
            let txt = body(get(false, path, host).await).await;
            assert!(
                txt.starts_with("User-agent: *\nAllow: /\n"),
                "{path}: {txt}"
            );
            assert!(
                txt.contains("Sitemap: https://example.com/sitemap.xml"),
                "{txt}"
            );
        }
        // Docs on the cabinet origin itself (no dedicated docs URL) stay crawlable.
        let mut seo = seo();
        seo.origins.docs = None;
        let req = Request::get("/robots.txt").body(Body::empty()).unwrap();
        let res = router::<()>(dist(), false, true, false, seo)
            .oneshot(req)
            .await
            .unwrap();
        assert_eq!(
            body(res).await,
            "User-agent: *\nDisallow: /\nAllow: /docs\n"
        );

        let res = get(false, "/sitemap.xml", true).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers()[header::CONTENT_TYPE],
            "application/xml; charset=utf-8"
        );
        let xml = body(res).await;
        assert!(xml.contains("<loc>https://example.com/</loc>"), "{xml}");
        assert!(
            xml.contains("<loc>https://docs.example.com/docs/quick-start</loc>"),
            "{xml}"
        );
        assert!(xml.find("example.com/</loc>").unwrap() < xml.find("/docs</loc>").unwrap());

        // The cabinet without a landing has no public site to map.
        let res = get(false, "/sitemap.xml", false).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
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

    #[tokio::test]
    async fn docs_can_be_switched_off() {
        let res = get_docs(true, false, "/docs/quick-start", false).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let res = get_docs(true, true, "/docs/quick-start", false).await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[test]
    fn docs_stay_on_the_landing_origin() {
        assert!(is_docs("/docs"));
        assert!(is_docs("/docs/self-hosting/overview"));
        assert!(!is_docs("/docsy"));
        assert!(!is_docs("/account"));
    }

    #[test]
    fn api_paths_are_recognised_except_the_landing_passthroughs() {
        assert!(is_api("/api/v1/vaults"));
        assert!(is_api("/api/v1/auth/login/start"));
        assert!(is_api("/api"));
        assert!(!is_api("/apiary"));
        assert!(!is_api("/sponsor"));
        assert!(!is_api("/docs/api"));
    }

    #[test]
    fn routes_are_extensionless() {
        assert!(is_route("/"));
        assert!(is_route("/team/1"));
        assert!(!is_route("/favicon.svg"));
        assert!(!is_route("/assets/x"));
        assert!(is_cabinet("/login"));
        assert!(is_cabinet("/invite/abc?x=1"));
        assert!(is_cabinet("/admin/teams/1"));
        assert!(!is_cabinet("/signin"));
        assert!(!is_cabinet("/sponsor"));
        assert!(!is_cabinet("/docs/x"));
    }
}
