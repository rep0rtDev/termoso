//! Command line: `termoso ssh deploy@10.0.1.12`, `termoso sftp prod-web`,
//! `termoso telnet switch.lab:23`, `termoso open <saved host>`. Every form is
//! turned into a `termoso://` link the web view already routes, so a second
//! launch hands it to the running window and a first launch opens it once the
//! window is up. Files and URLs on the command line keep their old meaning.

use std::fmt::Write as _;

pub const SUBCOMMANDS: [&str; 6] = ["ssh", "sftp", "telnet", "open", "local", "new"];

/// What to print (and exit) instead of starting the app, if anything.
pub enum EarlyExit {
    Stdout(String),
    Stderr(String),
}

pub fn help() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "Termoso {}", env!("CARGO_PKG_VERSION"));
    s.push_str(
        "\nUsage:\n  termoso                          open the app\n  termoso ssh [user@]host[:port]   connect over SSH (saved host by label, or ad hoc)\n  termoso ssh -l user -p port host same, OpenSSH-style flags\n  termoso telnet host[:port]       connect over Telnet\n  termoso sftp <host>              open the SFTP browser on a saved host\n  termoso open <host>              open a saved host (label, address or termoso://host/<id>)\n  termoso local                    open a local terminal\n  termoso new                      new host dialog\n  termoso <file>…                  import a key / theme / ssh_config\n  termoso <ssh:// | telnet:// | termoso:// link>\n\nOptions:\n  -h, --help       show this help\n  -V, --version    print the version\n\nIf Termoso is already running, the command is handed to that window.\n",
    );
    s
}

/// `--help` / `--version` / an unknown subcommand: text to print and leave.
pub fn early_exit<I: IntoIterator<Item = String>>(args: I) -> Option<EarlyExit> {
    let args: Vec<String> = args.into_iter().collect();
    match args.first().map(String::as_str) {
        Some("-h" | "--help" | "help") => Some(EarlyExit::Stdout(help())),
        Some("-V" | "--version" | "version") => Some(EarlyExit::Stdout(format!(
            "termoso {}",
            env!("CARGO_PKG_VERSION")
        ))),
        Some(cmd) if SUBCOMMANDS.contains(&cmd) && parse(&args).is_none() => {
            Some(EarlyExit::Stderr(format!(
                "termoso {cmd}: missing or bad target\n\n{}",
                help()
            )))
        }
        _ => None,
    }
}

/// Links a command line asks for (empty when it is plain files / URLs / nothing).
pub fn links_from_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    let args: Vec<String> = args.into_iter().collect();
    parse(&args).into_iter().collect()
}

fn parse(args: &[String]) -> Option<String> {
    let (cmd, rest) = args.split_first()?;
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    match cmd.as_str() {
        "local" => Some("termoso://local".into()),
        "new" => Some("termoso://new".into()),
        "ssh" | "telnet" | "open" => connect_link(cmd, &rest),
        "sftp" => {
            let target = positional(&rest)?;
            if target.is_empty() {
                return Some("termoso://sftp".into());
            }
            Some(format!("termoso://sftp?{}", query(&[("host", target)])))
        }
        _ => None,
    }
}

/// `ssh [-p PORT] [-l USER] TARGET`, `telnet TARGET`, `open TARGET`. A full
/// `termoso://host/<id>` link is passed through.
fn connect_link(cmd: &str, rest: &[&str]) -> Option<String> {
    let mut user: Option<&str> = None;
    let mut port: Option<&str> = None;
    let mut target: Option<&str> = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i] {
            "-l" | "-p" if i + 1 < rest.len() => {
                if rest[i] == "-l" {
                    user = Some(rest[i + 1]);
                } else {
                    port = Some(rest[i + 1]);
                }
                i += 2;
            }
            a if a.starts_with("-l") && a.len() > 2 => {
                user = Some(&a[2..]);
                i += 1;
            }
            a if a.starts_with("-p") && a.len() > 2 => {
                port = Some(&a[2..]);
                i += 1;
            }
            a if a.starts_with('-') => return None,
            a => {
                if target.is_some() {
                    return None;
                }
                target = Some(a);
                i += 1;
            }
        }
    }
    let target = target.filter(|t| !t.is_empty())?;
    if target.starts_with("termoso://host/") {
        return Some(target.to_string());
    }
    if let Some(p) = port
        && p.parse::<u16>().map(|n| n == 0).unwrap_or(true)
    {
        return None;
    }
    let protocol = if cmd == "telnet" { "telnet" } else { "ssh" };
    let mut params: Vec<(&str, &str)> = vec![("host", target)];
    if let Some(u) = user {
        params.push(("user", u));
    }
    if let Some(p) = port {
        params.push(("port", p));
    }
    if cmd != "open" {
        params.push(("protocol", protocol));
    }
    Some(format!("termoso://connect?{}", query(&params)))
}

