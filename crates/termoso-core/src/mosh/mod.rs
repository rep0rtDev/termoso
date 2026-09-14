//! Mosh: bootstrap `mosh-server` over an authenticated SSH connection, then
//! talk to it directly over UDP with a Rust client that needs no local
//! `mosh` binary (the desktop may still prefer the system `mosh-client`).
//!
//! The session key never leaves this module except inside [`Bootstrap`],
//! which zeroizes it on drop.

mod crypto;
mod fragment;
mod proto;
mod transport;

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use zeroize::Zeroizing;

use crate::error::{CoreError, Result};
use crate::ssh::{IpVersion, SshClient};
use crate::terminal::{TermEvents, TermSize};

pub use transport::{CONNECT_TIMEOUT, MoshTerminal};

/// What Termius starts by default; the user can replace it per host.
pub const DEFAULT_SERVER_COMMAND: &str = "mosh-server new -s -c 256 -l LANG=en_US.UTF-8";

/// Fallback for the default command when the host lacks `en_US.UTF-8`
/// (minimal Debian/Alpine images ship only `C.UTF-8`).
const FALLBACK_SERVER_COMMAND: &str = "mosh-server new -s -c 256 -l LANG=C.UTF-8";

/// How long the remote gets to print its `MOSH CONNECT` line.
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(30);

/// Everything that can go wrong on the Mosh side; SSH errors stay SSH errors.
#[derive(Debug, thiserror::Error)]
pub enum MoshError {
    /// `mosh-server` printed nothing within [`BOOTSTRAP_TIMEOUT`].
    #[error("mosh-server did not answer in time — is Mosh installed on the host?")]
    BootstrapTimeout,
    /// `mosh-server` exited without a `MOSH CONNECT` line; carries its own words.
    #[error("mosh-server failed on the host: {0}")]
    Bootstrap(String),
    /// The shell could not find `mosh-server` (exit 127).
    #[error(
        "mosh-server is not installed on the host — install Mosh there (e.g. `apt install mosh`) or connect with SSH"
    )]
    NotInstalled,
    /// Jump hosts / proxies carry TCP only; Mosh needs a direct UDP path.
    #[error("Mosh needs a direct UDP path to the host; remove the {0} or connect with SSH")]
    NoDirectPath(&'static str),
    /// The host name has no address of the requested family.
    #[error("{host} did not resolve to any {family} address")]
    Resolve {
        /// Host name we tried to resolve.
        host: String,
        /// `IPv4` / `IPv6` / `IPv4 or IPv6`.
        family: String,
    },
    /// The `MOSH CONNECT` key is not 16 bytes of base64.
    #[error("mosh-server handed back an invalid session key")]
    Key,
    /// Could not bind or connect a UDP socket.
    #[error("UDP socket: {0}")]
    Socket(String),
    /// Server never answered our first datagram.
    #[error(
        "no answer from mosh-server at udp/{addr} — UDP 60000–61000 must be open on the way to the host{}",
        detail.as_ref().map(|d| format!(" ({d})")).unwrap_or_default()
    )]
    NoReply {
        /// `ip:port` we sent to.
        addr: String,
        /// Last socket error, if sends were failing.
        detail: Option<String>,
    },
    /// Server announced an incompatible protocol version.
    #[error("server speaks Mosh protocol {0}, this client speaks 2")]
    Version(u32),
    /// Datagram failed OCB authentication.
    #[error("packet failed authentication")]
    Cipher,
    /// 2^63 packets sent on one key.
    #[error("sequence counter exhausted")]
    CounterWrapped,
    /// Fragment header or reassembly inconsistency.
    #[error("malformed fragment")]
    Fragment,
    /// zlib stream did not inflate.
    #[error("malformed compressed payload")]
    Compression,
    /// Instruction did not decode.
    #[error("malformed protobuf")]
    Protobuf,
    /// Shutdown completed (or timed out) after a local close.
    #[error("session closed")]
    Closed,
}

