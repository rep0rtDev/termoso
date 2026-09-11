//! Guess which operating system a host runs so the UI can show its icon.
//!
//! Two sources, cheapest first:
//!
//! 1. the server identification string (`SSH-2.0-OpenSSH_9.6p1 Ubuntu-3ubuntu13`
//!    already says "ubuntu"; RouterOS, Windows and Cisco are just as chatty);
//! 2. a one-shot `exec` of [`DETECT_COMMAND`] on a separate channel, so the
//!    interactive shell never sees it and nothing lands in the user's history.
//!
//! The result is one of [`KNOWN`] — stable identifiers the clients map to
//! icons and brand colours.

use std::time::Duration;

use crate::ssh::SshClient;

/// Every OS identifier this module can produce.
pub const KNOWN: &[&str] = &[
    "ubuntu", "debian", "raspbian", "fedora", "redhat", "centos", "rocky", "alma", "amazon",
    "suse", "arch", "manjaro", "gentoo", "alpine", "nixos", "mint", "kali", "mageia", "linux",
    "freebsd", "openbsd", "netbsd", "osx", "windows", "android", "routeros", "cisco",
];

/// Runs in the login shell without a PTY. Plain `;`-separated commands so it
/// works in sh, bash, zsh, fish and dash alike; the `os-release` fields are
/// what [`classify`] keys on, `uname` covers the BSDs and macOS.
pub const DETECT_COMMAND: &str =
    "uname -s; cat /etc/os-release /usr/lib/os-release /etc/*-release 2>/dev/null";

const EXEC_TIMEOUT: Duration = Duration::from_secs(8);

/// Keyword → OS, most specific first (so "rocky" wins over the "linux" in
/// "Rocky Linux", and "raspbian" over the "debian" in its `ID_LIKE`).
const KEYWORDS: &[(&str, &str)] = &[
    ("raspbian", "raspbian"),
    ("raspberry", "raspbian"),
    ("linuxmint", "mint"),
    ("linux mint", "mint"),
    ("kali", "kali"),
    ("ubuntu", "ubuntu"),
    ("pop!_os", "ubuntu"),
    ("elementary", "ubuntu"),
    ("rocky", "rocky"),
    ("almalinux", "alma"),
    ("alma", "alma"),
    ("amazon", "amazon"),
    ("amzn", "amazon"),
    ("centos", "centos"),
    ("fedora", "fedora"),
    ("red hat", "redhat"),
    ("redhat", "redhat"),
    ("rhel", "redhat"),
    ("oracle", "redhat"),
    ("mageia", "mageia"),
    ("opensuse", "suse"),
    ("suse", "suse"),
    ("sles", "suse"),
    ("nixos", "nixos"),
    ("manjaro", "manjaro"),
    ("archlinux", "arch"),
    ("arch linux", "arch"),
    ("endeavouros", "arch"),
    ("artix", "arch"),
    ("gentoo", "gentoo"),
    ("alpine", "alpine"),
    ("debian", "debian"),
    ("routeros", "routeros"),
    ("mikrotik", "routeros"),
    ("cisco", "cisco"),
    ("ios-xe", "cisco"),
    ("nx-os", "cisco"),
    ("freebsd", "freebsd"),
    ("truenas", "freebsd"),
    ("pfsense", "freebsd"),
    ("openbsd", "openbsd"),
    ("netbsd", "netbsd"),
    ("darwin", "osx"),
    ("macos", "osx"),
    ("mac os", "osx"),
    ("android", "android"),
    ("termux", "android"),
    ("msys_nt", "windows"),
    ("mingw", "windows"),
    ("cygwin", "windows"),
    ("windows", "windows"),
    // Bare `ID=arch`; last because the substring is common in prose.
    ("arch", "arch"),
    ("linux", "linux"),
];

/// OS hinted by the server identification string, if it says anything.
pub fn from_server_id(id: &str) -> Option<&'static str> {
    let lower = id.to_ascii_lowercase();
    if lower.contains("rosssh") {
        return Some("routeros");
    }
    if lower.contains("openssh_for_windows") || lower.contains("winsshd") {
        return Some("windows");
    }
    if lower.contains("cisco") {
        return Some("cisco");
    }
    // Distros stamp their package version in the comment: "Ubuntu-3ubuntu13",
    // "Debian-2+deb12u2", "FreeBSD-20240806", "Raspbian-10+deb10u2".
    let comment = lower.split_once(' ').map(|(_, c)| c).unwrap_or("");
    KEYWORDS
        .iter()
        .filter(|(kw, os)| *os != "linux" && comment.contains(kw))
        .map(|(_, os)| *os)
        .next()
}

/// Map the output of [`DETECT_COMMAND`] (or anything similar: `uname`,
/// `/etc/os-release`, `lsb_release -a`) to an OS identifier.
pub fn classify(output: &str) -> Option<&'static str> {
    let lower = output.to_ascii_lowercase();
    if lower.trim().is_empty() {
        return None;
    }
    // `ID=` is authoritative when present; ID_LIKE only breaks ties.
    if let Some(id) = os_release_field(&lower, "id")
        && let Some(os) = keyword(&id)
    {
        return Some(os);
    }
    if let Some(like) = os_release_field(&lower, "id_like")
        && let Some(os) = keyword(&like)
    {
        return Some(os);
    }
    keyword(&lower)
}

