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
export const TERM_TYPES = [
  "xterm-256color",
  "xterm",
  "vt100",
  "vt220",
  "linux",
  "screen-256color",
  "tmux-256color",
] as const;
export type TermType = (typeof TERM_TYPES)[number];
export type SyncConflict = "newest_wins" | "local_wins" | "server_wins";

export interface Settings {
  theme: ThemeMode;
  hostsView: HostsView;
  forwardingView: HostsView;
  keychainView: HostsView;
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
  /** Install OSC 133 prompt markers into bash / zsh / fish after connecting. */
  shellIntegration: boolean;
  terminalBell: boolean;
  brightBold: boolean;
  termType: TermType;
  autoReconnect: boolean;
  keywordHighlight: boolean;
  /** Program (+ args) for local terminals; empty = login shell. */
  localShell: string;
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
  /** Start-up sign-in screen was dismissed with "Continue offline". */
  welcomeSeen: boolean;
  /** Shortcut overrides: command id → chord (`ctrl+shift+k`), `""` = unbound. */
  shortcuts: Record<string, string>;
  /** SFTP “Open with” associations: lower-case extension (`""` = none) → app. */
  sftpOpenWith: Record<string, string>;
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

/** Member of a team vault as the server sees it. */
export interface VaultMember {
  user_id: Uuid;
  email: string;
  display_name?: string | null;
  role: VaultRole;
  key_version: number;
  pending: boolean;
}

export type TeamRole = "member" | "admin" | "owner";

export interface Team {
  id: Uuid;
  name: string;
  created_at: string;
  my_role: TeamRole;
  member_count: number;
  multiplayer_enabled: boolean;
  require_mfa: boolean;
}

export interface TeamMember {
  user_id: Uuid;
  email: string;
  display_name: string | null;
  role: TeamRole;
  joined_at: string;
}

export interface TeamInvite {
  id: Uuid;
  email: string;
  role: TeamRole;
  invited_by: Uuid;
  created_at: string;
  expires_at: string;
}

export interface InviteResult {
  email: string;
  invite: TeamInvite | null;
  url: string | null;
  error: string | null;
}

/** Team-vault member still waiting for a manager to hand them the key. */
export interface PendingVaultKey {
  vault_id: Uuid;
  user_id: Uuid;
  role: VaultRole;
}

export interface VaultAccess {
  userId: Uuid;
  role: VaultRole;
}

export interface HostCard {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  address: string;
  groupId: Uuid | null;
  groupPath: string[];
  /** Primary protocol: `ssh` unless the host is Telnet-only. */
  protocol: HostProtocol;
  /** Effective username of the primary protocol. */
  username: string;
  /** Effective port of the primary protocol. */
  port: number;
  /** Effective Telnet port when the host also has a Telnet section. */
  telnetPort: number | null;
  tags: string[];
  osName: string | null;
  /** User-chosen icon id; overrides `osName` for display. */
  icon: string | null;
  ipVersion: IpVersion;
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
  /** The host has an SSH section (the SSH fields below belong to it). */
  ssh: boolean;
  port: number | null;
  username: string;
  password: string | null;
  sshKeyId: Uuid | null;
  identityId: Uuid | null;
  tagIds: Uuid[];
  notes: string;
  osName: string | null;
  /** User-chosen icon id; null follows OS detection. */
  icon: string | null;
  ipVersion: IpVersion;
  agentForwarding: boolean;
  startupSnippetId: Uuid | null;
  hostChainId: Uuid | null;
  proxyId: Uuid | null;
  /** Telnet section, when the host is also (or only) reachable over Telnet. */
  telnet: TelnetForm | null;
  envVariables: [string, string][];
  keepAliveInterval: number | null;
  timeout: number | null;
  /** Terminal colour scheme id; null follows the app setting. */
  colorScheme: string | null;
  hasPassword: boolean;
}

export interface TelnetForm {
  port: number | null;
  username: string;
  /** null keeps the stored password when editing; "" clears it. */
  password: string | null;
  identityId: Uuid | null;
  colorScheme: string | null;
  hasPassword: boolean;
}

export function emptyTelnetForm(): TelnetForm {
  return {
    port: null,
    username: "",
    password: null,
    identityId: null,
    colorScheme: null,
    hasPassword: false,
  };
}

export type HostProtocol = "ssh" | "telnet";

/** Protocols a saved host can be opened with. */
export function hostProtocols(h: Pick<HostCard, "protocol" | "telnetPort">): HostProtocol[] {
  if (h.protocol === "telnet") return ["telnet"];
  return h.telnetPort === null ? ["ssh"] : ["ssh", "telnet"];
}

export type IpVersion = "auto" | "4" | "6";

export type SerialParity = "none" | "odd" | "even";
export type SerialFlowControl = "none" | "software" | "hardware";

export interface SerialLine {
  baudRate: number;
  dataBits: 5 | 6 | 7 | 8;
  stopBits: 1 | 2;
  parity: SerialParity;
  flowControl: SerialFlowControl;
  /** WHATWG encoding label (`utf-8`, `koi8-r`, …); decoded/encoded in the core. */
  charset: string;
}

export const SERIAL_BAUD_RATES = [
  115200, 9600, 19200, 38400, 57600, 230400, 460800, 921600, 1200, 2400, 4800,
] as const;

/** Mirrors `termoso_core::serial::COMMON_CHARSETS`. */
export const SERIAL_CHARSETS: readonly { value: string; label: string }[] = [
  { value: "utf-8", label: "UTF-8" },
  { value: "iso-8859-1", label: "ISO-8859-1 (Latin-1)" },
  { value: "iso-8859-2", label: "ISO-8859-2 (Latin-2)" },
  { value: "iso-8859-15", label: "ISO-8859-15 (Latin-9)" },
  { value: "windows-1250", label: "Windows-1250" },
  { value: "windows-1251", label: "Windows-1251" },
  { value: "windows-1252", label: "Windows-1252" },
  { value: "koi8-r", label: "KOI8-R" },
  { value: "koi8-u", label: "KOI8-U" },
  { value: "gbk", label: "GBK" },
  { value: "gb18030", label: "GB18030" },
  { value: "big5", label: "Big5" },
  { value: "shift_jis", label: "Shift_JIS" },
  { value: "euc-jp", label: "EUC-JP" },
  { value: "euc-kr", label: "EUC-KR" },
];

export function defaultSerialLine(): SerialLine {
  return {
    baudRate: 115200,
    dataBits: 8,
    stopBits: 1,
    parity: "none",
    flowControl: "none",
    charset: "utf-8",
  };
}

/** A serial device found on this machine. */
export interface SerialPortInfo {
  path: string;
  kind: "usb" | "pci" | "bluetooth" | "unknown";
  manufacturer: string | null;
  product: string | null;
  serialNumber: string | null;
}

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
    ssh: true,
    port: null,
    username: "",
    password: null,
    sshKeyId: null,
    identityId: null,
    tagIds: [],
    notes: "",
    osName: null,
    icon: null,
    ipVersion: "auto",
    agentForwarding: false,
    startupSnippetId: null,
    hostChainId: null,
    proxyId: null,
    telnet: null,
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
  /** Hosts carrying the tag. */
  hosts: number;
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

/** Public metadata of an OpenSSH certificate attached to a key. */
export interface CertificateCard {
  /** `null` for an unsaved preview. */
  id: Uuid | null;
  certType: string;
  kind: string;
  keyId: string;
  serial: number;
  principals: string[];
  validAfter: string | null;
  validBefore: string | null;
  fingerprint: string;
  caFingerprint: string;
  caKeyType: string;
  validNow: boolean;
}

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
  certificate: CertificateCard | null;
  certificateUnreadable: boolean;
  updatedAt: string;
  dirty: boolean;
}

