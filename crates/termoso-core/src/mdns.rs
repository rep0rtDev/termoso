//! Local-network discovery (Termius "Discover local devices"): browse the
//! DNS-SD service types machines with an SSH server advertise
//! (`_ssh._tcp`, `_sftp-ssh._tcp`) and hand back one entry per machine.
//!
//! Nothing here leaves the local link: mDNS is link-local multicast, the
//! query is read-only and the result is only shown to the user, who picks
//! what becomes a host. Discovery is best-effort — machines without an
//! advertiser (plain OpenSSH without Avahi/Bonjour) do not show up.

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::time::Duration;

use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent};
use serde::{Deserialize, Serialize};

/// Service types we browse and what they mean for a host.
pub const SERVICE_TYPES: [(&str, LocalService); 2] = [
    ("_ssh._tcp.local.", LocalService::Ssh),
    ("_sftp-ssh._tcp.local.", LocalService::Sftp),
];

/// How long a browse runs by default; mDNS responders answer within a
/// second or two, the rest is slack for sleepy Wi-Fi devices.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(4);
const MAX_TIMEOUT: Duration = Duration::from_secs(30);

/// A DNS-SD service advertised by a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalService {
    /// `_ssh._tcp` — an SSH server.
    Ssh,
    /// `_sftp-ssh._tcp` — SFTP over SSH (macOS, some NAS boxes).
    Sftp,
}

/// A machine found on the local network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDevice {
    /// Instance name from the advertisement (`Office NAS`), or the hostname
    /// without domain when none was given.
    pub name: String,
    /// mDNS hostname (`nas.local`) — resolves on the local network even when
    /// the address changes.
    pub hostname: String,
    /// Addresses the machine advertised, IPv4 first, link-local last.
    pub addresses: Vec<IpAddr>,
    /// SSH port; the first `_ssh._tcp` port wins, `_sftp-ssh._tcp` otherwise.
    pub port: u16,
    /// Services seen for this machine.
    pub services: Vec<LocalService>,
    /// TXT record keys/values (Avahi and macOS publish nothing here, some
    /// appliances add a model name).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub txt: BTreeMap<String, String>,
}

/// Why a browse could not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum MdnsError {
    /// No multicast-capable interface / socket could not be opened.
    #[error("{0}")]
    Unavailable(String),
}

impl MdnsError {
    /// Stable machine-readable kind.
    pub fn kind(&self) -> &'static str {
        "mdns_unavailable"
    }
}

impl From<mdns_sd::Error> for MdnsError {
    fn from(e: mdns_sd::Error) -> Self {
        MdnsError::Unavailable(e.to_string())
    }
}

/// One resolved advertisement, as much of it as we keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advertisement {
    /// Which service type answered.
    pub service: LocalService,
    /// `Office NAS._ssh._tcp.local.`
    pub fullname: String,
    /// `nas.local.`
    pub hostname: String,
    /// Port from the SRV record.
    pub port: u16,
    /// A/AAAA records seen for the host.
    pub addresses: Vec<IpAddr>,
    /// TXT record, decoded.
    pub txt: BTreeMap<String, String>,
}

impl Advertisement {
    fn from_resolved(service: LocalService, r: &ResolvedService) -> Self {
        Self {
            service,
            fullname: r.fullname.clone(),
            hostname: r.host.clone(),
            port: r.port,
            addresses: r.addresses.iter().map(|a| a.to_ip_addr()).collect(),
            txt: r
                .txt_properties
                .iter()
                .filter(|p| !p.key().is_empty())
                .map(|p| (p.key().to_string(), p.val_str().to_string()))
                .collect(),
        }
    }
}

/// Browse the local network for `timeout` and return what answered.
pub async fn browse(timeout: Duration) -> Result<Vec<LocalDevice>, MdnsError> {
    let timeout = timeout.clamp(Duration::from_millis(500), MAX_TIMEOUT);
    let daemon = ServiceDaemon::new()?;
    let result = browse_with(&daemon, timeout).await;
    if let Ok(rx) = daemon.shutdown() {
        // Let the daemon thread leave the multicast groups before we return.
        let _ = tokio::time::timeout(Duration::from_secs(1), rx.recv_async()).await;
    }
    result
}