fn positional<'a>(rest: &[&'a str]) -> Option<&'a str> {
    match rest {
        [] => Some(""),
        [one] if !one.starts_with('-') => Some(one),
        _ => None,
    }
}

fn query(params: &[(&str, &str)]) -> String {
    let mut q = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in params {
        q.append_pair(k, v);
    }
    q.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(args: &[&str]) -> Vec<String> {
        links_from_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn ssh_forms() {
        assert_eq!(
            links(&["ssh", "deploy@10.0.1.12:2222"]),
            ["termoso://connect?host=deploy%4010.0.1.12%3A2222&protocol=ssh"]
        );
        assert_eq!(
            links(&["ssh", "-l", "root", "-p", "2200", "edge"]),
            ["termoso://connect?host=edge&user=root&port=2200&protocol=ssh"]
        );
        assert_eq!(
            links(&["ssh", "-p2200", "edge"]),
            ["termoso://connect?host=edge&port=2200&protocol=ssh"]
        );
        assert_eq!(
            links(&["telnet", "switch.lab:23"]),
            ["termoso://connect?host=switch.lab%3A23&protocol=telnet"]
        );
        assert_eq!(
            links(&["open", "prod web"]),
            ["termoso://connect?host=prod+web"]
        );
        assert_eq!(
            links(&[
                "open",
                "termoso://host/3f2b7c9e-1d4a-4e8b-9c6f-0a1b2c3d4e5f"
            ]),
            ["termoso://host/3f2b7c9e-1d4a-4e8b-9c6f-0a1b2c3d4e5f"]
        );
    }

    #[test]
    fn sftp_and_actions() {
        assert_eq!(
            links(&["sftp", "prod-web"]),
            ["termoso://sftp?host=prod-web"]
        );
        assert_eq!(links(&["sftp"]), ["termoso://sftp"]);
        assert_eq!(links(&["local"]), ["termoso://local"]);
        assert_eq!(links(&["new"]), ["termoso://new"]);
    }

    #[test]
    fn non_commands_are_left_alone() {
        assert!(links(&[]).is_empty());
        assert!(links(&["ssh://a@b"]).is_empty());
        assert!(links(&["/tmp/id_ed25519"]).is_empty());
        assert!(links(&["--foo"]).is_empty());
        assert!(early_exit(["ssh://a@b".to_string()]).is_none());
        assert!(early_exit(["/tmp/x".to_string()]).is_none());
    }

    #[test]
    fn bad_input_is_an_early_exit() {
        assert!(matches!(
            early_exit(["ssh".to_string()]),
            Some(EarlyExit::Stderr(_))
        ));
        assert!(matches!(
            early_exit([
                "ssh".to_string(),
                "-p".to_string(),
                "x".to_string(),
                "h".to_string()
            ]),
            Some(EarlyExit::Stderr(_))
        ));
        assert!(matches!(
            early_exit(["ssh".to_string(), "a".to_string(), "b".to_string()]),
            Some(EarlyExit::Stderr(_))
        ));
        assert!(matches!(
            early_exit(["--version".to_string()]),
            Some(EarlyExit::Stdout(v)) if v.starts_with("termoso ")
        ));
        assert!(matches!(
            early_exit(["--help".to_string()]),
            Some(EarlyExit::Stdout(_))
        ));
    }
}
