//! ZOC Terminal host directory (`HostDirectory.zhd`): a `[STRUCTURE]` block
//! with `Section#N=` names and `Folder#N=section.parent|name` entries, then
//! one `[HOST] … [/HOST]` block per entry (`name`, `connectto`, `username`,
//! `authfile`, `memo`, `folder`, `section`, `deviceid`). Values may be
//! quoted and prefixed with `utf8:`.

use std::collections::HashMap;

use super::{ImportPreview, ImportedHost};
use crate::error::{DesktopError, Result};

const DEVICE_SERIAL: i64 = 1;
const DEVICE_SSH: i64 = 9;

struct Folder {
    section: u32,
    parent: u32,
    name: String,
}

pub fn parse_into(text: &str, preview: &mut ImportPreview) -> Result<()> {
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}'))
        .collect();
    if !lines.iter().any(|l| l.eq_ignore_ascii_case("[HOST]")) {
        return Err(DesktopError::invalid(
            "Not a ZOC host directory — expected [HOST] entries (HostDirectory.zhd)",
        ));
    }
    let structure_end = lines
        .iter()
        .position(|l| l.eq_ignore_ascii_case("[/STRUCTURE]"))
        .unwrap_or(0);
    let mut sections: HashMap<u32, String> = HashMap::new();
    let mut folders: HashMap<u32, Folder> = HashMap::new();
    for line in &lines[..structure_end] {
        if let Some(rest) = line.strip_prefix("Section#") {
            if let Some((id, name)) = rest.split_once('=')
                && let Ok(id) = id.parse()
            {
                sections.insert(id, unquote(name));
            }
        } else if let Some(rest) = line.strip_prefix("Folder#") {
            let Some((id, value)) = rest.split_once('=') else {
                continue;
            };
            let Some((ids, name)) = value.split_once('|') else {
                continue;
            };
            let mut parts = ids.split('.');
            let (Ok(id), Some(Ok(section)), Some(Ok(parent))) = (
                id.parse::<u32>(),
                parts.next().map(str::parse::<u32>),
                parts.next().map(str::parse::<u32>),
            ) else {
                continue;
            };
            folders.insert(
                id,
                Folder {
                    section,
                    parent,
                    name: unquote(name),
                },
            );
        }
    }

    let mut entries: Vec<HashMap<String, String>> = Vec::new();
    let mut current: Option<HashMap<String, String>> = None;
    for line in &lines[structure_end..] {
        if line.eq_ignore_ascii_case("[HOST]") {
            current = Some(HashMap::new());
        } else if line.eq_ignore_ascii_case("[/HOST]") {
            if let Some(e) = current.take() {
                entries.push(e);
            }
        } else if let Some(e) = current.as_mut()
            && let Some((k, v)) = line.split_once('=')
        {
            e.insert(k.trim().to_ascii_lowercase(), unquote(v));
        }
    }

    let used_sections: std::collections::HashSet<u32> =
        entries.iter().map(|e| num(e, "section") as u32).collect();
    let with_sections = used_sections.len() > 1;
    for e in &entries {
        let mut group: Vec<String> = Vec::new();
        let mut handle = num(e, "folder") as u32;
        let mut depth = 0;
        while handle != 0 && depth < 32 {
            let Some(f) = folders.get(&handle) else {
                break;
            };
            group.push(f.name.clone());
            handle = f.parent;
            depth += 1;
        }
        group.reverse();
        if with_sections {
            let section = num(e, "section") as u32;
            let section = folders
                .get(&(num(e, "folder") as u32))
                .map(|f| f.section)
                .unwrap_or(section);
            group.insert(
                0,
                sections
                    .get(&section)
                    .cloned()
                    .unwrap_or_else(|| format!("Section {section}")),
            );
        }
        let label = e.get("name").map(String::as_str).unwrap_or("").trim();
        match entry_host(e, label, group) {
            Ok(h) => preview.hosts.push(h),
            Err(reason) => preview.warnings.push(format!(
                "Host {} skipped: {reason}",
                if label.is_empty() { "(unnamed)" } else { label }
            )),
        }
    }
    Ok(())
}

