//! Azure: service-principal token → subscriptions → VMs → NIC → public IP.

use std::collections::HashMap;

use futures::stream::{self, StreamExt, TryStreamExt};
use zeroize::Zeroizing;

use super::{AzureConfig, CloudError, CloudInstance, Endpoints, status_error, trimmed};

const SUBSCRIPTIONS_API: &str = "2022-12-01";
const COMPUTE_API: &str = "2024-07-01";
const NETWORK_API: &str = "2024-05-01";
const MAX_PAGES: usize = 50;
const PARALLEL_LOOKUPS: usize = 6;

pub(super) async fn virtual_machines(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    cfg: &AzureConfig,
) -> Result<Vec<CloudInstance>, CloudError> {
    let mgmt = endpoints.azure_management();
    let token = token(http, endpoints, cfg).await?;
    let arm = Arm {
        http,
        mgmt,
        token: &token,
    };

    let subscriptions = arm
        .list(&format!(
            "{mgmt}/subscriptions?api-version={SUBSCRIPTIONS_API}"
        ))
        .await?;
    let subscription_ids: Vec<String> = subscriptions
        .iter()
        .filter_map(|s| s["subscriptionId"].as_str().map(str::to_string))
        .collect();
    if subscription_ids.is_empty() {
        return Err(CloudError::Forbidden(
            "the service principal has no access to any subscription (assign it the Reader role)"
                .into(),
        ));
    }

    let mut vms = Vec::new();
    for sub in &subscription_ids {
        vms.extend(
            arm.list(&format!(
                "{mgmt}/subscriptions/{sub}/providers/Microsoft.Compute/virtualMachines?api-version={COMPUTE_API}"
            ))
            .await?,
        );
    }

    // Resolve every primary NIC once, then every public IP once.
    let nic_ids: Vec<String> = vms
        .iter()
        .filter_map(primary_nic_id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let nics: HashMap<String, serde_json::Value> = stream::iter(nic_ids)
        .map(|id| {
            let arm = arm.clone();
            async move {
                let v = arm
                    .get(&format!("{}{}?api-version={NETWORK_API}", arm.mgmt, id))
                    .await?;
                Ok::<_, CloudError>((id, v))
            }
        })
        .buffer_unordered(PARALLEL_LOOKUPS)
        .try_collect()
        .await?;

    let pip_ids: Vec<String> = nics
        .values()
        .filter_map(nic_addresses)
        .filter_map(|(_, pip)| pip)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let pips: HashMap<String, String> = stream::iter(pip_ids)
        .map(|id| {
            let arm = arm.clone();
            async move {
                let v = arm
                    .get(&format!("{}{}?api-version={NETWORK_API}", arm.mgmt, id))
                    .await?;
                let ip = v["properties"]["ipAddress"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_default();
                Ok::<_, CloudError>((id, ip))
            }
        })
        .buffer_unordered(PARALLEL_LOOKUPS)
        .try_collect()
        .await?;

    Ok(vms
        .iter()
        .filter_map(|vm| {
            let nic = primary_nic_id(vm).and_then(|id| nics.get(&id));
            let (_private, pip) = nic.and_then(nic_addresses).unwrap_or((None, None));
            let public = pip
                .and_then(|id| pips.get(&id))
                .filter(|ip| !ip.is_empty())
                .cloned();
            parse_vm(vm, public)
        })
        .collect())
}

/// The VM's primary NIC resource id (the only one when there is one).
fn primary_nic_id(vm: &serde_json::Value) -> Option<String> {
    let nics = vm["properties"]["networkProfile"]["networkInterfaces"].as_array()?;
    nics.iter()
        .find(|n| n["properties"]["primary"].as_bool() == Some(true))
        .or_else(|| nics.first())
        .and_then(|n| n["id"].as_str())
        .map(str::to_string)
}

/// `(private ip, public ip resource id)` of the NIC's primary ip configuration.
fn nic_addresses(nic: &serde_json::Value) -> Option<(Option<String>, Option<String>)> {
    let configs = nic["properties"]["ipConfigurations"].as_array()?;
    let cfg = configs
        .iter()
        .find(|c| c["properties"]["primary"].as_bool() == Some(true))
        .or_else(|| configs.first())?;
    Some((
        cfg["properties"]["privateIPAddress"]
            .as_str()
            .map(str::to_string),
        cfg["properties"]["publicIPAddress"]["id"]
            .as_str()
            .map(str::to_string),
    ))
}

/// A `virtualMachines` list entry plus its resolved public address.
pub(super) fn parse_vm(vm: &serde_json::Value, public_ip: Option<String>) -> Option<CloudInstance> {
    let id = vm["id"].as_str().filter(|s| !s.is_empty())?;
    let props = &vm["properties"];
    let image = &props["storageProfile"]["imageReference"];
    let os_type = props["storageProfile"]["osDisk"]["osType"].as_str();
    let os = match (image["offer"].as_str(), image["sku"].as_str(), os_type) {
        (Some(offer), Some(sku), _) => Some(format!("{offer} {sku}")),
        (Some(offer), None, _) => Some(offer.to_string()),
        (None, _, Some(t)) => Some(t.to_string()),
        _ => None,
    };
    Some(CloudInstance {
        instance_id: id.to_string(),
        label: vm["name"].as_str().unwrap_or("").to_string(),
        address: public_ip,
        state: props["provisioningState"].as_str().map(str::to_string),
        region: vm["location"].as_str().map(str::to_string),
        size: props["hardwareProfile"]["vmSize"]
            .as_str()
            .map(str::to_string),
        // Windows images are recognised by `osType` even when the offer
        // (`WindowsServer`) is missing from the image reference.
        os: match os_type {
            Some(t) if t.eq_ignore_ascii_case("windows") => Some("Windows".into()),
            _ => os,
        },
        os_name: None,
    })
}

async fn token(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    cfg: &AzureConfig,
) -> Result<Zeroizing<String>, CloudError> {
    let scope = format!("{}/.default", endpoints.azure_management());
    let resp = http
        .post(format!(
            "{}/{}/oauth2/v2.0/token",
            endpoints.azure_login(),
            cfg.tenant_id.trim()
        ))
        .form(&[
            ("client_id", cfg.client_id.trim()),
            ("client_secret", cfg.client_secret.trim()),
            ("scope", scope.as_str()),
            ("grant_type", "client_credentials"),
        ])
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(token_error(status, &text));
    }
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| CloudError::Malformed(format!("Azure token: {e}")))?;
    v["access_token"]
        .as_str()
        .filter(|t| !t.is_empty())
        .map(|t| Zeroizing::new(t.to_string()))
        .ok_or_else(|| CloudError::Malformed("Azure token response has no access_token".into()))
}

