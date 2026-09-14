//! WebAuthn ceremonies over a CTAP2 token — the part a browser plays
//! between a relying party's JSON options and the authenticator.
//!
//! This is *not* the SSH security-key path: `sk-*` keys sign a hash of
//! SSH data and yield OpenSSH signature blobs, whereas here the token
//! signs the SHA-256 of a `clientDataJSON` document (`webauthn.get` /
//! `webauthn.create` with the relying party's challenge and our origin),
//! and the relying party gets back the raw `authenticatorData` and
//! signature (assertion) or `attestationObject` (registration), as
//! `PublicKeyCredential` JSON with URL-safe base64 fields.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use super::Fido2Error;
use super::ctap::{Authenticator, CtapTransport, GetAssertionRequest, MakeCredentialRequest};

/// Origin the relying party sees in `clientDataJSON` for a server reached
/// at `server`: scheme, host and explicit port.
pub fn origin_for(server: &Url) -> String {
    server.origin().ascii_serialization()
}

/// Bytes a relying party exchanges as URL-safe base64.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bytes(pub Vec<u8>);

impl Serialize for Bytes {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&URL_SAFE_NO_PAD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        [URL_SAFE_NO_PAD, URL_SAFE, STANDARD]
            .iter()
            .find_map(|e| e.decode(&s).ok())
            .map(Bytes)
            .ok_or_else(|| serde::de::Error::custom("not base64"))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Descriptor {
    id: Bytes,
}

#[derive(Debug, Clone, Deserialize)]
struct Envelope<T> {
    #[serde(rename = "publicKey")]
    public_key: T,
}

/// `PublicKeyCredentialRequestOptions`, the part a token needs.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestOptions {
    challenge: Bytes,
    rp_id: String,
    #[serde(default)]
    allow_credentials: Vec<Descriptor>,
    #[serde(default)]
    user_verification: Option<String>,
}

impl RequestOptions {
    /// Parse the relying party's JSON (`{"publicKey": {...}}`).
    pub fn parse(options: &serde_json::Value) -> std::result::Result<Self, Fido2Error> {
        serde_json::from_value::<Envelope<Self>>(options.clone())
            .map(|e| e.public_key)
            .map_err(|e| Fido2Error::Other(format!("bad WebAuthn request options: {e}")))
    }

    /// Relying party id.
    pub fn rp_id(&self) -> &str {
        &self.rp_id
    }

    /// Credential ids the relying party accepts.
    pub fn allowed_credentials(&self) -> Vec<&[u8]> {
        self.allow_credentials
            .iter()
            .map(|d| d.id.0.as_slice())
            .collect()
    }

