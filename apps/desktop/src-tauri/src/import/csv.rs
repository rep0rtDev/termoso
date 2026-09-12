//! Termius-style CSV (`Groups,Label,Tags,Hostname/IP,Protocol,Port,Username,
//! Password`). Column order is free: headers are matched by name with a few
//! aliases, so exports from other tools usually work too. Only the address
//! column is required.

use crate::error::{DesktopError, Result};

use super::{ImportPreview, ImportedHost};

pub const TEMPLATE: &str = "\
Groups,Label,Tags,Hostname/IP,Protocol,Port,Username,Password
Production/Web,web-1,\"web, nginx\",web1.example.com,ssh,22,deploy,
Production/DB,db-1,postgres,10.0.0.5,ssh,2222,admin,
Lab,switch,,192.168.1.2,telnet,23,admin,
";

const MAX_ROWS: usize = 50_000;

#[derive(Debug, Default, Clone, Copy)]
struct Columns {
    groups: Option<usize>,
    label: Option<usize>,
    tags: Option<usize>,
    address: Option<usize>,
    protocol: Option<usize>,
    port: Option<usize>,
    username: Option<usize>,
    password: Option<usize>,
}

fn normalise(h: &str) -> String {
    h.trim()
        .trim_start_matches('\u{feff}')
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn columns(header: &[String]) -> Columns {
    let mut c = Columns::default();
    for (i, h) in header.iter().enumerate() {
        let slot = match normalise(h).as_str() {
            "groups" | "group" | "folder" | "folders" | "path" | "parentgroup" => &mut c.groups,
            "label" | "name" | "alias" | "title" | "session" | "sessionname" => &mut c.label,
            "tags" | "tag" | "labels" => &mut c.tags,
            "hostnameip" | "hostname" | "host" | "address" | "ip" | "ipaddress" | "server" => {
                &mut c.address
            }
            "protocol" | "type" => &mut c.protocol,
            "port" => &mut c.port,
            "username" | "user" | "login" => &mut c.username,
            "password" | "pass" | "passwd" => &mut c.password,
            _ => continue,
        };
        if slot.is_none() {
            *slot = Some(i);
        }
    }
    c
}

/// RFC 4180 records: quoted fields, `""` escapes, newlines inside quotes.
fn records(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let delimiter = detect_delimiter(text);
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                c => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            c if c == delimiter => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                if rows.len() > MAX_ROWS {
                    break;
                }
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    rows
}

/// `,` unless the header clearly uses `;` or a tab.
fn detect_delimiter(text: &str) -> char {
    let first = text.lines().next().unwrap_or("");
    let count = |d: char| first.matches(d).count();
    let (commas, semis, tabs) = (count(','), count(';'), count('\t'));
    if tabs > commas && tabs > semis {
        '\t'
    } else if semis > commas {
        ';'
    } else {
        ','
    }
}

fn cell(row: &[String], i: Option<usize>) -> &str {
    i.and_then(|i| row.get(i)).map(|s| s.trim()).unwrap_or("")
}

fn split_list(s: &str) -> Vec<String> {
    s.split([',', ';', '|'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn parse_into(text: &str, preview: &mut ImportPreview) -> Result<()> {
    let rows = records(text);
    let Some(header) = rows.first() else {
        return Err(DesktopError::invalid("The CSV file is empty"));
    };
    let cols = columns(header);
    let Some(addr_col) = cols.address else {
        return Err(DesktopError::invalid(
            "No host column found — the first row must be a header like \
             `Groups,Label,Tags,Hostname/IP,Protocol,Port,Username,Password`",
        ));
    };
    let mut skipped = 0usize;
    for (n, row) in rows.iter().enumerate().skip(1) {
        let address = cell(row, Some(addr_col));
        if address.is_empty() {
            skipped += 1;
            continue;
        }
        let protocol = cell(row, cols.protocol).to_ascii_lowercase();
        let protocol = match protocol.as_str() {
            "" | "ssh" => "ssh",
            "telnet" => "telnet",
            other => {
                preview.warnings.push(format!(
                    "Row {}: protocol {other:?} is not supported, skipped",
                    n + 1
                ));
                continue;
            }
        };
        let label = cell(row, cols.label);
        let port_text = cell(row, cols.port);
        let port = if port_text.is_empty() {
            None
        } else {
            match port_text.parse::<u16>() {
                Ok(p) if p > 0 => Some(p),
                _ => {
                    preview
                        .warnings
                        .push(format!("Row {}: port {port_text:?} ignored", n + 1));
                    None
                }
            }
        };
        let password = cell(row, cols.password);
        preview.hosts.push(ImportedHost {
            label: if label.is_empty() {
                address.to_string()
            } else {
                label.to_string()
            },
            address: address.to_string(),
            protocol: protocol.into(),
            port,
            username: cell(row, cols.username).to_string(),
            password: (!password.is_empty()).then(|| password.to_string()),
            group_path: cell(row, cols.groups)
                .split(['/', '\\', '>'])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            tags: split_list(cell(row, cols.tags)),
            ..ImportedHost::default()
        });
    }
    if skipped > 0 {
        preview
            .warnings
            .push(format!("{skipped} row(s) without a host name skipped"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::ImportSource;

    #[test]
    fn template_round_trips() {
        let mut p = ImportPreview::new(ImportSource::Csv, "test");
        parse_into(TEMPLATE, &mut p).unwrap();
        assert_eq!(p.hosts.len(), 3);
        let web = &p.hosts[0];
        assert_eq!(web.label, "web-1");
        assert_eq!(web.group_path, vec!["Production", "Web"]);
        assert_eq!(web.tags, vec!["web", "nginx"]);
        assert_eq!(web.address, "web1.example.com");
        assert_eq!(web.port, Some(22));
        assert_eq!(web.username, "deploy");
        assert_eq!(web.password, None);
        assert_eq!(p.hosts[1].port, Some(2222));
        assert_eq!(p.hosts[2].protocol, "telnet");
        assert!(p.hosts[2].tags.is_empty());
    }

    #[test]
    fn aliases_and_quotes() {
        let text = "\u{feff}Name;Host;User;Pass;Port\n\"Multi\nline\";10.0.0.1;root;\"p;w\"\"d\";bad\n;missing;;;\n";
        let mut p = ImportPreview::new(ImportSource::Csv, "test");
        parse_into(text, &mut p).unwrap();
        assert_eq!(p.hosts.len(), 2);
        assert_eq!(p.hosts[0].label, "Multi\nline");
        assert_eq!(p.hosts[0].username, "root");
        assert_eq!(p.hosts[0].password.as_deref(), Some("p;w\"d"));
        assert_eq!(p.hosts[0].port, None);
        assert!(p.warnings.iter().any(|w| w.contains("port")));
        assert_eq!(p.hosts[1].label, "missing");
    }

    #[test]
    fn rejects_without_host_column() {
        let mut p = ImportPreview::new(ImportSource::Csv, "test");
        assert!(parse_into("a,b,c\n1,2,3\n", &mut p).is_err());
        assert!(parse_into("", &mut p).is_err());
    }
}
