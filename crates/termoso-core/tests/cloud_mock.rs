//! Cloud discovery against an in-process mock of each provider API:
//! authentication headers are checked, paging is exercised and error bodies
//! are mapped to typed errors.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use serde_json::json;
use termoso_core::cloud::{
    AddressType, AwsConfig, AwsService, AzureConfig, CloudClient, CloudConfig, CloudError,
    DigitalOceanConfig, Endpoints,
};
use zeroize::Zeroizing;

const DO_TOKEN: &str = "dop_v1_validtoken";
const AZ_SECRET: &str = "az-secret";
const AZ_TOKEN: &str = "eyJ.mock.token";

#[derive(Default)]
struct Hits {
    ec2: AtomicUsize,
    droplets: AtomicUsize,
}

async fn spawn() -> (SocketAddr, Arc<Hits>) {
    let hits = Arc::new(Hits::default());
    let app = Router::new()
        .route("/aws/ec2/", post(ec2))
        .route("/aws/lightsail/", post(lightsail))
        .route("/do/v2/droplets", get(droplets))
        .route("/az/login/{tenant}/oauth2/v2.0/token", post(az_token))
        .route("/az/arm/subscriptions", get(az_subscriptions))
        .route(
            "/az/arm/subscriptions/{sub}/providers/Microsoft.Compute/virtualMachines",
            get(az_vms),
        )
        .route(
            "/az/arm/subscriptions/{sub}/resourceGroups/{rg}/providers/Microsoft.Network/networkInterfaces/{nic}",
            get(az_nic),
        )
        .route(
            "/az/arm/subscriptions/{sub}/resourceGroups/{rg}/providers/Microsoft.Network/publicIPAddresses/{pip}",
            get(az_pip),
        )
        .with_state(hits.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (addr, hits)
}

fn endpoints(addr: SocketAddr) -> Endpoints {
    Endpoints {
        aws_ec2: Some(format!("http://{addr}/aws/ec2/")),
        aws_lightsail: Some(format!("http://{addr}/aws/lightsail/")),
        digitalocean: Some(format!("http://{addr}/do")),
        azure_login: Some(format!("http://{addr}/az/login")),
        azure_management: Some(format!("http://{addr}/az/arm")),
    }
}

// ---- AWS -----------------------------------------------------------------

fn ec2_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        [("content-type", "text/xml")],
        format!(
            "<Response><Errors><Error><Code>{code}</Code><Message>{message}</Message></Error></Errors><RequestID>r</RequestID></Response>"
        ),
    )
        .into_response()
}

