//! The WebDAV client against an in-process `dav-server` (in-memory
//! filesystem): every verb we issue, Basic and Digest negotiation, byte-range
//! reads, positional file access, transfer progress/cancellation and TLS
//! pinning against a self-signed certificate.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use base64::Engine;
use dav_server::fakels::FakeLs;
use dav_server::memfs::MemFs;
use dav_server::{DavHandler, DavMethodSet};
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sha2::Digest as _;
use termoso_core::error::CoreError;
use termoso_core::remote::{RemoteFs, RemoteProtocol};
use termoso_core::sftp::{EntryKind, OpenMode, Progress, TransferOptions};
use termoso_core::webdav::{
    ClientIdentity, TlsPolicy, WebDav, WebDavConfig, client_certificate_fingerprint, fingerprint,
    probe_certificate, split_client_pem,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

const USER: &str = "alice";
const PASS: &str = "s3cret pass";
const REALM: &str = "termoso-tests";
const NONCE: &str = "dcd98b7102dd2f0e8b11d0f600bfb0c093";

const CERT_DER: &[u8] = include_bytes!("fixtures/webdav-test.crt.der");
const KEY_DER: &[u8] = include_bytes!("fixtures/webdav-test.key.der");
/// CA the mTLS server trusts for client certificates, and a client
/// certificate it issued.
const CLIENT_CA_DER: &[u8] = include_bytes!("fixtures/webdav-client-ca.crt.der");
const CLIENT_CERT_PEM: &str = include_str!("fixtures/webdav-client.crt.pem");
const CLIENT_KEY_PEM: &str = include_str!("fixtures/webdav-client.key.pem");
/// A self-signed client certificate the server has never heard of.
const STRANGER_CERT_PEM: &str = include_str!("fixtures/webdav-stranger.crt.pem");
const STRANGER_KEY_PEM: &str = include_str!("fixtures/webdav-stranger.key.pem");
const TOKEN: &str = "eyJ.opaque-access-token.sig";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Auth {
    Anonymous,
    Basic,
    /// `algorithm` as advertised in the challenge.
    Digest(&'static str),
    Bearer,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tls {
    Off,
    /// Server certificate only.
    Server,
    /// Server certificate + a client certificate from `CLIENT_CA_DER` required.
    Mutual,
}

struct ServerState {
    auth: Auth,
    /// Drop `Range` so the server always answers 200 with the whole body,
    /// like servers without partial-content support.
    strip_range: bool,
    /// Nonce counts seen in accepted Digest responses, in order.
    nonce_counts: Mutex<Vec<u32>>,
    /// Requests that arrived without any `Authorization`.
    challenges_sent: AtomicU32,
    dav: DavHandler,
}

struct Server {
    base: String,
    state: Arc<ServerState>,
    _task: tokio::task::JoinHandle<()>,
}

async fn spawn(auth: Auth, tls: bool) -> Server {
    spawn_with(auth, tls, false).await
}

async fn spawn_with(auth: Auth, tls: bool, strip_range: bool) -> Server {
    let tls = if tls { Tls::Server } else { Tls::Off };
    spawn_full(auth, tls, strip_range).await
}

async fn spawn_full(auth: Auth, tls: Tls, strip_range: bool) -> Server {
    let dav = DavHandler::builder()
        .filesystem(MemFs::new())
        .locksystem(FakeLs::new())
        .methods(DavMethodSet::all())
        .strip_prefix("/dav")
        .build_handler();
    let state = Arc::new(ServerState {
        auth,
        strip_range,
        nonce_counts: Mutex::new(Vec::new()),
        challenges_sent: AtomicU32::new(0),
        dav,
    });
    let router = Router::new()
        .route("/dav", any(handle))
        .route("/dav/", any(handle))
        .route("/dav/{*path}", any(handle))
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let task = if tls != Tls::Off {
        let cert = CertificateDer::from(CERT_DER.to_vec());
        let key = PrivateKeyDer::try_from(KEY_DER.to_vec()).unwrap();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let builder = rustls::ServerConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .unwrap();
        let builder = match tls {
            Tls::Mutual => {
                let mut roots = rustls::RootCertStore::empty();
                roots
                    .add(CertificateDer::from(CLIENT_CA_DER.to_vec()))
                    .unwrap();
                let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
                    Arc::new(roots),
                    provider,
                )
                .build()
                .unwrap();
                builder.with_client_cert_verifier(verifier)
            }
            _ => builder.with_no_client_auth(),
        };
        let cfg = builder.with_single_cert(vec![cert], key).unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(cfg));
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let acceptor = acceptor.clone();
                let svc = TowerToHyperService::new(router.clone());
                tokio::spawn(async move {
                    // Handshake failures are the point of the rejection
                    // tests; nothing to report.
                    if let Ok(tls) = acceptor.accept(stream).await {
                        let _ = hyper::server::conn::http1::Builder::new()
                            .serve_connection(TokioIo::new(tls), svc)
                            .await;
                    }
                });
            }
        })
    } else {
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        })
    };
    let scheme = if tls != Tls::Off { "https" } else { "http" };
    Server {
        base: format!("{scheme}://127.0.0.1:{}/dav/", addr.port()),
        state,
        _task: task,
    }
}