/** Public half of pasted/picked private key text (editor preview; not stored). */
export interface KeyPreview {
  keyType: string;
  bits: number;
  fingerprint: string;
  publicKey: string;
  comment: string;
  /** A passphrase is required to import. */
  encrypted: boolean;
  /** PuTTY .ppk; converted to OpenSSH on import. */
  putty: boolean;
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
  /** OpenSSH / PEM / PKCS#8 / PuTTY .ppk text. */
  privateKey: string;
  passphrase: string | null;
  rememberPassphrase: boolean;
  /** OpenSSH certificate for this key (`*-cert.pub`), verified before saving. */
  certificate: string | null;
}

/** Like `ImportKeyForm`, but the private key is read from `path` inside Rust. */
export interface ImportKeyFileForm {
  vaultId: Uuid;
  label: string;
  path: string;
  passphrase: string | null;
  rememberPassphrase: boolean;
  /** Pasted certificate text; ignored when `certificatePath` is set. */
  certificate: string | null;
  certificatePath: string | null;
}

export interface IdentityCard {
  id: Uuid;
  vaultId: Uuid;
  label: string;
  username: string;
  hasPassword: boolean;
  sshKeyId: Uuid | null;
  sshKeyLabel: string | null;
  /** Certificate pinned on the identity itself (`null` = the key's own). */
  sshCertificateId: Uuid | null;
  hasCertificate: boolean;
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
  /** Explicit certificate; `null` falls back to the key's own certificate. */
  sshCertificateId: Uuid | null;
}