/// Checks the SigV4 envelope (we cannot verify the signature without the
/// secret; `AKIABAD` simulates a rejected key) and pages twice.
async fn ec2(State(hits): State<Arc<Hits>>, headers: HeaderMap, body: String) -> Response {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !auth.starts_with("AWS4-HMAC-SHA256 Credential=")
        || !auth.contains("/eu-central-1/ec2/aws4_request")
        || !auth.contains("SignedHeaders=content-type;host;x-amz-date")
        || headers.get("x-amz-date").is_none()
    {
        return ec2_error(
            StatusCode::BAD_REQUEST,
            "IncompleteSignature",
            "missing sigv4 pieces",
        );
    }
    if auth.contains("Credential=AKIABAD/") {
        return ec2_error(
            StatusCode::UNAUTHORIZED,
            "AuthFailure",
            "AWS was not able to validate the provided access credentials",
        );
    }
    if auth.contains("Credential=AKIADENIED/") {
        return ec2_error(
            StatusCode::FORBIDDEN,
            "UnauthorizedOperation",
            "You are not authorized to perform this operation.",
        );
    }
    assert!(body.contains("Action=DescribeInstances"));
    hits.ec2.fetch_add(1, Ordering::SeqCst);
    let page2 = body.contains("NextToken=page2");
    let xml = if page2 {
        r#"<DescribeInstancesResponse xmlns="http://ec2.amazonaws.com/doc/2016-11-15/">
<reservationSet><item><instancesSet>
<item><instanceId>i-second</instanceId><instanceState><name>running</name></instanceState>
<ipAddress>18.0.0.2</ipAddress><privateIpAddress>10.0.0.2</privateIpAddress>
<instanceType>t3.small</instanceType><platformDetails>Linux/UNIX</platformDetails>
<tagSet><item><key>Name</key><value>db-1</value></item></tagSet></item>
</instancesSet></item></reservationSet></DescribeInstancesResponse>"#
            .to_string()
    } else {
        r#"<DescribeInstancesResponse xmlns="http://ec2.amazonaws.com/doc/2016-11-15/">
<reservationSet><item><instancesSet>
<item><instanceId>i-first</instanceId><instanceState><name>running</name></instanceState>
<ipAddress>18.0.0.1</ipAddress><privateIpAddress>10.0.0.1</privateIpAddress>
<instanceType>t3.micro</instanceType><platformDetails>Linux/UNIX</platformDetails>
<placement><availabilityZone>eu-central-1a</availabilityZone></placement>
<tagSet><item><key>Name</key><value>web-1</value></item></tagSet></item>
<item><instanceId>i-gone</instanceId><instanceState><name>terminated</name></instanceState></item>
<item><instanceId>i-stopped</instanceId><instanceState><name>stopped</name></instanceState>
<privateIpAddress>10.0.0.9</privateIpAddress><platform>windows</platform></item>
</instancesSet></item></reservationSet><nextToken>page2</nextToken></DescribeInstancesResponse>"#
            .to_string()
    };
    ([("content-type", "text/xml")], xml).into_response()
}

async fn lightsail(headers: HeaderMap, body: Bytes) -> Response {
    assert_eq!(
        headers.get("x-amz-target").unwrap(),
        "Lightsail_20161128.GetInstances"
    );
    let auth = headers["authorization"].to_str().unwrap();
    assert!(auth.contains("/eu-central-1/lightsail/aws4_request"));
    let req: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let page = if req["pageToken"].as_str() == Some("p2") {
        json!({"instances": [{
            "name": "ls-2", "arn": "arn:ls:2", "publicIpAddress": "3.0.0.2",
            "blueprintName": "Debian", "bundleId": "nano_3_0", "state": {"name": "running"},
            "location": {"availabilityZone": "eu-central-1b"}
        }]})
    } else {
        json!({"instances": [{
            "name": "ls-1", "arn": "arn:ls:1", "publicIpAddress": "3.0.0.1",
            "blueprintName": "Ubuntu", "bundleId": "nano_3_0", "state": {"name": "running"},
            "location": {"availabilityZone": "eu-central-1a"}
        }], "nextPageToken": "p2"})
    };
    axum::Json(page).into_response()
}

// ---- DigitalOcean --------------------------------------------------------

#[derive(serde::Deserialize)]
struct PageQuery {
    page: Option<u32>,
    per_page: Option<u32>,
}

async fn droplets(
    State(hits): State<Arc<Hits>>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Response {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if auth != format!("Bearer {DO_TOKEN}") {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({"id": "Unauthorized", "message": "Unable to authenticate you"})),
        )
            .into_response();
    }
    assert_eq!(q.per_page, Some(200));
    hits.droplets.fetch_add(1, Ordering::SeqCst);
    let body = match q.page.unwrap_or(1) {
        1 => json!({
            "droplets": [{
                "id": 101, "name": "web", "status": "active",
                "region": {"slug": "fra1"}, "size_slug": "s-1vcpu-1gb",
                "image": {"distribution": "Ubuntu", "name": "24.04 x64"},
                "networks": {"v4": [
                    {"ip_address": "10.0.0.1", "type": "private"},
                    {"ip_address": "104.0.0.1", "type": "public"}
                ]}
            }],
            "links": {"pages": {"next": "https://api.digitalocean.com/v2/droplets?page=2&per_page=200"}}
        }),
        _ => json!({
            "droplets": [{
                "id": 102, "name": "db", "status": "off",
                "region": {"slug": "fra1"}, "size_slug": "s-2vcpu-4gb",
                "image": {"distribution": "Debian", "name": "12 x64"},
                "networks": {"v4": [{"ip_address": "104.0.0.2", "type": "public"}]}
            }],
            "links": {"pages": {}}
        }),
    };
    axum::Json(body).into_response()
}

