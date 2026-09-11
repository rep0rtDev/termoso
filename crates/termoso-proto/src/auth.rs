//! Authentication: OPAQUE registration/login, MFA, device approval, recovery, SSO.
//!
//! All `opaque_*` fields are base64-encoded OPAQUE protocol messages produced
//! by `termoso_crypto::opaque`.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::account::{AccountKeys, UserProfile};
use crate::schema;

schema! {
    /// Client platform.
    #[derive(Copy, PartialEq, Eq, Hash)]
    #[serde(rename_all = "snake_case")]
    pub enum Platform {
        /// Windows desktop.
        Windows,
        /// Linux desktop.
        Linux,
        /// macOS desktop.
        Macos,
        /// Android.
        Android,
        /// iOS / iPadOS.
        Ios,
        /// Browser (account portal).
        Web,
        /// Command line / scripts.
        Cli,
    }
}

schema! {
    /// Description of the device performing the request.
    pub struct DeviceInfo {
        /// Human-readable device name (e.g. "Ivan's ThinkPad").
        pub name: String,
        /// Platform.
        pub platform: Platform,
        /// Client application version.
        pub app_version: String,
        /// Client-generated stable device identifier (UUID). Lets a reinstalled
        /// app re-attach to an existing device record after approval.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_device_id: Option<Uuid>,
    }
}

schema! {
    /// Keys uploaded at registration (see termoso-crypto docs for the hierarchy).
    pub struct AccountKeysUpload {
        /// X25519 public key (base64, 32 bytes).
        pub public_key: String,
        /// Account private key wrapped with the account KEK (envelope, base64).
        pub wrapped_private_key: String,
        /// Account private key wrapped with the recovery KEK (envelope, base64).
        pub recovery_wrapped_private_key: String,
        /// Recovery verifier (base64, 32 bytes). The server stores only its hash.
        pub recovery_verifier: String,
        /// Personal vault key sealed to `public_key` (base64 sealed box).
        pub personal_vault_sealed_key: String,
    }
}

schema! {
    /// `POST /auth/register/start`
    pub struct RegisterStartRequest {
        /// Email address (will be lower-cased).
        pub email: String,
        /// OPAQUE `RegistrationRequest`.
        pub opaque_request: String,
    }
}

schema! {
    /// Response to `RegisterStartRequest`.
    pub struct RegisterStartResponse {
        /// OPAQUE `RegistrationResponse`.
        pub opaque_response: String,
    }
}

schema! {
    /// `POST /auth/register/finish`
    pub struct RegisterFinishRequest {
        /// Same email as in the start request.
        pub email: String,
        /// OPAQUE `RegistrationUpload`.
        pub opaque_upload: String,
        /// Optional display name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        /// Device performing the registration (gets a session immediately).
        pub device: DeviceInfo,
        /// Account keys.
        pub keys: AccountKeysUpload,
        /// Team invite token, if signing up from an invitation link.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite_token: Option<String>,
        /// SSO session proving the email was verified by an identity provider.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sso_session: Option<String>,
    }
}

schema! {
    /// `POST /auth/login/start`
    pub struct LoginStartRequest {
        /// Email.
        pub email: String,
        /// OPAQUE `CredentialRequest`.
        pub opaque_request: String,
        /// Device performing the login.
        pub device: DeviceInfo,
        /// SSO session (skips new-device email approval).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sso_session: Option<String>,
    }
}

schema! {
    /// Response to `LoginStartRequest`.
    pub struct LoginStartResponse {
        /// Opaque handle for the finish step (valid for a short time).
        pub login_id: String,
        /// OPAQUE `CredentialResponse`.
        pub opaque_response: String,
    }
}

schema! {
    /// `POST /auth/login/finish`
    pub struct LoginFinishRequest {
        /// From `LoginStartResponse`.
        pub login_id: String,
        /// OPAQUE `CredentialFinalization`.
        pub opaque_finalization: String,
    }
}

schema! {
    /// Available second factors.
    #[derive(Copy, PartialEq, Eq, Hash)]
    #[serde(rename_all = "snake_case")]
    pub enum MfaMethod {
        /// Time-based one-time password (RFC 6238).
        Totp,
        /// WebAuthn / passkey / FIDO2 security key.
        Webauthn,
        /// One of the single-use backup codes.
        BackupCode,
        /// Code sent by email.
        Email,
    }
}

