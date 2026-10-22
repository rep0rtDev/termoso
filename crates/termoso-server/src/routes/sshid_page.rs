//! Browser view of SSH ID. The same lists `GET /sshid/{handle}[/{type}]`
//! serve as `authorized_keys` text are rendered as a page when the client
//! asks for HTML (`Accept: text/html`): the command that installs the keys,
//! the key types with their OpenSSH requirements, the published keys with
//! fingerprints and a short explanation of what SSH ID is and why to trust
//! it. `curl`/`wget` never send `text/html` first, so scripts keep getting
//! plain text.

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};
use termoso_proto::sshid::SshIdKeyType;

/// `true` when the client prefers HTML over plain text — i.e. a browser.
pub fn wants_html(headers: &HeaderMap) -> bool {
    let Some(accept) = headers.get(header::ACCEPT).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let mut html = None;
    let mut text = None;
    for (i, part) in accept.split(',').enumerate() {
        let mut it = part.trim().split(';');
        let mime = it.next().unwrap_or("").trim();
        let q = it
            .find_map(|p| p.trim().strip_prefix("q="))
            .and_then(|q| q.trim().parse::<f32>().ok())
            .unwrap_or(1.0);
        if q <= 0.0 {
            continue;
        }
        match mime {
            "text/html" | "application/xhtml+xml" => html.get_or_insert((q, i)),
            "text/plain" => text.get_or_insert((q, i)),
            _ => continue,
        };
    }
    match (html, text) {
        (Some((hq, hi)), Some((tq, ti))) => hq > tq || (hq == tq && hi < ti),
        (Some(_), None) => true,
        _ => false,
    }
}

/// One published key: the type and its `<alg> <base64>` line.
pub struct Key {
    pub key_type: SshIdKeyType,
    pub public_key: String,
}

/// Everything the handle page needs.
pub struct HandlePage<'a> {
    pub handle: &'a str,
    /// Canonical base the handle is published under, without the handle.
    pub base: &'a str,
    /// `None` = `all`.
    pub selected: Option<SshIdKeyType>,
    pub keys: &'a [Key],
    pub server_name: &'a str,
    pub home_url: &'a str,
    pub docs_url: Option<&'a str>,
}

fn nonce() -> String {
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// `SHA256:<base64>` of the key blob, as `ssh-keygen -l` prints it.
pub fn fingerprint(public_key: &str) -> Option<String> {
    let blob = public_key.split_whitespace().nth(1)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(blob)
        .ok()?;
    let digest = Sha256::digest(&bytes);
    Some(format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest)
    ))
}

fn label(t: SshIdKeyType) -> &'static str {
    match t {
        SshIdKeyType::Ed25519 => "ED25519",
        SshIdKeyType::Ecdsa => "ECDSA",
        SshIdKeyType::Rsa => "RSA",
        SshIdKeyType::EcdsaSk => "ECDSA-SK",
        SshIdKeyType::Ed25519Sk => "ED25519-SK",
    }
}

/// Minimum OpenSSH on the *server* that accepts the type.
fn requires(t: SshIdKeyType) -> &'static str {
    match t {
        SshIdKeyType::Ed25519 => "OpenSSH 6.5+",
        SshIdKeyType::Ecdsa => "OpenSSH 5.7+",
        SshIdKeyType::Rsa => "any OpenSSH",
        SshIdKeyType::EcdsaSk | SshIdKeyType::Ed25519Sk => "OpenSSH 8.2+",
    }
}

fn describe(t: SshIdKeyType) -> &'static str {
    match t {
        SshIdKeyType::Ed25519 => "Fast, small, the modern default.",
        SshIdKeyType::Ecdsa => "NIST P-256; for hosts that lack ED25519.",
        SshIdKeyType::Rsa => "Widest compatibility, 3072-bit.",
        SshIdKeyType::EcdsaSk => "Private key lives in a FIDO2 security key.",
        SshIdKeyType::Ed25519Sk => "Private key lives in a FIDO2 security key.",
    }
}

