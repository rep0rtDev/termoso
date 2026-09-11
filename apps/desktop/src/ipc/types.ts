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

export interface Settings {
  theme: ThemeMode;
  hostsView: HostsView;
  terminalFontSize: number;
  terminalFontFamily: string;
  cursorBlink: boolean;
  scrollback: number;
  copyOnSelect: boolean;
  pasteOnRightClick: boolean;
  confirmCloseTab: boolean;
  confirmPasteMultiline: boolean;
  autocomplete: boolean;
  keepAliveSeconds: number;
}

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
  hasPassword: boolean;
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

export interface IdentityData {
  label: string;
  username: string;
  ssh_key_id?: Uuid | null;
  is_visible: boolean;
}

export interface SshKeyData {
  label: string;
  key_type: string;
  public_key?: string | null;
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
