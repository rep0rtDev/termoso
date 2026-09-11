// SFTP connections and transfers as seen by the UI. Rust owns the SFTP
// channel, file I/O and cancellation; this mirrors its events.

import type { QueryClient } from "@tanstack/react-query";
import * as ipc from "@/ipc/commands";
import type {
  Direction,
  SftpEvent,
  SftpInfo,
  SftpTarget,
  TransferEvent,
  TransferInfo,
  Uuid,
} from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { createStore, omit, useStore } from "@/lib/store";

export type ConnStatus = "connecting" | "open" | "error" | "closed";

export interface SftpConn {
  id: Uuid;
  target: SftpTarget;
  title: string;
  hostId: Uuid | null;
  status: ConnStatus;
  message: string | null;
  info: SftpInfo | null;
}

export type TransferStatus = "running" | "done" | "failed" | "cancelled";

export interface Transfer {
  id: Uuid;
  sftpId: Uuid;
  direction: Direction;
  local: string;
  remote: string;
  status: TransferStatus;
  done: number;
  total: number | null;
  filesDone: number;
  filesTotal: number;
  current: string;
  message: string | null;
}

export interface SftpState {
  conns: Record<Uuid, SftpConn>;
  order: Uuid[];
  activeId: Uuid | null;
  transfers: Record<Uuid, Transfer>;
  transferOrder: Uuid[];
}

export const sftpStore = createStore<SftpState>({
  conns: {},
  order: [],
  activeId: null,
  transfers: {},
  transferOrder: [],
});

export const useSftp = <S>(selector: (s: SftpState) => S) => useStore(sftpStore, selector);

const update = (fn: (s: SftpState) => SftpState) => sftpStore.set(fn);

function patchConn(id: Uuid, patch: Partial<SftpConn>) {
  update((s) => {
    const c = s.conns[id];
    return c ? { ...s, conns: { ...s.conns, [id]: { ...c, ...patch } } } : s;
  });
}

function patchTransfer(id: Uuid, patch: Partial<Transfer>) {
  update((s) => {
    const t = s.transfers[id];
    return t ? { ...s, transfers: { ...s.transfers, [id]: { ...t, ...patch } } } : s;
  });
}

export const fsQueryKey = (side: "local" | "remote", sftpId: Uuid | null, path: string | null) =>
  ["fs", side, sftpId, path] as const;

// ───────────────────────────── connections ─────────────────────────────

function connect(id: Uuid, target: SftpTarget) {
  ipc
    .sftpOpen(id, target)
    .then((info) => patchConn(id, { status: "open", info, title: info.title, message: null }))
    .catch((e: unknown) => {
      if (sftpStore.get().conns[id]?.status === "closed") return;
      patchConn(id, { status: "error", message: errorMessage(e) });
    });
}

export function openSftp(target: SftpTarget, title: string, hostId: Uuid | null): Uuid {
  const s = sftpStore.get();
  if (target.kind === "session") {
    const existing = Object.values(s.conns).find(
      (c) =>
        c.target.kind === "session" &&
        c.target.session_id === target.session_id &&
        c.status !== "closed" &&
        c.status !== "error",
    );
    if (existing) {
      setActiveSftp(existing.id);
      return existing.id;
    }
  }
  const id = crypto.randomUUID();
  const conn: SftpConn = {
    id,
    target,
    title,
    hostId,
    status: "connecting",
    message: null,
    info: null,
  };
  update((st) => ({
    ...st,
    conns: { ...st.conns, [id]: conn },
    order: [...st.order, id],
    activeId: id,
  }));
  connect(id, target);
  return id;
}

export const openSftpForHost = (hostId: Uuid, title: string) =>
  openSftp({ kind: "host", host_id: hostId }, title, hostId);

export const openSftpForSession = (sessionId: Uuid, title: string, hostId: Uuid | null) =>
  openSftp({ kind: "session", session_id: sessionId }, title, hostId);

export function reconnectSftp(id: Uuid) {
  const c = sftpStore.get().conns[id];
  if (!c) return;
  patchConn(id, { status: "connecting", message: null, info: null });
  connect(id, c.target);
}

export function setActiveSftp(id: Uuid | null) {
  update((s) => (s.activeId === id ? s : { ...s, activeId: id }));
}

export async function closeSftp(id: Uuid) {
  update((s) => {
    const conns = omit(s.conns, id);
    const order = s.order.filter((x) => x !== id);
    const activeId = s.activeId === id ? (order[order.length - 1] ?? null) : s.activeId;
    return { ...s, conns, order, activeId };
  });
  await ipc.sftpClose(id).catch(() => undefined);
}

// ───────────────────────────── transfers ─────────────────────────────

function addTransfer(info: TransferInfo) {
  update((s) => {
    if (s.transfers[info.id]) return s;
    const t: Transfer = {
      id: info.id,
      sftpId: info.sftpId,
      direction: info.direction,
      local: info.local,
      remote: info.remote,
      status: "running",
      done: 0,
      total: null,
      filesDone: 0,
      filesTotal: 1,
      current: "",
      message: null,
    };
    return {
      ...s,
      transfers: { ...s.transfers, [info.id]: t },
      transferOrder: [info.id, ...s.transferOrder],
    };
  });
}

export async function startTransfer(args: {
  sftpId: Uuid;
  direction: Direction;
  local: string;
  remote: string;
}) {
  const info = await ipc.transferStart(args);
  addTransfer(info);
  return info;
}

export const cancelTransfer = (id: Uuid) => ipc.transferCancel(id);

export function clearFinishedTransfers() {
  update((s) => {
    const transfers: Record<Uuid, Transfer> = {};
    const transferOrder = s.transferOrder.filter((id) => {
      const t = s.transfers[id];
      if (t?.status === "running") {
        transfers[id] = t;
        return true;
      }
      return false;
    });
    return { ...s, transfers, transferOrder };
  });
}

// ───────────────────────────── events ─────────────────────────────

function onSftpEvent(ev: SftpEvent) {
  switch (ev.type) {
    case "opened":
      if (sftpStore.get().conns[ev.id]) {
        patchConn(ev.id, { status: "open", info: ev.info, title: ev.info.title });
      }
      break;
    case "closed": {
      const c = sftpStore.get().conns[ev.id];
      if (c && c.status !== "closed") {
        patchConn(ev.id, { status: "closed", message: "Connection closed" });
      }
      break;
    }
  }
}

function onTransferEvent(ev: TransferEvent, queryClient: QueryClient) {
  switch (ev.type) {
    case "started":
      addTransfer(ev.info);
      break;
    case "progress":
      patchTransfer(ev.id, {
        done: ev.done,
        total: ev.total,
        filesDone: ev.files_done,
        filesTotal: ev.files_total,
        current: ev.current,
      });
      break;
    case "finished":
    case "failed":
    case "cancelled": {
      const t = sftpStore.get().transfers[ev.id];
      patchTransfer(ev.id, {
        status: ev.type === "finished" ? "done" : ev.type,
        message: ev.type === "failed" ? ev.message : null,
        done: ev.type === "finished" ? ev.bytes : (t?.done ?? 0),
      });
      const side = t?.direction === "upload" ? "remote" : "local";
      void queryClient.invalidateQueries({ queryKey: ["fs", side] });
      break;
    }
  }
}

let started = false;
export function startSftpEvents(queryClient: QueryClient) {
  if (started) return;
  started = true;
  void ipc.onSftpEvent(onSftpEvent);
  void ipc.onTransferEvent((ev) => onTransferEvent(ev, queryClient));
}
