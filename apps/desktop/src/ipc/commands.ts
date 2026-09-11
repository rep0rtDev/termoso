// Thin typed wrappers over Tauri `invoke`. One function per Rust command.

import { invoke, Channel } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AccountStatus,
  AppInfo,
  BookmarkCard,
  ConnectionHistory,
  Device,
  Direction,
  Entity,
  ForwardEvent,
  FsEntry,
  GenerateKeyForm,
  GroupNode,
  HistoryItem,
  HostCard,
  HostForm,
  IdentityCard,
  IdentityForm,
  ImportKeyForm,
  ImportReport,
  KeyCard,
  KnownHostCard,
  Listing,
  LocalVault,
  LogBody,
  LogCard,
  LoginForm,
  LoginOutcome,
  MasterKeySource,
  MfaCredential,
  OpenTarget,
  PackageNode,
  PfRuleCard,
  PfRuleForm,
  PfRuntime,
  PromptAnswer,
  PromptClosedEvent,
  PromptEvent,
  RegisterForm,
  Registered,
  RunResult,
  ServerInfo,
  SessionEvent,
  SessionInfo,
  Settings,
  SftpEvent,
  SftpInfo,
  SftpTarget,
  SnippetCard,
  SnippetForm,
  SyncNotice,
  SyncStatus,
  TagInfo,
  TransferEvent,
  TransferInfo,
  UpdateEvent,
  UpdateInfo,
  Uuid,
} from "./types";

export const appInfo = () => invoke<AppInfo>("app_info");

export const settingsGet = () => invoke<Settings>("settings_get");
export const settingsSet = (settings: Settings) => invoke<Settings>("settings_set", { settings });

export const vaultsList = () => invoke<LocalVault[]>("vaults_list");
export const vaultDefault = () => invoke<LocalVault>("vault_default");

export const entitiesList = <T>(kind: string, vaultId?: Uuid | null) =>
  invoke<Entity<T>[]>("entities_list", { kind, vaultId: vaultId ?? null });
export const entitySave = <T>(kind: string, vaultId: Uuid, id: Uuid | null, data: T) =>
  invoke<Entity<T>>("entity_save", { kind, vaultId, id, data });
export const entityDelete = (id: Uuid) => invoke<null>("entity_delete", { id });

export const hostsList = (vaultId?: Uuid | null) =>
  invoke<HostCard[]>("hosts_list", { vaultId: vaultId ?? null });
export const hostForm = (id: Uuid) => invoke<HostForm>("host_form", { id });
export const hostSave = (form: HostForm) => invoke<HostCard>("host_save", { form });
export const hostDelete = (id: Uuid) => invoke<null>("host_delete", { id });

export const groupsList = (vaultId?: Uuid | null) =>
  invoke<GroupNode[]>("groups_list", { vaultId: vaultId ?? null });
export const groupSave = (args: {
  vaultId: Uuid;
  id: Uuid | null;
  label: string;
  parentId: Uuid | null;
}) => invoke<GroupNode>("group_save", args);
export const groupDelete = (id: Uuid) => invoke<null>("group_delete", { id });

export const tagsList = (vaultId?: Uuid | null) =>
  invoke<TagInfo[]>("tags_list", { vaultId: vaultId ?? null });

export const historyConnections = (limit = 50) =>
  invoke<HistoryItem<ConnectionHistory>[]>("history_connections", { limit });

export const sessionsList = () => invoke<SessionInfo[]>("sessions_list");

function outputChannel(onOutput: (bytes: Uint8Array) => void) {
  const output = new Channel<ArrayBuffer | number[]>();
  output.onmessage = (msg) =>
    onOutput(msg instanceof ArrayBuffer ? new Uint8Array(msg) : Uint8Array.from(msg));
  return output;
}

export function terminalOpen(
  id: Uuid,
  target: OpenTarget,
  cols: number,
  rows: number,
  onOutput: (bytes: Uint8Array) => void,
) {
  const output = outputChannel(onOutput);
  return invoke<SessionInfo>("terminal_open", { id, target, cols, rows, output });
}

export const terminalAttach = (id: Uuid, onOutput: (bytes: Uint8Array) => void) =>
  invoke<null>("terminal_attach", { id, output: outputChannel(onOutput) });

export const terminalWrite = (id: Uuid, data: string) =>
  invoke<null>("terminal_write", { id, data });
export const terminalResize = (id: Uuid, cols: number, rows: number) =>
  invoke<null>("terminal_resize", { id, cols, rows });
export const terminalClose = (id: Uuid) => invoke<null>("terminal_close", { id });

export const promptAnswer = (id: Uuid, answer: PromptAnswer) =>
  invoke<boolean>("prompt_answer", { id, answer });

export const onSessionEvent = (cb: (e: SessionEvent) => void): Promise<UnlistenFn> =>
  listen<SessionEvent>("session", (ev) => cb(ev.payload));
export const onPrompt = (cb: (e: PromptEvent) => void): Promise<UnlistenFn> =>
  listen<PromptEvent>("prompt", (ev) => cb(ev.payload));