async fn handle(State(st): State<Arc<ServerState>>, req: Request) -> impl IntoResponse {
    st.dav.handle(req).await
}

/// Authentication gate in front of the DAV handler.
async fn guard(State(st): State<Arc<ServerState>>, mut req: Request, next: Next) -> Response {
    if st.strip_range {
        req.headers_mut().remove(header::RANGE);
    }
    let authorized = match st.auth {
        Auth::Anonymous => true,
        Auth::Basic => check_basic(req.headers()),
        Auth::Digest(alg) => check_digest(&st, alg, req.method(), req.uri().path(), req.headers()),
        Auth::Bearer => check_bearer(req.headers()),
    };
    if authorized {
        return next.run(req).await;
    }
    if req.headers().get(header::AUTHORIZATION).is_none() {
        st.challenges_sent.fetch_add(1, Ordering::Relaxed);
    }
    let mut resp = Response::new(Body::from("who are you?"));
    *resp.status_mut() = StatusCode::UNAUTHORIZED;
    match st.auth {
        Auth::Anonymous => unreachable!(),
        Auth::Basic => {
            resp.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_str(&format!("Basic realm=\"{REALM}\", charset=\"UTF-8\""))
                    .unwrap(),
            );
        }
        Auth::Digest(alg) => {
            // Digest first, Basic as a decoy the client must not fall back to.
            resp.headers_mut().append(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_str(&format!(
                    "Digest realm=\"{REALM}\", qop=\"auth\", algorithm={alg}, nonce=\"{NONCE}\", opaque=\"5ccc069c403ebaf9f0171e9517f40e41\""
                ))
                .unwrap(),
            );
            resp.headers_mut().append(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_str(&format!("Basic realm=\"{REALM}\"")).unwrap(),
            );
        }
        Auth::Bearer => {
            resp.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_str(&format!(
                    "Bearer realm=\"{REALM}\", error=\"invalid_token\""
                ))
                .unwrap(),
            );
        }
    }
    resp
}

fn check_bearer(headers: &HeaderMap) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        == Some(TOKEN)
}

fn check_basic(headers: &HeaderMap) -> bool {
    let Some(v) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let Some(tok) = v.strip_prefix("Basic ") else {
        return false;
    };
    let expected = base64::engine::general_purpose::STANDARD.encode(format!("{USER}:{PASS}"));
    tok == expected
}

fn digest_params(v: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = v.trim();
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else { break };
        let key = rest[..eq].trim().trim_start_matches(',').trim().to_string();
        rest = &rest[eq + 1..];
        let value;
        if let Some(r) = rest.strip_prefix('"') {
            let end = r.find('"').unwrap();
            value = r[..end].to_string();
            rest = r[end + 1..].trim_start_matches(',').trim();
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            value = rest[..end].trim().to_string();
            rest = rest[end..].trim_start_matches(',').trim();
        }
        out.push((key, value));
    }
    out
}