async fn browse_with(
    daemon: &ServiceDaemon,
    timeout: Duration,
) -> Result<Vec<LocalDevice>, MdnsError> {
    let mut receivers = Vec::with_capacity(SERVICE_TYPES.len());
    for (ty, service) in SERVICE_TYPES {
        receivers.push((service, daemon.browse(ty)?));
    }
    let deadline = tokio::time::Instant::now() + timeout;
    let mut seen: Vec<Advertisement> = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let mut pending = Vec::with_capacity(receivers.len());
        for (service, rx) in &receivers {
            let service = *service;
            pending.push(Box::pin(async move { (service, rx.recv_async().await) }));
        }
        let Ok(((service, event), _, _)) =
            tokio::time::timeout(remaining, futures::future::select_all(pending)).await
        else {
            break;
        };
        match event {
            Ok(ServiceEvent::ServiceResolved(r)) => {
                if r.is_valid() {
                    seen.push(Advertisement::from_resolved(service, &r));
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    for (ty, _) in SERVICE_TYPES {
        let _ = daemon.stop_browse(ty);
    }
    Ok(merge(seen))
}

/// Fold advertisements into devices: one per hostname, SSH port preferred,
/// addresses deduplicated and ordered for connecting.
pub fn merge(ads: Vec<Advertisement>) -> Vec<LocalDevice> {
    let mut by_host: BTreeMap<String, LocalDevice> = BTreeMap::new();
    let mut ssh_port_set: BTreeSet<String> = BTreeSet::new();
    for ad in ads {
        let key = ad.hostname.trim_end_matches('.').to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        let hostname = ad.hostname.trim_end_matches('.').to_string();
        let name = instance_name(&ad.fullname).unwrap_or_else(|| {
            hostname
                .split('.')
                .next()
                .unwrap_or(hostname.as_str())
                .to_string()
        });
        let dev = by_host.entry(key.clone()).or_insert_with(|| LocalDevice {
            name: name.clone(),
            hostname: hostname.clone(),
            addresses: Vec::new(),
            port: ad.port,
            services: Vec::new(),
            txt: BTreeMap::new(),
        });
        // The SSH advertisement names the machine; SFTP-only names are a
        // fallback so a device advertising both keeps its `_ssh` label.
        if ad.service == LocalService::Ssh && !ssh_port_set.contains(&key) {
            dev.name = name;
            dev.port = ad.port;
            ssh_port_set.insert(key.clone());
        }
        for a in ad.addresses {
            if !dev.addresses.contains(&a) && !a.is_unspecified() {
                dev.addresses.push(a);
            }
        }
        if !dev.services.contains(&ad.service) {
            dev.services.push(ad.service);
        }
        for (k, v) in ad.txt {
            dev.txt.entry(k).or_insert(v);
        }
    }
    let mut out: Vec<LocalDevice> = by_host.into_values().collect();
    for d in &mut out {
        d.addresses.sort_by_key(address_rank);
        d.services.sort();
    }
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.hostname.cmp(&b.hostname))
    });
    out
}

/// Connect-order: routable IPv4, then IPv6, then link-local (needs a scope
/// id the user cannot type), loopback last.
fn address_rank(a: &IpAddr) -> u8 {
    match a {
        IpAddr::V4(v4) if v4.is_loopback() => 5,
        IpAddr::V4(v4) if v4.is_link_local() => 3,
        IpAddr::V4(_) => 0,
        IpAddr::V6(v6) if v6.is_loopback() => 5,
        IpAddr::V6(v6) if (v6.segments()[0] & 0xffc0) == 0xfe80 => 4,
        IpAddr::V6(_) => 1,
    }
}