/// Coordinates `mosh-server` handed back.
#[derive(Debug)]
pub struct Bootstrap {
    /// Address the server told us to use (from `-s`), if it did.
    pub ip: Option<String>,
    /// UDP port `mosh-server` listens on.
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

/// Start the remote side over an already authenticated SSH connection.
///
/// A custom command is run verbatim. The default one is retried with
/// `C.UTF-8` if the host rejects `en_US.UTF-8`; any other failure surfaces as
/// a typed error with the server's own explanation.
pub async fn start_server(client: &SshClient, command: Option<&str>) -> Result<Bootstrap> {
    let custom = command.map(str::trim).filter(|c| !c.is_empty());
    match run_server(client, custom.unwrap_or(DEFAULT_SERVER_COMMAND)).await {
        Err(CoreError::Mosh(MoshError::Bootstrap(detail)))
            if custom.is_none() && detail.contains("locale") =>
        {
            run_server(client, FALLBACK_SERVER_COMMAND).await
        }
        r => r,
    }
}

async fn run_server(client: &SshClient, command: &str) -> Result<Bootstrap> {
    let out = tokio::time::timeout(BOOTSTRAP_TIMEOUT, client.exec(command, None))
        .await
        .map_err(|_| MoshError::BootstrapTimeout)??;
    let stdout = String::from_utf8_lossy(&out.stdout);
    match parse_server_output(&stdout) {
        Some(b) => Ok(b),
        None => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if out.exit_code == Some(127) || stderr.contains("mosh-server: not found") {
                return Err(MoshError::NotInstalled.into());
            }
            let detail = failure_detail(&stderr, &stdout)
                .unwrap_or_else(|| "no MOSH CONNECT line in the output".to_string());
            Err(MoshError::Bootstrap(detail).into())
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

/// Where to send the datagrams: the address the client itself resolves for
/// `host` — the same path the SSH leg just took. The address `mosh-server`
/// reports (`MOSH IP`, taken from `SSH_CONNECTION`) is the server's own view
/// of its interface, which is wrong behind NAT or a port forward, so it only
/// serves as a fallback when local resolution fails.
pub async fn udp_target(host: &str, ip_version: IpVersion, boot: &Bootstrap) -> Result<String> {
    match resolve_ip(host, ip_version).await {
        Ok(ip) => Ok(ip),
        Err(e) => boot.ip.clone().ok_or(e),
    }
}

/// Resolve `host` to a single address of the requested family.
pub async fn resolve_ip(host: &str, ip_version: IpVersion) -> Result<String> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip.to_string());
    }
    let addrs = ip_version.filter(tokio::net::lookup_host((host, 0)).await?.collect());
    addrs.first().map(|a| a.ip().to_string()).ok_or_else(|| {
        MoshError::Resolve {
            host: host.to_string(),
            family: ip_version.label().to_string(),
        }
        .into()
    })
}

/// Open the UDP session with the built-in client and wait for the first
/// screen from the server.
pub async fn connect(
    ip: &str,
    boot: &Bootstrap,
    size: TermSize,
) -> Result<(std::sync::Arc<MoshTerminal>, TermEvents)> {
    let ip: IpAddr = ip
        .parse()
        .map_err(|_| CoreError::Invalid(format!("{ip} is not an IP address")))?;
    let key = crypto::Key::parse(&boot.key)?;
    transport::connect(transport::Options {
        addr: SocketAddr::new(ip, boot.port),
        key,
        size,
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connect_and_ip_lines() {
        let out = "Warning: something\r\nMOSH IP 203.0.113.7\r\nMOSH CONNECT 60001 aBcDeFgHiJkLmNoPqRsTuV\r\n\r\n";
        let b = parse_server_output(out).expect("bootstrap");
        assert_eq!(b.ip.as_deref(), Some("203.0.113.7"));
        assert_eq!(b.port, 60001);
        assert_eq!(b.key.as_str(), "aBcDeFgHiJkLmNoPqRsTuV");
    }

    #[test]
    fn falls_back_to_ssh_connection_line() {
        let out = "MOSH SSH_CONNECTION 198.51.100.2 51234 203.0.113.9 22\nMOSH CONNECT 60002 AAAAAAAAAAAAAAAAAAAAAA\n";
        let b = parse_server_output(out).expect("bootstrap");
        assert_eq!(b.ip.as_deref(), Some("203.0.113.9"));
        let out = "MOSH CONNECT 60003 AAAAAAAAAAAAAAAAAAAAAA\n";
        assert!(parse_server_output(out).expect("bootstrap").ip.is_none());
    }

    #[test]
    fn rejects_missing_or_malformed_connect() {
        assert!(parse_server_output("bash: mosh-server: command not found\n").is_none());
        assert!(parse_server_output("MOSH CONNECT 60001 short\n").is_none());
        assert!(parse_server_output("MOSH CONNECT notaport aBcDeFgHiJkLmNoPqRsTuV\n").is_none());
        assert!(parse_server_output("MOSH CONNECT 60001 aBcDeFgHiJkLmNoPqRsTu!\n").is_none());
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
        assert!(!d.contains("LC_ALL="));
        assert!(!d.contains("Warning: SSH_CONNECTION"));
        assert_eq!(
            failure_detail("bash: mosh-server: command not found\n", "").as_deref(),
            Some("bash: mosh-server: command not found")
        );
        assert_eq!(failure_detail("", "\n"), None);
    }

    #[test]
    fn errors_carry_actionable_text() {
        let e = CoreError::from(MoshError::NoReply {
            addr: "203.0.113.7:60001".into(),
            detail: None,
        });
        assert_eq!(e.kind(), "mosh");
        assert!(e.to_string().contains("UDP 60000–61000"));
        assert!(
            MoshError::NoReply {
                addr: "a:1".into(),
                detail: Some("send: boom".into()),
            }
            .to_string()
            .ends_with("(send: boom)")
        );
    }
}
