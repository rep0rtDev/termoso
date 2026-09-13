//! DigitalOcean API v2: `GET /v2/droplets`.

use super::{CloudError, CloudInstance, DigitalOceanConfig, Endpoints, status_error};

const PER_PAGE: u32 = 200;
const MAX_PAGES: u32 = 50;

pub(super) async fn droplets(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    cfg: &DigitalOceanConfig,
) -> Result<Vec<CloudInstance>, CloudError> {
    let base = endpoints.digitalocean();
    let mut out = Vec::new();
    for page in 1..=MAX_PAGES {
        let resp = http
            .get(format!(
                "{base}/v2/droplets?per_page={PER_PAGE}&page={page}"
            ))
            .bearer_auth(cfg.token.trim())
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(do_error(status, &text));
        }
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CloudError::Malformed(format!("DigitalOcean: {e}")))?;
        let Some(list) = v["droplets"].as_array() else {
            return Err(CloudError::Malformed(
                "DigitalOcean: response has no `droplets`".into(),
            ));
        };
        out.extend(list.iter().filter_map(parse_droplet));
        let has_next = v["links"]["pages"]["next"]
            .as_str()
            .is_some_and(|s| !s.is_empty());
        if !has_next || list.is_empty() {
            break;
        }
    }
    Ok(out)
}

/// One droplet object. Archived droplets are history; skipped.
pub(super) fn parse_droplet(d: &serde_json::Value) -> Option<CloudInstance> {
    let id = match &d["id"] {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) if !s.is_empty() => s.clone(),
        _ => return None,
    };
    let status = d["status"].as_str().map(str::to_string);
    if status.as_deref() == Some("archive") {
        return None;
    }
    let nets = &d["networks"];
    let v4 = nets["v4"].as_array().cloned().unwrap_or_default();
    let v6 = nets["v6"].as_array().cloned().unwrap_or_default();
    let ip = |n: &serde_json::Value| n["ip_address"].as_str().map(str::to_string);
    let address = v4
        .iter()
        .chain(v6.iter())
        .find(|n| n["type"].as_str() == Some("public"))
        .and_then(ip)
        .or_else(|| v4.first().and_then(ip))
        .or_else(|| v6.first().and_then(ip));
    let image = &d["image"];
    let os = match (image["distribution"].as_str(), image["name"].as_str()) {
        (Some(dist), Some(name)) if !dist.is_empty() => Some(format!("{dist} {name}")),
        (Some(dist), None) if !dist.is_empty() => Some(dist.to_string()),
        (_, Some(name)) => Some(name.to_string()),
        _ => None,
    };
    Some(CloudInstance {
        instance_id: id,
        label: d["name"].as_str().unwrap_or("").to_string(),
        address,
        state: status,
        region: d["region"]["slug"]
            .as_str()
            .or_else(|| d["region"]["name"].as_str())
            .map(str::to_string),
        size: d["size_slug"]
            .as_str()
            .or_else(|| d["size"]["slug"].as_str())
            .map(str::to_string),
        os,
        os_name: None,
    })
}

fn do_error(status: reqwest::StatusCode, body: &str) -> CloudError {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body)
        && let Some(message) = v["message"].as_str()
    {
        let id = v["id"].as_str().unwrap_or("");
        return match (id, status.as_u16()) {
            ("Unauthorized" | "unauthorized", _) | (_, 401) => {
                CloudError::InvalidCredentials(message.to_string())
            }
            ("forbidden" | "Forbidden", _) | (_, 403) => CloudError::Forbidden(message.to_string()),
            ("too_many_requests", _) | (_, 429) => CloudError::RateLimited(message.to_string()),
            (_, 500..=599) => CloudError::Unavailable(message.to_string()),
            _ => CloudError::Malformed(format!("{id}: {message}")),
        };
    }
    status_error(status, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn page() -> serde_json::Value {
        serde_json::json!({
            "droplets": [{
                "id": 3164444,
                "name": "example.com",
                "status": "active",
                "region": {"slug": "nyc3", "name": "New York 3"},
                "size_slug": "s-1vcpu-1gb",
                "image": {"distribution": "Ubuntu", "name": "22.04 (LTS) x64"},
                "networks": {
                    "v4": [
                        {"ip_address": "10.132.0.5", "type": "private"},
                        {"ip_address": "104.131.186.241", "type": "public"}
                    ],
                    "v6": [{"ip_address": "2604:a880:800:10::1", "type": "public"}]
                }
            }, {
                "id": 3164445,
                "name": "private-only",
                "status": "off",
                "region": {"slug": "fra1"},
                "size_slug": "s-2vcpu-2gb",
                "image": {"distribution": "Debian", "name": "12 x64"},
                "networks": {"v4": [{"ip_address": "10.0.0.7", "type": "private"}]}
            }, {
                "id": 3164446,
                "name": "old",
                "status": "archive",
                "networks": {}
            }],
            "links": {"pages": {}},
            "meta": {"total": 3}
        })
    }

    #[test]
    fn droplets_map_public_ip_first() {
        let p = page();
        let list: Vec<_> = p["droplets"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(parse_droplet)
            .collect();
        assert_eq!(list.len(), 2, "archived droplet skipped");
        assert_eq!(list[0].instance_id, "3164444");
        assert_eq!(list[0].label, "example.com");
        assert_eq!(list[0].address.as_deref(), Some("104.131.186.241"));
        assert_eq!(list[0].os.as_deref(), Some("Ubuntu 22.04 (LTS) x64"));
        assert_eq!(list[0].region.as_deref(), Some("nyc3"));
        assert_eq!(
            list[1].address.as_deref(),
            Some("10.0.0.7"),
            "falls back to first v4"
        );
        assert_eq!(list[1].state.as_deref(), Some("off"));
    }

    #[test]
    fn errors_map() {
        let e = do_error(
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"id":"Unauthorized","message":"Unable to authenticate you"}"#,
        );
        assert_eq!(
            e,
            CloudError::InvalidCredentials("Unable to authenticate you".into())
        );
        let e = do_error(
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            r#"{"id":"too_many_requests","message":"API Rate limit exceeded."}"#,
        );
        assert!(matches!(e, CloudError::RateLimited(_)));
        assert!(matches!(
            do_error(reqwest::StatusCode::SERVICE_UNAVAILABLE, ""),
            CloudError::Unavailable(_)
        ));
    }
}
