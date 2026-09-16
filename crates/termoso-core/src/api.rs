//! Typed HTTP client for the Termoso server (`/api/v1`).
//!
//! Thin and deliberately dumb: every method maps 1:1 onto a route and speaks
//! the `termoso_proto` types. Cryptography, key handling and persistence live
//! in [`crate::account`] and [`crate::sync`]; nothing here ever sees a
//! password, a private key or a vault key.
//!
//! Failures come back as [`CoreError::Api`] carrying the server's machine
//! readable `code`, or [`CoreError::Http`] for transport problems.

use std::sync::RwLock;
use std::time::Duration;

use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use termoso_proto::account::{
    AccountKeys, PresenceVisibilityRequest, ServerInfo, SettingsBlob, UpdateProfileRequest,
    UserProfile,
};
use termoso_proto::ai::{AiCommandRequest, AiCommandResponse, AiSettingsRequest, AiStatus};
use termoso_proto::auth::{
    AuthResponse, Device, DeviceApproveRequest, DeviceApproveResendRequest, DeviceList,
    LoginFinishRequest, LoginStartRequest, LoginStartResponse, MfaCredential, MfaStatus,
    MfaVerifyRequest, ReauthFinishRequest, ReauthStartRequest, ReauthStartResponse,
    RegisterFinishRequest, RegisterStartRequest, RegisterStartResponse, WebauthnChallengeRequest,
    WebauthnCredentialInfo, WebauthnRegisterFinishRequest,
};
use termoso_proto::error::ApiError;
use termoso_proto::live::{CreateLiveSessionRequest, LiveSession, LiveSessionList};
use termoso_proto::logs::{
    CreateLogRequest, CreateLogResponse, DownloadLogResponse, LogListResponse, SessionLog,
    UpdateLogRequest,
};
use termoso_proto::sshid::{
    AddFido2KeyRequest, CreateSshIdRequest, PutDeviceKeysRequest, SshIdKey, SshIdProfile,
};
use termoso_proto::sync::{
    HistoryClearRequest, HistoryKind, HistoryPullResponse, HistoryPushRequest, PullRequest,
    PullResponse, PushRequest, PushResponse,
};
use termoso_proto::team::{
    AuditEventList, CreateInviteRequest, CreateTeamRequest, CreatedInvite, InviteList,
    PendingVaultKeys, Team, TeamList, TeamMemberList, TeamPresence, UpdateTeamMemberRequest,
    UpdateTeamRequest,
};
use termoso_proto::vault::{
    CreateVaultRequest, RotateVaultKeyRequest, RotateVaultKeyResponse, UpdateVaultRequest, Vault,
    VaultList, VaultMemberList, VaultMemberUpsert,
};
use url::Url;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Filters for [`ApiClient::team_audit`]; `None` fields are omitted from the
/// query string.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AuditQuery {
    /// Only events with an id below this (cursor for older pages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<i64>,
    /// Page size (server caps it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Exact action, or a prefix ending in `.` (e.g. `vault.`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Only events by this user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<Uuid>,
    /// Only events touching this vault.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault: Option<Uuid>,
}

/// `GET /account` body (profile + public key material).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AccountResponse {
    /// Profile.
    pub user: UserProfile,
    /// Public key and wrapped private key.
    pub keys: AccountKeys,
}

/// Default request timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Client for one server. Cheap to clone the `Arc` it usually lives in; the
/// bearer token can be swapped at runtime (sign-in / sign-out).
pub struct ApiClient {
    http: reqwest::Client,
    base: Url,
    token: RwLock<Option<String>>,
    user_agent: String,
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiClient")
            .field("base", &self.base.as_str())
            .field("signed_in", &self.token().is_some())
            .finish()
    }
}

impl ApiClient {
    /// Connect to `server_url` (scheme + host, optional path prefix; the
    /// `/api/v1` segment is added here).
    pub fn new(server_url: &str) -> Result<Self> {
        Self::with_timeout(server_url, DEFAULT_TIMEOUT)
    }

