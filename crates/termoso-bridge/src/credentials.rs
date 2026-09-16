//! Credentials file written by the cabinet and mounted into the container.

use std::path::Path;

use termoso_crypto::encoding::unb64;
use termoso_crypto::keys::KeyPair;
use termoso_proto::bridge::{BridgeCredentials, CREDENTIALS_VERSION};

use crate::error::{BridgeError, Result};

/// Parsed credentials: server, token and the bridge key pair.
pub struct Credentials {
    pub server: String,
    pub bridge_id: uuid::Uuid,
    pub token: String,
    pub key_pair: KeyPair,
}

/// Load and validate `termoso-bridge.json`.
pub fn load_credentials(path: &Path) -> Result<Credentials> {
    let raw = std::fs::read(path)
        .map_err(|e| BridgeError::Credentials(format!("cannot read {}: {e}", path.display())))?;
    parse_credentials(&raw)
}

pub fn parse_credentials(raw: &[u8]) -> Result<Credentials> {
    let c: BridgeCredentials = serde_json::from_slice(raw)
        .map_err(|e| BridgeError::Credentials(format!("malformed credentials file: {e}")))?;
    if c.version != CREDENTIALS_VERSION {
        return Err(BridgeError::Credentials(format!(
            "unsupported credentials version {} (expected {CREDENTIALS_VERSION})",
            c.version
        )));
    }
    let server = c.server.trim().trim_end_matches('/').to_string();
    if !(server.starts_with("https://") || server.starts_with("http://")) {
        return Err(BridgeError::Credentials(
            "server must be an http(s) URL".into(),
        ));
    }
    if c.token.trim().is_empty() {
        return Err(BridgeError::Credentials("token is empty".into()));
    }
    let secret = unb64(c.private_key.trim())
        .map_err(|_| BridgeError::Credentials("private_key is not base64".into()))?;
    let key_pair = KeyPair::from_secret_bytes(&secret)
        .map_err(|_| BridgeError::Credentials("private_key must be 32 bytes".into()))?;
    Ok(Credentials {
        server,
        bridge_id: c.bridge_id,
        token: c.token.trim().to_string(),
        key_pair,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_crypto::encoding::b64;

    fn file(version: u32, server: &str, key: &str, token: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "version": version,
            "server": server,
            "bridge_id": uuid::Uuid::new_v4(),
            "private_key": key,
            "token": token,
        }))
        .unwrap()
    }

    #[test]
    fn parses_and_validates() {
        let kp = KeyPair::generate();
        let key = b64(&kp.secret_bytes());
        let c = parse_credentials(&file(1, "https://x.example/", &key, "tok")).unwrap();
        assert_eq!(c.server, "https://x.example");
        assert_eq!(c.key_pair.public_b64(), kp.public_b64());

        assert!(parse_credentials(&file(2, "https://x", &key, "tok")).is_err());
        assert!(parse_credentials(&file(1, "ftp://x", &key, "tok")).is_err());
        assert!(parse_credentials(&file(1, "https://x", &key, " ")).is_err());
        assert!(parse_credentials(&file(1, "https://x", "AAAA", "tok")).is_err());
        assert!(parse_credentials(b"{").is_err());
    }
}