fn hash(alg: &str, s: &str) -> String {
    match alg {
        "MD5" => format!("{:x}", md5::compute(s.as_bytes())),
        "SHA-256" => hex::encode(sha2::Sha256::digest(s.as_bytes())),
        other => panic!("unexpected algorithm {other}"),
    }
}

fn check_digest(
    st: &ServerState,
    alg: &str,
    method: &Method,
    path: &str,
    headers: &HeaderMap,
) -> bool {
    let Some(v) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let Some(params) = v.strip_prefix("Digest ") else {
        return false;
    };
    let params = digest_params(params);
    let get = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    if get("username") != Some(USER)
        || get("realm") != Some(REALM)
        || get("nonce") != Some(NONCE)
        || get("qop") != Some("auth")
        || get("opaque") != Some("5ccc069c403ebaf9f0171e9517f40e41")
        || get("algorithm") != Some(alg)
    {
        return false;
    }
    let uri = get("uri").unwrap_or_default();
    if uri != path {
        return false;
    }
    let nc = get("nc").unwrap_or_default();
    let cnonce = get("cnonce").unwrap_or_default();
    let ha1 = hash(alg, &format!("{USER}:{REALM}:{PASS}"));
    let ha2 = hash(alg, &format!("{}:{uri}", method.as_str()));
    let expected = hash(alg, &format!("{ha1}:{NONCE}:{nc}:{cnonce}:auth:{ha2}"));
    if get("response") != Some(expected.as_str()) {
        return false;
    }
    st.nonce_counts
        .lock()
        .unwrap()
        .push(u32::from_str_radix(nc, 16).unwrap());
    true
}

fn cfg(server: &Server, creds: Option<(&str, &str)>) -> WebDavConfig {
    WebDavConfig {
        url: server.base.clone(),
        username: creds.map(|(u, _)| u.to_string()),
        password: creds.map(|(_, p)| p.to_string()),
        bearer_token: None,
        tls: TlsPolicy::System,
        client_identity: None,
        connect_timeout: Duration::from_secs(5),
        spool_dir: Some(std::env::temp_dir()),
    }
}

fn err_text(e: CoreError) -> String {
    e.to_string()
}

#[tokio::test]
async fn bearer_token_is_sent_from_the_first_request() {
    let server = spawn(Auth::Bearer, false).await;

    let mut c = cfg(&server, None);
    c.bearer_token = Some(TOKEN.into());
    let dav = WebDav::connect(c).await.unwrap();
    dav.write("/t.txt", b"token").await.unwrap();
    assert_eq!(dav.read("/t.txt").await.unwrap(), b"token");
    // No 401 round trip: the token went out with the very first request.
    assert_eq!(server.state.challenges_sent.load(Ordering::Relaxed), 0);

    // The token wins over a username/password given alongside.
    let mut both = cfg(&server, Some((USER, PASS)));
    both.bearer_token = Some(TOKEN.into());
    WebDav::connect(both).await.unwrap();

    // A wrong token is not "negotiated" into anything else.
    let mut wrong = cfg(&server, None);
    wrong.bearer_token = Some("nope".into());
    match WebDav::connect(wrong).await {
        Err(CoreError::AuthFailed { remaining }) => assert_eq!(remaining, vec!["bearer"]),
        other => panic!("expected auth failure, got {other:?}"),
    }
    // ...and Basic credentials do not satisfy a Bearer-only server.
    match WebDav::connect(cfg(&server, Some((USER, PASS)))).await {
        Err(CoreError::AuthFailed { remaining }) => assert_eq!(remaining, vec!["bearer"]),
        other => panic!("expected auth failure, got {other:?}"),
    }
}