const CSS: &str = r#"
:root{--bg:#141826;--base:#1C2032;--high:#262B3F;--highest:#30364C;--strong:#3E4459;--text:#F1F3F8;--muted:#8D91A5;--accent:#2BB884;--accent-dark:#1F8F66;--accent-ink:#08170F;--sky:#5AA9E6;--warn:#F8AA4B;--radius:14px}
@media (prefers-color-scheme:light){:root{--bg:#EEF1F5;--base:#F7F9FB;--high:#FFFFFF;--highest:#E7EBF1;--strong:#C9D0DB;--text:#171B26;--muted:#626D80;--accent-dark:#1F8F66}}
*{box-sizing:border-box}html{-webkit-text-size-adjust:100%}
body{margin:0;background:var(--bg);color:var(--text);font:15px/1.55 "Inter Variable",Inter,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;-webkit-font-smoothing:antialiased}
a{color:var(--sky);text-decoration:none}a:hover{text-decoration:underline}
code,pre,.mono{font-family:"JetBrains Mono Variable","JetBrains Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:13.5px}
.wrap{max-width:760px;margin:0 auto;padding:28px 20px 64px}
header{display:flex;align-items:center;justify-content:space-between;gap:16px;margin-bottom:36px}
.brand{display:flex;align-items:center;gap:10px;color:var(--text);font-weight:600;letter-spacing:.01em}
.brand svg{width:28px;height:28px;border-radius:7px;flex:none}
.brand span{color:var(--muted);font-weight:500}
.btn{display:inline-flex;align-items:center;gap:8px;height:34px;padding:0 14px;border-radius:9px;border:0;font:inherit;font-size:14px;font-weight:600;cursor:pointer;white-space:nowrap}
.btn-primary{background:var(--accent);color:var(--accent-ink)}.btn-primary:hover{background:var(--accent-dark);text-decoration:none}
.btn-ghost{background:var(--high);color:var(--text)}.btn-ghost:hover{background:var(--highest);text-decoration:none}
h1{font-size:30px;line-height:1.2;margin:0 0 8px;letter-spacing:-.02em;font-weight:700;overflow-wrap:anywhere}
h1 .at{color:var(--accent)}
.lead{color:var(--muted);margin:0 0 28px;font-size:16px}
h2{font-size:13px;text-transform:uppercase;letter-spacing:.08em;color:var(--muted);margin:32px 0 12px;font-weight:600}
.card{background:var(--base);border-radius:var(--radius);padding:18px 20px}
.tabs{display:flex;gap:4px;margin-bottom:12px}
.card>input[type=radio]{position:absolute;opacity:0;pointer-events:none;width:0;height:0}
.tabs label{padding:6px 12px;border-radius:8px;color:var(--muted);font-size:13.5px;font-weight:600;cursor:pointer;user-select:none}
.tabs label:hover{color:var(--text)}
.cmd{display:none;position:relative;background:var(--high);border-radius:10px;padding:14px 112px 14px 16px;overflow-x:auto}
.cmd pre{margin:0;white-space:pre-wrap;overflow-wrap:anywhere}
.cmd .prompt{color:var(--muted);user-select:none}
.copy{position:absolute;top:10px;right:10px;height:30px;padding:0 12px;border-radius:8px;border:0;background:var(--highest);color:var(--text);font:inherit;font-size:13px;font-weight:600;cursor:pointer}
.copy:hover{background:var(--strong)}.copy[data-done]{background:var(--accent);color:var(--accent-ink)}
#t-sh:checked~.tabs label[for=t-sh],#t-ps:checked~.tabs label[for=t-ps],#t-raw:checked~.tabs label[for=t-raw]{background:var(--high);color:var(--text)}
#t-sh:checked~.cmd.sh,#t-ps:checked~.cmd.ps,#t-raw:checked~.cmd.raw{display:block}
.hint{color:var(--muted);font-size:13.5px;margin:12px 0 0}
.types{display:grid;grid-template-columns:repeat(auto-fill,minmax(210px,1fr));gap:10px}
.group{grid-column:1/-1;font-size:12px;text-transform:uppercase;letter-spacing:.08em;color:var(--muted);font-weight:600;margin-top:6px}
.group:first-child{margin-top:0}
.type{display:block;background:var(--base);border-radius:12px;padding:14px 16px;color:var(--text);position:relative;outline:2px solid transparent;outline-offset:-2px}
.type:hover{background:var(--high);text-decoration:none}
.type.on{outline-color:var(--accent)}
.type .name{font-weight:700;display:flex;align-items:center;justify-content:space-between;gap:8px}
.type .n{font-size:12px;font-weight:600;color:var(--accent-ink);background:var(--accent);border-radius:999px;padding:1px 8px}
.type.none .n{background:var(--highest);color:var(--muted)}
.type .req{color:var(--muted);font-size:13px;margin-top:4px}
.type .desc{font-size:13px;margin-top:6px;color:var(--text);opacity:.85}
.type.none{color:var(--muted)}.type.none .desc{opacity:.6}
.all{display:inline-block;margin-top:12px;font-size:13.5px}
.keys{display:flex;flex-direction:column;gap:8px}
.key{background:var(--base);border-radius:12px;padding:12px 16px}
.key .row{display:flex;align-items:center;gap:12px;flex-wrap:wrap}
.badge{font-size:12px;font-weight:700;letter-spacing:.04em;padding:2px 8px;border-radius:6px;background:var(--high);color:var(--accent)}
.badge.hw{color:var(--warn)}
.fp{color:var(--muted);overflow-wrap:anywhere}
details.line{margin-top:8px}
details.line summary{cursor:pointer;color:var(--sky);font-size:13px;list-style:none}
details.line summary::-webkit-details-marker{display:none}
details.line pre{margin:8px 0 0;background:var(--high);border-radius:8px;padding:10px 12px;white-space:pre-wrap;overflow-wrap:anywhere}
.empty{background:var(--base);border-radius:12px;padding:18px 20px;color:var(--muted)}
.faq details{background:var(--base);border-radius:12px;margin-bottom:8px}
.faq summary{padding:14px 20px;cursor:pointer;font-weight:600;list-style:none;display:flex;justify-content:space-between;align-items:center;gap:12px}
.faq summary::-webkit-details-marker{display:none}
.faq summary::after{content:"+";color:var(--muted);font-weight:400;font-size:20px;line-height:1}
.faq details[open] summary::after{content:"–"}
.faq .body{padding:0 20px 16px;color:var(--text);opacity:.9}
.faq .body p{margin:0 0 10px}.faq .body p:last-child{margin:0}
.faq pre{background:var(--high);border-radius:8px;padding:10px 12px;white-space:pre-wrap;overflow-wrap:anywhere;margin:8px 0}
footer{margin-top:48px;color:var(--muted);font-size:13px;display:flex;flex-wrap:wrap;gap:6px 18px}
footer a{color:var(--muted)}
@media (max-width:560px){h1{font-size:24px}.cmd{padding-right:16px;padding-bottom:50px}.copy{top:auto;bottom:10px}header{margin-bottom:28px}}
"#;

const JS: &str = r#"
for(const b of document.querySelectorAll('.copy')){b.addEventListener('click',async()=>{const t=document.getElementById(b.dataset.copy);const s=t.dataset.text||t.textContent;try{await navigator.clipboard.writeText(s)}catch(e){const r=document.createRange();r.selectNodeContents(t);const sel=getSelection();sel.removeAllRanges();sel.addRange(r);document.execCommand('copy');sel.removeAllRanges()}const was=b.textContent;b.textContent='Copied';b.dataset.done='1';setTimeout(()=>{b.textContent=was;delete b.dataset.done},1400)})}
"#;

const LOGO: &str = r##"<svg viewBox="0 0 64 64" aria-hidden="true"><rect width="64" height="64" rx="14" fill="#12151F"/><path d="M16 20h32v8H36v20h-8V28H16z" fill="#2BB884"/><rect x="42" y="40" width="8" height="8" rx="2" fill="#5AA9E6"/></svg>"##;

fn document(title: &str, description: &str, body: &str, nonce: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="dark light">
<meta name="robots" content="noindex">
<meta name="description" content="{description}">
<title>{title}</title>
<style nonce="{nonce}">{CSS}</style>
</head>
<body><div class="wrap">
{body}
</div>
<script nonce="{nonce}">{JS}</script>
</body>
</html>
"#,
        title = escape(title),
        description = escape(description),
    )
}

fn respond(status: StatusCode, html: String, nonce: &str, cache: &'static str) -> Response {
    let csp = format!(
        "default-src 'none'; style-src 'nonce-{nonce}'; script-src 'nonce-{nonce}'; \
         img-src data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    );
    let mut res = (status, html).into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    h.insert(header::VARY, HeaderValue::from_static("Accept"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    if let Ok(v) = HeaderValue::from_str(&csp) {
        h.insert(header::CONTENT_SECURITY_POLICY, v);
    }
    res
}

fn header_html(server_name: &str, home_url: &str, cta: &str) -> String {
    format!(
        r#"<header><a class="brand" href="{home}">{LOGO}SSH ID <span>· {name}</span></a><a class="btn btn-primary" href="{home}">{cta}</a></header>"#,
        home = escape(home_url),
        name = escape(server_name),
        cta = escape(cta),
    )
}

fn footer_html(p_home: &str, server_name: &str, docs_url: Option<&str>) -> String {
    let docs = match docs_url {
        Some(d) => format!(
            r#"<a href="{}/keychain/ssh-id">How SSH ID works</a>"#,
            escape(d)
        ),
        None => String::new(),
    };
    format!(
        r#"<footer><span>SSH ID is part of Termoso, running at <a href="{home}">{name}</a>.</span>{docs}<span>Only public keys are published.</span></footer>"#,
        home = escape(p_home),
        name = escape(server_name),
    )
}

/// Install command block for `url`, with Shell / PowerShell / raw tabs.
fn command_html(url: &str, id: &str) -> String {
    let u = escape(url);
    format!(
        r#"<div class="card"><input type="radio" name="tab-{id}" id="t-sh" checked><input type="radio" name="tab-{id}" id="t-ps"><input type="radio" name="tab-{id}" id="t-raw">
<div class="tabs"><label for="t-sh">Shell</label><label for="t-ps">PowerShell</label><label for="t-raw">URL</label></div>
<div class="cmd sh"><pre id="c-sh-{id}" data-text="curl -fsSL {u} &gt;&gt; ~/.ssh/authorized_keys"><span class="prompt">$ </span>curl -fsSL {u} &gt;&gt; ~/.ssh/authorized_keys</pre><button class="copy" type="button" data-copy="c-sh-{id}">Copy</button></div>
<div class="cmd ps"><pre id="c-ps-{id}" data-text="Invoke-RestMethod {u} | Add-Content -Encoding ascii $HOME\.ssh\authorized_keys"><span class="prompt">PS&gt; </span>Invoke-RestMethod {u} | Add-Content -Encoding ascii $HOME\.ssh\authorized_keys</pre><button class="copy" type="button" data-copy="c-ps-{id}">Copy</button></div>
<div class="cmd raw"><pre id="c-raw-{id}">{u}</pre><button class="copy" type="button" data-copy="c-raw-{id}">Copy</button></div>
<p class="hint">Run it on the server you want to sign in to, as the user you will log in as. The list is plain <code>authorized_keys</code> text, so it also works with <code>AuthorizedKeysCommand</code> — see below.</p></div>"#
    )
}

fn faq_html(url: &str, server_name: &str) -> String {
    let u = escape(url);
    let name = escape(server_name);
    format!(
        r#"<section class="faq">
<details><summary>Where do these keys come from?</summary><div class="body"><p>Termoso generates an SSH key pair on each device the account is signed in on, and the owner can add FIDO2 security keys. Only the <em>public</em> halves are uploaded; private keys never leave the device or the security key.</p></div></details>
<details><summary>Why are they published online?</summary><div class="body"><p>A public key is meant to be shared — it can only verify, never sign. Publishing them under a handle lets a server trust a <em>person</em> instead of a copy-pasted file: one command installs every current key, and a new laptop or a lost one is handled by the owner in Termoso rather than by editing <code>authorized_keys</code> on every host.</p></div></details>
<details><summary>Why trust this list?</summary><div class="body"><p>It is served over HTTPS by <strong>{name}</strong>, so it can only be changed by someone signed in to this account — which is protected by the account password (never sent to the server) and, if enabled, MFA. A device's keys are listed only while that device is signed in; signing it out or removing it withdraws its keys at once.</p><p>Compare the fingerprints below with what the owner shows you in Termoso if the access matters. The operator of {name} could in principle serve a different list, as with any key server, so for high-value hosts pin the fingerprints after the first install.</p></div></details>
<details><summary>Which type is the safest?</summary><div class="body"><p><strong>Hardware</strong> keys (ECDSA-SK, ED25519-SK) — the private key is inside a FIDO2 security key and each login needs a touch, so malware on the device cannot use or copy it. Among software keys, <strong>ED25519</strong> is the modern default; RSA only for hosts that cannot do better.</p></div></details>
<details><summary>Keep it current with AuthorizedKeysCommand</summary><div class="body"><p>Instead of appending once, let <code>sshd</code> fetch the list at every login so removed keys stop working immediately:</p><pre>AuthorizedKeysCommand /usr/bin/curl -fsSL --max-time 5 {u}
AuthorizedKeysCommandUser nobody</pre><p>Put it in a <code>Match User</code> block in <code>sshd_config</code>. Keep a local key as a fallback for when the network is down.</p></div></details>
</section>"#
    )
}

/// The handle page: `GET /sshid/{handle}[/{type}]` for a browser.
pub fn handle_page(p: &HandlePage<'_>) -> Response {
    let nonce = nonce();
    let handle_e = escape(p.handle);
    let base_url = format!("{}/{}", p.base, p.handle);
    let (url, selected_name) = match p.selected {
        Some(SshIdKeyType::Ed25519) => (base_url.clone(), "ED25519"),
        Some(t) => (format!("{base_url}/{}", t.url_name()), label(t)),
        None => (format!("{base_url}/all"), "all"),
    };
    let count = |t: SshIdKeyType| p.keys.iter().filter(|k| k.key_type == t).count();

    let mut types = String::from(r#"<div class="types"><div class="group">Hardware</div>"#);
    let push_type = |types: &mut String, t: SshIdKeyType| {
        let n = count(t);
        let href = if t == SshIdKeyType::Ed25519 {
            base_url.clone()
        } else {
            format!("{base_url}/{}", t.url_name())
        };
        let on = p.selected == Some(t);
        types.push_str(&format!(
            r#"<a class="type{on}{none}" href="{href}"{cur}><div class="name">{name}<span class="n">{n}</span></div><div class="req">{req}</div><div class="desc">{desc}</div></a>"#,
            on = if on { " on" } else { "" },
            none = if n == 0 { " none" } else { "" },
            href = escape(&href),
            cur = if on { r#" aria-current="page""# } else { "" },
            name = label(t),
            req = requires(t),
            desc = describe(t),
        ));
    };
    for t in [SshIdKeyType::EcdsaSk, SshIdKeyType::Ed25519Sk] {
        push_type(&mut types, t);
    }
    types.push_str(r#"<div class="group">Software</div>"#);
    for t in [
        SshIdKeyType::Ed25519,
        SshIdKeyType::Ecdsa,
        SshIdKeyType::Rsa,
    ] {
        push_type(&mut types, t);
    }
    types.push_str("</div>");
    types.push_str(&format!(
        r#"<a class="all" href="{}/all">{}</a>"#,
        escape(&base_url),
        if p.selected.is_none() {
            "Showing every type"
        } else {
            "Or list every type at once"
        }
    ));

    let shown: Vec<&Key> = p
        .keys
        .iter()
        .filter(|k| p.selected.is_none_or(|s| s == k.key_type))
        .collect();
    let mut keys = String::new();
    if shown.is_empty() {
        keys.push_str(&format!(
            r#"<div class="empty">No {selected_name} keys are published right now. Device keys appear while the device is signed in to Termoso; hardware keys are added by the owner.</div>"#
        ));
    } else {
        keys.push_str(r#"<div class="keys">"#);
        for (i, k) in shown.iter().enumerate() {
            let fp = fingerprint(&k.public_key).unwrap_or_default();
            let line = format!("{} #SSH ID - @{}", k.public_key, p.handle);
            keys.push_str(&format!(
                r#"<div class="key"><div class="row"><span class="badge{hw}">{name}</span><span class="fp mono">{fp}</span></div><details class="line"><summary>Show key</summary><pre id="k-{i}">{line}</pre><button class="copy" type="button" data-copy="k-{i}">Copy</button></details></div>"#,
                hw = if k.key_type.is_hardware() { " hw" } else { "" },
                name = label(k.key_type),
                fp = escape(&fp),
                line = escape(&line),
            ));
        }
        keys.push_str("</div>");
    }

    let n_all = p.keys.len();
    let lead = match (n_all, p.selected) {
        (0, _) => "This SSH ID has no keys published at the moment.".to_string(),
        (n, None) => format!(
            "{n} public {} published by the owner of this SSH ID. Install them on a server and the owner can sign in with any of their devices.",
            if n == 1 { "key" } else { "keys" }
        ),
        (_, Some(t)) => {
            let n = count(t);
            format!(
                "{n} {} {} published. Install {} on a server and the owner can sign in with their {} key.",
                label(t),
                if n == 1 { "key" } else { "keys" },
                if n == 1 { "it" } else { "them" },
                label(t)
            )
        }
    };

    let body = format!(
        r#"{header}
<h1>SSH keys of <span class="at">@{handle}</span></h1>
<p class="lead">{lead}</p>
<h2>Add to a server</h2>
{command}
<h2>Key type</h2>
{types}
<h2>Published keys</h2>
{keys}
<h2>About SSH ID</h2>
{faq}
{footer}"#,
        header = header_html(p.server_name, p.home_url, "Get Termoso"),
        handle = handle_e,
        lead = escape(&lead),
        command = command_html(&url, "cmd"),
        faq = faq_html(&url, p.server_name),
        footer = footer_html(p.home_url, p.server_name, p.docs_url),
    );
    let html = document(
        &format!("@{} · SSH ID · {}", p.handle, p.server_name),
        &format!(
            "Public SSH keys of @{} published with Termoso SSH ID.",
            p.handle
        ),
        &body,
        &nonce,
    );
    respond(StatusCode::OK, html, &nonce, "public, max-age=60")
}

/// Root of the dedicated SSH ID host for a browser.
pub fn index_page(
    base: &str,
    server_name: &str,
    home_url: &str,
    docs_url: Option<&str>,
) -> Response {
    let nonce = nonce();
    let body = format!(
        r#"{header}
<h1>SSH ID</h1>
<p class="lead">A handle for your SSH public keys. Every key of every signed-in Termoso device, and every security key you add, is published here as plain <code>authorized_keys</code> text — one URL to trust a person.</p>
<h2>Trust a handle on a server</h2>
{command}
<h2>Addresses</h2>
<div class="card"><p style="margin:0 0 8px"><code>{b}/&lt;handle&gt;</code> — ED25519 keys (default)</p><p style="margin:0 0 8px"><code>{b}/&lt;handle&gt;/&lt;type&gt;</code> — <code>ED25519</code>, <code>ECDSA</code>, <code>RSA</code>, <code>ECDSA-SK</code>, <code>ED25519-SK</code></p><p style="margin:0"><code>{b}/&lt;handle&gt;/all</code> — every type</p></div>
<h2>About SSH ID</h2>
{faq}
{footer}"#,
        header = header_html(server_name, home_url, "Get Termoso"),
        command = command_html(&format!("{base}/<handle>"), "cmd"),
        b = escape(base),
        faq = faq_html(&format!("{base}/<handle>"), server_name),
        footer = footer_html(home_url, server_name, docs_url),
    );
    let html = document(
        &format!("SSH ID · {server_name}"),
        "Publish SSH public keys under a handle with Termoso SSH ID.",
        &body,
        &nonce,
    );
    respond(StatusCode::OK, html, &nonce, "public, max-age=300")
}

/// 404 for a browser: unknown handle or key type.
pub fn not_found_page(
    what: &str,
    server_name: &str,
    home_url: &str,
    docs_url: Option<&str>,
) -> Response {
    let nonce = nonce();
    let body = format!(
        r#"{header}
<h1>No such SSH ID</h1>
<p class="lead">{what}. Handles are 3–32 characters: letters, digits, <code>-</code> and <code>_</code>. Key types are <code>ED25519</code>, <code>ECDSA</code>, <code>RSA</code>, <code>ECDSA-SK</code>, <code>ED25519-SK</code> or <code>all</code>.</p>
{footer}"#,
        header = header_html(server_name, home_url, "Get Termoso"),
        what = escape(what),
        footer = footer_html(home_url, server_name, docs_url),
    );
    let html = document(
        &format!("Not found · SSH ID · {server_name}"),
        "No such SSH ID.",
        &body,
        &nonce,
    );
    respond(StatusCode::NOT_FOUND, html, &nonce, "no-store")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accept(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::ACCEPT, HeaderValue::from_str(v).unwrap());
        h
    }

    #[test]
    fn browsers_get_html_tools_get_text() {
        assert!(wants_html(&accept(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,*/*;q=0.8"
        )));
        assert!(wants_html(&accept("text/html")));
        assert!(!wants_html(&accept("*/*"))); // curl
        assert!(!wants_html(&accept("text/plain, text/html;q=0.5")));
        assert!(!wants_html(&accept("text/html;q=0, */*")));
        assert!(!wants_html(&HeaderMap::new()));
    }

    #[test]
    fn fingerprint_matches_ssh_keygen() {
        // ssh-keygen -lf on this key prints SHA256:FqZxWxsvbfbuXGgQdTzGrdE5fDxuEIdcX4nrSb73ozQ
        let key =
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGb2iMX5kz3o2vS3QqL2GcyKTSaqJ3FTj2cXH1gLRXfS";
        let fp = fingerprint(key).unwrap();
        assert!(fp.starts_with("SHA256:"));
        assert_eq!(fp.len(), 7 + 43);
        assert!(fingerprint("garbage").is_none());
    }

    #[test]
    fn page_escapes_and_links() {
        let keys = [Key {
            key_type: SshIdKeyType::Ed25519,
            public_key:
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGb2iMX5kz3o2vS3QqL2GcyKTSaqJ3FTj2cXH1gLRXfS"
                    .into(),
        }];
        let p = HandlePage {
            handle: "alice",
            base: "https://sshid.example.com",
            selected: Some(SshIdKeyType::Ed25519),
            keys: &keys,
            server_name: "Termoso <test>",
            home_url: "https://example.com",
            docs_url: Some("https://docs.example.com"),
        };
        let res = handle_page(&p);
        assert_eq!(res.status(), StatusCode::OK);
        let csp = res.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .to_string();
        assert!(csp.contains("script-src 'nonce-"));
        let body =
            futures::executor::block_on(axum::body::to_bytes(res.into_body(), usize::MAX)).unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains(
            "curl -fsSL https://sshid.example.com/alice &gt;&gt; ~/.ssh/authorized_keys"
        ));
        assert!(html.contains("https://sshid.example.com/alice/RSA"));
        assert!(html.contains("Termoso &lt;test&gt;"));
        assert!(!html.contains("<test>"));
        assert!(html.contains("SHA256:"));
    }
}
