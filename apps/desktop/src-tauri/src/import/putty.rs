//! PuTTY saved sessions, from a `.reg` export (any OS) or straight from the
//! registry on Windows (`reg.exe export`). Only the fields Termoso can
//! represent are mapped: address, protocol, port, user, key file, agent
//! forwarding, environment, proxy and port forwardings.

use std::collections::BTreeMap;

use crate::error::{DesktopError, Result};
use crate::forwarding::PfKind;

use super::{ImportPreview, ImportedHost, ImportedPfRule, ImportedProxy};

pub const REGISTRY_ORIGIN: &str = "HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions";
const SESSIONS_MARKER: &str = "\\simontatham\\putty\\sessions\\";

#[derive(Debug, Clone, PartialEq, Eq)]
enum RegValue {
    Str(String),
    Dword(u32),
    Other,
}

impl RegValue {
    fn as_str(&self) -> Option<&str> {
        match self {
            RegValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn as_u32(&self) -> Option<u32> {
        match self {
            RegValue::Dword(d) => Some(*d),
            RegValue::Str(s) => s.trim().parse().ok(),
            RegValue::Other => None,
        }
    }
}

type Session = BTreeMap<String, RegValue>;

/// Registry-export text → sessions keyed by decoded name.
fn parse_reg(text: &str) -> Vec<(String, Session)> {
    let mut out: Vec<(String, Session)> = Vec::new();
    let mut current: Option<usize> = None;
    let mut pending = String::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        // Hex values continue on following lines ending with `\`.
        if !pending.is_empty() {
            pending.push_str(line.trim());
            if line.trim_end().ends_with('\\') {
                pending.pop();
                continue;
            }
            let full = std::mem::take(&mut pending);
            if let Some(i) = current
                && let Some((k, v)) = parse_value(&full)
            {
                out[i].1.insert(k, v);
            }
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            let lower = inner.to_ascii_lowercase();
            current = lower.find(SESSIONS_MARKER).and_then(|pos| {
                let name = &inner[pos + SESSIONS_MARKER.len()..];
                if name.is_empty() || name.contains('\\') || inner.starts_with('-') {
                    return None;
                }
                let name = percent_decode(name);
                out.push((name, Session::new()));
                Some(out.len() - 1)
            });
            continue;
        }
        if let Some(head) = trimmed.strip_suffix('\\') {
            pending = head.to_string();
            continue;
        }
        if let Some(i) = current
            && let Some((k, v)) = parse_value(trimmed)
        {
            out[i].1.insert(k, v);
        }
    }
    out
}

/// `"Name"="value"`, `"Name"=dword:0000001a`, `"Name"=hex:..`.
fn parse_value(line: &str) -> Option<(String, RegValue)> {
    let rest = line.strip_prefix('"')?;
    let mut name = String::new();
    let mut chars = rest.chars();
    loop {
        match chars.next()? {
            '\\' => name.push(chars.next()?),
            '"' => break,
            c => name.push(c),
        }
    }
    let rest: String = chars.collect();
    let value = rest.trim_start().strip_prefix('=')?.trim();
    let v = if let Some(s) = value.strip_prefix('"') {
        RegValue::Str(unescape(s.strip_suffix('"').unwrap_or(s)))
    } else if let Some(d) = value
        .strip_prefix("dword:")
        .or_else(|| value.strip_prefix("DWORD:"))
    {
        RegValue::Dword(u32::from_str_radix(d.trim(), 16).ok()?)
    } else {
        RegValue::Other
    };
    Some((name, v))
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// PuTTY stores session names URL-encoded (`My%20Server`).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(hex, 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn parse_reg_into(text: &str, preview: &mut ImportPreview) -> Result<()> {
    let sessions = parse_reg(text);
    if sessions.is_empty() {
        let looks_reg = text
            .lines()
            .take(5)
            .any(|l| l.contains("Windows Registry Editor") || l.starts_with("REGEDIT"));
        if !looks_reg {
            return Err(DesktopError::invalid(
                "Not a registry export (.reg) — export PuTTY sessions with regedit or `reg export`",
            ));
        }
    }
    for (name, s) in sessions {
        if name.eq_ignore_ascii_case("Default Settings")
            || name.eq_ignore_ascii_case("Default%20Settings")
        {
            continue;
        }
        match session_host(&name, &s, preview) {
            Ok(h) => preview.hosts.push(h),
            Err(reason) => preview
                .warnings
                .push(format!("Session {name} skipped: {reason}")),
        }
    }
    Ok(())
}

fn session_host(
    name: &str,
    s: &Session,
    preview: &mut ImportPreview,
) -> std::result::Result<ImportedHost, String> {
    let raw_host = s
        .get("HostName")
        .and_then(RegValue::as_str)
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .ok_or_else(|| "no host name".to_string())?;
    let protocol = s
        .get("Protocol")
        .and_then(RegValue::as_str)
        .unwrap_or("ssh")
        .to_ascii_lowercase();
    let protocol = match protocol.as_str() {
        "ssh" => "ssh",
        "telnet" => "telnet",
        other => return Err(format!("protocol {other} is not supported")),
    };
    // PuTTY accepts `user@host` in the host field.
    let (user_in_host, address) = match raw_host.rsplit_once('@') {
        Some((u, h)) => (Some(u.to_string()), h.to_string()),
        None => (None, raw_host.to_string()),
    };
    let username = s
        .get("UserName")
        .and_then(RegValue::as_str)
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string)
        .or(user_in_host)
        .unwrap_or_default();
    let port = s
        .get("PortNumber")
        .and_then(RegValue::as_u32)
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p > 0);

    let mut host = ImportedHost {
        label: name.to_string(),
        address,
        protocol: protocol.into(),
        port,
        username,
        agent_forwarding: s.get("AgentFwd").and_then(RegValue::as_u32) == Some(1),
        forward_x11: s.get("X11Forward").and_then(RegValue::as_u32) == Some(1),
        ..ImportedHost::default()
    };
    if let Some(k) = s
        .get("PublicKeyFile")
        .and_then(RegValue::as_str)
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        host.key_path = Some(k.to_string());
        if k.to_ascii_lowercase().ends_with(".ppk") {
            host.warnings.push(
                "PuTTY .ppk key must be exported as OpenSSH in PuTTYgen before it can be imported"
                    .into(),
            );
        }
    }
    if let Some(env) = s.get("Environment").and_then(RegValue::as_str) {
        for pair in split_escaped_commas(env) {
            if let Some((k, v)) = pair.split_once('=') {
                host.env_variables
                    .push((k.trim().to_string(), v.to_string()));
            }
        }
    }
    if let Some(kind) = match s.get("ProxyMethod").and_then(RegValue::as_u32) {
        Some(1) => Some("socks4"),
        Some(2) => Some("socks5"),
        Some(3) => Some("http"),
        Some(0) | None => None,
        Some(_) => {
            host.warnings
                .push("Proxy type (telnet/local command) is not supported, ignored".into());
            None
        }
    } {
        let phost = s
            .get("ProxyHost")
            .and_then(RegValue::as_str)
            .unwrap_or("")
            .trim();
        let pport = s
            .get("ProxyPort")
            .and_then(RegValue::as_u32)
            .and_then(|p| u16::try_from(p).ok())
            .unwrap_or(match kind {
                "http" => 80,
                _ => 1080,
            });
        if !phost.is_empty() {
            host.proxy = Some(ImportedProxy {
                kind: kind.into(),
                host: phost.to_string(),
                port: pport,
                username: s
                    .get("ProxyUsername")
                    .and_then(RegValue::as_str)
                    .unwrap_or("")
                    .to_string(),
                password: s
                    .get("ProxyPassword")
                    .and_then(RegValue::as_str)
                    .filter(|p| !p.is_empty())
                    .map(str::to_string),
                has_password: false,
            });
        }
    }
    if protocol == "ssh"
        && let Some(pf) = s.get("PortForwardings").and_then(RegValue::as_str)
    {
        for spec in pf.split(',') {
            match forwarding(name, spec.trim()) {
                Some(r) => preview.pf_rules.push(r),
                None if spec.trim().is_empty() => {}
                None => host
                    .warnings
                    .push(format!("Port forwarding {spec:?} not understood, skipped")),
            }
        }
    }
    Ok(host)
}