/// Azure AD returns `{"error": "invalid_client", "error_description": "AADSTS7000215: …"}`.
fn token_error(status: reqwest::StatusCode, body: &str) -> CloudError {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body)
        && let Some(code) = v["error"].as_str()
    {
        let description = v["error_description"]
            .as_str()
            .map(|d| d.lines().next().unwrap_or(d).trim().to_string())
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| code.to_string());
        return match code {
            "invalid_client" | "unauthorized_client" | "invalid_grant" => {
                CloudError::InvalidCredentials(description)
            }
            "invalid_request" if description.contains("AADSTS90002") => {
                CloudError::InvalidCredentials(description)
            }
            "temporarily_unavailable" | "server_error" => CloudError::Unavailable(description),
            _ => CloudError::Malformed(format!("{code}: {description}")),
        };
    }
    status_error(status, body)
}

/// Azure Resource Manager caller: bearer token, `{"error":{code,message}}`
/// envelopes and `nextLink` paging.
#[derive(Clone)]
struct Arm<'a> {
    http: &'a reqwest::Client,
    mgmt: &'a str,
    token: &'a str,
}

impl Arm<'_> {
    async fn get(&self, url: &str) -> Result<serde_json::Value, CloudError> {
        let resp = self
            .http
            .get(url)
            .bearer_auth(self.token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(arm_error(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| CloudError::Malformed(format!("Azure: {e}")))
    }

    /// Follow `nextLink` and concatenate `value`.
    async fn list(&self, url: &str) -> Result<Vec<serde_json::Value>, CloudError> {
        let mut out = Vec::new();
        let mut next = Some(url.to_string());
        let mut pages = 0;
        while let Some(u) = next.take() {
            pages += 1;
            if pages > MAX_PAGES {
                break;
            }
            let v = self.get(&u).await?;
            let Some(values) = v["value"].as_array() else {
                return Err(CloudError::Malformed(
                    "Azure: list response has no `value`".into(),
                ));
            };
            out.extend(values.iter().cloned());
            next = v["nextLink"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| self.rebase(s));
        }
        Ok(out)
    }

    /// `nextLink` is absolute against the public ARM host; keep custom
    /// endpoints (tests, sovereign clouds) working by swapping the origin.
    fn rebase(&self, link: &str) -> String {
        match (url::Url::parse(link), url::Url::parse(self.mgmt)) {
            (Ok(l), Ok(m)) if l.host_str() != m.host_str() || l.port() != m.port() => {
                format!(
                    "{}{}{}",
                    self.mgmt,
                    l.path(),
                    l.query().map(|q| format!("?{q}")).unwrap_or_default()
                )
            }
            _ => link.to_string(),
        }
    }
}

fn arm_error(status: reqwest::StatusCode, body: &str) -> CloudError {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body)
        && let Some(code) = v["error"]["code"].as_str()
    {
        let message = v["error"]["message"]
            .as_str()
            .map(str::to_string)
            .or_else(|| trimmed(body))
            .unwrap_or_else(|| code.to_string());
        return match (code, status.as_u16()) {
            ("InvalidAuthenticationToken" | "ExpiredAuthenticationToken", _) | (_, 401) => {
                CloudError::InvalidCredentials(message)
            }
            ("AuthorizationFailed", _) | (_, 403) => CloudError::Forbidden(message),
            ("TooManyRequests", _) | (_, 429) => CloudError::RateLimited(message),
            (_, 500..=599) => CloudError::Unavailable(message),
            _ => CloudError::Malformed(format!("{code}: {message}")),
        };
    }
    status_error(status, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn vm(name: &str, nic: &str, os: (&str, &str, &str)) -> serde_json::Value {
        serde_json::json!({
            "id": format!("/subscriptions/s1/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/{name}"),
            "name": name,
            "location": "westeurope",
            "properties": {
                "provisioningState": "Succeeded",
                "hardwareProfile": {"vmSize": "Standard_B1s"},
                "storageProfile": {
                    "osDisk": {"osType": os.0},
                    "imageReference": {"publisher": "x", "offer": os.1, "sku": os.2}
                },
                "networkProfile": {"networkInterfaces": [{"id": nic, "properties": {"primary": true}}]}
            }
        })
    }

    #[test]
    fn vm_maps_fields() {
        let v = vm(
            "web",
            "/nic1",
            ("Linux", "0001-com-ubuntu-server-jammy", "22_04-lts"),
        );
        let i = parse_vm(&v, Some("20.1.2.3".into())).unwrap();
        assert_eq!(i.label, "web");
        assert_eq!(i.address.as_deref(), Some("20.1.2.3"));
        assert_eq!(i.region.as_deref(), Some("westeurope"));
        assert_eq!(i.size.as_deref(), Some("Standard_B1s"));
        assert_eq!(
            i.os.as_deref(),
            Some("0001-com-ubuntu-server-jammy 22_04-lts")
        );
        assert_eq!(primary_nic_id(&v).as_deref(), Some("/nic1"));

        let w = vm("win", "/nic2", ("Windows", "WindowsServer", "2022"));
        assert_eq!(parse_vm(&w, None).unwrap().os.as_deref(), Some("Windows"));
    }

    #[test]
    fn nic_prefers_primary_config() {
        let nic = serde_json::json!({"properties": {"ipConfigurations": [
            {"properties": {"primary": false, "privateIPAddress": "10.0.0.5"}},
            {"properties": {"primary": true, "privateIPAddress": "10.0.0.4",
                            "publicIPAddress": {"id": "/pip1"}}}
        ]}});
        assert_eq!(
            nic_addresses(&nic),
            Some((Some("10.0.0.4".into()), Some("/pip1".into())))
        );
    }

    #[test]
    fn token_errors_map() {
        let e = token_error(
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"error":"invalid_client","error_description":"AADSTS7000215: Invalid client secret provided.\r\nTrace ID: x"}"#,
        );
        assert_eq!(
            e,
            CloudError::InvalidCredentials("AADSTS7000215: Invalid client secret provided.".into())
        );
        let e = token_error(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"error":"invalid_request","error_description":"AADSTS90002: Tenant 'nope' not found."}"#,
        );
        assert!(matches!(e, CloudError::InvalidCredentials(_)));
    }

    #[test]
    fn arm_errors_map() {
        let e = arm_error(
            reqwest::StatusCode::FORBIDDEN,
            r#"{"error":{"code":"AuthorizationFailed","message":"The client does not have authorization"}}"#,
        );
        assert!(matches!(e, CloudError::Forbidden(_)));
        let e = arm_error(
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            r#"{"error":{"code":"TooManyRequests","message":"slow"}}"#,
        );
        assert!(matches!(e, CloudError::RateLimited(_)));
    }

    #[test]
    fn rebase_swaps_origin() {
        let http = reqwest::Client::new();
        let arm = Arm {
            http: &http,
            mgmt: "http://127.0.0.1:9999/arm",
            token: "t",
        };
        assert_eq!(
            arm.rebase("https://management.azure.com/subscriptions?api-version=1&$skiptoken=abc"),
            "http://127.0.0.1:9999/arm/subscriptions?api-version=1&$skiptoken=abc"
        );
        assert_eq!(
            arm.rebase("http://127.0.0.1:9999/arm/x?y=1"),
            "http://127.0.0.1:9999/arm/x?y=1"
        );
    }
}
