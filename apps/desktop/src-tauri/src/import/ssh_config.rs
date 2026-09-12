//! OpenSSH `ssh_config` reader. Concrete `Host` aliases become hosts; the
//! directives that apply to each come from every matching block (globals,
//! wildcard patterns, the alias itself) with OpenSSH's first-wins rule.
//! `Include` is expanded (relative to `~/.ssh`), `Match` blocks and
//! `ProxyCommand` are reported as ignored.

use std::path::{Path, PathBuf};

use crate::forwarding::PfKind;

use super::{ImportPreview, ImportedHost, ImportedPfRule, expand_tilde};

const MAX_INCLUDE_DEPTH: usize = 8;
const MULTI_VALUED: &[&str] = &[
    "identityfile",
    "localforward",
    "remoteforward",
    "dynamicforward",
    "sendenv",
    "setenv",
];

#[derive(Debug, Clone)]
struct Directive {
    key: String,
    args: Vec<String>,
}

#[derive(Debug)]
struct Block {
    /// `Host` patterns; empty for the global block. `None` for `Match`.
    patterns: Option<Vec<String>>,
    directives: Vec<Directive>,
}

#[derive(Debug, Default)]
struct Parsed {
    blocks: Vec<Block>,
    matches: usize,
    includes: usize,
    missing_includes: Vec<String>,
}

pub fn parse_into(text: &str, path: &Path, preview: &mut ImportPreview) {
    let base = path
        .parent()
        .map(Path::to_path_buf)
        .or_else(super::default_ssh_dir)
        .unwrap_or_default();
    let mut parsed = Parsed::default();
    parsed.blocks.push(Block {
        patterns: Some(Vec::new()),
        directives: Vec::new(),
    });
    read(text, &base, 0, &mut parsed);
    build(&parsed, preview);
    if parsed.matches > 0 {
        preview.warnings.push(format!(
            "{} Match block(s) ignored — Termoso matches hosts by alias only",
            parsed.matches
        ));
    }
    for inc in &parsed.missing_includes {
        preview
            .warnings
            .push(format!("Include {inc} not found, skipped"));
    }
}

fn read(text: &str, base: &Path, depth: usize, out: &mut Parsed) {
    for raw in text.lines() {
        let Some(d) = tokenize(raw) else { continue };
        match d.key.as_str() {
            "host" => out.blocks.push(Block {
                patterns: Some(d.args.clone()),
                directives: Vec::new(),
            }),
            "match" => {
                out.matches += 1;
                out.blocks.push(Block {
                    patterns: None,
                    directives: Vec::new(),
                });
            }
            "include" => {
                out.includes += 1;
                if depth >= MAX_INCLUDE_DEPTH {
                    continue;
                }
                for pat in &d.args {
                    let files = include_files(pat, base);
                    if files.is_empty() {
                        out.missing_includes.push(pat.clone());
                    }
                    for f in files {
                        if let Ok(t) = std::fs::read_to_string(&f) {
                            read(&t, base, depth + 1, out);
                        }
                    }
                }
            }
            _ => {
                if let Some(b) = out.blocks.last_mut() {
                    b.directives.push(d);
                }
            }
        }
    }
}

/// `Key value`, `Key=value`, `Key "quoted value"`; comments stripped.
fn tokenize(line: &str) -> Option<Directive> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut chars = line.chars().peekable();
    let mut key = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == '=' {
            break;
        }
        key.push(c);
        chars.next();
    }
    if key.is_empty() {
        return None;
    }
    let rest: String = chars.collect();
    let rest = rest
        .trim_start()
        .strip_prefix('=')
        .unwrap_or(rest.trim_start());
    let args = split_args(rest.trim());
    Some(Directive {
        key: key.to_ascii_lowercase(),
        args,
    })
}

fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut had_quote = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                had_quote = true;
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() || had_quote {
                    out.push(std::mem::take(&mut cur));
                    had_quote = false;
                }
            }
            '#' if !quoted && cur.is_empty() => break,
            c => cur.push(c),
        }
    }
    if !cur.is_empty() || had_quote {
        out.push(cur);
    }
    out
}