export const onPromptClosed = (cb: (e: PromptClosedEvent) => void): Promise<UnlistenFn> =>
  listen<PromptClosedEvent>("prompt-closed", (ev) => cb(ev.payload));

// ───────────────────────────── SFTP ─────────────────────────────

export const sftpSessionsList = () => invoke<SftpInfo[]>("sftp_sessions_list");
export const sftpOpen = (id: Uuid, target: SftpTarget) =>
  invoke<SftpInfo>("sftp_open", { id, target });
export const sftpClose = (id: Uuid) => invoke<null>("sftp_close", { id });
export const sftpList = (id: Uuid, path: string | null) =>
  invoke<Listing>("sftp_list", { id, path });
export const sftpStat = (id: Uuid, path: string) => invoke<FsEntry>("sftp_stat", { id, path });
export const sftpMkdir = (id: Uuid, path: string) => invoke<null>("sftp_mkdir", { id, path });
export const sftpRename = (id: Uuid, from: string, to: string) =>
  invoke<null>("sftp_rename", { id, from, to });
export const sftpRemove = (id: Uuid, path: string, recursive: boolean) =>
  invoke<null>("sftp_remove", { id, path, recursive });
export const sftpChmod = (id: Uuid, path: string, mode: number) =>
  invoke<null>("sftp_chmod", { id, path, mode });

export const localHome = () => invoke<string>("local_home");
export const localList = (path: string | null) => invoke<Listing>("local_list", { path });
export const localStat = (path: string) => invoke<FsEntry>("local_stat", { path });
export const localMkdir = (path: string) => invoke<null>("local_mkdir", { path });
export const localRename = (from: string, to: string) => invoke<null>("local_rename", { from, to });
export const localRemove = (path: string, recursive: boolean) =>
  invoke<null>("local_remove", { path, recursive });

export const transferStart = (args: {
  sftpId: Uuid;
  direction: Direction;
  local: string;
  remote: string;
  resume?: boolean;
}) => invoke<TransferInfo>("transfer_start", args);
export const transferCancel = (id: Uuid) => invoke<boolean>("transfer_cancel", { id });

export const onSftpEvent = (cb: (e: SftpEvent) => void): Promise<UnlistenFn> =>
  listen<SftpEvent>("sftp", (ev) => cb(ev.payload));
export const onTransferEvent = (cb: (e: TransferEvent) => void): Promise<UnlistenFn> =>
  listen<TransferEvent>("transfer", (ev) => cb(ev.payload));

// ───────────────────────────── keychain ─────────────────────────────

export const keysList = (vaultId?: Uuid | null) =>
  invoke<KeyCard[]>("keys_list", { vaultId: vaultId ?? null });
export const keyGenerate = (form: GenerateKeyForm) => invoke<KeyCard>("key_generate", { form });
export const keyImport = (form: ImportKeyForm) => invoke<KeyCard>("key_import", { form });
export const keyImportFile = (args: {
  vaultId: Uuid;
  label: string;
  path: string;
  passphrase: string | null;
  rememberPassphrase: boolean;
}) => invoke<KeyCard>("key_import_file", args);
export const keyRename = (id: Uuid, label: string) => invoke<KeyCard>("key_rename", { id, label });
export const keyChangePassphrase = (args: {
  id: Uuid;
  current: string | null;
  next: string | null;
  remember: boolean;
}) => invoke<KeyCard>("key_change_passphrase", args);
export const keyRememberPassphrase = (id: Uuid, passphrase: string | null) =>
  invoke<KeyCard>("key_remember_passphrase", { id, passphrase });
export const keyPublic = (id: Uuid) => invoke<string>("key_public", { id });
/** Returns private key text; only call on explicit user action. */
export const keyExport = (args: {
  id: Uuid;
  passphrase: string | null;
  exportPassphrase: string | null;
}) => invoke<string>("key_export", args);
export const keyExportFile = (args: {
  id: Uuid;
  path: string;
  passphrase: string | null;
  exportPassphrase: string | null;
}) => invoke<null>("key_export_file", args);
export const keyDelete = (id: Uuid) => invoke<null>("key_delete", { id });

export const identitiesList = (vaultId?: Uuid | null) =>
  invoke<IdentityCard[]>("identities_list", { vaultId: vaultId ?? null });
export const identitySave = (form: IdentityForm) => invoke<IdentityCard>("identity_save", { form });
export const identityDelete = (id: Uuid) => invoke<null>("identity_delete", { id });

export const masterKeyMigrate = () => invoke<MasterKeySource>("master_key_migrate");

// ───────────────────────────── port forwarding ─────────────────────────────

export const pfRules = (vaultId?: Uuid | null) =>
  invoke<PfRuleCard[]>("pf_rules", { vaultId: vaultId ?? null });