#[test]
fn client_identity_parses_pem_and_rejects_the_rest() {
    let id = ClientIdentity::from_pem(CLIENT_CERT_PEM, CLIENT_KEY_PEM).unwrap();
    assert_eq!(
        id.fingerprint(),
        client_certificate_fingerprint(CLIENT_CERT_PEM).unwrap()
    );
    assert_eq!(id.fingerprint().len(), 32 * 3 - 1);
    // The private key never shows up in Debug output.
    let dbg = format!("{id:?}");
    assert!(dbg.contains(&id.fingerprint()));
    assert!(!dbg.contains("PRIVATE"));

    assert!(matches!(
        ClientIdentity::from_pem("not a cert", CLIENT_KEY_PEM),
        Err(CoreError::Invalid(_))
    ));
    assert!(matches!(
        ClientIdentity::from_pem(CLIENT_CERT_PEM, "garbage"),
        Err(CoreError::Invalid(_))
    ));
    // The key by itself is not a certificate.
    assert!(matches!(
        ClientIdentity::from_pem(CLIENT_KEY_PEM, CLIENT_KEY_PEM),
        Err(CoreError::Invalid(_))
    ));
    let encrypted =
        "-----BEGIN ENCRYPTED PRIVATE KEY-----\nMIIB\n-----END ENCRYPTED PRIVATE KEY-----\n";
    match ClientIdentity::from_pem(CLIENT_CERT_PEM, encrypted) {
        Err(CoreError::Invalid(m)) => assert!(m.contains("encrypted"), "{m}"),
        other => panic!("expected invalid, got {other:?}"),
    }
}

#[test]
fn split_client_pem_separates_bundles() {
    let bundle = format!("{CLIENT_KEY_PEM}\n{CLIENT_CERT_PEM}");
    let parts = split_client_pem(&bundle).unwrap();
    assert!(parts.certificate.starts_with("-----BEGIN CERTIFICATE-----"));
    assert!(parts.private_key.contains("PRIVATE KEY-----"));
    // Same identity as the two originals.
    let id = ClientIdentity::from_pem(&parts.certificate, &parts.private_key).unwrap();
    assert_eq!(
        id.fingerprint(),
        client_certificate_fingerprint(CLIENT_CERT_PEM).unwrap()
    );

    let cert_only = split_client_pem(CLIENT_CERT_PEM).unwrap();
    assert!(cert_only.private_key.is_empty());
    let key_only = split_client_pem(CLIENT_KEY_PEM).unwrap();
    assert!(key_only.certificate.is_empty());
    assert!(matches!(
        split_client_pem("hello"),
        Err(CoreError::Invalid(_))
    ));
    assert!(matches!(
        split_client_pem(
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\nMIIB\n-----END ENCRYPTED PRIVATE KEY-----\n"
        ),
        Err(CoreError::Invalid(_))
    ));
}

#[tokio::test]
async fn mutual_tls_requires_a_certificate_the_server_trusts() {
    let server = spawn_full(Auth::Anonymous, Tls::Mutual, false).await;
    let fp = fingerprint(CERT_DER);
    let identity = ClientIdentity::from_pem(CLIENT_CERT_PEM, CLIENT_KEY_PEM).unwrap();

    // Pinned server certificate + our client certificate: full round trip.
    let mut pinned = cfg(&server, None);
    pinned.tls = TlsPolicy::Pinned(fp.clone());
    pinned.client_identity = Some(identity.clone());
    let dav = WebDav::connect(pinned).await.unwrap();
    dav.write("/mtls.txt", b"both ways").await.unwrap();
    assert_eq!(dav.read("/mtls.txt").await.unwrap(), b"both ways");
    dav.mkdir("/d").await.unwrap();
    assert_eq!(names(&dav.list("/").await.unwrap()), vec!["d", "mtls.txt"]);

    // Without a client certificate the server refuses, and the error says so
    // instead of surfacing a raw transport failure.
    let mut none = cfg(&server, None);
    none.tls = TlsPolicy::Pinned(fp.clone());
    let msg = err_text(WebDav::connect(none).await.unwrap_err());
    assert!(msg.contains("requires a client certificate"), "{msg}");

    // A certificate from an unknown issuer is rejected with a distinct message.
    let mut stranger = cfg(&server, None);
    stranger.tls = TlsPolicy::Pinned(fp.clone());
    stranger.client_identity =
        Some(ClientIdentity::from_pem(STRANGER_CERT_PEM, STRANGER_KEY_PEM).unwrap());
    let msg = err_text(WebDav::connect(stranger).await.unwrap_err());
    assert!(msg.contains("rejected the client certificate"), "{msg}");

    // Server verification is untouched by the client identity: an untrusted
    // server certificate still surfaces as a rejection to pin.
    let mut system = cfg(&server, None);
    system.client_identity = Some(identity.clone());
    match WebDav::connect(system).await {
        Err(CoreError::CertificateRejected {
            fingerprint: got, ..
        }) => assert_eq!(got, fp),
        other => panic!("expected certificate rejection, got {other:?}"),
    }

    // The system-roots path builds a client too (reqwest identity); it just
    // cannot trust this self-signed server.
    WebDav::new(WebDavConfig {
        client_identity: Some(identity),
        ..cfg(&server, None)
    })
    .unwrap();
}