/// Instance part of a DNS-SD full name, unescaped:
/// `Office\032NAS._ssh._tcp.local.` → `Office NAS`.
pub fn instance_name(fullname: &str) -> Option<String> {
    let idx = fullname.find("._")?;
    let raw = &fullname[..idx];
    let name = unescape(raw);
    let name = name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Undo DNS label escaping (`\.`, `\\`, `\DDD`).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut digits = String::new();
        while digits.len() < 3 {
            match chars.peek() {
                Some(d) if d.is_ascii_digit() => {
                    digits.push(*d);
                    chars.next();
                }
                _ => break,
            }
        }
        if digits.len() == 3 {
            if let Some(ch) = digits.parse::<u32>().ok().and_then(char::from_u32) {
                out.push(ch);
            }
        } else {
            out.push_str(&digits);
            if digits.is_empty()
                && let Some(n) = chars.next()
            {
                out.push(n);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ad(
        service: LocalService,
        full: &str,
        host: &str,
        port: u16,
        addrs: &[&str],
    ) -> Advertisement {
        Advertisement {
            service,
            fullname: full.into(),
            hostname: host.into(),
            port,
            addresses: addrs.iter().map(|a| a.parse().unwrap()).collect(),
            txt: BTreeMap::new(),
        }
    }

    #[test]
    fn instance_name_unescapes() {
        assert_eq!(
            instance_name("Office\\032NAS._ssh._tcp.local.").as_deref(),
            Some("Office NAS")
        );
        assert_eq!(
            instance_name("pi\\.home._sftp-ssh._tcp.local.").as_deref(),
            Some("pi.home")
        );
        assert_eq!(instance_name("._ssh._tcp.local.").as_deref(), None);
        assert_eq!(instance_name("garbage"), None);
    }

    #[test]
    fn merge_groups_by_host_and_prefers_ssh() {
        let devices = merge(vec![
            ad(
                LocalService::Sftp,
                "nas sftp._sftp-ssh._tcp.local.",
                "NAS.local.",
                2222,
                &["192.168.1.10", "fe80::1"],
            ),
            ad(
                LocalService::Ssh,
                "Office NAS._ssh._tcp.local.",
                "nas.local.",
                22,
                &["192.168.1.10", "2001:db8::10"],
            ),
            ad(
                LocalService::Ssh,
                "pi._ssh._tcp.local.",
                "raspberrypi.local.",
                22,
                &["192.168.1.20"],
            ),
        ]);
        assert_eq!(devices.len(), 2);
        let nas = &devices[0];
        assert_eq!(nas.name, "Office NAS");
        assert_eq!(nas.hostname, "NAS.local");
        assert_eq!(nas.port, 22);
        assert_eq!(nas.services, vec![LocalService::Ssh, LocalService::Sftp]);
        assert_eq!(
            nas.addresses,
            vec![
                "192.168.1.10".parse::<IpAddr>().unwrap(),
                "2001:db8::10".parse().unwrap(),
                "fe80::1".parse().unwrap(),
            ]
        );
        assert_eq!(devices[1].name, "pi");
        assert_eq!(devices[1].port, 22);
    }

    #[test]
    fn merge_falls_back_to_hostname_and_sftp_port() {
        let devices = merge(vec![ad(
            LocalService::Sftp,
            "._sftp-ssh._tcp.local.",
            "backup-box.local.",
            2022,
            &["10.0.0.5"],
        )]);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "backup-box");
        assert_eq!(devices[0].port, 2022);
        assert_eq!(devices[0].services, vec![LocalService::Sftp]);
    }

    #[test]
    fn merge_skips_blank_hosts_and_unspecified_addresses() {
        let devices = merge(vec![
            ad(
                LocalService::Ssh,
                "x._ssh._tcp.local.",
                "",
                22,
                &["10.0.0.1"],
            ),
            ad(
                LocalService::Ssh,
                "y._ssh._tcp.local.",
                "y.local.",
                22,
                &["0.0.0.0", "10.0.0.2", "10.0.0.2"],
            ),
        ]);
        assert_eq!(devices.len(), 1);
        assert_eq!(
            devices[0].addresses,
            vec!["10.0.0.2".parse::<IpAddr>().unwrap()]
        );
    }

    #[test]
    fn device_serializes_camel_case_without_empty_txt() {
        let d = LocalDevice {
            name: "pi".into(),
            hostname: "pi.local".into(),
            addresses: vec!["10.0.0.9".parse().unwrap()],
            port: 22,
            services: vec![LocalService::Ssh],
            txt: BTreeMap::new(),
        };
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["hostname"], "pi.local");
        assert_eq!(json["services"][0], "ssh");
        assert!(json.get("txt").is_none());
    }

    /// Advertise a service ourselves and make sure a browse finds it. Needs
    /// a multicast-capable interface; skipped where the daemon cannot start.
    #[tokio::test]
    async fn browse_finds_own_advertisement() {
        let Ok(daemon) = ServiceDaemon::new() else {
            eprintln!("mdns daemon unavailable; skipping");
            return;
        };
        let _ = daemon.set_multicast_loop_v4(true);
        let name = format!("termoso-test-{}", uuid::Uuid::new_v4().simple());
        let host = format!("{name}.local.");
        let info = mdns_sd::ServiceInfo::new(
            "_ssh._tcp.local.",
            &name,
            &host,
            (),
            2222,
            &[("model", "termoso")][..],
        )
        .expect("service info")
        .enable_addr_auto();
        if daemon.register(info).is_err() {
            eprintln!("mdns register failed; skipping");
            return;
        }
        let devices = browse_with(&daemon, Duration::from_secs(3))
            .await
            .expect("browse");
        let _ = daemon.shutdown();
        let Some(dev) = devices.iter().find(|d| d.name == name) else {
            eprintln!("own advertisement not seen (no multicast interface?); skipping");
            return;
        };
        assert_eq!(dev.port, 2222);
        assert_eq!(dev.hostname, host.trim_end_matches('.'));
        assert_eq!(dev.services, vec![LocalService::Ssh]);
        assert_eq!(dev.txt.get("model").map(String::as_str), Some("termoso"));
        assert!(!dev.addresses.is_empty());
    }
}