// ───────────────────────────── port forwarding ─────────────────────────────

export type PfKind = "local" | "remote" | "dynamic";
export type PfState = "stopped" | "starting" | "running" | "reconnecting";

export interface PfRuntime {
  state: PfState;
  startedAt: string | null;
  bound: string | null;
  connections: number;
  active: number;
  bytesIn: number;
  bytesOut: number;
  lastError: string | null;
  /** Reconnect attempt number while `state === "reconnecting"`. */
  attempt: number;
  nextRetryAt: string | null;
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
  /** Hosts the snippet is configured to run on, in execution order. */
  targetHostIds: Uuid[];
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

// ───────────────────────────── import (ssh_config / PuTTY / CSV) ─────────────────────────────

export type ImportSource = "ssh_config" | "putty" | "csv";

export interface ImportedProxy {
  kind: string;
  host: string;
  port: number;
  username: string;
  /** The password itself never reaches the UI. */
  hasPassword: boolean;
}

export interface ImportedHost {
  label: string;
  address: string;
  protocol: string;
  port: number | null;
  username: string;
  hasPassword: boolean;
  groupPath: string[];
  tags: string[];
  keyPath: string | null;
  jumpHosts: string[];
  proxy: ImportedProxy | null;
  agentForwarding: boolean;
  envVariables: [string, string][];
  keepAliveInterval: number | null;
  timeout: number | null;
  warnings: string[];
}

export interface ImportedKey {
  path: string;
  name: string;
  keyType: string;
  bits: number;
  fingerprint: string;
  encrypted: boolean;
  publicKey: string;
}

export interface ImportedKnownHost {
  hostname: string;
  keyType: string;
  fingerprint: string;
  line: string;
}

export interface ImportedPfRule {
  hostLabel: string;
  kind: PfKind;
  boundAddress: string;
  localPort: number;
  remoteHost: string;
  remotePort: number;
}

/** Parsed source; nothing is written until `importApply` with a selection. */
export interface ImportPreview {
  id: Uuid;
  source: ImportSource;
  origin: string;
  hosts: ImportedHost[];
  keys: ImportedKey[];
  knownHosts: ImportedKnownHost[];
  pfRules: ImportedPfRule[];
  warnings: string[];
}

/** Indexes into the preview lists. */
export interface ImportSelection {
  hosts: number[];
  keys: number[];
  knownHosts: number[];
  pfRules: number[];
}

// ───────────────────────────── export / backup ─────────────────────────────

export interface CsvExportReport {
  hosts: number;
  /** True when the file contains plaintext passwords. */
  passwordsIncluded: boolean;
  path: string;
}

export interface BackupVaultSummary {
  id: Uuid;
  kind: LocalVaultKind;
  name: string;
  entities: number;
  /** Count per entity kind, only kinds that are present. */
  counts: Record<string, number>;
}

export interface BackupSummary {
  /** Token for `backupRestore`; nil UUID when describing a fresh export. */
  previewId: Uuid;
  createdAt: string;
  appVersion: string;
  vaults: BackupVaultSummary[];
  path: string | null;
}

export interface RestoreReport {
  added: number;
  replaced: number;
  /** Entities whose id already lives in another vault; left untouched. */
  skipped: number;
  warnings: string[];
}

export interface ImportApplyReport {
  hosts: number;
  groups: number;
  tags: number;
  keys: number;
  knownHosts: number;
  pfRules: number;
  hostChains: number;
  proxies: number;
  skippedHosts: number;
  skippedKeys: number;
  warnings: string[];
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

/** A command line typed in a terminal (encrypted at rest; never the output). */
export interface CommandHistory {
  host_id: Uuid | null;
  command: string;
}

export interface HistoryItem<T> {
  id: Uuid;
  created_at: string;
  data: T;
}

export interface DirEntry {
  name: string;
  dir: boolean;
}

export type SessionState = "connecting" | "connected";

export interface SessionInfo {
  id: Uuid;
  protocol: "ssh" | "telnet" | "serial" | "local" | "multiplayer";
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
  /** Base name of the user's shell (`bash`, `zsh`, …) once known. */
  shell: string | null;
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

/** Stage of an SSH connection attempt (see `ConnectPhase` in termoso-core). */
export type ConnectPhase =
  | { kind: "resolving" }
  | { kind: "connecting"; via: string }
  | { kind: "handshake" }
  | { kind: "host_key" }
  | { kind: "auth"; method: string }
  | { kind: "authenticated" };

export type SessionEvent =
  | { type: "connecting"; id: Uuid; info: SessionInfo }
  | { type: "connected"; id: Uuid; info: SessionInfo }
  | { type: "progress"; id: Uuid; hop: string | null; phase: ConnectPhase }
  | { type: "notice"; id: Uuid; message: string }
  | { type: "exit"; id: Uuid; code: number | null; signal: string | null }
  | { type: "error"; id: Uuid; message: string }
  | { type: "shell"; id: Uuid; shell: string }
  | { type: "closed"; id: Uuid };

export type OpenTarget =
  | {
      kind: "host";
      host_id: Uuid;
      /** Which section to open; defaults to SSH when the host has one. */
      protocol?: HostProtocol | null;
    }
  | { kind: "serial"; path: string; line: SerialLine }
  | {
      kind: "quick";
      address: string;
      username?: string | null;
      port?: number | null;
      /** `ssh` (default) or `telnet`. */
      protocol?: "ssh" | "telnet" | null;
    }
  | { kind: "local" }
  /** Someone else's terminal, from a `termoso://join/…` multiplayer link. */
  | { kind: "live"; link: string };

// ───────────────────────────── multiplayer ─────────────────────────────

export interface LiveParticipant {
  userId: Uuid;
  email: string;
  displayName: string | null;
  isHost: boolean;
  /** Keystrokes of this person reach the shared terminal. */
  canWrite: boolean;
  isMe: boolean;
}

/** A shared (host) or watched (viewer) terminal tab. */
export interface ShareInfo {
  /** Local session (pane) id. */
  id: Uuid;
  liveId: Uuid;
  role: "host" | "viewer";
  /** Host only: the link to hand out. */
  link: string | null;
  participants: LiveParticipant[];
  /** Viewer: whether our keystrokes reach the host terminal. */
  canWrite: boolean;
}

export type LiveEvent =
  | { type: "participants"; id: Uuid; participants: LiveParticipant[] }
  | { type: "control"; id: Uuid; canWrite: boolean }
  | { type: "resize"; id: Uuid; cols: number; rows: number }
  | { type: "title"; id: Uuid; title: string }
  | { type: "ended"; id: Uuid; reason: "stopped" | "disconnected" | "error"; message: string };

/** Split tree of a saved tab; leaves are connection targets. */
export type LayoutTemplate =
  | { kind: "leaf"; target: OpenTarget }
  | {
      kind: "split";
      direction: "row" | "column";
      ratio: number;
      first: LayoutTemplate;
      second: LayoutTemplate;
    };

export type TabViewMode = "split" | "list";

export interface WorkspaceTemplate {
  id: Uuid;
  name: string;
  viewMode: TabViewMode;
  layout: LayoutTemplate;
  createdAt: string;
  updatedAt: string;
}

export interface SnapshotTab {
  /** Workspace name; `null` for a plain session tab. */
  name: string | null;
  viewMode: TabViewMode;
  templateId: Uuid | null;
  layout: LayoutTemplate;
}

export interface SessionSnapshot {
  savedAt: string;
  tabs: SnapshotTab[];
}

export interface WorkspacesState {
  templates: WorkspaceTemplate[];
  lastSession: SessionSnapshot | null;
}

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
  /** What a symlink points at; `null` for non-links and dangling links. */
  target_kind: EntryKind | null;
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

/** What to do with files that already exist at the destination. */
export type Conflict = "replace" | "skip" | "rename" | "resume";

export interface TransferInfo {
  id: Uuid;
  sftpId: Uuid;
  direction: Direction;
  local: string;
  remote: string;
  conflict: Conflict;
  startedAt: string;
}

export type TransferEvent =
  | { type: "started"; id: Uuid; info: TransferInfo }
  | { type: "queued"; id: Uuid }
  | { type: "running"; id: Uuid }
  | { type: "paused"; id: Uuid }
  | {
      type: "progress";
      id: Uuid;
      done: number;
      total: number | null;
      files_done: number;
      files_total: number;
      files_skipped: number;
      current: string;
    }
  | { type: "finished"; id: Uuid; bytes: number; files_skipped: number }
  | { type: "failed"; id: Uuid; message: string }
  | { type: "cancelled"; id: Uuid };

/** A remote file opened locally; saves are uploaded back while it is open. */
export interface EditInfo {
  id: Uuid;
  sftpId: Uuid;
  remote: string;
  local: string;
  name: string;
  app: string | null;
  size: number | null;
  startedAt: string;
}

export type EditEvent =
  | { type: "opened"; id: Uuid; info: EditInfo }
  | { type: "uploading"; id: Uuid }
  | { type: "uploaded"; id: Uuid; bytes: number; at: string }
  | { type: "failed"; id: Uuid; message: string }
  | { type: "closed"; id: Uuid };
