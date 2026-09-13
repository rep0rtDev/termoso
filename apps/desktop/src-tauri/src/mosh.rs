//! Mosh the way the `mosh` wrapper script does it: our own SSH connection
//! (so keys, FIDO2, SSH ID and everything else keep working) runs
//! `mosh-server new …` on the remote, we read back `MOSH CONNECT <port> <key>`
//! and hand the UDP side over to a local `mosh-client` in a PTY. SSH is
//! closed once the server is up; the roaming session lives on UDP only.
//!
//! The key never reaches the webview: it is passed to `mosh-client` through
//! `MOSH_KEY` in its environment, like the reference client does.

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use termoso_core::pty::{LocalShellOptions, LocalTerminal};
use termoso_core::ssh::{IpVersion, SshClient};
use termoso_core::terminal::{TermEvents, TermSize};
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};

/// What Termius starts by default; the user can replace it per host.
pub const DEFAULT_SERVER_COMMAND: &str = "mosh-server new -s -c 256 -l LANG=en_US.UTF-8";

/// How long the remote gets to print its `MOSH CONNECT` line.
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(30);

/// Where `mosh-client` lives on this machine, if anywhere.
pub fn client_path() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "mosh-client.exe"
    } else {
        "mosh-client"
    };
    let path = std::env::var_os("PATH")?;
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    // Homebrew / MacPorts / Nix profiles are often missing from the PATH a
    // desktop app inherits.
    for extra in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/opt/local/bin",
        "/run/current-system/sw/bin",
    ] {
        dirs.push(PathBuf::from(extra));
    }
    if let Some(home) = crate::sessions::dirs_home() {
        dirs.push(home.join(".nix-profile/bin"));
        dirs.push(home.join(".local/bin"));
    }
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// Coordinates `mosh-server` handed back.
#[derive(Debug)]
pub struct Bootstrap {
    /// Address the server told us to use (from `-s`), if it did.
    pub ip: Option<String>,
    pub port: u16,
    /// 22-character base64 session key.
    pub key: Zeroizing<String>,
}

/// Pick `MOSH IP` / `MOSH CONNECT` out of what `mosh-server` printed. Anything
/// else (login banners, warnings) is ignored, like the wrapper does.
pub fn parse_server_output(stdout: &str) -> Option<Bootstrap> {
    let mut ip = None;
    let mut connect = None;
    for line in stdout.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("MOSH IP ") {
            let v = rest.trim();
            if !v.is_empty() {
                ip = Some(v.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("MOSH SSH_CONNECTION ") {
            // `mosh-server` without `-s` on newer versions: client ip, client
            // port, server ip, server port.
            if ip.is_none()
                && let Some(server) = rest.split_whitespace().nth(2)
            {
                ip = Some(server.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("MOSH CONNECT ") {
            let mut it = rest.split_whitespace();
            let port = it.next().and_then(|p| p.parse::<u16>().ok());
            let key = it.next().filter(|k| {
                k.len() == 22
                    && k.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'/' || b == b'+')
            });
            if let (Some(port), Some(key)) = (port, key) {
                connect = Some((port, key.to_string()));
            }
        }
    }
    let (port, key) = connect?;
    Some(Bootstrap {
        ip,
        port,
        key: Zeroizing::new(key),
    })
}

/// Fallback for the default command when the host lacks `en_US.UTF-8`
/// (minimal Debian/Alpine images ship only `C.UTF-8`).
const FALLBACK_SERVER_COMMAND: &str = "mosh-server new -s -c 256 -l LANG=C.UTF-8";

/// Start the remote side over an already authenticated SSH connection.
///
/// A custom command is run verbatim. The default one is retried with
/// `C.UTF-8` if the host rejects `en_US.UTF-8`; any other failure surfaces as
/// a typed error with the server's own explanation.
pub async fn start_server(client: &SshClient, command: Option<&str>) -> Result<Bootstrap> {
    let custom = command.map(str::trim).filter(|c| !c.is_empty());
    match run_server(client, custom.unwrap_or(DEFAULT_SERVER_COMMAND)).await {
        Err(e) if custom.is_none() && e.kind == "mosh" && e.message.contains("locale") => {
            run_server(client, FALLBACK_SERVER_COMMAND).await
        }
        r => r,
    }
}

async fn run_server(client: &SshClient, command: &str) -> Result<Bootstrap> {
    let out = tokio::time::timeout(BOOTSTRAP_TIMEOUT, client.exec(command, None))
        .await
        .map_err(|_| {
            DesktopError::new(
                "mosh",
                "mosh-server did not answer in time — is Mosh installed on the host?",
            )
        })??;
    let stdout = String::from_utf8_lossy(&out.stdout);
    match parse_server_output(&stdout) {
        Some(b) => Ok(b),
        None => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let detail = failure_detail(&stderr, &stdout)
                .unwrap_or_else(|| "no MOSH CONNECT line in the output".to_string());
            Err(DesktopError::new(
                "mosh",
                format!("mosh-server failed on the host: {detail}"),
            ))
        }
    }
}

/// Pick the lines worth showing from a failed bootstrap. `mosh-server` prints
/// its explanation first and then dumps `locale` (`LANG=…`, `LC_ALL=…`), so
/// the last line alone is useless; keep the prose, drop the variable dump and
/// the harmless warnings, and cap the length.
fn failure_detail(stderr: &str, stdout: &str) -> Option<String> {
    let is_env_dump = |l: &str| {
        l.split_once('=').is_some_and(|(k, _)| {
            !k.is_empty() && k.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
        })
    };
    let lines: Vec<&str> = stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("Warning: SSH_CONNECTION not found"))
        .filter(|l| !l.starts_with("locale: Cannot set "))
        .filter(|l| !is_env_dump(l))
        .collect();
    if lines.is_empty() {
        return None;
    }
    let mut seen = Vec::new();
    for l in lines {
        if !seen.contains(&l) {
            seen.push(l);
        }
        if seen.len() == 3 {
            break;
        }
    }
    Some(seen.join(" "))
}

/// Resolve the UDP target when the server did not tell us its address.
pub async fn resolve_ip(host: &str, ip_version: IpVersion) -> Result<String> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip.to_string());
    }
    let addrs = ip_version.filter(tokio::net::lookup_host((host, 0)).await?.collect());
    addrs.first().map(|a| a.ip().to_string()).ok_or_else(|| {
        DesktopError::new(
            "mosh",
            format!(
                "{host} did not resolve to any {} address",
                ip_version.label()
            ),
        )
    })
}

