// Thin typed wrappers over Tauri `invoke`. One function per Rust command.

import { invoke, Channel } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppInfo,
  ConnectionHistory,
  Direction,
  Entity,
  FsEntry,
  GroupNode,
  HistoryItem,
  HostCard,
  HostForm,
  Listing,
  LocalVault,
  OpenTarget,
  PromptAnswer,
  PromptClosedEvent,
  PromptEvent,
  SessionEvent,
  SessionInfo,
  Settings,
  SftpEvent,
  SftpInfo,
  SftpTarget,
  TagInfo,
  TransferEvent,
  TransferInfo,
  Uuid,
} from "./types";

export const appInfo = () => invoke<AppInfo>("app_info");

export const settingsGet = () => invoke<Settings>("settings_get");
export const settingsSet = (settings: Settings) => invoke<Settings>("settings_set", { settings });

export const vaultsList = () => invoke<LocalVault[]>("vaults_list");
export const vaultDefault = () => invoke<LocalVault>("vault_default");

export const entitiesList = <T>(kind: string, vaultId?: Uuid | null) =>
  invoke<Entity<T>[]>("entities_list", { kind, vaultId: vaultId ?? null });
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
