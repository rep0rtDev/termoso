//! MobaXterm bookmarks: a `.mxtsessions` export or the `[Bookmarks*]`
//! sections of `MobaXterm.ini`. Each section carries `SubRep=Folder\Sub`
//! and one `Name=#TYPE#flags%field%field…` line per session; SSH is type
//! 109, Telnet 98. Passwords live encrypted in the registry and are not
//! part of the file, so they are never imported.

use super::{ImportPreview, ImportedHost, ImportedProxy};
use crate::error::{DesktopError, Result};

const TYPE_SSH: u32 = 109;
const TYPE_TELNET: u32 = 98;
const TYPE_RDP: u32 = 91;
const ENABLED: &str = "-1";

pub fn parse_into(text: &str, preview: &mut ImportPreview) -> Result<()> {
    let mut in_bookmarks = false;
    let mut seen_section = false;
    let mut group: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let lower = section.trim().to_ascii_lowercase();
            in_bookmarks = lower == "bookmarks" || lower.starts_with("bookmarks_");
            if in_bookmarks {
                seen_section = true;
                group.clear();
            }
            continue;
        }
        if !in_bookmarks {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key.eq_ignore_ascii_case("SubRep") {
            group = value
                .split('\\')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            continue;
        }
        if key.eq_ignore_ascii_case("ImgNum") || key.is_empty() {
            continue;
        }
        let Some(rest) = value.strip_prefix('#') else {
            continue;
        };
        let Some((kind, rest)) = rest.split_once('#') else {
            continue;
        };
        let Ok(kind) = kind.trim().parse::<u32>() else {
            continue;
        };
        // Everything after the next `#` is terminal appearance.
        let params: Vec<&str> = rest.split('#').next().unwrap_or("").split('%').collect();
        match session_host(key, kind, &params, &group) {
            Ok(h) => preview.hosts.push(h),
            Err(reason) => preview
                .warnings
                .push(format!("Session {key} skipped: {reason}")),
        }
    }
    if !seen_section {
        return Err(DesktopError::invalid(
            "Not a MobaXterm sessions file — expected a [Bookmarks] section (.mxtsessions export or MobaXterm.ini)",
        ));
    }
    Ok(())
}

fn session_host(
    name: &str,
    kind: u32,
    p: &[&str],
    group: &[String],
) -> std::result::Result<ImportedHost, String> {
    let field = |i: usize| p.get(i).map(|s| s.trim()).unwrap_or("");
    let protocol = match kind {
        TYPE_SSH => "ssh",
        TYPE_TELNET => "telnet",
        TYPE_RDP => return Err("RDP sessions are not supported".into()),
        other => return Err(format!("session type {other} is not supported")),
    };
    let address = field(1);
    if address.is_empty() {
        return Err("no host name".into());
    }
    let mut host = ImportedHost {
        label: name.to_string(),
        address: address.to_string(),
        protocol: protocol.into(),
        port: field(2).parse::<u16>().ok().filter(|p| *p > 0),
        group_path: group.to_vec(),
        ..Default::default()
    };
    if protocol == "telnet" {
        return Ok(host);
    }
    let user = field(3);
    if !user.is_empty() && user != "<default>" {
        host.username = user.to_string();
    }
    host.forward_x11 = field(5) == ENABLED;
    let command = field(7);
    if !command.is_empty() {
        host.warnings.push(format!(
            "Login command not imported: {}",
            decode_text(command)
        ));
    }
    let gw_hosts = split_list(field(8));
    let gw_ports = split_list(field(9));
    let gw_users = split_list(field(10));
    for (i, gh) in gw_hosts.iter().enumerate() {
        if gh.is_empty() {
            continue;
        }
        let mut spec = String::new();
        if let Some(u) = gw_users.get(i).filter(|u| !u.is_empty()) {
            spec.push_str(u);
            spec.push('@');
        }
        spec.push_str(gh);
        if let Some(port) = gw_ports.get(i).filter(|p| !p.is_empty() && **p != "22") {
            spec.push(':');
            spec.push_str(port);
        }
        host.jump_hosts.push(spec);
    }
    let key = field(14);
    if !key.is_empty() {
        let path = key.replace("_CurrentDrive_", "C");
        if path.to_ascii_lowercase().ends_with(".ppk") {
            host.warnings.push(format!(
                "{path}: PuTTY .ppk keys must be converted to OpenSSH format before import"
            ));
        }
        host.key_path = Some(path);
    }
    let proxy_kind = match field(19) {
        "" | "0" => None,
        "1" => Some("socks4"),
        "2" => Some("socks5"),
        "3" => Some("http"),
        "4" => {
            host.warnings.push("Telnet proxy is not supported".into());
            None
        }
        _ => {
            host.warnings
                .push("Local / SSH proxy commands are not supported".into());
            None
        }
    };
    if let Some(kind) = proxy_kind {
        let proxy_host = field(20);
        if proxy_host.is_empty() {
            host.warnings
                .push("Proxy without a host was ignored".into());
        } else {
            host.proxy = Some(ImportedProxy {
                kind: kind.into(),
                host: proxy_host.into(),
                port: field(21).parse().unwrap_or(1080),
                username: field(22).into(),
                password: None,
                has_password: false,
            });
        }
    }
    host.agent_forwarding = field(34) == ENABLED;
    Ok(host)
}

