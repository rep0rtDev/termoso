import { fetchBlob, http, requestWithStatus, upload } from "./client";
import type {
  AccountResponse,
  AdminStats,
  AdminTeam,
  AdminUpdateUserRequest,
  AdminUser,
  AuthResponse,
  Bridge,
  BridgeVaultKey,
  CreateBridgeResponse,
  Device,
  DeviceInfo,
  Invite,
  InvitePreview,
  LogListResponse,
  LoginStartResponse,
  MfaCredential,
  MfaStatus,
  Page,
  PasswordSetupFinishRequest,
  PendingVaultKey,
  ReauthStartResponse,
  RecoveryRotate,
  RecoveryStartResponse,
  RegisterFinishRequest,
  SecurityEvent,
  ServerInfo,
  ServerSettings,
  SessionLog,
  SettingsBlob,
  SsoProvider,
  SsoResult,
  SsoStartResponse,
  StartOverFinishRequest,
  StartOverRequestResponse,
  StartOverScheduled,
  StartOverStatus,
  Team,
  TeamMember,
  TeamRole,
  TotpSetupResponse,
  UserProfile,
  Vault,
  VaultMember,
  VaultMemberUpsert,
  VaultRole,
  WebauthnCredentialInfo,
  SealedKeyFor,
} from "./types";

const anon = { anonymous: true } as const;

export const serverApi = {
  info: () => http.get<ServerInfo>("/server/info", anon),
};

export const authApi = {
  registerStart: (email: string, opaque_request: string) =>
    http.post<{ opaque_response: string }>("/auth/register/start", { email, opaque_request }, anon),
  registerFinish: (req: RegisterFinishRequest) =>
    http.post<AuthResponse>("/auth/register/finish", req, anon),

  loginStart: (email: string, opaque_request: string, device: DeviceInfo, sso_session?: string) =>
    http.post<LoginStartResponse>(
      "/auth/login/start",
      { email, opaque_request, device, sso_session },
      anon,
    ),
  loginFinish: (login_id: string, opaque_finalization: string) =>
    http.post<AuthResponse>("/auth/login/finish", { login_id, opaque_finalization }, anon),

  mfaVerify: (mfa_token: string, credential: MfaCredential) =>
    http.post<AuthResponse>("/auth/mfa/verify", { mfa_token, credential }, anon),
  mfaWebauthnChallenge: (mfa_token: string) =>
    http.post<unknown>("/auth/mfa/webauthn/challenge", { mfa_token }, anon),
  mfaEmailSend: (mfa_token: string) =>
    http.post<undefined>("/auth/mfa/email/send", { mfa_token }, anon),

  deviceApprove: (approval_token: string, code: string) =>
    http.post<AuthResponse>("/auth/device/approve", { approval_token, code }, anon),
  deviceApproveResend: (approval_token: string) =>
    http.post<undefined>("/auth/device/approve/resend", { approval_token }, anon),

  recoveryStart: (email: string, recovery_verifier: string) =>
    http.post<RecoveryStartResponse>("/auth/recovery/start", { email, recovery_verifier }, anon),
  passwordStart: (opaque_request: string, recovery_token?: string) =>
    http.post<{ opaque_response: string }>(
      "/auth/password/start",
      { opaque_request, recovery_token },
      recovery_token ? anon : undefined,
    ),
  passwordFinish: (req: PasswordSetupFinishRequest) =>
    http.post<AuthResponse>("/auth/password/finish", req, req.recovery_token ? anon : undefined),

  logout: () => http.post<undefined>("/auth/logout"),

  reauthStart: (opaque_request?: string) =>
    http.post<ReauthStartResponse>("/auth/reauth/start", { opaque_request }),
  reauthFinish: (reauth_id: string, proof: { opaque_finalization?: string; code?: string }) =>
    http.post<AuthResponse>("/auth/reauth/finish", { reauth_id, ...proof }),

  startOverRequest: (email: string) =>
    http.post<StartOverRequestResponse>("/auth/start-over/request", { email }, anon),
  startOverConfirm: (request_token: string, code: string, mfa_code?: string) =>
    http.post<StartOverScheduled>(
      "/auth/start-over/confirm",
      { request_token, code, mfa_code },
      anon,
    ),
  startOverStatus: (token: string) =>
    http.get<StartOverStatus>(`/auth/start-over/${encodeURIComponent(token)}`, anon),
  startOverCancel: (cancel_token: string) =>
    http.post<undefined>("/auth/start-over/cancel", { cancel_token }, anon),
  startOverPasswordStart: (token: string, opaque_request: string) =>
    http.post<{ opaque_response: string }>(
      "/auth/start-over/password/start",
      { token, opaque_request },
      anon,
    ),
  startOverFinish: (req: StartOverFinishRequest) =>
    http.post<AuthResponse>("/auth/start-over/finish", req, anon),

  ssoProviders: () => http.get<SsoProvider[]>("/auth/sso/providers", anon),
  ssoStart: (provider: string, redirect: string) =>
    http.get<SsoStartResponse>(`/auth/sso/${encodeURIComponent(provider)}/start`, {
      ...anon,
      query: { redirect },
    }),
  ssoPoll: (flow_id: string) =>
    http.get<SsoResult>(`/auth/sso/flow/${encodeURIComponent(flow_id)}`, anon),
};

