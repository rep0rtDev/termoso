//! SSH ID: passkeys for SSH.
//!
//! Every signed-in device generates its own set of SSH keys; the private
//! halves never leave the device. Only public keys reach the server, where
//! they are listed under a public handle so a host can be provisioned with
//! one command:
//!
//! ```text
//! curl -fs https://<server>/sshid/<handle> >> ~/.ssh/authorized_keys
//! ```
//!
//! Device keys are published while the device has a live session and vanish
//! when it logs out or is revoked. FIDO2 keys are bound to the hardware token
//! instead and stay until removed.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::schema;

/// Handle rules: 3–32 chars, lowercase letters, digits, `-` and `_`, must
/// start with a letter or digit.
pub const HANDLE_MIN: usize = 3;
/// See [`HANDLE_MIN`].
pub const HANDLE_MAX: usize = 32;

/// Validate and normalise a handle (lowercased). `None` when it is not
/// acceptable.
pub fn normalize_handle(input: &str) -> Option<String> {
    let h = input.trim().trim_start_matches('@').to_ascii_lowercase();
    let n = h.chars().count();
    if !(HANDLE_MIN..=HANDLE_MAX).contains(&n) {
        return None;
    }
    let mut chars = h.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    if !h
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return None;
    }
    Some(h)
}

schema! {
    /// Passkey types an SSH ID can hold.
    #[derive(Copy, PartialEq, Eq, Hash)]
    #[serde(rename_all = "snake_case")]
    pub enum SshIdKeyType {
        /// `ssh-ed25519` — the default (OpenSSH 6.5+).
        Ed25519,
        /// `ecdsa-sha2-nistp256` (OpenSSH 5.7+).
        Ecdsa,
        /// `ssh-rsa` for legacy devices.
        Rsa,
        /// `sk-ecdsa-sha2-nistp256@openssh.com` — FIDO2 hardware (OpenSSH 8.4+).
        EcdsaSk,
        /// `sk-ssh-ed25519@openssh.com` — FIDO2 hardware (OpenSSH 8.4+).
        Ed25519Sk,
    }
}

impl SshIdKeyType {
    /// Every type, default first.
    pub const ALL: [SshIdKeyType; 5] = [
        SshIdKeyType::Ed25519,
        SshIdKeyType::Ecdsa,
        SshIdKeyType::Rsa,
        SshIdKeyType::EcdsaSk,
        SshIdKeyType::Ed25519Sk,
    ];

    /// Types generated on every device (software passkeys).
    pub const DEVICE: [SshIdKeyType; 3] = [
        SshIdKeyType::Ed25519,
        SshIdKeyType::Ecdsa,
        SshIdKeyType::Rsa,
    ];

    /// Name used in the public URL (`/sshid/{handle}/{type}`), matching the
    /// labels shown in the UI.
    pub fn url_name(self) -> &'static str {
        match self {
            SshIdKeyType::Ed25519 => "ED25519",
            SshIdKeyType::Ecdsa => "ECDSA",
            SshIdKeyType::Rsa => "RSA",
            SshIdKeyType::EcdsaSk => "ECDSA-SK",
            SshIdKeyType::Ed25519Sk => "ED25519-SK",
        }
    }

    /// Parse the URL segment (case-insensitive; also accepts the wire names).
    pub fn from_url_name(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_uppercase().replace('_', "-");
        Self::ALL
            .into_iter()
            .find(|t| t.url_name() == s || t.wire_name().to_ascii_uppercase() == s)
    }

    /// The `authorized_keys` algorithm name.
    pub fn wire_name(self) -> &'static str {
        match self {
            SshIdKeyType::Ed25519 => "ssh-ed25519",
            SshIdKeyType::Ecdsa => "ecdsa-sha2-nistp256",
            SshIdKeyType::Rsa => "ssh-rsa",
            SshIdKeyType::EcdsaSk => "sk-ecdsa-sha2-nistp256@openssh.com",
            SshIdKeyType::Ed25519Sk => "sk-ssh-ed25519@openssh.com",
        }
    }

    /// Hardware-backed (needs a FIDO2 token to sign).
    pub fn is_hardware(self) -> bool {
        matches!(self, SshIdKeyType::EcdsaSk | SshIdKeyType::Ed25519Sk)
    }
}

schema! {
    /// `POST /account/sshid`
    pub struct CreateSshIdRequest {
        /// Desired handle (see [`normalize_handle`]).
        pub handle: String,
    }
}

schema! {
    /// A public key published under the handle.
    pub struct SshIdKey {
        /// Id (for removal).
        pub id: Uuid,
        /// Passkey type.
        pub key_type: SshIdKeyType,
        /// `<algorithm> <base64>` — no comment.
        pub public_key: String,
        /// Device that holds the private key; `None` for FIDO2 keys, which
        /// work from any signed-in device that has the token plugged in.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub device_id: Option<Uuid>,
        /// Device name (device keys) or the label given to a FIDO2 key.
        pub label: String,
        /// This key belongs to the calling device.
        #[serde(default)]
        pub current_device: bool,
        /// Created / last rotated.
        pub updated_at: DateTime<Utc>,
    }
}

schema! {
    /// `GET /account/sshid`
    pub struct SshIdProfile {
        /// Handle.
        pub handle: String,
        /// Public URL of the default key list (`…/sshid/{handle}`).
        pub url: String,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// Published keys, newest first.
        pub keys: Vec<SshIdKey>,
    }
}

schema! {
    /// One key of the calling device, `PUT /account/sshid/keys/device`.
    pub struct DeviceKeyUpload {
        /// Type.
        pub key_type: SshIdKeyType,
        /// `<algorithm> <base64>`.
        pub public_key: String,
    }
}

schema! {
    /// `PUT /account/sshid/keys/device` — replaces the calling device's keys.
    pub struct PutDeviceKeysRequest {
        /// The full set; types not listed are removed.
        pub keys: Vec<DeviceKeyUpload>,
    }
}

schema! {
    /// `POST /account/sshid/keys/fido2` — publish a hardware key.
    pub struct AddFido2KeyRequest {
        /// Label shown in the key list.
        pub label: String,
        /// Must be a hardware type.
        pub key_type: SshIdKeyType,
        /// `<algorithm> <base64>`.
        pub public_key: String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles() {
        assert_eq!(normalize_handle(" @Alice_1 ").as_deref(), Some("alice_1"));
        assert_eq!(normalize_handle("ab"), None);
        assert_eq!(normalize_handle("-abc"), None);
        assert_eq!(normalize_handle("a.b.c"), None);
        assert_eq!(normalize_handle(&"x".repeat(33)), None);
    }

    #[test]
    fn url_names_round_trip() {
        for t in SshIdKeyType::ALL {
            assert_eq!(SshIdKeyType::from_url_name(t.url_name()), Some(t));
            assert_eq!(SshIdKeyType::from_url_name(t.wire_name()), Some(t));
        }
        assert_eq!(
            SshIdKeyType::from_url_name("ecdsa_sk"),
            Some(SshIdKeyType::EcdsaSk)
        );
        assert_eq!(SshIdKeyType::from_url_name("dsa"), None);
    }
}