/// `NAME=value,NAME2=value` with `\,` escaping commas.
fn split_escaped_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&',') => {
                cur.push(',');
                chars.next();
            }
            ',' => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `[4|6][L|R|D][bind:]port[=host:port]`.
fn forwarding(host_label: &str, spec: &str) -> Option<ImportedPfRule> {
    let spec = spec.strip_prefix(['4', '6']).unwrap_or(spec);
    let (kind, rest) = match spec.chars().next()? {
        'L' => (PfKind::Local, &spec[1..]),
        'R' => (PfKind::Remote, &spec[1..]),
        'D' => (PfKind::Dynamic, &spec[1..]),
        _ => return None,
    };
    let (src, dst) = match rest.split_once('=') {
        Some((s, d)) => (s, Some(d)),
        None => (rest, None),
    };
    let (bound_address, local_port) = match src.rsplit_once(':') {
        Some((b, p)) if !b.contains(':') || b.starts_with('[') => {
            (b.trim_matches(['[', ']']).to_string(), p.parse().ok()?)
        }
        _ => (String::new(), src.parse().ok()?),
    };
    let (remote_host, remote_port) = match kind {
        PfKind::Dynamic => (String::new(), 0),
        _ => {
            let d = dst?;
            let (h, p) = d.rsplit_once(':')?;
            (h.trim_matches(['[', ']']).to_string(), p.parse().ok()?)
        }
    };
    Some(ImportedPfRule {
        host_label: host_label.to_string(),
        kind,
        bound_address,
        local_port,
        remote_host,
        remote_port,
    })
}

