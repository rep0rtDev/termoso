/** Wire types mirroring `crates/termoso-proto`. Dates are RFC 3339 strings. */

export type Platform = "windows" | "linux" | "macos" | "android" | "ios" | "web" | "cli";

export interface DeviceInfo {
  name: string;
  platform: Platform;
  app_version: string;
  client_device_id?: string;
}

export interface ApiErrorBody {
  code: string;
  message: string;
  details?: unknown;
}

// ── server ──────────────────────────────────────────────────────────────

export interface ServerFeatures {
  session_logs: boolean;
  email: boolean;
  webauthn: boolean;
  teams: boolean;
}

export type SsoKind = "oidc" | "saml";

export interface SsoProvider {
  id: string;
  name: string;
  kind: SsoKind;
}

export interface ServerInfo {
  name: string;
  version: string;
  registration_open: boolean;
  sso_providers: SsoProvider[];
  features: ServerFeatures;
  max_entity_bytes: number;
  max_log_bytes: number;
  /** Base under which SSH ID handles are published (`<sshid_url>/<handle>`). */
  sshid_url: string;
}

// ── account ─────────────────────────────────────────────────────────────

export interface UserProfile {
  id: string;
  email: string;
  email_verified: boolean;
  display_name?: string;
  created_at: string;
  is_admin: boolean;
  mfa_enabled: boolean;
  /** A destructive "start over" reset is scheduled for this moment (cancellable until then). */
  reset_scheduled_for?: string | null;
  /** Content tag of the profile picture (`GET /users/{id}/avatar`); absent when none. */
  avatar?: string;
}

export interface AccountKeys {
  public_key: string;
  wrapped_private_key: string;
  key_version: number;
}

export interface AccountResponse {
  user: UserProfile;
  keys: AccountKeys;
}

export interface UpdateProfileRequest {
  display_name: string | null;
}

export interface SecurityEvent {
  id: string;
  kind: string;
  device_id?: string;
  ip?: string;
  user_agent?: string;
  details?: unknown;
  created_at: string;
}

export interface SettingsBlob {
  data: string;
  version: number;
  updated_at: string;
}

// ── auth ────────────────────────────────────────────────────────────────

export interface AccountKeysUpload {
  public_key: string;
  wrapped_private_key: string;
  recovery_wrapped_private_key: string;
  recovery_verifier: string;
  personal_vault_sealed_key: string;
}

export interface RegisterFinishRequest {
  email: string;
  opaque_upload: string;
  display_name?: string;
  device: DeviceInfo;
  keys: AccountKeysUpload;
  invite_token?: string;
  sso_session?: string;
}

export interface LoginStartResponse {
  login_id: string;
  opaque_response: string;
}

export type MfaMethod = "totp" | "webauthn" | "backup_code" | "email";

export type MfaCredential =
  | { method: "totp"; code: string }
  | { method: "backup_code"; code: string }
  | { method: "email"; code: string }
  | { method: "webauthn"; credential: unknown };

export interface Session {
  token: string;
  expires_at: string;
  device_id: string;
  user: UserProfile;
  keys: AccountKeys;
}

export type AuthResponse =
  | ({ status: "authenticated" } & Session)
  | { status: "mfa_required"; mfa_token: string; methods: MfaMethod[] }
  | { status: "device_approval_required"; approval_token: string; email_hint: string }
  | { status: "reauthenticated"; reauth_expires_at: string };

export const REAUTH_REQUIRED = "reauth_required";

export type ReauthMethod = "password" | "email" | "none";

export interface ReauthStartResponse {
  reauth_id: string;
  method: ReauthMethod;
  opaque_response?: string | null;
  email_hint?: string | null;
}

export interface StartOverRequestResponse {
  request_token: string;
  email_hint: string;
}

export interface StartOverScheduled {
  scheduled_for: string;
  email_hint: string;
}

export interface StartOverStatus {
  email_hint: string;
  email: string;
  scheduled_for: string;
  ready: boolean;
}

export interface StartOverFinishRequest {
  token: string;
  opaque_upload: string;
  keys: AccountKeysUpload;
  device: DeviceInfo;
}

export interface RecoveryRotate {
  recovery_wrapped_private_key: string;
  recovery_verifier: string;
}

export interface PasswordSetupFinishRequest {
  recovery_token?: string;
  opaque_upload: string;
  wrapped_private_key: string;
  new_recovery?: RecoveryRotate;
  revoke_other_sessions: boolean;
  device?: DeviceInfo;
}

export interface RecoveryStartResponse {
  recovery_token: string;
  recovery_wrapped_private_key: string;
  public_key: string;
}

export interface Device {
  id: string;
  name: string;
  platform: Platform;
  app_version: string;
  created_at: string;
  last_seen_at: string;
  last_ip?: string;
  current: boolean;
}

export interface WebauthnCredentialInfo {
  id: string;
  name: string;
  created_at: string;
  last_used_at?: string;
}

export interface MfaStatus {
  totp_enabled: boolean;
  webauthn_credentials: WebauthnCredentialInfo[];
  backup_codes_remaining: number;
}

export interface TotpSetupResponse {
  secret: string;
  otpauth_url: string;
}

export interface SsoStartResponse {
  authorization_url: string;
  flow_id: string;
}

