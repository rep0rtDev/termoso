// Thin typed wrappers over Tauri `invoke`. One function per Rust command.

import { invoke, Channel } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppInfo,
  ConnectionHistory,
  Entity,
  GroupNode,
  HistoryItem,
  HostCard,
  HostForm,
  LocalVault,
  OpenTarget,
  PromptAnswer,
  PromptClosedEvent,
  PromptEvent,
  SessionEvent,
  SessionInfo,
  Settings,
  TagInfo,
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

export function terminalOpen(
  target: OpenTarget,
  cols: number,
  rows: number,
  onOutput: (bytes: Uint8Array) => void,
) {
  const output = new Channel<ArrayBuffer | number[]>();
  output.onmessage = (msg) =>
    onOutput(msg instanceof ArrayBuffer ? new Uint8Array(msg) : Uint8Array.from(msg));
  return invoke<SessionInfo>("terminal_open", { target, cols, rows, output });
}

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