/// Files an `Include` argument expands to. Relative paths are under `base`
/// (`~/.ssh`); a glob in the last component matches directory entries.
fn include_files(pattern: &str, base: &Path) -> Vec<PathBuf> {
    let p = if pattern.starts_with('~') {
        expand_tilde(pattern)
    } else if Path::new(pattern).is_absolute() {
        PathBuf::from(pattern)
    } else {
        base.join(pattern)
    };
    let Some(name) = p.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    if !name.contains(['*', '?']) {
        return if p.is_file() { vec![p] } else { Vec::new() };
    }
    let Some(dir) = p.parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|f| {
            f.is_file()
                && f.file_name()
                    .is_some_and(|n| glob_match(&name, &n.to_string_lossy()))
        })
        .collect();
    files.sort();
    files
}

/// OpenSSH-style pattern: `*`, `?`, case-insensitive.
fn glob_match(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], t) || (!t.is_empty() && go(p, &t[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b) && go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    go(&p, &t)
}

/// Whether a `Host` pattern list applies to `name`: any positive pattern
/// matches and no negated one does.
fn patterns_match(patterns: &[String], name: &str) -> bool {
    let mut positive = false;
    for p in patterns {
        if let Some(neg) = p.strip_prefix('!') {
            if glob_match(neg, name) {
                return false;
            }
        } else if glob_match(p, name) {
            positive = true;
        }
    }
    positive
}

fn is_pattern(alias: &str) -> bool {
    alias.contains(['*', '?', '!'])
}

/// Effective directives for one alias: first value wins, except the
/// multi-valued ones which accumulate.
struct Effective {
    single: Vec<(String, Vec<String>)>,
    multi: Vec<(String, Vec<String>)>,
}

impl Effective {
    fn compute(parsed: &Parsed, alias: &str) -> Self {
        let mut eff = Self {
            single: Vec::new(),
            multi: Vec::new(),
        };
        for block in &parsed.blocks {
            let applies = match &block.patterns {
                None => false,
                Some(p) if p.is_empty() => true,
                Some(p) => patterns_match(p, alias),
            };
            if !applies {
                continue;
            }
            for d in &block.directives {
                if MULTI_VALUED.contains(&d.key.as_str()) {
                    eff.multi.push((d.key.clone(), d.args.clone()));
                } else if !eff.single.iter().any(|(k, _)| *k == d.key) {
                    eff.single.push((d.key.clone(), d.args.clone()));
                }
            }
        }
        eff
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.single
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, a)| a.first())
            .map(String::as_str)
    }

    fn all(&self, key: &str) -> impl Iterator<Item = &Vec<String>> {
        self.multi
            .iter()
            .filter(move |(k, _)| k == key)
            .map(|(_, a)| a)
    }
}

fn yes(v: Option<&str>) -> bool {
    v.is_some_and(|s| s.eq_ignore_ascii_case("yes") || s.eq_ignore_ascii_case("true"))
}