/// Export the live PuTTY sessions key via `reg.exe` (Windows only).
#[cfg(windows)]
pub fn export_registry() -> Result<String> {
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("termoso-putty-{}.reg", uuid::Uuid::new_v4()));
    let status = Command::new("reg")
        .args(["export", REGISTRY_ORIGIN, &dir.to_string_lossy(), "/y"])
        .output()?;
    if !status.status.success() {
        let _ = std::fs::remove_file(&dir);
        return Err(DesktopError::not_found(
            "PuTTY has no saved sessions in the registry (or PuTTY is not installed)",
        ));
    }
    let bytes = std::fs::read(&dir)?;
    let _ = std::fs::remove_file(&dir);
    Ok(super::decode_text(&bytes))
}

#[cfg(not(windows))]
pub fn export_registry() -> Result<String> {
    Err(DesktopError::invalid(
        "Reading the PuTTY registry works on Windows only — import a .reg export instead",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::ImportSource;

    const REG: &str = r#"Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions]

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\Default%20Settings]
"HostName"=""

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\Prod%20Web]
"Present"=dword:00000001
"HostName"="web.example.com"
"Protocol"="ssh"
"PortNumber"=dword:0000082a
"UserName"="deploy"
"PublicKeyFile"="C:\\Users\\me\\keys\\prod.ppk"
"AgentFwd"=dword:00000001
"X11Forward"=dword:00000001
"Environment"="LC_ALL=C.UTF-8,TERM_PROGRAM=putty"
"ProxyMethod"=dword:00000002
"ProxyHost"="proxy.local"
"ProxyPort"=dword:00000438
"ProxyUsername"="pu"
"ProxyPassword"="secret"
"PortForwardings"="L8080=localhost:80,D1080,R2222=127.0.0.1:22,4L127.0.0.1:9000=db:5432"
"LineCodePage"="UTF-8"

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\router]
"HostName"="admin@10.0.0.1"
"Protocol"="telnet"
"PortNumber"=dword:00000017

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\serial]
"HostName"="COM3"
"Protocol"="serial"
"#;

    #[test]
    fn parses_sessions() {
        let mut p = ImportPreview::new(ImportSource::Putty, "test");
        parse_reg_into(REG, &mut p).unwrap();
        assert_eq!(p.hosts.len(), 2);
        let web = &p.hosts[0];
        assert_eq!(web.label, "Prod Web");
        assert_eq!(web.address, "web.example.com");
        assert_eq!(web.port, Some(2090));
        assert_eq!(web.username, "deploy");
        assert!(web.agent_forwarding);
        assert!(web.forward_x11);
        assert_eq!(
            web.key_path.as_deref(),
            Some("C:\\Users\\me\\keys\\prod.ppk")
        );
        assert_eq!(web.env_variables.len(), 2);
        let proxy = web.proxy.as_ref().unwrap();
        assert_eq!(proxy.kind, "socks5");
        assert_eq!(proxy.host, "proxy.local");
        assert_eq!(proxy.port, 1080);
        assert_eq!(proxy.username, "pu");
        assert_eq!(proxy.password.as_deref(), Some("secret"));
        assert!(web.warnings.iter().any(|w| w.contains(".ppk")));

        let router = &p.hosts[1];
        assert_eq!(router.protocol, "telnet");
        assert_eq!(router.address, "10.0.0.1");
        assert_eq!(router.username, "admin");
        assert_eq!(router.port, Some(23));

        assert_eq!(p.pf_rules.len(), 4);
        assert_eq!(p.pf_rules[0].kind, PfKind::Local);
        assert_eq!(p.pf_rules[0].local_port, 8080);
        assert_eq!(p.pf_rules[0].remote_host, "localhost");
        assert_eq!(p.pf_rules[1].kind, PfKind::Dynamic);
        assert_eq!(p.pf_rules[1].local_port, 1080);
        assert_eq!(p.pf_rules[2].kind, PfKind::Remote);
        assert_eq!(p.pf_rules[3].bound_address, "127.0.0.1");
        assert_eq!(p.pf_rules[3].remote_host, "db");
        assert_eq!(p.pf_rules[3].remote_port, 5432);
        assert!(p.warnings.iter().any(|w| w.contains("serial")));
    }

    #[test]
    fn rejects_non_reg() {
        let mut p = ImportPreview::new(ImportSource::Putty, "test");
        assert!(parse_reg_into("Host foo\n  HostName bar\n", &mut p).is_err());
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("My%20Server%21"), "My Server!");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("bad%"), "bad%");
    }
}