    /// Like [`ApiClient::new`] with a custom request timeout.
    pub fn with_timeout(server_url: &str, timeout: Duration) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(default_user_agent())
            .build()?;
        Self::with_http(server_url, http)
    }

    /// Use a caller-configured `reqwest::Client` (proxy, custom roots,
    /// extra headers). The bearer token is still managed here.
    pub fn with_http(server_url: &str, http: reqwest::Client) -> Result<Self> {
        Ok(Self {
            http,
            base: normalize_server_url(server_url)?,
            token: RwLock::new(None),
            user_agent: default_user_agent(),
        })
    }

    /// Server base URL (normalised, trailing slash).
    pub fn server_url(&self) -> &Url {
        &self.base
    }

    /// `User-Agent` sent with every request.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Current bearer token.
    pub fn token(&self) -> Option<String> {
        self.token.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Install (or clear) the bearer token.
    pub fn set_token(&self, token: Option<String>) {
        *self.token.write().unwrap_or_else(|p| p.into_inner()) = token;
    }

    /// WebSocket endpoint (`ws[s]://…/api/v1/ws`).
    pub fn ws_url(&self) -> Result<Url> {
        self.ws_url_for("ws")
    }

    /// Multiplayer relay endpoint (`ws[s]://…/api/v1/live/{id}/ws`).
    pub fn live_ws_url(&self, id: Uuid) -> Result<Url> {
        self.ws_url_for(&format!("live/{id}/ws"))
    }

    fn ws_url_for(&self, path: &str) -> Result<Url> {
        let mut u = self.api_url(path);
        let scheme = match u.scheme() {
            "https" => "wss",
            "http" => "ws",
            other => return Err(CoreError::Invalid(format!("unsupported scheme {other}"))),
        };
        u.set_scheme(scheme)
            .map_err(|_| CoreError::Invalid("cannot set ws scheme".into()))?;
        Ok(u)
    }

    fn api_url(&self, path: &str) -> Url {
        // `base` always ends with `/`, so joining a relative path keeps any
        // prefix the operator mounted the server under.
        self.base
            .join(&format!("api/v1/{}", path.trim_start_matches('/')))
            .expect("relative api path")
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let mut rb = self.http.request(method, self.api_url(path));
        if let Some(t) = self.token() {
            rb = rb.bearer_auth(t);
        }
        rb
    }

    async fn send<T: DeserializeOwned>(rb: RequestBuilder) -> Result<T> {
        let resp = rb.send().await?;
        let resp = check(resp).await?;
        Ok(resp.json().await?)
    }

    async fn send_empty(rb: RequestBuilder) -> Result<()> {
        let resp = rb.send().await?;
        check(resp).await?;
        Ok(())
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        Self::send(self.request(Method::GET, path)).await
    }

    async fn get_query<T: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        path: &str,
        query: &Q,
    ) -> Result<T> {
        Self::send(self.request(Method::GET, path).query(query)).await
    }

    async fn post<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        Self::send(self.request(Method::POST, path).json(body)).await
    }

    async fn post_empty<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<()> {
        Self::send_empty(self.request(Method::POST, path).json(body)).await
    }

    async fn patch<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        Self::send(self.request(Method::PATCH, path).json(body)).await
    }

    async fn patch_empty<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<()> {
        Self::send_empty(self.request(Method::PATCH, path).json(body)).await
    }

    async fn put_empty<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<()> {
        Self::send_empty(self.request(Method::PUT, path).json(body)).await
    }

    async fn delete(&self, path: &str) -> Result<()> {
        Self::send_empty(self.request(Method::DELETE, path)).await
    }

    // ───────────────────────────── server ─────────────────────────────

    /// `GET /server/info` — unauthenticated capability probe.
    pub async fn server_info(&self) -> Result<ServerInfo> {
        self.get("server/info").await
    }

    // ───────────────────────────── auth ─────────────────────────────

    /// `POST /auth/register/start`.
    pub async fn register_start(
        &self,
        req: &RegisterStartRequest,
    ) -> Result<RegisterStartResponse> {
        self.post("auth/register/start", req).await
    }

    /// `POST /auth/register/finish`.
    pub async fn register_finish(&self, req: &RegisterFinishRequest) -> Result<AuthResponse> {
        self.post("auth/register/finish", req).await
    }

    /// `POST /auth/login/start`.
    pub async fn login_start(&self, req: &LoginStartRequest) -> Result<LoginStartResponse> {
        self.post("auth/login/start", req).await
    }

    /// `POST /auth/login/finish`.
    pub async fn login_finish(&self, req: &LoginFinishRequest) -> Result<AuthResponse> {
        self.post("auth/login/finish", req).await
    }

    /// `POST /auth/mfa/verify`.
    pub async fn mfa_verify(
        &self,
        mfa_token: &str,
        credential: MfaCredential,
    ) -> Result<AuthResponse> {
        self.post(
            "auth/mfa/verify",
            &MfaVerifyRequest {
                mfa_token: mfa_token.into(),
                credential,
            },
        )
        .await
    }

    /// `POST /auth/reauth/start` — step-up for the current session; sensitive
    /// account mutations answer `reauth_required` until this succeeds.
    pub async fn reauth_start(&self, req: &ReauthStartRequest) -> Result<ReauthStartResponse> {
        self.post("auth/reauth/start", req).await
    }

    /// `POST /auth/reauth/finish`. `MfaRequired` means the second factor is
    /// still pending: verify it with the returned `mfa_token` as for login.
    pub async fn reauth_finish(&self, req: &ReauthFinishRequest) -> Result<AuthResponse> {
        self.post("auth/reauth/finish", req).await
    }

    /// `POST /auth/mfa/webauthn/challenge` — raw options for the platform
    /// authenticator.
    pub async fn mfa_webauthn_challenge(&self, mfa_token: &str) -> Result<serde_json::Value> {
        self.post(
            "auth/mfa/webauthn/challenge",
            &WebauthnChallengeRequest {
                mfa_token: mfa_token.into(),
            },
        )
        .await
    }

    /// `POST /auth/mfa/email/send`.
    pub async fn mfa_email_send(&self, mfa_token: &str) -> Result<()> {
        self.post_empty(
            "auth/mfa/email/send",
            &WebauthnChallengeRequest {
                mfa_token: mfa_token.into(),
            },
        )
        .await
    }

    /// `POST /auth/device/approve`.
    pub async fn device_approve(&self, approval_token: &str, code: &str) -> Result<AuthResponse> {
        self.post(
            "auth/device/approve",
            &DeviceApproveRequest {
                approval_token: approval_token.into(),
                code: code.into(),
            },
        )
        .await
    }

    /// `POST /auth/device/approve/resend`.
    pub async fn device_approve_resend(&self, approval_token: &str) -> Result<()> {
        self.post_empty(
            "auth/device/approve/resend",
            &DeviceApproveResendRequest {
                approval_token: approval_token.into(),
            },
        )
        .await
    }

    /// `POST /auth/logout` — revokes the current session server-side.
    pub async fn logout(&self) -> Result<()> {
        Self::send_empty(self.request(Method::POST, "auth/logout")).await
    }

    // ───────────────────────────── account ─────────────────────────────

    /// `GET /account`.
    pub async fn account(&self) -> Result<AccountResponse> {
        self.get("account").await
    }

    /// `PATCH /account/profile`.
    pub async fn update_profile(&self, display_name: Option<String>) -> Result<UserProfile> {
        self.patch("account/profile", &UpdateProfileRequest { display_name })
            .await
    }

    /// `PUT /account/avatar` — replace the profile picture with `image`
    /// (any common raster format; the server shrinks it).
    pub async fn put_avatar(&self, image: Vec<u8>, content_type: &str) -> Result<UserProfile> {
        Self::send(
            self.request(Method::PUT, "account/avatar")
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(image),
        )
        .await
    }

    /// `DELETE /account/avatar`.
    pub async fn delete_avatar(&self) -> Result<UserProfile> {
        Self::send(self.request(Method::DELETE, "account/avatar")).await
    }

    /// `GET /users/{id}/avatar` — the normalised WebP, or `None` when the
    /// user has no picture.
    pub async fn user_avatar(&self, user_id: Uuid) -> Result<Option<Vec<u8>>> {
        let resp = self
            .request(Method::GET, &format!("users/{user_id}/avatar"))
            .send()
            .await?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let resp = check(resp).await?;
        Ok(Some(resp.bytes().await?.to_vec()))
    }

    /// `PUT /account/presence` — hide / show this account in team presence.
    pub async fn set_presence_hidden(&self, hidden: bool) -> Result<UserProfile> {
        Self::send(
            self.request(Method::PUT, "account/presence")
                .json(&PresenceVisibilityRequest { hidden }),
        )
        .await
    }

    // ───────────────────────────── ai ─────────────────────────────

    /// `GET /account/ai` — provider on offer, opt-in state and today's quota.
    pub async fn ai_status(&self) -> Result<AiStatus> {
        self.get("account/ai").await
    }

    /// `PUT /account/ai` — opt in to (or out of) AI command suggestions.
    pub async fn set_ai_enabled(&self, enabled: bool) -> Result<AiStatus> {
        Self::send(
            self.request(Method::PUT, "account/ai")
                .json(&AiSettingsRequest { enabled }),
        )
        .await
    }

    /// `POST /ai/command` — one shell command for a short request. Only the
    /// request text and the OS/shell labels leave the machine; the caller
    /// decides what (if anything) to do with the answer.
    pub async fn ai_command(&self, req: &AiCommandRequest) -> Result<AiCommandResponse> {
        self.post("ai/command", req).await
    }

    /// `GET /account/settings` — encrypted settings blob.
    pub async fn settings(&self) -> Result<SettingsBlob> {
        self.get("account/settings").await
    }

    /// `GET /account/devices`.
    pub async fn devices(&self) -> Result<Vec<Device>> {
        let l: DeviceList = self.get("account/devices").await?;
        Ok(l.devices)
    }

    /// `DELETE /account/devices/{id}`.
    pub async fn revoke_device(&self, id: Uuid) -> Result<()> {
        self.delete(&format!("account/devices/{id}")).await
    }

    // ─────────────────────────────── MFA ──────────────────────────────

    /// `GET /account/mfa`.
    pub async fn mfa_status(&self) -> Result<MfaStatus> {
        self.get("account/mfa").await
    }

    /// `POST /account/mfa/webauthn/register/start` — WebAuthn
    /// `PublicKeyCredentialCreationOptions` (`{"publicKey": …}`).
    pub async fn webauthn_register_start(&self) -> Result<serde_json::Value> {
        self.post(
            "account/mfa/webauthn/register/start",
            &serde_json::json!({}),
        )
        .await
    }

    /// `POST /account/mfa/webauthn/register/finish` with the
    /// `RegisterPublicKeyCredential` the authenticator produced.
    pub async fn webauthn_register_finish(
        &self,
        name: &str,
        credential: serde_json::Value,
    ) -> Result<WebauthnCredentialInfo> {
        self.post(
            "account/mfa/webauthn/register/finish",
            &WebauthnRegisterFinishRequest {
                name: name.into(),
                credential,
            },
        )
        .await
    }

    /// `DELETE /account/mfa/webauthn/{id}`.
    pub async fn webauthn_delete(&self, id: Uuid) -> Result<()> {
        self.delete(&format!("account/mfa/webauthn/{id}")).await
    }

    // ───────────────────────────── SSH ID ─────────────────────────────

    /// `GET /account/sshid` — the account's SSH ID, `None` until claimed.
    pub async fn sshid(&self) -> Result<Option<SshIdProfile>> {
        self.get("account/sshid").await
    }

    /// `POST /account/sshid` — claim `handle`.
    pub async fn create_sshid(&self, handle: &str) -> Result<SshIdProfile> {
        self.post(
            "account/sshid",
            &CreateSshIdRequest {
                handle: handle.to_string(),
            },
        )
        .await
    }

    /// `DELETE /account/sshid` — drop the handle and every published key.
    pub async fn delete_sshid(&self) -> Result<()> {
        self.delete("account/sshid").await
    }

    /// `PUT /account/sshid/keys/device` — replace this device's public keys.
    pub async fn put_sshid_device_keys(&self, req: &PutDeviceKeysRequest) -> Result<SshIdProfile> {
        Self::send(
            self.request(Method::PUT, "account/sshid/keys/device")
                .json(req),
        )
        .await
    }

    /// `POST /account/sshid/keys/fido2` — publish a security-key public key.
    pub async fn add_sshid_fido2_key(&self, req: &AddFido2KeyRequest) -> Result<SshIdKey> {
        self.post("account/sshid/keys/fido2", req).await
    }

    /// `DELETE /account/sshid/keys/{id}`.
    pub async fn remove_sshid_key(&self, id: Uuid) -> Result<()> {
        self.delete(&format!("account/sshid/keys/{id}")).await
    }

    // ───────────────────────────── vaults / teams ─────────────────────────────

    /// `GET /vaults` — every vault we are a member of, with our sealed key.
    pub async fn vaults(&self) -> Result<Vec<Vault>> {
        let l: VaultList = self.get("vaults").await?;
        Ok(l.vaults)
    }

    /// `GET /vaults/{id}`.
    pub async fn vault(&self, id: Uuid) -> Result<Vault> {
        self.get(&format!("vaults/{id}")).await
    }

    /// `GET /vaults/{id}/members`.
    pub async fn vault_members(&self, id: Uuid) -> Result<VaultMemberList> {
        self.get(&format!("vaults/{id}/members")).await
    }

    /// `PUT /vaults/{id}/members/{user_id}` — grant / reseal a key.
    pub async fn upsert_vault_member(&self, id: Uuid, member: &VaultMemberUpsert) -> Result<()> {
        self.put_empty(&format!("vaults/{id}/members/{}", member.user_id), member)
            .await
    }

    /// `DELETE /vaults/{id}/members/{user_id}` — revoke access (rotate the key afterwards).
    pub async fn remove_vault_member(&self, id: Uuid, user_id: Uuid) -> Result<()> {
        self.delete(&format!("vaults/{id}/members/{user_id}")).await
    }

    /// `PATCH /vaults/{id}` — rename.
    pub async fn update_vault(&self, id: Uuid, req: &UpdateVaultRequest) -> Result<Vault> {
        self.patch(&format!("vaults/{id}"), req).await
    }

    /// `DELETE /vaults/{id}` — delete a team vault with everything in it.
    pub async fn delete_vault(&self, id: Uuid) -> Result<()> {
        self.delete(&format!("vaults/{id}")).await
    }

    /// `POST /vaults/{id}/rotate-key`.
    pub async fn rotate_vault_key(
        &self,
        id: Uuid,
        req: &RotateVaultKeyRequest,
    ) -> Result<RotateVaultKeyResponse> {
        self.post(&format!("vaults/{id}/rotate-key"), req).await
    }

    /// `GET /teams`.
    pub async fn teams(&self) -> Result<TeamList> {
        self.get("teams").await
    }

    /// `POST /teams`.
    pub async fn create_team(&self, req: &CreateTeamRequest) -> Result<Team> {
        self.post("teams", req).await
    }

    /// `GET /teams/{id}`.
    pub async fn team(&self, id: Uuid) -> Result<Team> {
        self.get(&format!("teams/{id}")).await
    }

    /// `PATCH /teams/{id}` — rename.
    pub async fn update_team(&self, id: Uuid, req: &UpdateTeamRequest) -> Result<Team> {
        self.patch(&format!("teams/{id}"), req).await
    }

    /// `DELETE /teams/{id}` — owner only; removes the team and its vaults.
    pub async fn delete_team(&self, id: Uuid) -> Result<()> {
        self.delete(&format!("teams/{id}")).await
    }

    /// `POST /teams/{id}/leave`.
    pub async fn leave_team(&self, id: Uuid) -> Result<()> {
        self.post_empty(&format!("teams/{id}/leave"), &()).await
    }

    /// `GET /teams/{id}/members`.
    pub async fn team_members(&self, id: Uuid) -> Result<TeamMemberList> {
        self.get(&format!("teams/{id}/members")).await
    }

    /// `PATCH /teams/{id}/members/{user_id}` — change a member's team role.
    pub async fn update_team_member(
        &self,
        id: Uuid,
        user_id: Uuid,
        req: &UpdateTeamMemberRequest,
    ) -> Result<()> {
        self.patch_empty(&format!("teams/{id}/members/{user_id}"), req)
            .await
    }

    /// `DELETE /teams/{id}/members/{user_id}`.
    pub async fn remove_team_member(&self, id: Uuid, user_id: Uuid) -> Result<()> {
        self.delete(&format!("teams/{id}/members/{user_id}")).await
    }

    /// Owner-only: delete the account of a member the team created
    /// (sign-up through its invitation). Requires a fresh step-up.
    pub async fn delete_team_member_account(&self, id: Uuid, user_id: Uuid) -> Result<()> {
        self.delete(&format!("teams/{id}/members/{user_id}/account"))
            .await
    }

    /// `GET /teams/{id}/invites` — pending invitations.
    pub async fn team_invites(&self, id: Uuid) -> Result<InviteList> {
        self.get(&format!("teams/{id}/invites")).await
    }

    /// `POST /teams/{id}/invites`.
    pub async fn create_invite(
        &self,
        id: Uuid,
        req: &CreateInviteRequest,
    ) -> Result<CreatedInvite> {
        self.post(&format!("teams/{id}/invites"), req).await
    }

    /// `DELETE /teams/{id}/invites/{invite_id}` — revoke.
    pub async fn delete_invite(&self, id: Uuid, invite_id: Uuid) -> Result<()> {
        self.delete(&format!("teams/{id}/invites/{invite_id}"))
            .await
    }

    /// `POST /teams/{id}/vaults` — create a team vault with pre-sealed member keys.
    pub async fn create_team_vault(&self, id: Uuid, req: &CreateVaultRequest) -> Result<Vault> {
        self.post(&format!("teams/{id}/vaults"), req).await
    }

    /// `POST /invites/{token}/accept` — join the team behind an invitation link.
    pub async fn accept_invite(&self, token: &str) -> Result<Team> {
        let token = token.trim();
        if token.is_empty()
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(CoreError::Invalid("malformed invitation token".into()));
        }
        self.post(&format!("invites/{token}/accept"), &()).await
    }

    /// `GET /teams/{id}/pending-keys`.
    pub async fn team_pending_keys(&self, team_id: Uuid) -> Result<PendingVaultKeys> {
        self.get(&format!("teams/{team_id}/pending-keys")).await
    }

    /// `GET /teams/{id}/presence` — who is connected to which team-vault host.
    pub async fn team_presence(&self, team_id: Uuid) -> Result<TeamPresence> {
        self.get(&format!("teams/{team_id}/presence")).await
    }

    /// `GET /teams/{id}/audit` — team activity log, newest first.
    pub async fn team_audit(&self, team_id: Uuid, q: &AuditQuery) -> Result<AuditEventList> {
        self.get_query(&format!("teams/{team_id}/audit"), q).await
    }

    // ───────────────────────────── sync ─────────────────────────────

    /// `POST /sync/push`.
    pub async fn sync_push(&self, req: &PushRequest) -> Result<PushResponse> {
        self.post("sync/push", req).await
    }

    /// `POST /sync/pull`.
    pub async fn sync_pull(&self, req: &PullRequest) -> Result<PullResponse> {
        self.post("sync/pull", req).await
    }

    /// `POST /history/push`.
    pub async fn history_push(&self, req: &HistoryPushRequest) -> Result<HistoryPullResponse> {
        self.post("history/push", req).await
    }

    /// `GET /history/pull?since=&limit=`.
    pub async fn history_pull(&self, since: i64, limit: u32) -> Result<HistoryPullResponse> {
        self.get_query(
            "history/pull",
            &[("since", since.to_string()), ("limit", limit.to_string())],
        )
        .await
    }

    /// `POST /history/clear`.
    pub async fn history_clear(&self, kind: Option<HistoryKind>) -> Result<()> {
        self.post_empty("history/clear", &HistoryClearRequest { kind })
            .await
    }

    // ───────────────────────────── session logs ─────────────────────────────

    /// `GET /logs?since=&limit=` — metadata only.
    pub async fn logs(&self, since: i64, limit: u32) -> Result<LogListResponse> {
        self.get_query(
            "logs",
            &[("since", since.to_string()), ("limit", limit.to_string())],
        )
        .await
    }

    /// `GET /vaults/{id}/logs?since=&limit=` — every member's logs in a
    /// (team) vault, paged by the vault counter.
    pub async fn vault_logs(
        &self,
        vault_id: Uuid,
        since: i64,
        limit: u32,
    ) -> Result<LogListResponse> {
        self.get_query(
            &format!("vaults/{vault_id}/logs"),
            &[("since", since.to_string()), ("limit", limit.to_string())],
        )
        .await
    }

    /// `POST /logs` — register a log and get a presigned upload URL.
    pub async fn create_log(&self, req: &CreateLogRequest) -> Result<CreateLogResponse> {
        self.post("logs", req).await
    }

    /// `PATCH /logs/{id}` — mark uploaded (with the exact byte size) or
    /// update metadata.
    pub async fn update_log(&self, id: Uuid, req: &UpdateLogRequest) -> Result<SessionLog> {
        self.patch(&format!("logs/{id}"), req).await
    }

    /// `GET /logs/{id}/download` — presigned download URL.
    pub async fn log_download_url(&self, id: Uuid) -> Result<DownloadLogResponse> {
        self.get(&format!("logs/{id}/download")).await
    }

    /// `DELETE /logs/{id}`.
    pub async fn delete_log(&self, id: Uuid) -> Result<()> {
        self.delete(&format!("logs/{id}")).await
    }

    // ---- multiplayer ------------------------------------------------------

    /// `POST /live` — register a live terminal session. The server only ever
    /// sees the join token (and stores its hash), never the link secret.
    pub async fn create_live_session(&self, join_token: String) -> Result<LiveSession> {
        self.post("live", &CreateLiveSessionRequest { join_token })
            .await
    }

    /// `GET /live` — sessions this account is hosting.
    pub async fn live_sessions(&self) -> Result<LiveSessionList> {
        self.get("live").await
    }

    /// `POST /live/{id}/stop`.
    pub async fn stop_live_session(&self, id: Uuid) -> Result<()> {
        self.post_empty(&format!("live/{id}/stop"), &()).await
    }

    /// PUT an already-encrypted log body to the presigned URL from
    /// [`ApiClient::create_log`]. No bearer token is sent (object storage
    /// authenticates via the URL).
    pub async fn upload_log_object(&self, target: &CreateLogResponse, body: Vec<u8>) -> Result<()> {
        let mut rb = self.http.put(&target.upload_url).body(body);
        for (k, v) in &target.upload_headers {
            rb = rb.header(k.as_str(), v.as_str());
        }
        let resp = rb.send().await?;
        if !resp.status().is_success() {
            return Err(CoreError::Api {
                status: resp.status().as_u16(),
                code: "storage_upload_failed".into(),
                message: format!("object storage answered {}", resp.status()),
            });
        }
        Ok(())
    }

    /// Fetch the encrypted body from a presigned download URL.
    pub async fn download_log_object(&self, target: &DownloadLogResponse) -> Result<Vec<u8>> {
        let resp = self.http.get(&target.download_url).send().await?;
        if !resp.status().is_success() {
            return Err(CoreError::Api {
                status: resp.status().as_u16(),
                code: "storage_download_failed".into(),
                message: format!("object storage answered {}", resp.status()),
            });
        }
        Ok(resp.bytes().await?.to_vec())
    }
}