fn os_release_field(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.trim().split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

fn keyword(text: &str) -> Option<&'static str> {
    KEYWORDS
        .iter()
        .find(|(kw, _)| text.contains(kw))
        .map(|(_, os)| *os)
}

/// Detect the OS of a connected host. Returns `None` when neither source
/// says anything recognisable or the server refuses a second channel.
pub async fn detect(client: &SshClient) -> Option<&'static str> {
    let hint = client.server_id().and_then(from_server_id);
    // A hinted RouterOS or Cisco box has no POSIX shell to probe further;
    // Windows is certain from its identification too.
    if let Some(os @ ("routeros" | "cisco" | "windows")) = hint {
        return Some(os);
    }
    let probe = tokio::time::timeout(EXEC_TIMEOUT, client.exec(DETECT_COMMAND, None)).await;
    let detected = match probe {
        Ok(Ok(out)) => {
            let text = format!(
                "{}\n{}",
                out.stdout_str(),
                String::from_utf8_lossy(&out.stderr)
            );
            classify(&text)
        }
        Ok(Err(e)) => {
            tracing::debug!("os probe failed: {e}");
            None
        }
        Err(_) => {
            tracing::debug!("os probe timed out");
            None
        }
    };
    match (detected, hint) {
        // The probe's generic answer must not override a specific hint.
        (Some("linux"), Some(h)) => Some(h),
        (Some(os), _) => Some(os),
        (None, h) => h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_id_hints() {
        assert_eq!(
            from_server_id("SSH-2.0-OpenSSH_9.6p1 Ubuntu-3ubuntu13.5"),
            Some("ubuntu")
        );
        assert_eq!(
            from_server_id("SSH-2.0-OpenSSH_9.2p1 Debian-2+deb12u3"),
            Some("debian")
        );
        assert_eq!(
            from_server_id("SSH-2.0-OpenSSH_7.9p1 Raspbian-10+deb10u2"),
            Some("raspbian")
        );
        assert_eq!(from_server_id("SSH-2.0-ROSSSH"), Some("routeros"));
        assert_eq!(
            from_server_id("SSH-2.0-OpenSSH_for_Windows_9.5"),
            Some("windows")
        );
        assert_eq!(from_server_id("SSH-2.0-Cisco-1.25"), Some("cisco"));
        assert_eq!(
            from_server_id("SSH-2.0-OpenSSH_9.7 FreeBSD-20240806"),
            Some("freebsd")
        );
        assert_eq!(from_server_id("SSH-2.0-OpenSSH_9.6"), None);
        assert_eq!(from_server_id("SSH-2.0-dropbear_2022.83"), None);
    }

    #[test]
    fn os_release_id_wins_over_id_like() {
        let ubuntu =
            "Linux\nPRETTY_NAME=\"Ubuntu 24.04 LTS\"\nNAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n";
        assert_eq!(classify(ubuntu), Some("ubuntu"));
        let rocky = "Linux\nNAME=\"Rocky Linux\"\nID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n";
        assert_eq!(classify(rocky), Some("rocky"));
        let alma = "Linux\nNAME=\"AlmaLinux\"\nID=\"almalinux\"\n";
        assert_eq!(classify(alma), Some("alma"));
        let amazon = "Linux\nNAME=\"Amazon Linux\"\nID=\"amzn\"\nID_LIKE=\"fedora\"\n";
        assert_eq!(classify(amazon), Some("amazon"));
        let mint = "Linux\nNAME=\"Linux Mint\"\nID=linuxmint\nID_LIKE=\"ubuntu debian\"\n";
        assert_eq!(classify(mint), Some("mint"));
        let kali = "Linux\nNAME=\"Kali GNU/Linux\"\nID=kali\nID_LIKE=debian\n";
        assert_eq!(classify(kali), Some("kali"));
        let nixos = "Linux\nNAME=NixOS\nID=nixos\n";
        assert_eq!(classify(nixos), Some("nixos"));
    }

    #[test]
    fn id_like_breaks_unknown_ids() {
        let derivative = "Linux\nNAME=\"Some Distro\"\nID=somedistro\nID_LIKE=\"arch\"\n";
        assert_eq!(classify(derivative), Some("arch"));
        let arch = "Linux\nNAME=\"Arch Linux\"\nID=arch\nBUILD_ID=rolling\n";
        assert_eq!(classify(arch), Some("arch"));
    }

    #[test]
    fn uname_only_systems() {
        assert_eq!(classify("FreeBSD\n"), Some("freebsd"));
        assert_eq!(classify("OpenBSD\n"), Some("openbsd"));
        assert_eq!(classify("Darwin\n"), Some("osx"));
        assert_eq!(classify("MSYS_NT-10.0-19045\n"), Some("windows"));
        assert_eq!(classify("Linux\n"), Some("linux"));
        assert_eq!(classify("   \n"), None);
        assert_eq!(classify("something else"), None);
    }

    #[test]
    fn all_keyword_targets_are_known() {
        for (_, os) in KEYWORDS {
            assert!(KNOWN.contains(os), "{os} missing from KNOWN");
        }
    }
}
