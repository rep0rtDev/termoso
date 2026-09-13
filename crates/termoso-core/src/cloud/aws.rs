//! AWS: EC2 `DescribeInstances` (Query API, XML) and Lightsail
//! `GetInstances` (JSON 1.1), both signed with SigV4.

use chrono::Utc;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use url::Url;

use super::sigv4;
use super::xml::{self, Node};
use super::{AddressType, AwsConfig, CloudError, CloudInstance, Endpoints, status_error};

const EC2_VERSION: &str = "2016-11-15";
const LIGHTSAIL_TARGET: &str = "Lightsail_20161128.GetInstances";
const MAX_PAGES: usize = 50;

pub(super) async fn ec2_instances(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    cfg: &AwsConfig,
) -> Result<Vec<CloudInstance>, CloudError> {
    let base = endpoints.aws_ec2(cfg.region.trim());
    let mut out = Vec::new();
    let mut next: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut body = format!("Action=DescribeInstances&Version={EC2_VERSION}&MaxResults=1000");
        if let Some(t) = &next {
            body.push_str("&NextToken=");
            body.push_str(&form_encode(t));
        }
        let text = signed_post(
            http,
            &base,
            "ec2",
            cfg,
            &[(
                "content-type",
                "application/x-www-form-urlencoded; charset=utf-8",
            )],
            body.into_bytes(),
            ec2_error,
        )
        .await?;
        let root = xml::parse(&text).map_err(|e| CloudError::Malformed(format!("EC2: {e}")))?;
        if root.name != "DescribeInstancesResponse" {
            return Err(CloudError::Malformed(format!(
                "EC2: unexpected <{}> response",
                root.name
            )));
        }
        out.extend(parse_ec2(&root, cfg.address_type));
        next = root.text_of("nextToken").map(str::to_string);
        if next.is_none() {
            break;
        }
    }
    Ok(out)
}

/// Instances of one `DescribeInstancesResponse` page. Terminated instances
/// are gone for good and carry no address; they are dropped.
pub(super) fn parse_ec2(root: &Node, address_type: AddressType) -> Vec<CloudInstance> {
    let mut out = Vec::new();
    let Some(reservations) = root.child("reservationSet") else {
        return out;
    };
    for reservation in reservations.children("item") {
        let Some(instances) = reservation.child("instancesSet") else {
            continue;
        };
        for inst in instances.children("item") {
            let Some(id) = inst.text_of("instanceId") else {
                continue;
            };
            let state = inst
                .path(&["instanceState", "name"])
                .map(|n| n.text.clone());
            if matches!(state.as_deref(), Some("terminated") | Some("shutting-down")) {
                continue;
            }
            let address = match address_type {
                AddressType::Public => inst
                    .text_of("ipAddress")
                    .or_else(|| inst.text_of("ipv6Address")),
                AddressType::Private => inst.text_of("privateIpAddress"),
            }
            .map(str::to_string);
            let label = inst
                .child("tagSet")
                .and_then(|tags| {
                    tags.children("item").find_map(|t| {
                        let key = t.text_of("key")?.to_ascii_lowercase();
                        matches!(key.as_str(), "name" | "label" | "title")
                            .then(|| t.text_of("value"))
                            .flatten()
                    })
                })
                .unwrap_or("")
                .to_string();
            let os = inst
                .text_of("platform")
                .filter(|p| p.eq_ignore_ascii_case("windows"))
                .or_else(|| inst.text_of("platformDetails"))
                .map(str::to_string);
            out.push(CloudInstance {
                instance_id: id.to_string(),
                label,
                address,
                state,
                region: inst
                    .path(&["placement", "availabilityZone"])
                    .map(|n| n.text.clone()),
                size: inst.text_of("instanceType").map(str::to_string),
                os,
                os_name: None,
            });
        }
    }
    out
}