pub(crate) fn default_user_agent() -> String {
    format!("termoso-core/{}", env!("CARGO_PKG_VERSION"))
}

/// Turn a user-typed server address into a base URL with a trailing slash.
/// Bare hosts get `https://`.
pub fn normalize_server_url(input: &str) -> Result<Url> {
    let s = input.trim();
    if s.is_empty() {
        return Err(CoreError::Invalid("empty server url".into()));
    }
    let with_scheme = if s.contains("://") {
        s.to_string()
    } else {
        format!("https://{s}")
    };
    let mut u =
        Url::parse(&with_scheme).map_err(|e| CoreError::Invalid(format!("server url: {e}")))?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err(CoreError::Invalid(format!(
            "unsupported scheme {}",
            u.scheme()
        )));
    }
    if u.host_str().is_none() {
        return Err(CoreError::Invalid("server url has no host".into()));
    }
    u.set_query(None);
    u.set_fragment(None);
    let mut path = u.path().trim_end_matches('/').to_string();
    if path.ends_with("/api/v1") {
        path.truncate(path.len() - "/api/v1".len());
    }
    path.push('/');
    u.set_path(&path);
    Ok(u)
}

async fn check(resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().await.unwrap_or_default();
    Err(api_error(status, &body))
}

fn api_error(status: StatusCode, body: &str) -> CoreError {
    match serde_json::from_str::<ApiError>(body) {
        Ok(e) => CoreError::Api {
            status: status.as_u16(),
            code: e.code,
            message: e.message,
        },
        Err(_) => CoreError::Api {
            status: status.as_u16(),
            code: if status == StatusCode::UNAUTHORIZED {
                termoso_proto::error::codes::UNAUTHORIZED.into()
            } else {
                "http_error".into()
            },
            message: if body.trim().is_empty() {
                status.to_string()
            } else {
                body.chars().take(200).collect()
            },
        },
    }
}