export const pfRuntimes = () => invoke<Record<Uuid, PfRuntime>>("pf_runtimes");
export const pfSave = (form: PfRuleForm) => invoke<PfRuleCard>("pf_save", { form });
export const pfStart = (id: Uuid) => invoke<PfRuleCard>("pf_start", { id });
export const pfStop = (id: Uuid) => invoke<null>("pf_stop", { id });
export const pfDelete = (id: Uuid) => invoke<null>("pf_delete", { id });
export const onForwardEvent = (cb: (e: ForwardEvent) => void): Promise<UnlistenFn> =>
  listen<ForwardEvent>("forward", (ev) => cb(ev.payload));

// ───────────────────────────── snippets ─────────────────────────────

export const snippetsList = (vaultId?: Uuid | null) =>
  invoke<SnippetCard[]>("snippets_list", { vaultId: vaultId ?? null });
export const snippetSave = (form: SnippetForm) => invoke<SnippetCard>("snippet_save", { form });
export const snippetDelete = (id: Uuid) => invoke<null>("snippet_delete", { id });
export const snippetRun = (id: Uuid, sessionIds: Uuid[], vars: Record<string, string>) =>
  invoke<RunResult>("snippet_run", { id, sessionIds, vars });
export const snippetPackages = (vaultId?: Uuid | null) =>
  invoke<PackageNode[]>("snippet_packages", { vaultId: vaultId ?? null });
export const snippetPackageSave = (args: {
  vaultId: Uuid;
  id: Uuid | null;
  label: string;
  parentId: Uuid | null;
}) => invoke<PackageNode>("snippet_package_save", args);
export const snippetPackageDelete = (id: Uuid) => invoke<null>("snippet_package_delete", { id });

// ───────────────────────────── known hosts ─────────────────────────────

export const knownHostsList = () => invoke<KnownHostCard[]>("known_hosts_list");
export const knownHostForget = (id: Uuid) => invoke<null>("known_host_forget", { id });
export const knownHostForgetHost = (hostname: string) =>
  invoke<number>("known_host_forget_host", { hostname });
export const knownHostsImportText = (contents: string) =>
  invoke<ImportReport>("known_hosts_import_text", { contents });
export const knownHostsImportFile = (path: string) =>
  invoke<ImportReport>("known_hosts_import_file", { path });
export const knownHostsExportText = () => invoke<string>("known_hosts_export_text");
export const knownHostsExportFile = (path: string) =>
  invoke<number>("known_hosts_export_file", { path });
export const knownHostsDefaultPath = () => invoke<string | null>("known_hosts_default_path");

// ───────────────────────────── logs ─────────────────────────────

export const logsList = () => invoke<LogCard[]>("logs_list");
export const logRead = (id: Uuid) => invoke<LogBody>("log_read", { id });
export const logExport = (id: Uuid, path: string) => invoke<number>("log_export", { id, path });
export const logDelete = (id: Uuid) => invoke<null>("log_delete", { id });
export const logBookmarks = (logId: Uuid) => invoke<BookmarkCard[]>("log_bookmarks", { logId });
export const logBookmarkAdd = (logId: Uuid, offset: number, note: string) =>
  invoke<BookmarkCard>("log_bookmark_add", { logId, offset, note });
export const logBookmarkDelete = (id: Uuid) => invoke<null>("log_bookmark_delete", { id });

// ───────────────────────────── account / sync ─────────────────────────────

export const accountStatus = () => invoke<AccountStatus>("account_status");
export const accountServerInfo = (serverUrl: string) =>
  invoke<ServerInfo>("account_server_info", { serverUrl });
export const accountLogin = (form: LoginForm) => invoke<LoginOutcome>("account_login", { form });
export const accountMfa = (credential: MfaCredential) =>
  invoke<LoginOutcome>("account_mfa", { credential });
export const accountMfaEmailSend = () => invoke<null>("account_mfa_email_send");
export const accountWebauthnChallenge = () => invoke<unknown>("account_webauthn_challenge");
export const accountDeviceApprove = (code: string) =>
  invoke<LoginOutcome>("account_device_approve", { code });
export const accountDeviceResend = () => invoke<null>("account_device_resend");
export const accountCancelLogin = () => invoke<null>("account_cancel_login");
export const accountRegister = (form: RegisterForm) =>
  invoke<Registered>("account_register", { form });
export const accountSignOut = () => invoke<null>("account_sign_out");
export const accountSyncNow = () => invoke<SyncStatus>("account_sync_now");
export const accountDevices = () => invoke<Device[]>("account_devices");
export const accountDeviceRevoke = (id: Uuid) => invoke<null>("account_device_revoke", { id });
export const onSyncNotice = (cb: (e: SyncNotice) => void): Promise<UnlistenFn> =>
  listen<SyncNotice>("sync", (ev) => cb(ev.payload));

// ───────────────────────────── updates ─────────────────────────────

export const updateCheck = () => invoke<UpdateInfo | null>("update_check");
export const updateInstall = () => invoke<UpdateInfo>("update_install");
export const updateRestart = () => invoke<null>("update_restart");
export const onUpdateEvent = (cb: (e: UpdateEvent) => void): Promise<UnlistenFn> =>
  listen<UpdateEvent>("update", (ev) => cb(ev.payload));