// ---- Azure ---------------------------------------------------------------

#[derive(serde::Deserialize)]
struct TokenForm {
    client_id: String,
    client_secret: String,
    scope: String,
    grant_type: String,
}

async fn az_token(Path(tenant): Path<String>, Form(f): Form<TokenForm>) -> Response {
    assert_eq!(f.grant_type, "client_credentials");
    assert!(f.scope.ends_with("/az/arm/.default"), "scope {}", f.scope);
    if tenant != "tenant-1" {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({"error": "invalid_request",
                "error_description": format!("AADSTS90002: Tenant '{tenant}' not found.\r\nTrace ID: x")})),
        )
            .into_response();
    }
    if f.client_id != "client-1" || f.client_secret != AZ_SECRET {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({"error": "invalid_client",
                "error_description": "AADSTS7000215: Invalid client secret provided.\r\nTrace ID: y"})),
        )
            .into_response();
    }
    axum::Json(json!({"token_type": "Bearer", "expires_in": 3599, "access_token": AZ_TOKEN}))
        .into_response()
}

fn az_auth(headers: &HeaderMap) -> Option<Response> {
    let ok = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|a| a == format!("Bearer {AZ_TOKEN}"));
    (!ok).then(|| {
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(
                json!({"error": {"code": "InvalidAuthenticationToken", "message": "bad token"}}),
            ),
        )
            .into_response()
    })
}

#[derive(serde::Deserialize)]
struct ArmQuery {
    #[serde(rename = "api-version")]
    api_version: String,
    #[serde(rename = "$skiptoken")]
    skiptoken: Option<String>,
}

async fn az_subscriptions(headers: HeaderMap, Query(q): Query<ArmQuery>) -> Response {
    if let Some(r) = az_auth(&headers) {
        return r;
    }
    assert!(!q.api_version.is_empty());
    axum::Json(json!({"value": [
        {"subscriptionId": "sub-a", "displayName": "A"},
        {"subscriptionId": "sub-b", "displayName": "B"}
    ]}))
    .into_response()
}

fn vm(sub: &str, name: &str, nic: &str, os_type: &str, offer: &str) -> serde_json::Value {
    json!({
        "id": format!("/subscriptions/{sub}/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/{name}"),
        "name": name,
        "location": "westeurope",
        "properties": {
            "provisioningState": "Succeeded",
            "hardwareProfile": {"vmSize": "Standard_B2s"},
            "storageProfile": {"osDisk": {"osType": os_type},
                "imageReference": {"offer": offer, "sku": "latest"}},
            "networkProfile": {"networkInterfaces": [{
                "id": format!("/subscriptions/{sub}/resourceGroups/rg/providers/Microsoft.Network/networkInterfaces/{nic}"),
                "properties": {"primary": true}
            }]}
        }
    })
}

async fn az_vms(
    Path(sub): Path<String>,
    headers: HeaderMap,
    Query(q): Query<ArmQuery>,
) -> Response {
    if let Some(r) = az_auth(&headers) {
        return r;
    }
    let body = match (sub.as_str(), q.skiptoken.as_deref()) {
        ("sub-a", None) => json!({
            "value": [vm("sub-a", "vm-a1", "nic-a1", "Linux", "0001-com-ubuntu-server-jammy")],
            "nextLink": format!("https://management.azure.com/subscriptions/sub-a/providers/Microsoft.Compute/virtualMachines?api-version={}&$skiptoken=t2", q.api_version)
        }),
        ("sub-a", Some("t2")) => json!({
            "value": [vm("sub-a", "vm-a2", "nic-a2", "Windows", "WindowsServer")]
        }),
        ("sub-b", _) => json!({
            "value": [vm("sub-b", "vm-b1", "nic-b1", "Linux", "debian-12")]
        }),
        _ => json!({"value": []}),
    };
    axum::Json(body).into_response()
}