fn ec2_error(status: reqwest::StatusCode, body: &str) -> CloudError {
    if let Ok(root) = xml::parse(body)
        && let Some(err) = root
            .path(&["Errors", "Error"])
            .or_else(|| root.child("Error"))
    {
        let code = err.text_of("Code").unwrap_or("");
        let message = err
            .text_of("Message")
            .map(str::to_string)
            .unwrap_or_else(|| format!("AWS error {code}"));
        return aws_code_error(code, message, status);
    }
    status_error(status, body)
}

/// Map an AWS error code (shared vocabulary across EC2 and Lightsail).
fn aws_code_error(code: &str, message: String, status: reqwest::StatusCode) -> CloudError {
    match code {
        "AuthFailure"
        | "InvalidClientTokenId"
        | "SignatureDoesNotMatch"
        | "UnrecognizedClientException"
        | "InvalidSignatureException"
        | "UnauthenticatedException"
        | "IncompleteSignature"
        | "MissingAuthenticationToken"
        | "ExpiredToken" => CloudError::InvalidCredentials(message),
        "UnauthorizedOperation" | "AccessDeniedException" | "AccessDenied" | "OptInRequired" => {
            CloudError::Forbidden(message)
        }
        "RequestLimitExceeded" | "Throttling" | "ThrottlingException" => {
            CloudError::RateLimited(message)
        }
        "" => status_error(status, &message),
        _ if status.is_server_error() => CloudError::Unavailable(message),
        _ => CloudError::Malformed(format!("{code}: {message}")),
    }
}

pub(super) async fn lightsail_instances(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    cfg: &AwsConfig,
) -> Result<Vec<CloudInstance>, CloudError> {
    let base = endpoints.aws_lightsail(cfg.region.trim());
    let mut out = Vec::new();
    let mut next: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let body = match &next {
            Some(t) => serde_json::json!({ "pageToken": t }),
            None => serde_json::json!({}),
        };
        let text = signed_post(
            http,
            &base,
            "lightsail",
            cfg,
            &[
                ("content-type", "application/x-amz-json-1.1"),
                ("x-amz-target", LIGHTSAIL_TARGET),
            ],
            serde_json::to_vec(&body).expect("json"),
            lightsail_error,
        )
        .await?;
        let page: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CloudError::Malformed(format!("Lightsail: {e}")))?;
        out.extend(parse_lightsail(&page, cfg.address_type));
        next = page["nextPageToken"].as_str().map(str::to_string);
        if next.is_none() {
            break;
        }
    }
    Ok(out)
}

/// Instances of one `GetInstances` page.
pub(super) fn parse_lightsail(
    page: &serde_json::Value,
    address_type: AddressType,
) -> Vec<CloudInstance> {
    let Some(list) = page["instances"].as_array() else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|i| {
            let name = i["name"].as_str().unwrap_or("").to_string();
            let id = i["arn"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(name.as_str())
                .to_string();
            if id.is_empty() {
                return None;
            }
            let address = match address_type {
                AddressType::Public => i["publicIpAddress"].as_str(),
                AddressType::Private => i["privateIpAddress"].as_str(),
            }
            .map(str::to_string);
            Some(CloudInstance {
                instance_id: id,
                label: name,
                address,
                state: i["state"]["name"].as_str().map(str::to_string),
                region: i["location"]["availabilityZone"]
                    .as_str()
                    .or_else(|| i["location"]["regionName"].as_str())
                    .map(str::to_string),
                size: i["bundleId"].as_str().map(str::to_string),
                os: i["blueprintName"]
                    .as_str()
                    .or_else(|| i["blueprintId"].as_str())
                    .map(str::to_string),
                os_name: None,
            })
        })
        .collect()
}

fn lightsail_error(status: reqwest::StatusCode, body: &str) -> CloudError {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        let code = v["__type"]
            .as_str()
            .or_else(|| v["code"].as_str())
            .unwrap_or("");
        // `com.amazonaws.lightsail#AccessDeniedException` → the bare name.
        let code = code.rsplit(['#', ':']).next().unwrap_or(code);
        let message = v["message"]
            .as_str()
            .or_else(|| v["Message"].as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("AWS error {code}"));
        return aws_code_error(code, message, status);
    }
    status_error(status, body)
}

