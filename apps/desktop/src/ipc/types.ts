// Mirrors of the Rust IPC types (apps/desktop/src-tauri/src/*.rs). These are
// shapes only; all validation and domain logic lives in Rust.

export type Uuid = string;

export interface DesktopError {
  kind: string;
  message: string;
}

export function isDesktopError(e: unknown): e is DesktopError {
  return (
    typeof e === "object" &&
    e !== null &&
    typeof (e as DesktopError).kind === "string" &&
    typeof (e as DesktopError).message === "string"
  );
}

export function errorMessage(e: unknown): string {
  if (isDesktopError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

export type MasterKeySource = "keychain" | "file";

export interface AppInfo {
  version: string;
  profileDir: string;
  deviceId: Uuid;
  masterKeySource: MasterKeySource;
  signedIn: boolean;
  platform: string;
}

export type ThemeMode = "dark" | "light" | "system";
export type HostsView = "grid" | "list";
export type CursorStyle = "block" | "underline" | "bar";
export type SyncConflict = "newest_wins" | "local_wins" | "server_wins";

export interface Settings {
  theme: ThemeMode;
  hostsView: HostsView;
  terminalFontSize: number;
  terminalFontFamily: string;
  /** Line height multiplier (1.0 = natural). */
  terminalLineHeight: number;
  /** Colour scheme id; `auto` follows `theme` with Termoso Dark / Light. */
  terminalTheme: string;
  cursorBlink: boolean;
  cursorStyle: CursorStyle;
  scrollback: number;
  copyOnSelect: boolean;
  pasteOnRightClick: boolean;
  confirmCloseTab: boolean;
  confirmPasteMultiline: boolean;
  autocomplete: boolean;
  terminalBell: boolean;
  keepAliveSeconds: number;
  /** Probe the OS after the first successful connection to pick the host icon. */
  detectOs: boolean;
  /** Offer hybrid ML-KEM-768 + X25519 key exchange. */
  postQuantumKex: boolean;
  /** Offer keys from the system SSH agent (SSH_AUTH_SOCK / Pageant) when authenticating. */
  useSshAgent: boolean;
  recordSessions: boolean;
  logRetentionDays: number;
  autostartForwarding: boolean;
  syncConflict: SyncConflict;
  syncIntervalSeconds: number;
  uploadLogs: boolean;
  updateCheck: UpdateCheck;
  /** Release feed URL; empty = project default. */
  updateUrl: string;
}

export type UpdateCheck = "manual" | "startup";

export interface UpdateInfo {
  currentVersion: string;
  version: string;
  notes: string | null;
  publishedAt: string | null;
  downloadUrl: string;
  target: string;
}

export type UpdateEvent =
  | { type: "available"; info: UpdateInfo }
  | { type: "progress"; downloaded: number; total: number | null }
  | { type: "installed"; version: string }
  | { type: "failed"; message: string };

export type LocalVaultKind = "local" | "personal" | "team";
export type VaultRole = "viewer" | "editor" | "manager";

export interface LocalVault {
  id: Uuid;
  kind: LocalVaultKind;
  name: string;
  team_id: Uuid | null;
  role: VaultRole;
  unlocked: boolean;
  key_version: number;
  cursor: number;
}

export interface HostCard {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  address: string;
  groupId: Uuid | null;
  groupPath: string[];
  protocol: "ssh" | "telnet";
  username: string;
  port: number;
  tags: string[];
  osName: string | null;
  notes: string;
  sortOrder: number;
  updatedAt: string;
  lastConnected: string | null;
  dirty: boolean;
}

export interface HostForm {
  id: Uuid | null;
  vaultId: Uuid;
  label: string;
  address: string;
  groupId: Uuid | null;
  port: number | null;
  username: string;
  password: string | null;
  sshKeyId: Uuid | null;
  identityId: Uuid | null;
  tagIds: Uuid[];
  notes: string;
  osName: string | null;
  agentForwarding: boolean;
  startupSnippetId: Uuid | null;
  hostChainId: Uuid | null;
  proxyId: Uuid | null;
  protocol: HostProtocol;
  envVariables: [string, string][];
  keepAliveInterval: number | null;
  timeout: number | null;
  /** Terminal colour scheme id; null follows the app setting. */
  colorScheme: string | null;
  hasPassword: boolean;
}

export type HostProtocol = "ssh" | "telnet";

/** Raw `proxy` entity payload. */
export interface ProxyData {
  kind: "socks4" | "socks5" | "http";
  host: string;
  port: number;
  identity_id?: Uuid | null;
}

/** Raw `host_chain` entity payload. */
export interface HostChainData {
  label: string;
  host_ids: Uuid[];
}

export function emptyHostForm(vaultId: Uuid, groupId: Uuid | null): HostForm {
  return {
    id: null,
    vaultId,
    label: "",
    address: "",
    groupId,
    port: null,
    username: "",
    password: null,
    sshKeyId: null,
    identityId: null,
    tagIds: [],
    notes: "",
    osName: null,
    agentForwarding: false,
    startupSnippetId: null,
    hostChainId: null,
    proxyId: null,
    protocol: "ssh",
    envVariables: [],
    keepAliveInterval: null,
    timeout: null,
    colorScheme: null,
    hasPassword: false,
  };
}

export interface GroupNode {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  parentId: Uuid | null;
  sortOrder: number;
  hostCount: number;
  groupCount: number;
  hasConfig: boolean;
}

/** Group editor model: name, parent and the SSH defaults its hosts inherit. */
export interface GroupForm {
  id: Uuid | null;
  vaultId: Uuid;
  label: string;
  parentId: Uuid | null;
  port: number | null;
  username: string;
  password: string | null;
  sshKeyId: Uuid | null;
  identityId: Uuid | null;
  hasPassword: boolean;
  agentForwarding: boolean;
  hostChainId: Uuid | null;
  proxyId: Uuid | null;
  envVariables: [string, string][];
  keepAliveInterval: number | null;
  timeout: number | null;
}

export function emptyGroupForm(vaultId: Uuid, parentId: Uuid | null): GroupForm {
  return {
    id: null,
    vaultId,
    label: "",
    parentId,
    port: null,
    username: "",
    password: null,
    sshKeyId: null,
    identityId: null,
    hasPassword: false,
    agentForwarding: false,
    hostChainId: null,
    proxyId: null,
    envVariables: [],
    keepAliveInterval: null,
    timeout: null,
  };
}

/** What a host inherits from its group chain (placeholders in the editor). */
export interface Inherited {
  groupPath: string[];
  port: number | null;
  username: string | null;
  hasPassword: boolean;
  sshKeyId: Uuid | null;
  sshKeyLabel: string | null;
  identityId: Uuid | null;
  identityLabel: string | null;
  agentForwarding: boolean;
  hostChainId: Uuid | null;
  proxyId: Uuid | null;
  keepAliveInterval: number | null;
  timeout: number | null;
  envVariables: [string, string][];
}

export interface TagInfo {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  color: string | null;
}

export interface Entity<T> {
  id: Uuid;
  vault_id: Uuid;
  version: number;
  updated_at: string;
  dirty: boolean;
  data: T;
}

// ───────────────────────────── keychain ─────────────────────────────

/** Public view of a stored key. Never carries private material. */
export interface KeyCard {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  keyType: string;
  bits: number;
  fingerprint: string;
  publicKey: string;
  comment: string;
  encrypted: boolean;
  hasPassphrase: boolean;
  unreadable: boolean;
  usedBy: number;
  updatedAt: string;
  dirty: boolean;
}

export type KeyAlgorithm =
  "ed25519" | { rsa: { bits: number } } | "ecdsa_p256" | "ecdsa_p384" | "ecdsa_p521";

export type ExportOutcome = "added" | "already_present";

export interface ExportToHostResult {
  outcome: ExportOutcome;
  hostLabel: string;
  /** `user@host:port` the key was installed for. */
  target: string;
}

/** A key held by the system SSH agent (public half only). */
export interface AgentKey {
  keyType: string;
  fingerprint: string;
  publicKey: string;
  comment: string;
}

export interface AgentKeys {
  available: boolean;
  error: string | null;
  keys: AgentKey[];
}

export interface GenerateKeyForm {
  vaultId: Uuid;
  label: string;
  algorithm: KeyAlgorithm;
  comment: string;
  passphrase: string | null;
  rememberPassphrase: boolean;
}

export interface ImportKeyForm {
  vaultId: Uuid;
  label: string;
  privateKey: string;
  passphrase: string | null;
  rememberPassphrase: boolean;
}

export interface IdentityCard {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  username: string;
  hasPassword: boolean;
  sshKeyId: Uuid | null;
  sshKeyLabel: string | null;
  updatedAt: string;
}

export interface IdentityForm {
  id: Uuid | null;
  vaultId: Uuid;
  label: string;
  username: string;
  /** `null` keeps the stored password; `""` clears it. */
  password: string | null;
  sshKeyId: Uuid | null;
}

// ───────────────────────────── port forwarding ─────────────────────────────

export type PfKind = "local" | "remote" | "dynamic";
export type PfState = "stopped" | "starting" | "running";

export interface PfRuntime {
  state: PfState;
  startedAt: string | null;
  bound: string | null;
  connections: number;
  active: number;
  bytesIn: number;
  bytesOut: number;
  lastError: string | null;
}

export interface PfRuleCard {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  hostId: Uuid;
  hostLabel: string;
  kind: PfKind;
  boundAddress: string;
  localPort: number;
  remoteHost: string;
  remotePort: number;
  autoStart: boolean;
  updatedAt: string;
  runtime: PfRuntime;
}

export interface PfRuleForm {
  id: Uuid | null;
  vaultId: Uuid;
  label: string;
  hostId: Uuid;
  kind: PfKind;
  boundAddress: string;
  localPort: number;
  remoteHost: string;
  remotePort: number;
  autoStart: boolean;
}

export interface ForwardEvent {
  type: "changed";
  id: Uuid;
  runtime: PfRuntime;
}

// ───────────────────────────── snippets ─────────────────────────────

export interface SnippetCard {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  script: string;
  packageId: Uuid | null;
  closeAfterRun: boolean;
  sortOrder: number;
  variables: string[];
  updatedAt: string;
  dirty: boolean;
}

export interface SnippetForm {
  id: Uuid | null;
  vaultId: Uuid;
  label: string;
  script: string;
  packageId: Uuid | null;
  closeAfterRun: boolean;
  sortOrder: number;
}

export interface PackageNode {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  parentId: Uuid | null;
  snippetCount: number;
}

export interface RunResult {
  sessionIds: Uuid[];
  closeAfterRun: boolean;
}

// ───────────────────────────── known hosts ─────────────────────────────

export interface KnownHostCard {
  id: Uuid;
  vaultId: Uuid;
  hostname: string;
  keyType: string;
  fingerprint: string;
  publicKey: string;
  updatedAt: string;
}

export interface ImportReport {
  added: number;
}

// ───────────────────────────── session logs ─────────────────────────────

export interface LogCard {
  id: Uuid;
  vaultId: Uuid;
  hostId: Uuid | null;
  label: string;
  target: string;
  protocol: string;
  startedAt: string;
  endedAt: string | null;
  durationSecs: number | null;
  cols: number;
  rows: number;
  sizeBytes: number;
  cached: boolean;
  uploaded: boolean;
  completed: boolean;
  createdAt: string;
  bookmarks: number;
}

export interface BookmarkCard {
  id: Uuid;
  logId: Uuid;
  offset: number;
  note: string;
  updatedAt: string;
}

export interface LogBody {
  id: Uuid;
  text: string;
  bytes: number;
}

// ───────────────────────────── account / sync ─────────────────────────────

export interface AccountCard {
  serverUrl: string;
  userId: Uuid;
  email: string;
  displayName: string | null;
  isAdmin: boolean;
  deviceId: Uuid;
  signedInAt: string;
}

export type MfaMethod = "totp" | "webauthn" | "email" | "backup_code";

export type MfaCredential =
  | { method: "totp"; code: string }
  | { method: "backup_code"; code: string }
  | { method: "webauthn"; credential: unknown }
  | { method: "email"; code: string };

export type LoginOutcome =
  | { step: "done"; account: AccountCard }
  | { step: "mfaRequired"; methods: MfaMethod[] }
  | { step: "deviceApprovalRequired"; emailHint: string };

export interface Registered {
  account: AccountCard;
  /** Shown once; Rust never stores it. */
  recoveryPhrase: string;
}

export type SyncState = "idle" | "syncing" | "offline" | "error";

export interface SyncStatus {
  state: SyncState;
  realtime: boolean;
  lastSyncAt: string | null;
  lastError: string | null;
  pushed: number;
  pulled: number;
  conflicts: number;
}

export interface AccountStatus {
  account: AccountCard | null;
  pending: LoginOutcome | null;
  sync: SyncStatus;
  vaults: LocalVault[];
}

export type SyncNotice =
  | { kind: "status"; status: SyncStatus }
  | { kind: "entitiesChanged"; vaultId: Uuid }
  | { kind: "vaultsChanged" }
  | { kind: "historyChanged" }
  | { kind: "logsChanged" }
  | { kind: "accountChanged" }
  | { kind: "signedOut" };

export interface LoginForm {
  serverUrl: string;
  email: string;
  password: string;
}

export interface RegisterForm {
  serverUrl: string;
  email: string;
  password: string;
  displayName: string | null;
  inviteToken: string | null;
}

export type Platform = "windows" | "linux" | "macos" | "android" | "ios" | "web" | "cli";

export interface Device {
  id: Uuid;
  name: string;
  platform: Platform;
  app_version: string;
  created_at: string;
  last_seen_at: string;
  last_ip?: string | null;
  current: boolean;
}

export interface SsoProvider {
  id: string;
  name: string;
  kind: "oidc" | "saml";
}

export interface ServerInfo {
  name: string;
  version: string;
  registration_open: boolean;
  sso_providers: SsoProvider[];
  features: {
    session_logs: boolean;
    email: boolean;
    webauthn: boolean;
    teams: boolean;
  };
  max_entity_bytes: number;
  max_log_bytes: number;
}

export interface ConnectionHistory {
  host_id: Uuid | null;
  label: string;
  target: string;
  protocol: string;
  duration_secs: number | null;
  error: string | null;
}

export interface HistoryItem<T> {
  id: Uuid;
  created_at: string;
  data: T;
}

export type SessionState = "connecting" | "connected";

export interface SessionInfo {
  id: Uuid;
  protocol: "ssh" | "telnet" | "local";
  title: string;
  target: string;
  hostId: Uuid | null;
  startedAt: string;
  state: SessionState;
  /** Negotiated SSH algorithms (null for telnet/local or before key exchange). */
  algorithms: SshAlgorithms | null;
  /** Jump hosts the connection went through, outermost first (`user@host:port`). */
  via: string[];
  /** Colour scheme configured on the host (or inherited); null follows the app setting. */
  colorScheme: string | null;
}

export interface SshAlgorithms {
  kex: string;
  hostKey: string;
  cipher: string;
  mac: string;
}

/** Hybrid / post-quantum key exchanges announce themselves in the name. */
export function isPostQuantumKex(a: SshAlgorithms | null): boolean {
  return !!a && (a.kex.includes("mlkem") || a.kex.includes("sntrup"));
}

export type SessionEvent =
  | { type: "connecting"; id: Uuid; info: SessionInfo }
  | { type: "connected"; id: Uuid; info: SessionInfo }
  | { type: "notice"; id: Uuid; message: string }
  | { type: "exit"; id: Uuid; code: number | null; signal: string | null }
  | { type: "error"; id: Uuid; message: string }
  | { type: "closed"; id: Uuid };

export type OpenTarget =
  | { kind: "host"; host_id: Uuid }
  | { kind: "quick"; address: string; username?: string | null; port?: number | null }
  | { kind: "local" };

export interface HostKeyInfo {
  host: string;
  key_type: string;
  fingerprint: string;
  public_key: string;
}

export type HostKeyVerdict =
  | { status: "known" }
  | { status: "unknown"; key: HostKeyInfo }
  | { status: "changed"; old: HostKeyInfo; new: HostKeyInfo };

export interface Question {
  prompt: string;
  echo: boolean;
}

export type PromptRequest =
  | { kind: "host_key"; verdict: HostKeyVerdict }
  | { kind: "password"; username: string; retry: boolean }
  | { kind: "passphrase"; key_label: string }
  | { kind: "interactive"; name: string; instructions: string; questions: Question[] };

export type PromptEvent = { id: Uuid; session_id: Uuid; target: string } & PromptRequest;

export interface PromptClosedEvent {
  id: Uuid;
  session_id: Uuid;
}

export type PromptAnswer =
  | { kind: "host_key"; decision: "reject" | "accept_once" | "accept_and_save" }
  | { kind: "secret"; value: string; remember: boolean }
  | { kind: "interactive"; answers: string[] }
  | { kind: "cancel" };

// ───────────────────────────── SFTP ─────────────────────────────

export type EntryKind = "dir" | "file" | "symlink" | "other";

/** A file on either side (Rust fills the same shape for local and remote). */
export interface FsEntry {
  name: string;
  path: string;
  kind: EntryKind;
  size: number | null;
  mode: number | null;
  uid: number | null;
  gid: number | null;
  user: string | null;
  group: string | null;
  mtime: number | null;
  atime: number | null;
  link_target: string | null;
}

export interface Listing {
  path: string;
  parent: string | null;
  entries: FsEntry[];
}

export type SftpTarget = { kind: "host"; host_id: Uuid } | { kind: "session"; session_id: Uuid };

export interface SftpInfo {
  id: Uuid;
  title: string;
  target: string;
  hostId: Uuid | null;
  home: string;
  startedAt: string;
}

export type SftpEvent = { type: "opened"; id: Uuid; info: SftpInfo } | { type: "closed"; id: Uuid };

export type Direction = "upload" | "download";

export interface TransferInfo {
  id: Uuid;
  sftpId: Uuid;
  direction: Direction;
  local: string;
  remote: string;
  startedAt: string;
}

export type TransferEvent =
  | { type: "started"; id: Uuid; info: TransferInfo }
  | {
      type: "progress";
      id: Uuid;
      done: number;
      total: number | null;
      files_done: number;
      files_total: number;
      current: string;
    }
  | { type: "finished"; id: Uuid; bytes: number }
  | { type: "failed"; id: Uuid; message: string }
  | { type: "cancelled"; id: Uuid };