schema! {
    /// Credential presented to satisfy MFA.
    #[serde(tag = "method", rename_all = "snake_case")]
    pub enum MfaCredential {
        /// 6-digit TOTP.
        Totp {
            /// The code.
            code: String,
        },
        /// Backup code.
        BackupCode {
            /// The code.
            code: String,
        },
        /// WebAuthn assertion (JSON as produced by `navigator.credentials.get` / platform API).
        Webauthn {
            /// PublicKeyCredential JSON.
            credential: serde_json::Value,
        },
        /// Email code.
        Email {
            /// The code.
            code: String,
        },
    }
}

schema! {
    /// `POST /auth/mfa/verify`
    pub struct MfaVerifyRequest {
        /// From `AuthResponse::MfaRequired`.
        pub mfa_token: String,
        /// The credential.
        pub credential: MfaCredential,
    }
}

schema! {
    /// `POST /auth/mfa/webauthn/challenge` – request an assertion challenge for a pending MFA.
    pub struct WebauthnChallengeRequest {
        /// From `AuthResponse::MfaRequired`.
        pub mfa_token: String,
    }
}

schema! {
    /// `POST /auth/device/approve`
    pub struct DeviceApproveRequest {
        /// From `AuthResponse::DeviceApprovalRequired`.
        pub approval_token: String,
        /// Code from the email.
        pub code: String,
    }
}

schema! {
    /// `POST /auth/device/approve/resend`
    pub struct DeviceApproveResendRequest {
        /// From `AuthResponse::DeviceApprovalRequired`.
        pub approval_token: String,
    }
}

schema! {
    /// Successful authentication payload.
    pub struct Session {
        /// Bearer token. Send as `Authorization: Bearer <token>`.
        pub token: String,
        /// Absolute expiry (sliding on use).
        pub expires_at: DateTime<Utc>,
        /// Server-side device id.
        pub device_id: Uuid,
        /// The user.
        pub user: UserProfile,
        /// Account keys needed to unlock the vaults.
        pub keys: AccountKeys,
    }
}

schema! {
    /// Outcome of a login / registration / MFA / approval step.
    #[serde(tag = "status", rename_all = "snake_case")]
    pub enum AuthResponse {
        /// Done.
        Authenticated(Session),
        /// A second factor is required.
        MfaRequired {
            /// Handle for `MfaVerifyRequest`.
            mfa_token: String,
            /// Methods the user has enabled.
            methods: Vec<MfaMethod>,
        },
        /// New device must be approved with a code sent to the account email.
        DeviceApprovalRequired {
            /// Handle for `DeviceApproveRequest`.
            approval_token: String,
            /// Masked email the code was sent to (e.g. `i***@example.com`).
            email_hint: String,
        },
    }
}

// ───────────────────────────── password change / recovery ─────────────────────────────

schema! {
    /// Step 1 of setting a new password (authenticated change or recovery).
    pub struct PasswordSetupStartRequest {
        /// Recovery token (from `RecoveryStartResponse`); omitted for an authenticated change.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub recovery_token: Option<String>,
        /// OPAQUE `RegistrationRequest` for the new password.
        pub opaque_request: String,
    }
}

schema! {
    /// Response to `PasswordSetupStartRequest`.
    pub struct PasswordSetupStartResponse {
        /// OPAQUE `RegistrationResponse`.
        pub opaque_response: String,
    }
}

schema! {
    /// Step 2 of setting a new password.
    pub struct PasswordSetupFinishRequest {
        /// Recovery token; omitted for an authenticated change.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub recovery_token: Option<String>,
        /// OPAQUE `RegistrationUpload` for the new password.
        pub opaque_upload: String,
        /// Account private key re-wrapped with the new account KEK.
        pub wrapped_private_key: String,
        /// Optionally rotate the recovery key at the same time.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub new_recovery: Option<RecoveryRotate>,
        /// Whether to sign out every other device (default true).
        #[serde(default = "default_true")]
        pub revoke_other_sessions: bool,
        /// Device performing the recovery (ignored for authenticated change).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub device: Option<DeviceInfo>,
    }
}

fn default_true() -> bool {
    true
}

schema! {
    /// New recovery material.
    pub struct RecoveryRotate {
        /// Account private key wrapped with the new recovery KEK.
        pub recovery_wrapped_private_key: String,
        /// New recovery verifier (base64, 32 bytes).
        pub recovery_verifier: String,
    }
}

schema! {
    /// `POST /auth/recovery/start`
    pub struct RecoveryStartRequest {
        /// Email.
        pub email: String,
        /// Recovery verifier derived from the mnemonic (base64, 32 bytes).
        pub recovery_verifier: String,
    }
}