fn names(entries: &[termoso_core::sftp::RemoteEntry]) -> Vec<&str> {
    let mut v: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    v.sort_unstable();
    v
}

#[tokio::test]
async fn crud_over_plain_http() {
    let server = spawn(Auth::Anonymous, false).await;
    let dav = WebDav::connect(cfg(&server, None)).await.unwrap();
    assert_eq!(dav.protocol(), RemoteProtocol::WebDav);
    assert!(dav.capabilities().server_copy);
    assert!(!dav.capabilities().permissions);
    assert_eq!(dav.home(), "/");

    dav.mkdir("/docs").await.unwrap();
    dav.mkdir_all("/docs/a/b/c").await.unwrap();
    dav.write("/docs/hello.txt", b"hello webdav").await.unwrap();
    dav.write("/docs/a/b/c/deep.bin", &[7u8; 3000])
        .await
        .unwrap();

    let root = dav.list("/").await.unwrap();
    assert_eq!(names(&root), vec!["docs"]);
    assert_eq!(root[0].kind, EntryKind::Dir);
    assert_eq!(root[0].path, "/docs");

    let docs = dav.list("/docs").await.unwrap();
    assert_eq!(names(&docs), vec!["a", "hello.txt"]);
    let hello = docs.iter().find(|e| e.name == "hello.txt").unwrap();
    assert_eq!(hello.kind, EntryKind::File);
    assert_eq!(hello.size, Some(12));
    assert_eq!(hello.path, "/docs/hello.txt");
    assert!(hello.mtime.is_some());

    let st = dav.stat("/docs/a/b/c/deep.bin").await.unwrap();
    assert_eq!(st.size, Some(3000));
    assert_eq!(st.kind, EntryKind::File);
    let st = dav.stat("/docs/a").await.unwrap();
    assert_eq!(st.kind, EntryKind::Dir);
    assert_eq!(st.name, "a");

    assert_eq!(dav.read("/docs/hello.txt").await.unwrap(), b"hello webdav");
    assert!(dav.exists("/docs/hello.txt").await.unwrap());
    assert!(!dav.exists("/docs/nope").await.unwrap());
    assert!(matches!(
        dav.stat("/docs/nope").await,
        Err(CoreError::NotFound(p)) if p == "/docs/nope"
    ));
    assert!(matches!(
        dav.read("/docs/nope").await,
        Err(CoreError::NotFound(_))
    ));

    // Rename (MOVE) then server-side COPY.
    dav.rename("/docs/hello.txt", "/docs/a/moved.txt")
        .await
        .unwrap();
    assert!(!dav.exists("/docs/hello.txt").await.unwrap());
    dav.copy("/docs/a/moved.txt", "/docs/copy.txt")
        .await
        .unwrap();
    assert_eq!(
        dav.read("/docs/a/moved.txt").await.unwrap(),
        b"hello webdav"
    );
    assert_eq!(dav.read("/docs/copy.txt").await.unwrap(), b"hello webdav");
    // Overwrite via MOVE is refused for existing targets.
    assert!(matches!(
        dav.rename("/docs/copy.txt", "/docs/a/moved.txt").await,
        Err(CoreError::WebDav {
            status: Some(412),
            ..
        })
    ));

    // Unicode and spaces round-trip through percent-encoding.
    dav.write("/docs/имя файла #1.txt", b"utf8").await.unwrap();
    let docs = dav.list("/docs").await.unwrap();
    assert!(docs.iter().any(|e| e.name == "имя файла #1.txt"));
    assert_eq!(dav.read("/docs/имя файла #1.txt").await.unwrap(), b"utf8");

    // Directory removal: non-empty refused by our guard, recursive works.
    assert!(dav.remove_dir("/docs/a").await.is_err());
    dav.remove_file("/docs/copy.txt").await.unwrap();
    dav.remove_dir_all("/docs/a", &CancellationToken::new())
        .await
        .unwrap();
    assert!(!dav.exists("/docs/a").await.unwrap());
    dav.remove_dir_all("/docs", &CancellationToken::new())
        .await
        .unwrap();
    assert!(dav.list("/").await.unwrap().is_empty());

    // Missing parent maps to a typed error, not a panic or a bare status.
    match dav.mkdir("/missing/child").await {
        Err(CoreError::WebDav {
            status: Some(s), ..
        }) => assert!(s == 409 || s == 404, "{s}"),
        Err(CoreError::NotFound(_)) => {}
        other => panic!("unexpected {other:?}"),
    }
    assert!(matches!(
        dav.chmod("/x", 0o644).await,
        Err(CoreError::WebDav { .. })
    ));
    assert!(matches!(
        dav.mkdir("/../escape").await,
        Err(CoreError::Invalid(_))
    ));
    dav.close().await.unwrap();
}