fn entry_host(
    e: &HashMap<String, String>,
    label: &str,
    group: Vec<String>,
) -> std::result::Result<ImportedHost, String> {
    match num(e, "deviceid") {
        DEVICE_SSH | 0 | -1 => {}
        DEVICE_SERIAL => return Err("serial sessions are not supported".into()),
        other => return Err(format!("device type {other} is not supported")),
    }
    let connect = e.get("connectto").map(String::as_str).unwrap_or("").trim();
    let (address, port) = split_host_port(connect);
    if address.is_empty() {
        return Err("no host name".into());
    }
    let mut host = ImportedHost {
        label: if label.is_empty() {
            address.to_string()
        } else {
            label.to_string()
        },
        address: address.to_string(),
        protocol: "ssh".into(),
        port,
        username: e
            .get("username")
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string(),
        group_path: group,
        notes: e
            .get("memo")
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string(),
        ..Default::default()
    };
    if let Some(key) = e
        .get("authfile")
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
    {
        if key.to_ascii_lowercase().ends_with(".ppk") {
            host.warnings.push(format!(
                "{key}: PuTTY .ppk keys must be converted to OpenSSH format before import"
            ));
        }
        host.key_path = Some(key.to_string());
    }
    Ok(host)
}

fn num(e: &HashMap<String, String>, key: &str) -> i64 {
    e.get(key).and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

/// `utf8:"value"`, `"value"` or bare `value`.
fn unquote(raw: &str) -> String {
    let v = raw.trim();
    let v = v.strip_prefix("utf8:").unwrap_or(v);
    let v = v
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(v);
    v.to_string()
}

fn split_host_port(s: &str) -> (&str, Option<u16>) {
    if let Some(rest) = s.strip_prefix('[')
        && let Some((host, tail)) = rest.split_once(']')
    {
        let port = tail.strip_prefix(':').and_then(|p| p.parse().ok());
        return (host, port);
    }
    match s.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => match port.parse::<u16>() {
            Ok(p) if p > 0 => (host, Some(p)),
            _ => (s, None),
        },
        _ => (s, None),
    }
}

#[cfg(test)]
mod tests {
    use super::super::ImportSource;
    use super::*;

    const ZHD: &str = r#"[STRUCTURE]
Section#0="Default"
Section#1="Work"
Folder#1=1.0|Production
Folder#2=1.1.utf8|Databases
[/STRUCTURE]
[HOST]
name="Web 1"
connectto="web1.example.com:2200"
username="deploy"
authfile="/home/me/.ssh/id_ed25519"
memo=utf8:"Primary web node"
section=1
folder=1
deviceid=9
[/HOST]
[HOST]
name="DB"
connectto="[2001:db8::10]:22"
username="postgres"
section=1
folder=2
deviceid=0
[/HOST]
[HOST]
name="Console"
connectto=""
deviceid=1
deviceopts="[1]COM3:57600-8N1|0|0"
section=0
[/HOST]
[HOST]
name="Home"
connectto="home.example.net"
section=0
deviceid=-1
[/HOST]
"#;

    #[test]
    fn parses_hosts_and_folders() {
        let mut p = ImportPreview::new(ImportSource::Zoc, "test");
        parse_into(ZHD, &mut p).unwrap();
        assert_eq!(p.hosts.len(), 3);
        let web = &p.hosts[0];
        assert_eq!(web.label, "Web 1");
        assert_eq!(web.address, "web1.example.com");
        assert_eq!(web.port, Some(2200));
        assert_eq!(web.username, "deploy");
        assert_eq!(web.key_path.as_deref(), Some("/home/me/.ssh/id_ed25519"));
        assert_eq!(web.notes, "Primary web node");
        assert_eq!(web.group_path, vec!["Work", "Production"]);
        let db = &p.hosts[1];
        assert_eq!(db.address, "2001:db8::10");
        assert_eq!(db.port, Some(22));
        assert_eq!(db.group_path, vec!["Work", "Production", "Databases"]);
        let home = &p.hosts[2];
        assert_eq!(home.address, "home.example.net");
        assert_eq!(home.port, None);
        assert_eq!(home.group_path, vec!["Default"]);
        assert!(
            p.warnings
                .iter()
                .any(|w| w.contains("Console") && w.contains("serial"))
        );
    }

    #[test]
    fn single_section_adds_no_section_folder() {
        let text = "[HOST]\nname=a\nconnectto=a.example\nsection=0\n[/HOST]\n";
        let mut p = ImportPreview::new(ImportSource::Zoc, "test");
        parse_into(text, &mut p).unwrap();
        assert!(p.hosts[0].group_path.is_empty());
    }

    #[test]
    fn rejects_other_text() {
        let mut p = ImportPreview::new(ImportSource::Zoc, "test");
        assert!(parse_into("Host foo\n", &mut p).is_err());
    }
}