impl CoreError {
    /// Server error with this code?
    pub fn is_api_code(&self, wanted: &str) -> bool {
        matches!(self, CoreError::Api { code, .. } if code == wanted)
    }

    /// The session token was rejected — the caller should sign out.
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, CoreError::Api { status: 401, .. })
            || self.is_api_code(termoso_proto::error::codes::UNAUTHORIZED)
            || self.is_api_code(termoso_proto::error::codes::TOKEN_EXPIRED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_server_urls() {
        assert_eq!(
            normalize_server_url("termoso.example").unwrap().as_str(),
            "https://termoso.example/"
        );
        assert_eq!(
            normalize_server_url("http://localhost:8080/api/v1/")
                .unwrap()
                .as_str(),
            "http://localhost:8080/"
        );
        assert_eq!(
            normalize_server_url("https://host/prefix?x=1#f")
                .unwrap()
                .as_str(),
            "https://host/prefix/"
        );
        assert!(normalize_server_url("ftp://x").is_err());
        assert!(normalize_server_url("").is_err());
    }

    #[test]
    fn api_paths_keep_prefix() {
        let c = ApiClient::new("https://host/prefix").unwrap();
        assert_eq!(
            c.api_url("sync/push").as_str(),
            "https://host/prefix/api/v1/sync/push"
        );
        assert_eq!(c.ws_url().unwrap().as_str(), "wss://host/prefix/api/v1/ws");
        let c = ApiClient::new("http://127.0.0.1:1234").unwrap();
        assert_eq!(
            c.ws_url().unwrap().as_str(),
            "ws://127.0.0.1:1234/api/v1/ws"
        );
    }

    #[test]
    fn maps_error_bodies() {
        let e = api_error(
            StatusCode::CONFLICT,
            r#"{"code":"conflict","message":"Version mismatch"}"#,
        );
        assert!(e.is_api_code("conflict"));
        assert!(!e.is_unauthorized());
        let e = api_error(StatusCode::UNAUTHORIZED, "<html>nope</html>");
        assert!(e.is_unauthorized());
        let e = api_error(StatusCode::BAD_GATEWAY, "");
        assert!(matches!(e, CoreError::Api { status: 502, .. }));
    }
}