fn build(parsed: &Parsed, preview: &mut ImportPreview) {
    let mut seen = std::collections::HashSet::new();
    let mut proxy_command = 0usize;
    for block in &parsed.blocks {
        let Some(patterns) = &block.patterns else {
            continue;
        };
        for alias in patterns {
            if is_pattern(alias) || !seen.insert(alias.to_lowercase()) {
                continue;
            }
            let eff = Effective::compute(parsed, alias);
            let address = eff
                .get("hostname")
                .map(|h| h.replace("%h", alias))
                .unwrap_or_else(|| alias.clone());
            let mut host = ImportedHost {
                label: alias.clone(),
                address,
                protocol: "ssh".into(),
                port: eff.get("port").and_then(|p| p.parse().ok()),
                username: eff.get("user").unwrap_or_default().to_string(),
                agent_forwarding: yes(eff.get("forwardagent")),
                keep_alive_interval: eff
                    .get("serveraliveinterval")
                    .and_then(|v| v.parse().ok())
                    .filter(|v: &u32| *v > 0),
                timeout: eff
                    .get("connecttimeout")
                    .and_then(|v| v.parse().ok())
                    .filter(|v: &u32| *v > 0),
                ..ImportedHost::default()
            };
            host.key_path = eff
                .all("identityfile")
                .filter_map(|a| a.first())
                .find(|p| !p.eq_ignore_ascii_case("none"))
                .map(|p| p.replace("%d", "~").replace("%h", alias));
            if let Some(jump) = eff.get("proxyjump")
                && !jump.eq_ignore_ascii_case("none")
            {
                host.jump_hosts = jump
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            if eff.get("proxycommand").is_some() {
                proxy_command += 1;
                host.warnings.push("ProxyCommand ignored".into());
            }
            for args in eff.all("setenv") {
                for kv in args {
                    if let Some((k, v)) = kv.split_once('=') {
                        host.env_variables.push((k.to_string(), v.to_string()));
                    }
                }
            }
            for args in eff.all("sendenv") {
                for name in args {
                    if name.starts_with('-') || name.contains(['*', '?']) {
                        continue;
                    }
                    if let Ok(v) = std::env::var(name)
                        && !host.env_variables.iter().any(|(k, _)| k == name)
                    {
                        host.env_variables.push((name.clone(), v));
                    }
                }
            }
            for args in eff.all("localforward") {
                if let Some(r) = forward(PfKind::Local, alias, args) {
                    preview.pf_rules.push(r);
                }
            }
            for args in eff.all("remoteforward") {
                if let Some(r) = forward(PfKind::Remote, alias, args) {
                    preview.pf_rules.push(r);
                }
            }
            for args in eff.all("dynamicforward") {
                if let Some(r) = forward(PfKind::Dynamic, alias, args) {
                    preview.pf_rules.push(r);
                }
            }
            preview.hosts.push(host);
        }
    }
    if proxy_command > 0 {
        preview.warnings.push(format!(
            "ProxyCommand is not supported and was ignored for {proxy_command} host(s)"
        ));
    }
    if parsed.includes > 0 {
        preview
            .warnings
            .push(format!("{} Include directive(s) expanded", parsed.includes));
    }
}

/// `[bind:]port [host:hostport]`; unix-socket forwards are skipped.
fn forward(kind: PfKind, alias: &str, args: &[String]) -> Option<ImportedPfRule> {
    let (bind, local_port) = split_bind_port(args.first()?)?;
    let (remote_host, remote_port) = match kind {
        PfKind::Dynamic => (String::new(), 0),
        _ => {
            let (h, p) = split_host_port(args.get(1)?)?;
            (h, p)
        }
    };
    Some(ImportedPfRule {
        host_label: alias.to_string(),
        kind,
        bound_address: bind,
        local_port,
        remote_host,
        remote_port,
    })
}

fn split_bind_port(s: &str) -> Option<(String, u16)> {
    if let Some(inner) = s.strip_prefix('[')
        && let Some((host, port)) = inner.split_once("]:")
    {
        return Some((host.to_string(), port.parse().ok()?));
    }
    match s.rsplit_once(':') {
        Some((bind, port)) => Some((bind.to_string(), port.parse().ok()?)),
        None => match s.rsplit_once('/') {
            Some((bind, port)) => Some((bind.to_string(), port.parse().ok()?)),
            None => Some((String::new(), s.parse().ok()?)),
        },
    }
}

fn split_host_port(s: &str) -> Option<(String, u16)> {
    if let Some(inner) = s.strip_prefix('[')
        && let Some((host, port)) = inner.split_once("]:")
    {
        return Some((host.to_string(), port.parse().ok()?));
    }
    let (host, port) = s.rsplit_once([':', '/'])?;
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::ImportSource;

    fn parse(text: &str) -> ImportPreview {
        let mut p = ImportPreview::new(ImportSource::SshConfig, "test");
        parse_into(text, Path::new("/nonexistent/.ssh/config"), &mut p);
        p
    }

    #[test]
    fn globals_wildcards_and_aliases() {
        let p = parse(
            r#"
# global
ServerAliveInterval 30

Host *.prod
  User deploy
  IdentityFile ~/.ssh/prod_ed25519
  ForwardAgent yes

Host web1.prod db.prod
  HostName 10.0.0.%h

Host bastion
  HostName bastion.example.com
  Port 2222
  User ops
  ConnectTimeout 5

Host app
  HostName=app.internal
  ProxyJump ops@bastion:2222,relay
  LocalForward 8080 localhost:80
  DynamicForward 127.0.0.1:1080
  RemoteForward 9000 127.0.0.1:9000
  SetEnv "LC_ALL=C.UTF-8"
  ProxyCommand ssh -W %h:%p bastion

Host *
  User root
  IdentityFile ~/.ssh/id_rsa
"#,
        );
        assert_eq!(p.hosts.len(), 4);
        let web = &p.hosts[0];
        assert_eq!(web.label, "web1.prod");
        assert_eq!(web.address, "10.0.0.web1.prod");
        assert_eq!(web.username, "deploy");
        assert_eq!(web.key_path.as_deref(), Some("~/.ssh/prod_ed25519"));
        assert!(web.agent_forwarding);
        assert_eq!(web.keep_alive_interval, Some(30));

        let bastion = &p.hosts[2];
        assert_eq!(bastion.address, "bastion.example.com");
        assert_eq!(bastion.port, Some(2222));
        assert_eq!(bastion.username, "ops");
        assert_eq!(bastion.timeout, Some(5));
        assert_eq!(bastion.key_path.as_deref(), Some("~/.ssh/id_rsa"));

        let app = &p.hosts[3];
        assert_eq!(app.address, "app.internal");
        assert_eq!(app.username, "root");
        assert_eq!(app.jump_hosts, vec!["ops@bastion:2222", "relay"]);
        assert_eq!(app.env_variables, vec![("LC_ALL".into(), "C.UTF-8".into())]);
        assert_eq!(app.warnings, vec!["ProxyCommand ignored"]);

        assert_eq!(p.pf_rules.len(), 3);
        assert_eq!(p.pf_rules[0].kind, PfKind::Local);
        assert_eq!(p.pf_rules[0].local_port, 8080);
        assert_eq!(p.pf_rules[0].remote_host, "localhost");
        assert_eq!(p.pf_rules[0].remote_port, 80);
        assert_eq!(p.pf_rules[1].kind, PfKind::Remote);
        assert_eq!(p.pf_rules[2].kind, PfKind::Dynamic);
        assert_eq!(p.pf_rules[2].bound_address, "127.0.0.1");
        assert_eq!(p.pf_rules[2].local_port, 1080);
        assert!(p.warnings.iter().any(|w| w.contains("ProxyCommand")));
    }

    #[test]
    fn match_blocks_and_negation() {
        let p =
            parse("Match host x\n  User nope\nHost a !a\n  User no\nHost a\n  User yes\nHost b\n");
        assert_eq!(p.hosts.len(), 2);
        assert_eq!(p.hosts[0].username, "yes");
        assert_eq!(p.hosts[1].username, "");
        assert!(p.warnings.iter().any(|w| w.contains("Match")));
    }

    #[test]
    fn tokenizer() {
        let d = tokenize("  IdentityFile \"~/my keys/id\" # comment").unwrap();
        assert_eq!(d.key, "identityfile");
        assert_eq!(d.args, vec!["~/my keys/id"]);
        let d = tokenize("Port=22").unwrap();
        assert_eq!(d.args, vec!["22"]);
        assert!(tokenize("# only comment").is_none());
    }

    #[test]
    fn globs() {
        assert!(glob_match("*.prod", "web.PROD"));
        assert!(glob_match("web?", "web1"));
        assert!(!glob_match("web?", "web12"));
        assert!(glob_match("*", "anything"));
    }
}