export type SsoResult =
  | { status: "pending" }
  | { status: "login_required"; sso_session: string; email: string }
  | { status: "registration_required"; sso_session: string; email: string; display_name?: string }
  | { status: "failed"; message: string };

// ── teams ───────────────────────────────────────────────────────────────

export type TeamRole = "member" | "admin" | "owner";

export interface Team {
  id: string;
  name: string;
  created_at: string;
  my_role: TeamRole;
  member_count: number;
  require_mfa?: boolean;
}

export interface TeamMember {
  user_id: string;
  email: string;
  display_name?: string;
  avatar?: string;
  role: TeamRole;
  public_key: string;
  joined_at: string;
  /** Second factor enrolled; only disclosed to team admins and the member themself. */
  mfa_enabled?: boolean | null;
  /** Account was created by accepting this team's invitation; the owner may delete it. */
  managed?: boolean;
}

export interface Invite {
  id: string;
  email: string;
  role: TeamRole;
  invited_by: string;
  created_at: string;
  expires_at: string;
}

export interface InvitePreview {
  team_name: string;
  inviter: string;
  email: string;
  role: TeamRole;
  account_exists: boolean;
}

export interface PendingVaultKey {
  vault_id: string;
  user_id: string;
  public_key: string;
  role: VaultRole;
}

// ── vaults ──────────────────────────────────────────────────────────────

export type VaultKind = "personal" | "team";
export type VaultRole = "viewer" | "editor" | "manager";

export interface Vault {
  id: string;
  kind: VaultKind;
  team_id?: string;
  name: string;
  created_at: string;
  my_role: VaultRole;
  sealed_key?: string;
  key_version: number;
  /** Team vault: a manager turned on recording of every member's sessions. */
  session_logging: boolean;
}

/** Who recorded a session log, as the server shows it to vault members. */
export interface LogAuthor {
  user_id: string;
  email: string;
  display_name?: string;
  avatar_tag?: string;
}

/** A session recording as listed by the server; `meta` and the body are vault-key ciphertext. */
export interface SessionLog {
  id: string;
  vault_id: string;
  user_id: string;
  author?: LogAuthor;
  meta: string;
  key_version: number;
  size_bytes: number;
  completed: boolean;
  created_at: string;
  seq: number;
  deleted: boolean;
  pinned: boolean;
  note: string;
  note_by?: string;
}

export interface LogListResponse {
  logs: SessionLog[];
  since: number;
  has_more: boolean;
}

/** Plaintext of `SessionLog.meta` once decrypted with the vault key. */
export interface LogMeta {
  host_id?: string;
  label: string;
  target: string;
  protocol: string;
  started_at: string;
  ended_at?: string;
  cols: number;
  rows: number;
}

export interface VaultMember {
  user_id: string;
  email: string;
  display_name?: string;
  avatar?: string;
  role: VaultRole;
  key_version: number;
  pending: boolean;
  public_key: string;
  added_at: string;
}

export interface VaultMemberUpsert {
  user_id: string;
  role: VaultRole;
  sealed_key: string;
}

export interface SealedKeyFor {
  user_id: string;
  sealed_key: string;
}

// ── admin ───────────────────────────────────────────────────────────────

export interface AdminStats {
  users: number;
  active_users_30d: number;
  teams: number;
  vaults: number;
  entities: number;
  active_sessions: number;
  log_storage_bytes: number;
}

export interface AdminUser {
  id: string;
  email: string;
  email_verified: boolean;
  display_name?: string;
  is_admin: boolean;
  disabled: boolean;
  mfa_enabled: boolean;
  created_at: string;
  last_seen_at?: string;
  devices: number;
}

export interface AdminUpdateUserRequest {
  disabled?: boolean;
  is_admin?: boolean;
  email_verified?: boolean;
}

export interface ServerSettings {
  registration_open: boolean;
  allowed_domains: string[];
  require_email_verification: boolean;
  new_device_email_approval: boolean;
  users_can_create_teams: boolean;
  session_ttl_days: number;
  max_entity_bytes: number;
  max_log_bytes: number;
  log_quota_bytes: number;
  audit_retention_days: number;
}

export interface AdminTeam {
  id: string;
  name: string;
  owner_email: string;
  member_count: number;
  vault_count: number;
  created_at: string;
}

export interface Page<T> {
  items: T[];
  total: number;
}

// ── API bridges ─────────────────────────────────────────────────────────

/** A vault an API bridge may write to; `sealed_key` is absent after a key rotation until re-sealed. */
export interface BridgeVault {
  vault_id: string;
  name: string;
  kind: VaultKind;
  team_id?: string;
  role: VaultRole;
  key_version: number;
  sealed_key?: string;
}

export interface Bridge {
  id: string;
  name: string;
  device_id: string;
  public_key: string;
  vaults: BridgeVault[];
  created_at: string;
  last_used_at?: string;
  last_ip?: string;
}

export interface BridgeVaultKey {
  vault_id: string;
  sealed_key: string;
}

export interface CreateBridgeResponse {
  bridge: Bridge;
  /** Shown once; the server keeps only a hash. */
  token: string;
}

/** `termoso-bridge.json` mounted into the bridge container. */
export interface BridgeCredentials {
  version: 1;
  server: string;
  bridge_id: string;
  private_key: string;
  token: string;
}