export const accountApi = {
  get: (token?: string) => http.get<AccountResponse>("/account", token ? { token } : undefined),
  updateProfile: (display_name: string | null) =>
    http.patch<UserProfile>("/account/profile", { display_name }),
  putAvatar: (image: Blob) => upload<UserProfile>("PUT", "/account/avatar", image),
  deleteAvatar: () => http.delete<UserProfile>("/account/avatar"),
  /** `GET /users/{id}/avatar?v={tag}` — the tag pins the URL so the browser's HTTP cache never serves a replaced picture. */
  avatar: (user_id: string, tag: string) =>
    fetchBlob(`/users/${encodeURIComponent(user_id)}/avatar?v=${encodeURIComponent(tag)}`),
  emailVerifySend: () => http.post<undefined>("/account/email/verify/send"),
  emailVerifyConfirm: (code: string) =>
    http.post<undefined>("/account/email/verify/confirm", { code }),
  emailChange: (new_email: string) => http.post<undefined>("/account/email/change", { new_email }),
  emailChangeConfirm: (code: string) =>
    http.post<undefined>("/account/email/change/confirm", { code }),
  settings: () => http.get<SettingsBlob>("/account/settings"),
  putSettings: (data: string, base_version: number) =>
    http.put<SettingsBlob>("/account/settings", { data, base_version }),
  devices: () => http.get<{ devices: Device[] }>("/account/devices"),
  revokeDevice: (id: string) => http.delete<undefined>(`/account/devices/${id}`),
  securityEvents: () => http.get<{ events: SecurityEvent[] }>("/account/security-events"),
  rotateRecovery: (req: RecoveryRotate) => http.post<undefined>("/account/recovery/rotate", req),
  cancelStartOver: () => http.post<undefined>("/account/start-over/cancel"),
  /** Resolves to `"code_sent"` (202: a confirmation code was mailed) or `"deleted"` (204). */
  delete: async (code?: string): Promise<"code_sent" | "deleted"> => {
    const r = await requestWithStatus<undefined>(
      "DELETE",
      "/account",
      code === undefined ? undefined : { code },
    );
    return r.status === 202 ? "code_sent" : "deleted";
  },

  mfa: () => http.get<MfaStatus>("/account/mfa"),
  totpSetup: () => http.post<TotpSetupResponse>("/account/mfa/totp/setup"),
  totpConfirm: (code: string) =>
    http.post<{ codes: string[] }>("/account/mfa/totp/confirm", { code }),
  totpDisable: (code: string) => http.delete<undefined>("/account/mfa/totp", { code }),
  backupCodes: () => http.post<{ codes: string[] }>("/account/mfa/backup-codes"),
  webauthnRegisterStart: () => http.post<unknown>("/account/mfa/webauthn/register/start"),
  webauthnRegisterFinish: (name: string, credential: unknown) =>
    http.post<WebauthnCredentialInfo>("/account/mfa/webauthn/register/finish", {
      name,
      credential,
    }),
  webauthnDelete: (id: string) => http.delete<undefined>(`/account/mfa/webauthn/${id}`),
};