async fn az_nic(
    Path((sub, _rg, nic)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    if let Some(r) = az_auth(&headers) {
        return r;
    }
    let mut cfg = json!({"properties": {"primary": true, "privateIPAddress": "10.1.0.4"}});
    // nic-b1 has no public ip.
    if nic != "nic-b1" {
        cfg["properties"]["publicIPAddress"] = json!({
            "id": format!("/subscriptions/{sub}/resourceGroups/rg/providers/Microsoft.Network/publicIPAddresses/pip-{nic}")
        });
    }
    axum::Json(json!({"properties": {"ipConfigurations": [cfg]}})).into_response()
}

async fn az_pip(
    Path((_sub, _rg, pip)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    if let Some(r) = az_auth(&headers) {
        return r;
    }
    let ip = match pip.as_str() {
        "pip-nic-a1" => "20.0.0.1",
        "pip-nic-a2" => "20.0.0.2",
        _ => "20.0.0.99",
    };
    axum::Json(json!({"properties": {"ipAddress": ip}})).into_response()
}

// ---- tests ---------------------------------------------------------------

fn aws(access: &str, address_type: AddressType, service: AwsService) -> CloudConfig {
    CloudConfig::Aws(AwsConfig {
        region: "eu-central-1".into(),
        access_key_id: access.into(),
        secret_access_key: Zeroizing::new("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into()),
        service,
        address_type,
    })
}

#[tokio::test]
async fn aws_ec2_pages_and_filters() {
    let (addr, hits) = spawn().await;
    let client = CloudClient::new(endpoints(addr));
    let list = client
        .discover(&aws("AKIAGOOD", AddressType::Public, AwsService::Ec2))
        .await
        .unwrap();
    assert_eq!(hits.ec2.load(Ordering::SeqCst), 2, "two pages fetched");
    let ids: Vec<_> = list.iter().map(|i| i.instance_id.as_str()).collect();
    assert_eq!(ids, ["i-second", "i-stopped", "i-first"], "sorted by label");
    let web = list.iter().find(|i| i.instance_id == "i-first").unwrap();
    assert_eq!(web.label, "web-1");
    assert_eq!(web.address.as_deref(), Some("18.0.0.1"));
    assert_eq!(web.os_name.as_deref(), Some("linux"));
    assert_eq!(web.region.as_deref(), Some("eu-central-1a"));
    let stopped = list.iter().find(|i| i.instance_id == "i-stopped").unwrap();
    assert_eq!(stopped.address, None);
    assert_eq!(stopped.os_name.as_deref(), Some("windows"));
    assert_eq!(stopped.label, "i-stopped", "label falls back to id");

    let private = client
        .discover(&aws("AKIAGOOD", AddressType::Private, AwsService::Ec2))
        .await
        .unwrap();
    let stopped = private
        .iter()
        .find(|i| i.instance_id == "i-stopped")
        .unwrap();
    assert_eq!(stopped.address.as_deref(), Some("10.0.0.9"));
}

#[tokio::test]
async fn aws_errors_are_typed() {
    let (addr, _) = spawn().await;
    let client = CloudClient::new(endpoints(addr));
    let err = client
        .discover(&aws("AKIABAD", AddressType::Public, AwsService::Ec2))
        .await
        .unwrap_err();
    assert_eq!(
        err,
        CloudError::InvalidCredentials(
            "AWS was not able to validate the provided access credentials".into()
        )
    );
    assert_eq!(err.kind(), "cloud_invalid_credentials");
    let err = client
        .discover(&aws("AKIADENIED", AddressType::Public, AwsService::Ec2))
        .await
        .unwrap_err();
    assert!(matches!(err, CloudError::Forbidden(_)));
}

#[tokio::test]
async fn aws_lightsail_pages() {
    let (addr, _) = spawn().await;
    let client = CloudClient::new(endpoints(addr));
    let list = client
        .discover(&aws("AKIAGOOD", AddressType::Public, AwsService::Lightsail))
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].label, "ls-1");
    assert_eq!(list[0].address.as_deref(), Some("3.0.0.1"));
    assert_eq!(list[0].os_name.as_deref(), Some("ubuntu"));
    assert_eq!(list[1].os_name.as_deref(), Some("debian"));
}

#[tokio::test]
async fn digitalocean_pages_and_auth() {
    let (addr, hits) = spawn().await;
    let client = CloudClient::new(endpoints(addr));
    let list = client
        .discover(&CloudConfig::DigitalOcean(DigitalOceanConfig {
            token: Zeroizing::new(DO_TOKEN.into()),
        }))
        .await
        .unwrap();
    assert_eq!(hits.droplets.load(Ordering::SeqCst), 2);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].label, "db");
    assert_eq!(list[0].state.as_deref(), Some("off"));
    assert_eq!(list[1].label, "web");
    assert_eq!(
        list[1].address.as_deref(),
        Some("104.0.0.1"),
        "public over private"
    );
    assert_eq!(list[1].os_name.as_deref(), Some("ubuntu"));
    assert_eq!(list[1].instance_id, "101");

    let err = client
        .discover(&CloudConfig::DigitalOcean(DigitalOceanConfig {
            token: Zeroizing::new("nope".into()),
        }))
        .await
        .unwrap_err();
    assert_eq!(
        err,
        CloudError::InvalidCredentials("Unable to authenticate you".into())
    );
}