#[tokio::test]
async fn basic_auth_is_negotiated_after_challenge() {
    let server = spawn(Auth::Basic, false).await;

    let dav = WebDav::connect(cfg(&server, Some((USER, PASS))))
        .await
        .unwrap();
    dav.write("/secret.txt", b"only for alice").await.unwrap();
    assert_eq!(dav.read("/secret.txt").await.unwrap(), b"only for alice");
    // One challenge for the first request; credentials are reused afterwards.
    assert_eq!(server.state.challenges_sent.load(Ordering::Relaxed), 1);

    let wrong = WebDav::connect(cfg(&server, Some((USER, "nope")))).await;
    assert!(
        matches!(wrong, Err(CoreError::AuthFailed { ref remaining }) if remaining == &["basic"]),
        "{wrong:?}"
    );
    let anon = WebDav::connect(cfg(&server, None)).await;
    assert!(
        matches!(anon, Err(CoreError::AuthFailed { .. })),
        "{anon:?}"
    );
}

#[tokio::test]
async fn digest_auth_md5_and_sha256() {
    for alg in ["MD5", "SHA-256"] {
        let server = spawn(Auth::Digest(alg), false).await;
        let dav = WebDav::connect(cfg(&server, Some((USER, PASS))))
            .await
            .unwrap_or_else(|e| panic!("{alg}: {e}"));
        dav.mkdir("/d").await.unwrap();
        dav.write("/d/f.txt", b"digest body").await.unwrap();
        assert_eq!(dav.read("/d/f.txt").await.unwrap(), b"digest body");
        dav.rename("/d/f.txt", "/d/g.txt").await.unwrap();
        assert_eq!(names(&dav.list("/d").await.unwrap()), vec!["g.txt"]);

        // The nonce count grows monotonically across requests.
        let counts = server.state.nonce_counts.lock().unwrap().clone();
        assert!(counts.len() >= 5, "{alg}: {counts:?}");
        assert!(counts.windows(2).all(|w| w[1] > w[0]), "{alg}: {counts:?}");
        // Only the first request was unauthenticated.
        assert_eq!(server.state.challenges_sent.load(Ordering::Relaxed), 1);

        let wrong = WebDav::connect(cfg(&server, Some((USER, "nope")))).await;
        assert!(
            matches!(wrong, Err(CoreError::AuthFailed { ref remaining }) if remaining == &["digest", "basic"]),
            "{alg}: {wrong:?}"
        );
    }
}