schema! {
    /// Response to `RecoveryStartRequest`.
    pub struct RecoveryStartResponse {
        /// Short-lived token for the password setup steps.
        pub recovery_token: String,
        /// Account private key wrapped with the recovery KEK – unwrap it locally,
        /// then re-wrap with the new account KEK.
        pub recovery_wrapped_private_key: String,
        /// Public key (to verify the unwrapped private key).
        pub public_key: String,
    }
}

// ───────────────────────────── devices ─────────────────────────────

schema! {
    /// A device / session.
    pub struct Device {
        /// Server id.
        pub id: Uuid,
        /// Name.
        pub name: String,
        /// Platform.
        pub platform: Platform,
        /// App version at last use.
        pub app_version: String,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// Last request.
        pub last_seen_at: DateTime<Utc>,
        /// Last known IP (as seen by the server; stored for the user's own security review only).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_ip: Option<String>,
        /// Whether this is the device making the request.
        pub current: bool,
    }
}

schema! {
    /// `GET /account/devices`
    pub struct DeviceList {
        /// Devices.
        pub devices: Vec<Device>,
    }
}

// ───────────────────────────── MFA management ─────────────────────────────

schema! {
    /// `GET /account/mfa`
    pub struct MfaStatus {
        /// TOTP enabled.
        pub totp_enabled: bool,
        /// Registered WebAuthn credentials.
        pub webauthn_credentials: Vec<WebauthnCredentialInfo>,
        /// Remaining backup codes.
        pub backup_codes_remaining: u32,
    }
}

schema! {
    /// A registered WebAuthn credential.
    pub struct WebauthnCredentialInfo {
        /// Id.
        pub id: Uuid,
        /// User-given name.
        pub name: String,
        /// Created.
        pub created_at: DateTime<Utc>,
        /// Last used.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_used_at: Option<DateTime<Utc>>,
    }
}

schema! {
    /// `POST /account/mfa/totp/setup`
    pub struct TotpSetupResponse {
        /// Base32 secret (to display for manual entry).
        pub secret: String,
        /// `otpauth://` URL for QR code.
        pub otpauth_url: String,
    }
}

schema! {
    /// `POST /account/mfa/totp/confirm` and `DELETE /account/mfa/totp`
    pub struct TotpCodeRequest {
        /// 6-digit code.
        pub code: String,
    }
}

schema! {
    /// Freshly generated backup codes (shown once).
    pub struct BackupCodes {
        /// Codes.
        pub codes: Vec<String>,
    }
}

schema! {
    /// `POST /account/mfa/webauthn/register/finish`
    pub struct WebauthnRegisterFinishRequest {
        /// Name for the credential.
        pub name: String,
        /// `RegisterPublicKeyCredential` JSON.
        pub credential: serde_json::Value,
    }
}

// ───────────────────────────── SSO ─────────────────────────────

schema! {
    /// Public SSO provider description.
    pub struct SsoProvider {
        /// Provider slug used in URLs (e.g. `google`).
        pub id: String,
        /// Display name.
        pub name: String,
        /// Provider kind.
        pub kind: SsoKind,
    }
}

schema! {
    /// SSO protocol.
    #[derive(Copy, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum SsoKind {
        /// OpenID Connect.
        Oidc,
        /// SAML 2.0.
        Saml,
    }
}

schema! {
    /// `GET /auth/sso/{provider}/start`
    pub struct SsoStartResponse {
        /// URL to open in the browser.
        pub authorization_url: String,
        /// Poll handle (for desktop clients without a redirect target).
        pub flow_id: String,
    }
}

schema! {
    /// Result of an SSO round-trip (`GET /auth/sso/flow/{flow_id}` or deep link).
    #[serde(tag = "status", rename_all = "snake_case")]
    pub enum SsoResult {
        /// Waiting for the browser.
        Pending,
        /// Email verified via IdP; account exists → proceed to OPAQUE login with `sso_session`.
        LoginRequired {
            /// Handle proving IdP authentication.
            sso_session: String,
            /// Verified email.
            email: String,
        },
        /// Email verified via IdP; no account → proceed to registration with `sso_session`.
        RegistrationRequired {
            /// Handle proving IdP authentication.
            sso_session: String,
            /// Verified email.
            email: String,
            /// Display name from the IdP, if any.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            display_name: Option<String>,
        },
        /// IdP returned an error.
        Failed {
            /// Message.
            message: String,
        },
    }
}