export const teamsApi = {
  list: () => http.get<{ teams: Team[] }>("/teams"),
  create: (name: string) => http.post<Team>("/teams", { name }),
  get: (id: string) => http.get<Team>(`/teams/${id}`),
  update: (id: string, name: string) => http.patch<Team>(`/teams/${id}`, { name }),
  delete: (id: string) => http.delete<undefined>(`/teams/${id}`),
  leave: (id: string) => http.post<undefined>(`/teams/${id}/leave`),
  members: (id: string) => http.get<{ members: TeamMember[] }>(`/teams/${id}/members`),
  updateMember: (id: string, userId: string, role: TeamRole) =>
    http.patch<TeamMember>(`/teams/${id}/members/${userId}`, { role }),
  removeMember: (id: string, userId: string) =>
    http.delete<undefined>(`/teams/${id}/members/${userId}`),
  invites: (id: string) => http.get<{ invites: Invite[] }>(`/teams/${id}/invites`),
  createInvite: (id: string, email: string, role: TeamRole, vault_ids: string[]) =>
    http.post<Invite & { url: string }>(`/teams/${id}/invites`, { email, role, vault_ids }),
  deleteInvite: (id: string, inviteId: string) =>
    http.delete<undefined>(`/teams/${id}/invites/${inviteId}`),
  pendingKeys: (id: string) => http.get<{ items: PendingVaultKey[] }>(`/teams/${id}/pending-keys`),
  createVault: (id: string, name: string, members: VaultMemberUpsert[]) =>
    http.post<Vault>(`/teams/${id}/vaults`, { name, members }),
  invitePreview: (token: string) =>
    http.get<InvitePreview>(`/invites/${encodeURIComponent(token)}`, anon),
  acceptInvite: (token: string) => http.post<Team>(`/invites/${encodeURIComponent(token)}/accept`),
};

export const logsApi = {
  /** Pin / comment (editors), or fix up one's own recording (author). */
  update: (id: string, patch: { pinned?: boolean; note?: string }) =>
    http.patch<SessionLog>(`/logs/${id}`, patch),
  delete: (id: string) => http.delete<undefined>(`/logs/${id}`),
};

export const bridgesApi = {
  list: () => http.get<{ bridges: Bridge[] }>("/account/bridges"),
  create: (name: string, public_key: string, vaults: BridgeVaultKey[]) =>
    http.post<CreateBridgeResponse>("/account/bridges", { name, public_key, vaults }),
  setVaults: (id: string, vaults: BridgeVaultKey[]) =>
    http.put<Bridge>(`/account/bridges/${id}/vaults`, vaults),
  revoke: (id: string) => http.delete<undefined>(`/account/bridges/${id}`),
};

export const vaultsApi = {
  list: () => http.get<{ vaults: Vault[] }>("/vaults"),
  get: (id: string) => http.get<Vault>(`/vaults/${id}`),
  update: (id: string, patch: { name?: string; session_logging?: boolean }) =>
    http.patch<Vault>(`/vaults/${id}`, patch),
  delete: (id: string) => http.delete<undefined>(`/vaults/${id}`),
  /** Recordings every member of a team vault shares, oldest first from `since`. */
  logs: (id: string, since = 0, limit = 100) =>
    http.get<LogListResponse>(`/vaults/${id}/logs?since=${since}&limit=${limit}`),
  members: (id: string) => http.get<{ members: VaultMember[] }>(`/vaults/${id}/members`),
  upsertMember: (id: string, userId: string, role: VaultRole, sealed_key: string) =>
    http.put<VaultMember>(`/vaults/${id}/members/${userId}`, { user_id: userId, role, sealed_key }),
  removeMember: (id: string, userId: string) =>
    http.delete<undefined>(`/vaults/${id}/members/${userId}`),
  rotateKey: (id: string, base_key_version: number, members: SealedKeyFor[]) =>
    http.post<{ key_version: number }>(`/vaults/${id}/rotate-key`, { base_key_version, members }),
};

export interface ListParams {
  q?: string;
  offset?: number;
  limit?: number;
}

export const adminApi = {
  stats: () => http.get<AdminStats>("/admin/stats"),
  users: async (p: ListParams): Promise<Page<AdminUser>> => {
    const r = await http.get<{ users: AdminUser[]; total: number }>("/admin/users", { query: p });
    return { items: r.users, total: r.total };
  },
  user: (id: string) => http.get<AdminUser>(`/admin/users/${id}`),
  updateUser: (id: string, req: AdminUpdateUserRequest) =>
    http.patch<AdminUser>(`/admin/users/${id}`, req),
  deleteUser: (id: string) => http.delete<undefined>(`/admin/users/${id}`),
  revokeSessions: (id: string) => http.post<undefined>(`/admin/users/${id}/revoke-sessions`),
  resetMfa: (id: string) => http.post<undefined>(`/admin/users/${id}/reset-mfa`),
  teams: async (p: ListParams): Promise<Page<AdminTeam>> => {
    const r = await http.get<{ teams: AdminTeam[]; total: number }>("/admin/teams", { query: p });
    return { items: r.teams, total: r.total };
  },
  deleteTeam: (id: string) => http.delete<undefined>(`/admin/teams/${id}`),
  settings: () => http.get<ServerSettings>("/admin/settings"),
  putSettings: (s: ServerSettings) => http.put<ServerSettings>("/admin/settings", s),
  testEmail: (to: string) => http.post<undefined>("/admin/email/test", { to }),
};