#[tokio::test]
async fn range_reads_and_positional_files() {
    let server = spawn(Auth::Anonymous, false).await;
    let dav = WebDav::connect(cfg(&server, None)).await.unwrap();

    let data: Vec<u8> = (0..1_000_000u32).map(|i| (i % 251) as u8).collect();
    dav.write("/big.bin", &data).await.unwrap();

    let f = dav.open_file("/big.bin", OpenMode::Read).await.unwrap();
    assert_eq!(f.size().await.unwrap(), data.len() as u64);
    assert_eq!(f.read_at(0, 10).await.unwrap(), &data[..10]);
    assert_eq!(
        f.read_at(123_456, 70_000).await.unwrap(),
        &data[123_456..193_456]
    );
    assert_eq!(f.read_at(999_990, 100).await.unwrap(), &data[999_990..]);
    assert!(f.read_at(5_000_000, 10).await.unwrap().is_empty());
    assert!(f.read_at(0, 0).await.unwrap().is_empty());
    assert!(matches!(
        f.write_at(0, b"x").await,
        Err(CoreError::Invalid(_)) | Err(CoreError::WebDav { .. })
    ));
    f.close().await.unwrap();

    // Write mode: sparse positional writes are spooled and PUT as a whole.
    let f = dav.open_file("/new.txt", OpenMode::Write).await.unwrap();
    f.write_at(6, b"world").await.unwrap();
    f.write_at(0, b"hello ").await.unwrap();
    f.sync().await.unwrap();
    assert_eq!(dav.read("/new.txt").await.unwrap(), b"hello world");
    f.truncate(5).await.unwrap();
    f.close().await.unwrap();
    assert_eq!(dav.read("/new.txt").await.unwrap(), b"hello");

    // ReadWrite starts from the remote contents.
    let f = dav
        .open_file("/new.txt", OpenMode::ReadWrite)
        .await
        .unwrap();
    assert_eq!(f.read_at(0, 100).await.unwrap(), b"hello");
    f.write_at(1, b"a").await.unwrap();
    f.write_at(5, b"!").await.unwrap();
    assert_eq!(f.read_at(0, 100).await.unwrap(), b"hallo!");
    f.close().await.unwrap();
    assert_eq!(dav.read("/new.txt").await.unwrap(), b"hallo!");

    // Closing without writes does not clobber the file.
    let f = dav
        .open_file("/new.txt", OpenMode::ReadWrite)
        .await
        .unwrap();
    f.close().await.unwrap();
    assert_eq!(dav.read("/new.txt").await.unwrap(), b"hallo!");
}

#[tokio::test]
async fn positional_reads_survive_servers_that_ignore_range() {
    let server = spawn_with(Auth::Anonymous, false, true).await;
    let dav = WebDav::connect(cfg(&server, None)).await.unwrap();
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 253) as u8).collect();
    dav.write("/nr.bin", &data).await.unwrap();

    let f = dav.open_file("/nr.bin", OpenMode::Read).await.unwrap();
    assert_eq!(f.read_at(0, 16).await.unwrap(), &data[..16]);
    assert_eq!(
        f.read_at(150_000, 1000).await.unwrap(),
        &data[150_000..151_000]
    );
    assert_eq!(f.read_at(299_990, 100).await.unwrap(), &data[299_990..]);
    assert!(f.read_at(400_000, 8).await.unwrap().is_empty());
    f.close().await.unwrap();

    // Resume without Range support restarts from zero and still ends correct.
    let tmp = tempfile::tempdir().unwrap();
    let partial = tmp.path().join("p.bin");
    std::fs::write(&partial, &data[..1000]).unwrap();
    let opts = TransferOptions {
        resume: true,
        ..Default::default()
    };
    dav.download("/nr.bin", &partial, &opts).await.unwrap();
    assert_eq!(std::fs::read(&partial).unwrap(), data);
}