/// POST `body` to `base` signed for `service`, returning the body text on
/// 2xx and the mapped error otherwise.
async fn signed_post(
    http: &reqwest::Client,
    base: &str,
    service: &str,
    cfg: &AwsConfig,
    extra_headers: &[(&str, &str)],
    body: Vec<u8>,
    on_error: fn(reqwest::StatusCode, &str) -> CloudError,
) -> Result<String, CloudError> {
    let url = Url::parse(base).map_err(|e| CloudError::Invalid(format!("endpoint: {e}")))?;
    let host = match (url.host_str(), url.port()) {
        (Some(h), Some(p)) => format!("{h}:{p}"),
        (Some(h), None) => h.to_string(),
        _ => return Err(CloudError::Invalid("endpoint has no host".into())),
    };
    let path = if url.path().is_empty() {
        "/"
    } else {
        url.path()
    };
    let signed = sigv4::sign(
        &sigv4::Request {
            service,
            region: cfg.region.trim(),
            host: &host,
            path,
            headers: extra_headers,
            body: &body,
        },
        cfg.access_key_id.trim(),
        cfg.secret_access_key.trim(),
        Utc::now(),
    );
    let mut headers = HeaderMap::new();
    for (k, v) in extra_headers {
        let name: reqwest::header::HeaderName = k
            .parse()
            .map_err(|_| CloudError::Invalid(format!("header {k}")))?;
        headers.insert(name, HeaderValue::from_str(v).expect("ascii header"));
    }
    headers.insert(
        "x-amz-date",
        HeaderValue::from_str(&signed.amz_date).expect("ascii"),
    );
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&signed.authorization)
            .map_err(|_| CloudError::Invalid("access key id contains invalid characters".into()))?,
    );
    if !headers.contains_key(CONTENT_TYPE) {
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
    }
    let resp = http.post(url).headers(headers).body(body).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if status.is_success() {
        Ok(text)
    } else {
        Err(on_error(status, &text))
    }
}

fn form_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const EC2_PAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DescribeInstancesResponse xmlns="http://ec2.amazonaws.com/doc/2016-11-15/">
  <requestId>7e2c…</requestId>
  <reservationSet>
    <item>
      <reservationId>r-1</reservationId>
      <instancesSet>
        <item>
          <instanceId>i-0aaa</instanceId>
          <instanceState><code>16</code><name>running</name></instanceState>
          <privateIpAddress>10.0.1.10</privateIpAddress>
          <ipAddress>54.10.20.30</ipAddress>
          <instanceType>t3.micro</instanceType>
          <placement><availabilityZone>eu-central-1a</availabilityZone></placement>
          <platformDetails>Linux/UNIX</platformDetails>
          <tagSet>
            <item><key>env</key><value>prod</value></item>
            <item><key>Name</key><value>web-1 &amp; co</value></item>
          </tagSet>
        </item>
        <item>
          <instanceId>i-0bbb</instanceId>
          <instanceState><code>80</code><name>stopped</name></instanceState>
          <privateIpAddress>10.0.1.11</privateIpAddress>
          <instanceType>t3.small</instanceType>
          <platform>windows</platform>
          <platformDetails>Windows</platformDetails>
        </item>
        <item>
          <instanceId>i-0ccc</instanceId>
          <instanceState><code>48</code><name>terminated</name></instanceState>
        </item>
      </instancesSet>
    </item>
  </reservationSet>