fn azure(tenant: &str, secret: &str) -> CloudConfig {
    CloudConfig::Azure(AzureConfig {
        tenant_id: tenant.into(),
        client_id: "client-1".into(),
        client_secret: Zeroizing::new(secret.into()),
    })
}

#[tokio::test]
async fn azure_resolves_public_ips_across_subscriptions() {
    let (addr, _) = spawn().await;
    let client = CloudClient::new(endpoints(addr));
    let list = client
        .discover(&azure("tenant-1", AZ_SECRET))
        .await
        .unwrap();
    let names: Vec<_> = list.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(names, ["vm-a1", "vm-a2", "vm-b1"]);
    assert_eq!(list[0].address.as_deref(), Some("20.0.0.1"));
    assert_eq!(list[0].os_name.as_deref(), Some("ubuntu"));
    assert_eq!(list[1].address.as_deref(), Some("20.0.0.2"));
    assert_eq!(list[1].os_name.as_deref(), Some("windows"));
    assert_eq!(list[2].address, None, "no public ip");
    assert_eq!(list[2].os_name.as_deref(), Some("debian"));
    assert!(list[2].instance_id.starts_with("/subscriptions/sub-b/"));
}

#[tokio::test]
async fn azure_errors_are_typed() {
    let (addr, _) = spawn().await;
    let client = CloudClient::new(endpoints(addr));
    let err = client
        .discover(&azure("tenant-1", "wrong"))
        .await
        .unwrap_err();
    assert_eq!(
        err,
        CloudError::InvalidCredentials("AADSTS7000215: Invalid client secret provided.".into())
    );
    let err = client
        .discover(&azure("tenant-x", AZ_SECRET))
        .await
        .unwrap_err();
    assert!(matches!(err, CloudError::InvalidCredentials(m) if m.contains("AADSTS90002")));
}

#[tokio::test]
async fn unreachable_endpoint_is_unavailable() {
    let client = CloudClient::new(Endpoints {
        digitalocean: Some("http://127.0.0.1:9".into()),
        ..Endpoints::default()
    });
    let err = client
        .discover(&CloudConfig::DigitalOcean(DigitalOceanConfig {
            token: Zeroizing::new("t".into()),
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, CloudError::Unavailable(_)), "{err:?}");
}