fn split_list(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split("__PIPE__").map(str::trim).collect()
}

/// MobaXterm escapes characters that would break its `%`/`#` encoding.
fn decode_text(s: &str) -> String {
    s.replace("__PTVIRG__", ";")
        .replace("__DBLQUO__", "\"")
        .replace("__PIPE__", "|")
        .replace("__DIESE__", "#")
        .replace("__PCENT__", "%")
}

#[cfg(test)]
mod tests {
    use super::super::ImportSource;
    use super::*;

    const EXPORT: &str = r#"[Bookmarks]
SubRep=Production\GPU
ImgNum=42
Prod GPU=#109#0%prod-gpu.example.com%2200%deploy%%-1%-1%sudo -i%bastion.example.com%2222%jump%0%0%Interactive shell%_CurrentDrive_:\Users\me\keys\prod.ppk%%-1%0%0%2%proxy.local%1080%pu%0%-1%1%%0%%%%0%-1%-1%-1#MobaFont%10%0%0%-1%15%236,236,236%30,30,30%180,180,192%0%-1%0%%xterm%-1%0%_Std_Colors_0_%80%24%0%0%-1%<none>%%0%1%-1%-1#0# #-1
Ignored RDP=#91#0%desktop.example.com%3389%user%

[Bookmarks_1]
SubRep=Imported Sessions\SSH
ImgNum=41
SSH with default username= #109#0%imported.ssh.local%22%<default>%%0%-1%%%%%0%0%0%%%-1%0%0%0%%1080%%0%0%1%%0%%%%0%-1%-1%0#MobaFont%10%0%0%-1%15%236,236,236%30,30,30%180,180,192%0%-1%0%%xterm%-1%0%_Std_Colors_0_%80%24%0%0%-1%<none>%%0%1%-1%-1#0# #-1
Router=#98#0%10.0.0.1%2323%%
"#;

    #[test]
    fn parses_bookmarks() {
        let mut p = ImportPreview::new(ImportSource::Mobaxterm, "test");
        parse_into(EXPORT, &mut p).unwrap();
        assert_eq!(p.hosts.len(), 3);
        let gpu = &p.hosts[0];
        assert_eq!(gpu.label, "Prod GPU");
        assert_eq!(gpu.address, "prod-gpu.example.com");
        assert_eq!(gpu.port, Some(2200));
        assert_eq!(gpu.username, "deploy");
        assert_eq!(gpu.group_path, vec!["Production", "GPU"]);
        assert!(gpu.forward_x11);
        assert!(gpu.agent_forwarding);
        assert_eq!(gpu.jump_hosts, vec!["jump@bastion.example.com:2222"]);
        assert_eq!(
            gpu.key_path.as_deref(),
            Some("C:\\Users\\me\\keys\\prod.ppk")
        );
        let proxy = gpu.proxy.as_ref().unwrap();
        assert_eq!(proxy.kind, "socks5");
        assert_eq!(proxy.host, "proxy.local");
        assert_eq!(proxy.port, 1080);
        assert_eq!(proxy.username, "pu");
        assert!(gpu.warnings.iter().any(|w| w.contains("sudo -i")));
        assert!(gpu.warnings.iter().any(|w| w.contains(".ppk")));

        let plain = &p.hosts[1];
        assert_eq!(plain.label, "SSH with default username");
        assert_eq!(plain.username, "");
        assert_eq!(plain.port, Some(22));
        assert_eq!(plain.group_path, vec!["Imported Sessions", "SSH"]);
        assert!(!plain.forward_x11);
        assert!(plain.jump_hosts.is_empty());
        assert!(plain.proxy.is_none());

        let router = &p.hosts[2];
        assert_eq!(router.protocol, "telnet");
        assert_eq!(router.address, "10.0.0.1");
        assert_eq!(router.port, Some(2323));

        assert!(
            p.warnings
                .iter()
                .any(|w| w.contains("Ignored RDP") && w.contains("RDP"))
        );
    }

    #[test]
    fn rejects_other_ini() {
        let mut p = ImportPreview::new(ImportSource::Mobaxterm, "test");
        assert!(parse_into("[Misc]\nFoo=bar\n", &mut p).is_err());
    }
}