</DescribeInstancesResponse>"#;

    #[test]
    fn ec2_page_maps_instances() {
        let root = xml::parse(EC2_PAGE).unwrap();
        let list = parse_ec2(&root, AddressType::Public);
        assert_eq!(list.len(), 2, "terminated instance dropped");
        let web = &list[0];
        assert_eq!(web.instance_id, "i-0aaa");
        assert_eq!(web.label, "web-1 & co");
        assert_eq!(web.address.as_deref(), Some("54.10.20.30"));
        assert_eq!(web.state.as_deref(), Some("running"));
        assert_eq!(web.region.as_deref(), Some("eu-central-1a"));
        assert_eq!(web.size.as_deref(), Some("t3.micro"));
        assert_eq!(web.os.as_deref(), Some("Linux/UNIX"));
        let win = &list[1];
        assert_eq!(win.address, None, "stopped instance has no public ip");
        assert_eq!(win.os.as_deref(), Some("windows"));
        assert_eq!(win.label, "", "no Name tag");

        let private = parse_ec2(&root, AddressType::Private);
        assert_eq!(private[0].address.as_deref(), Some("10.0.1.10"));
        assert_eq!(private[1].address.as_deref(), Some("10.0.1.11"));
    }

    #[test]
    fn ec2_errors_map_by_code() {
        let body = r#"<Response><Errors><Error><Code>AuthFailure</Code><Message>AWS was not able to validate the provided access credentials</Message></Error></Errors><RequestID>x</RequestID></Response>"#;
        assert_eq!(
            ec2_error(reqwest::StatusCode::UNAUTHORIZED, body),
            CloudError::InvalidCredentials(
                "AWS was not able to validate the provided access credentials".into()
            )
        );
        let body = r#"<Response><Errors><Error><Code>UnauthorizedOperation</Code><Message>You are not authorized to perform this operation.</Message></Error></Errors></Response>"#;
        assert!(matches!(
            ec2_error(reqwest::StatusCode::FORBIDDEN, body),
            CloudError::Forbidden(_)
        ));
        let body = r#"<Response><Errors><Error><Code>RequestLimitExceeded</Code><Message>slow down</Message></Error></Errors></Response>"#;
        assert!(matches!(
            ec2_error(reqwest::StatusCode::BAD_REQUEST, body),
            CloudError::RateLimited(_)
        ));
        assert!(matches!(
            ec2_error(reqwest::StatusCode::BAD_GATEWAY, "<html>oops</html>"),
            CloudError::Unavailable(_)
        ));
    }

    #[test]
    fn lightsail_page_maps_instances() {
        let page = serde_json::json!({
            "instances": [{
                "name": "WordPress-1",
                "arn": "arn:aws:lightsail:eu-central-1:1:Instance/abc",
                "location": {"availabilityZone": "eu-central-1a", "regionName": "eu-central-1"},
                "blueprintId": "wordpress",
                "blueprintName": "WordPress",
                "bundleId": "nano_2_0",
                "publicIpAddress": "3.1.2.3",
                "privateIpAddress": "172.26.0.1",
                "state": {"code": 16, "name": "running"}
            }, {
                "name": "Debian-1",
                "arn": "arn:aws:lightsail:eu-central-1:1:Instance/def",
                "blueprintName": "Debian",
                "privateIpAddress": "172.26.0.2",
                "state": {"name": "stopped"}
            }],
            "nextPageToken": "t2"
        });
        let list = parse_lightsail(&page, AddressType::Public);
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[0].instance_id,
            "arn:aws:lightsail:eu-central-1:1:Instance/abc"
        );
        assert_eq!(list[0].label, "WordPress-1");
        assert_eq!(list[0].address.as_deref(), Some("3.1.2.3"));
        assert_eq!(list[0].size.as_deref(), Some("nano_2_0"));
        assert_eq!(list[1].address, None);
        assert_eq!(list[1].os.as_deref(), Some("Debian"));
    }

    #[test]
    fn lightsail_errors_map_by_type() {
        let e = lightsail_error(
            reqwest::StatusCode::FORBIDDEN,
            r#"{"__type":"com.amazonaws.lightsail#AccessDeniedException","message":"no"}"#,
        );
        assert_eq!(e, CloudError::Forbidden("no".into()));
        let e = lightsail_error(
            reqwest::StatusCode::FORBIDDEN,
            r#"{"__type":"UnrecognizedClientException","message":"The security token included in the request is invalid."}"#,
        );
        assert!(matches!(e, CloudError::InvalidCredentials(_)));
        let e = lightsail_error(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"__type":"ThrottlingException","message":"Rate exceeded"}"#,
        );
        assert!(matches!(e, CloudError::RateLimited(_)));
    }
}