#[tokio::test]
async fn transfers_report_progress_and_honour_cancellation() {
    let server = spawn(Auth::Anonymous, false).await;
    let dav = WebDav::connect(cfg(&server, None)).await.unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let local = tmp.path().join("up.bin");
    let payload = vec![0xA5u8; 3 * 1024 * 1024 + 17];
    std::fs::write(&local, &payload).unwrap();

    let seen: Arc<Mutex<Vec<Progress>>> = Arc::default();
    let sink = seen.clone();
    let opts = TransferOptions {
        progress: Some(Arc::new(move |p| sink.lock().unwrap().push(p))),
        ..Default::default()
    };
    let n = dav.upload(&local, "/up.bin", &opts).await.unwrap();
    assert_eq!(n, payload.len() as u64);
    {
        let seen = seen.lock().unwrap();
        let last = seen.last().unwrap();
        assert_eq!(last.done, payload.len() as u64);
        assert_eq!(last.total, Some(payload.len() as u64));
        assert!(seen.iter().all(|p| p.total == Some(payload.len() as u64)));
        assert!(seen.windows(2).all(|w| w[1].done >= w[0].done));
    }
    assert_eq!(
        dav.stat("/up.bin").await.unwrap().size,
        Some(payload.len() as u64)
    );

    let down = tmp.path().join("down.bin");
    let seen: Arc<Mutex<Vec<Progress>>> = Arc::default();
    let sink = seen.clone();
    let opts = TransferOptions {
        progress: Some(Arc::new(move |p| sink.lock().unwrap().push(p))),
        ..Default::default()
    };
    let n = dav.download("/up.bin", &down, &opts).await.unwrap();
    assert_eq!(n, payload.len() as u64);
    assert_eq!(std::fs::read(&down).unwrap(), payload);
    assert_eq!(
        seen.lock().unwrap().last().unwrap().done,
        payload.len() as u64
    );

    // Resume continues from the partial local file using a Range request.
    let partial = tmp.path().join("partial.bin");
    std::fs::write(&partial, &payload[..1_000_000]).unwrap();
    let opts = TransferOptions {
        resume: true,
        ..Default::default()
    };
    dav.download("/up.bin", &partial, &opts).await.unwrap();
    assert_eq!(std::fs::read(&partial).unwrap(), payload);

    let cancel = CancellationToken::new();
    cancel.cancel();
    let opts = TransferOptions {
        cancel,
        ..Default::default()
    };
    assert!(matches!(
        dav.upload(&local, "/never.bin", &opts).await,
        Err(CoreError::Cancelled)
    ));
    assert!(matches!(
        dav.download("/up.bin", &tmp.path().join("never.bin"), &opts)
            .await,
        Err(CoreError::Cancelled)
    ));
    assert!(!dav.exists("/never.bin").await.unwrap());
}

#[tokio::test]
async fn tls_pinning_accepts_only_the_pinned_certificate() {
    let server = spawn(Auth::Anonymous, true).await;
    let fp = fingerprint(CERT_DER);

    // A self-signed certificate is rejected by the system policy and the
    // error carries the fingerprint the user can pin.
    let rejected = WebDav::connect(cfg(&server, None)).await;
    match rejected {
        Err(CoreError::CertificateRejected {
            host,
            fingerprint: got,
        }) => {
            assert!(host.starts_with("127.0.0.1:"), "{host}");
            assert_eq!(got, fp);
        }
        other => panic!("expected certificate rejection, got {other:?}"),
    }

    // Probing on its own returns the same fingerprint.
    let probed = probe_certificate(&server.base, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(probed, fp);

    // Pinned (any spelling) works end to end.
    let mut pinned = cfg(&server, None);
    pinned.tls = TlsPolicy::Pinned(fp.replace(':', "").to_lowercase());
    let dav = WebDav::connect(pinned).await.unwrap();
    dav.write("/tls.txt", b"over tls").await.unwrap();
    assert_eq!(dav.read("/tls.txt").await.unwrap(), b"over tls");

    // A different pin is refused, again with the presented fingerprint.
    let mut wrong = cfg(&server, None);
    wrong.tls = TlsPolicy::Pinned("00".repeat(32));
    match WebDav::connect(wrong).await {
        Err(CoreError::CertificateRejected {
            fingerprint: got, ..
        }) => assert_eq!(got, fp),
        other => panic!("expected certificate rejection, got {other:?}"),
    }
    assert!(matches!(
        WebDav::new(WebDavConfig {
            tls: TlsPolicy::Pinned("zz".into()),
            ..cfg(&server, None)
        }),
        Err(CoreError::Invalid(_))
    ));
}