/// Run `mosh-client` in a local PTY against a started server.
pub fn spawn_client(
    client: PathBuf,
    ip: &str,
    boot: &Bootstrap,
    term_type: &str,
    size: TermSize,
) -> Result<(Arc<LocalTerminal>, TermEvents)> {
    let mut env = vec![
        ("MOSH_KEY".to_string(), boot.key.to_string()),
        ("TERM".to_string(), term_type.to_string()),
    ];
    if std::env::var_os("LANG").is_none() {
        env.push(("LANG".to_string(), "en_US.UTF-8".to_string()));
    }
    Ok(LocalTerminal::spawn(LocalShellOptions {
        argv: vec![
            client.to_string_lossy().into_owned(),
            ip.to_string(),
            boot.port.to_string(),
        ],
        cwd: None,
        env,
        size,
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ip_and_connect_lines_and_skips_noise() {
        let out =
            "Welcome!\r\nMOSH IP 203.0.113.7\r\n\r\nMOSH CONNECT 60001 aBcDeFgHiJkLmNoPqRsTuV\r\n";
        let b = parse_server_output(out).expect("bootstrap");
        assert_eq!(b.ip.as_deref(), Some("203.0.113.7"));
        assert_eq!(b.port, 60001);
        assert_eq!(b.key.as_str(), "aBcDeFgHiJkLmNoPqRsTuV");
    }

    #[test]
    fn ssh_connection_line_supplies_the_server_address() {
        let out = "MOSH SSH_CONNECTION 198.51.100.2 51234 2001:db8::1 22\nMOSH CONNECT 60002 aBcDeFgHiJkLmNoPqRsTuV\n";
        let b = parse_server_output(out).expect("bootstrap");
        assert_eq!(b.ip.as_deref(), Some("2001:db8::1"));
        assert_eq!(b.port, 60002);
    }

    #[test]
    fn failure_detail_keeps_the_explanation_not_the_locale_dump() {
        let stderr = "Warning: SSH_CONNECTION not found; binding to any interface.\n\
The locale requested by LANG=en_US.UTF-8 isn't available here.\n\
Running `locale-gen en_US.UTF-8' may be necessary.\n\n\
The locale requested by LANG=en_US.UTF-8 isn't available here.\n\
Running `locale-gen en_US.UTF-8' may be necessary.\n\n\
mosh-server needs a UTF-8 native locale to run.\n\n\
Unfortunately, the local environment (LANG=en_US.UTF-8) specifies\n\
the character set \"US-ASCII\",\n\n\
locale: Cannot set LC_CTYPE to default locale: No such file or directory\n\
LANG=en_US.UTF-8\nLANGUAGE=\nLC_ALL=\n";
        let d = failure_detail(stderr, "").expect("detail");
        assert!(d.starts_with("The locale requested by LANG=en_US.UTF-8 isn't available here."));
        assert!(d.contains("mosh-server needs a UTF-8 native locale to run."));
        assert!(d.ends_with("to run."));
        assert!(!d.contains("LC_ALL="));
        assert!(!d.contains("Warning: SSH_CONNECTION"));
        assert_eq!(
            failure_detail("bash: mosh-server: command not found\n", "").as_deref(),
            Some("bash: mosh-server: command not found")
        );
        assert_eq!(failure_detail("", "\n"), None);
    }

    #[test]
    fn rejects_missing_or_malformed_connect() {
        assert!(parse_server_output("bash: mosh-server: command not found\n").is_none());
        assert!(parse_server_output("MOSH CONNECT 60001 short\n").is_none());
        assert!(parse_server_output("MOSH CONNECT notaport aBcDeFgHiJkLmNoPqRsTuV\n").is_none());
        assert!(parse_server_output("MOSH CONNECT 60001 aBcDeFgHiJkLmNoPqRsTu!\n").is_none());
    }
}