    /// The relying party demands user verification.
    pub fn requires_user_verification(&self) -> bool {
        self.user_verification.as_deref() == Some("required")
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RelyingParty {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
    id: Bytes,
    name: String,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Param {
    alg: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Selection {
    #[serde(default)]
    resident_key: Option<String>,
    #[serde(default)]
    require_resident_key: Option<bool>,
    #[serde(default)]
    user_verification: Option<String>,
}

/// `PublicKeyCredentialCreationOptions`, the part a token needs.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationOptions {
    rp: RelyingParty,
    user: User,
    challenge: Bytes,
    pub_key_cred_params: Vec<Param>,
    #[serde(default)]
    exclude_credentials: Option<Vec<Descriptor>>,
    #[serde(default)]
    authenticator_selection: Option<Selection>,
}

impl CreationOptions {
    /// Parse the relying party's JSON (`{"publicKey": {...}}`).
    pub fn parse(options: &serde_json::Value) -> std::result::Result<Self, Fido2Error> {
        serde_json::from_value::<Envelope<Self>>(options.clone())
            .map(|e| e.public_key)
            .map_err(|e| Fido2Error::Other(format!("bad WebAuthn creation options: {e}")))
    }

    /// Relying party id; falls back to the origin's host when the relying
    /// party left it out, as browsers do.
    pub fn rp_id(&self, origin: &str) -> std::result::Result<String, Fido2Error> {
        match &self.rp.id {
            Some(id) => Ok(id.clone()),
            None => Url::parse(origin)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
                .ok_or_else(|| Fido2Error::Other("WebAuthn options name no relying party".into())),
        }
    }

    /// Acceptable COSE algorithms, most preferred first.
    pub fn algorithms(&self) -> Vec<i64> {
        self.pub_key_cred_params.iter().map(|p| p.alg).collect()
    }

    fn resident(&self) -> bool {
        let s = self.authenticator_selection.clone().unwrap_or_default();
        s.require_resident_key == Some(true) || s.resident_key.as_deref() == Some("required")
    }

    /// The relying party demands user verification (WebAuthn's default).
    pub fn requires_user_verification(&self) -> bool {
        self.authenticator_selection
            .as_ref()
            .and_then(|s| s.user_verification.as_deref())
            .unwrap_or("required")
            == "required"
    }
}

#[derive(Serialize)]
struct ClientData<'a> {
    #[serde(rename = "type")]
    type_: &'a str,
    challenge: &'a Bytes,
    origin: &'a str,
    #[serde(rename = "crossOrigin")]
    cross_origin: bool,
}

/// Serialized `clientDataJSON` and its SHA-256, exactly as signed.
pub fn client_data(kind: &str, challenge: &[u8], origin: &str) -> (Vec<u8>, [u8; 32]) {
    let json = serde_json::to_vec(&ClientData {
        type_: kind,
        challenge: &Bytes(challenge.to_vec()),
        origin,
        cross_origin: false,
    })
    .expect("client data serializes");
    let hash = Sha256::digest(&json).into();
    (json, hash)
}

#[derive(Serialize)]
struct AssertionResponse {
    #[serde(rename = "authenticatorData")]
    authenticator_data: Bytes,
    #[serde(rename = "clientDataJSON")]
    client_data_json: Bytes,
    signature: Bytes,
    #[serde(rename = "userHandle")]
    user_handle: Option<Bytes>,
}

#[derive(Serialize)]
struct AttestationResponse {
    #[serde(rename = "attestationObject")]
    attestation_object: Bytes,
    #[serde(rename = "clientDataJSON")]
    client_data_json: Bytes,
    transports: Vec<&'static str>,
}

#[derive(Serialize)]
struct Credential<R> {
    id: String,
    #[serde(rename = "rawId")]
    raw_id: Bytes,
    response: R,
    #[serde(rename = "type")]
    type_: &'static str,
    #[serde(rename = "clientExtensionResults")]
    extensions: serde_json::Map<String, serde_json::Value>,
}

fn credential_json<R: Serialize>(id: Vec<u8>, response: R) -> serde_json::Value {
    serde_json::to_value(Credential {
        id: URL_SAFE_NO_PAD.encode(&id),
        raw_id: Bytes(id),
        response,
        type_: "public-key",
        extensions: serde_json::Map::new(),
    })
    .expect("credential serializes")
}

/// Answer `options` (from the relying party's `challenge` endpoint) with
/// `token`: `PublicKeyCredential` JSON for its `verify` endpoint. `origin`
/// must be one the relying party allows.
pub fn assert<T: CtapTransport>(
    token: &mut Authenticator<T>,
    options: &RequestOptions,
    origin: &str,
    pin: Option<&str>,
) -> std::result::Result<serde_json::Value, Fido2Error> {
    let (client_data_json, client_data_hash) =
        client_data("webauthn.get", &options.challenge.0, origin);
    let allow = options.allowed_credentials();
    let assertion = token.get_assertion_raw(
        &GetAssertionRequest {
            rp_id: &options.rp_id,
            client_data_hash,
            allow: &allow,
            user_presence: true,
            user_verification: options.requires_user_verification(),
        },
        pin,
    )?;
    Ok(credential_json(
        assertion.credential_id,
        AssertionResponse {
            authenticator_data: Bytes(assertion.data.auth_data),
            client_data_json: Bytes(client_data_json),
            signature: Bytes(assertion.data.signature),
            user_handle: assertion.user_id.map(Bytes),
        },
    ))
}

/// Mint a credential for `options` (from the relying party's registration
/// `start` endpoint) on `token`: `RegisterPublicKeyCredential` JSON for
/// its `finish` endpoint. `transport` names how the token was reached
/// (`usb`, `nfc`, …).
pub fn register<T: CtapTransport>(
    token: &mut Authenticator<T>,
    options: &CreationOptions,
    origin: &str,
    transport: &'static str,
    pin: Option<&str>,
) -> std::result::Result<serde_json::Value, Fido2Error> {
    let rp_id = options.rp_id(origin)?;
    let (client_data_json, client_data_hash) =
        client_data("webauthn.create", &options.challenge.0, origin);
    let exclude: Vec<Vec<u8>> = options
        .exclude_credentials
        .iter()
        .flatten()
        .map(|d| d.id.0.clone())
        .collect();
    let attestation = token.make_credential_raw(
        &MakeCredentialRequest {
            rp_id: &rp_id,
            rp_name: options.rp.name.as_deref(),
            user_id: &options.user.id.0,
            user_name: &options.user.name,
            user_display_name: options
                .user
                .display_name
                .as_deref()
                .unwrap_or(&options.user.name),
            algorithms: &options.algorithms(),
            exclude: &exclude,
            resident: options.resident(),
            user_verification: options.requires_user_verification(),
            client_data_hash,
        },
        pin,
    )?;
    let id = attestation.credential()?.id;
    Ok(credential_json(
        id,
        AttestationResponse {
            attestation_object: Bytes(attestation.attestation_object()),
            client_data_json: Bytes(client_data_json),
            transports: vec![transport],
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_data_is_what_browsers_send() {
        let (json, hash) = client_data("webauthn.get", &[1, 2, 3, 250], "https://rp.example");
        assert_eq!(
            std::str::from_utf8(&json).unwrap(),
            r#"{"type":"webauthn.get","challenge":"AQID-g","origin":"https://rp.example","crossOrigin":false}"#
        );
        assert_eq!(hash, <[u8; 32]>::from(Sha256::digest(&json)));
    }

    #[test]
    fn origin_keeps_explicit_port_only() {
        assert_eq!(
            origin_for(&Url::parse("https://termoso.example.com/api/").unwrap()),
            "https://termoso.example.com"
        );
        assert_eq!(
            origin_for(&Url::parse("http://localhost:8443").unwrap()),
            "http://localhost:8443"
        );
        assert_eq!(
            origin_for(&Url::parse("https://x.example:443/").unwrap()),
            "https://x.example"
        );
    }

    #[test]
    fn bytes_accept_any_base64_flavour() {
        for s in ["\"AQID-g\"", "\"AQID-g==\"", "\"AQID+g==\""] {
            let b: Bytes = serde_json::from_str(s).unwrap();
            assert_eq!(b.0, vec![1, 2, 3, 250]);
        }
        assert!(serde_json::from_str::<Bytes>("\"*\"").is_err());
        assert_eq!(
            serde_json::to_string(&Bytes(vec![1, 2, 3, 250])).unwrap(),
            "\"AQID-g\""
        );
    }

    #[test]
    fn request_options_parse_the_relying_party_shape() {
        let o = RequestOptions::parse(&serde_json::json!({
            "publicKey": {
                "challenge": "Y2hhbGxlbmdl",
                "timeout": 60000,
                "rpId": "termoso.example.com",
                "allowCredentials": [
                    {"type": "public-key", "id": "AQID"},
                    {"type": "public-key", "id": "BAUG", "transports": ["usb"]}
                ],
                "userVerification": "preferred"
            }
        }))
        .unwrap();
        assert_eq!(o.rp_id(), "termoso.example.com");
        assert_eq!(o.challenge.0, b"challenge");
        assert_eq!(o.allowed_credentials(), vec![&[1u8, 2, 3][..], &[4, 5, 6]]);
        assert!(!o.requires_user_verification());
        assert!(RequestOptions::parse(&serde_json::json!({"challenge": "x"})).is_err());
    }

    #[test]
    fn creation_options_default_to_required_verification() {
        let o = CreationOptions::parse(&serde_json::json!({
            "publicKey": {
                "rp": {"name": "Termoso", "id": "termoso.example.com"},
                "user": {"id": "dXNlcg", "name": "a@b.c", "displayName": "A"},
                "challenge": "Y2hhbGxlbmdl",
                "pubKeyCredParams": [{"type": "public-key", "alg": -7}, {"type": "public-key", "alg": -8}],
                "attestation": "none",
                "extensions": {"credProps": true}
            }
        }))
        .unwrap();
        assert_eq!(
            o.rp_id("https://other.example").unwrap(),
            "termoso.example.com"
        );
        assert_eq!(o.algorithms(), vec![-7, -8]);
        assert!(o.requires_user_verification());
        assert!(!o.resident());

        let o = CreationOptions::parse(&serde_json::json!({
            "publicKey": {
                "rp": {"name": "Termoso"},
                "user": {"id": "dXNlcg", "name": "a@b.c"},
                "challenge": "Y2hhbGxlbmdl",
                "pubKeyCredParams": [{"type": "public-key", "alg": -7}],
                "authenticatorSelection": {"residentKey": "required", "userVerification": "discouraged"}
            }
        }))
        .unwrap();
        assert_eq!(o.rp_id("https://rp.example:8443").unwrap(), "rp.example");
        assert!(!o.requires_user_verification());
        assert!(o.resident());
    }
}
