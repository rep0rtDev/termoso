import { toast } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import {
  errorMessage,
  isDesktopError,
  type LiveEvent,
  type ShareInfo,
  type Uuid,
} from "@/ipc/types";
import { createStore, omit, useStore } from "@/lib/store";
import { setPaneFixedSize, terminalStore, writeSystemLine } from "./store";
import { tr } from "@/i18n";

/** Shared (host) and watched (viewer) panes, by pane id. */
export interface MultiplayerState {
  shares: Record<Uuid, ShareInfo>;
  /** Viewer pane that was just handed remote control — shows the "start typing" hint. */
  controlHint: Uuid | null;
  /** Who typed last in each shared pane (badge at the cursor); `at` is `Date.now()`. */
  typing: Record<Uuid, { userId: Uuid; at: number }>;
}

export const multiplayerStore = createStore<MultiplayerState>({
  shares: {},
  controlHint: null,
  typing: {},
});

export const useMultiplayer = <S>(selector: (s: MultiplayerState) => S) =>
  useStore(multiplayerStore, selector);

export const useShare = (paneId: Uuid | undefined) =>
  useMultiplayer((s) => (paneId ? (s.shares[paneId] ?? null) : null));

export const useTyping = (paneId: Uuid) => useMultiplayer((s) => s.typing[paneId] ?? null);

const update = (fn: (s: MultiplayerState) => MultiplayerState) =>
  multiplayerStore.set(fn(multiplayerStore.get()));

function put(share: ShareInfo) {
  update((s) => ({ ...s, shares: { ...s.shares, [share.id]: share } }));
}

/** How long the "who is typing" badge stays after the last keystroke notice. */
const TYPING_LINGER_MS = 2500;
const typingTimers = new Map<Uuid, ReturnType<typeof setTimeout>>();

function noteTyping(paneId: Uuid, userId: Uuid) {
  update((st) => ({ ...st, typing: { ...st.typing, [paneId]: { userId, at: Date.now() } } }));
  const prev = typingTimers.get(paneId);
  if (prev) clearTimeout(prev);
  typingTimers.set(
    paneId,
    setTimeout(() => {
      typingTimers.delete(paneId);
      update((st) => ({ ...st, typing: omit(st.typing, paneId) }));
    }, TYPING_LINGER_MS),
  );
}

function forget(paneId: Uuid) {
  update((s) => ({
    ...s,
    shares: omit(s.shares, paneId),
    controlHint: s.controlHint === paneId ? null : s.controlHint,
    typing: omit(s.typing, paneId),
  }));
}

/** Is the multiplayer prohibited by one of the user's teams (server-side flag)? */
export const isMultiplayerDisabled = (e: unknown) =>
  isDesktopError(e) && e.kind === "multiplayer_disabled";

/** Not signed in to any Termoso account — sharing needs the relay. */
export const isNotSignedIn = (e: unknown) =>
  isDesktopError(e) && e.kind === "invalid" && /not signed in/i.test(e.message);

/** Start sharing a pane; resolves with the link to hand out. Rethrows so the UI can explain. */
export async function startShare(paneId: Uuid): Promise<ShareInfo> {
  const info = await ipc.multiplayerStart(paneId);
  put(info);
  return info;
}

export async function stopShare(paneId: Uuid) {
  try {
    await ipc.multiplayerStop(paneId);
  } finally {
    forget(paneId);
  }
}

export async function setControl(paneId: Uuid, userId: Uuid, enabled: boolean) {
  await ipc.multiplayerSetControl(paneId, userId, enabled);
  const who = multiplayerStore.get().shares[paneId]?.participants.find((p) => p.userId === userId);
  const name = who?.displayName ?? who?.email ?? "Participant";
  toast(
    enabled
      ? tr("{name} has remote control", { name })
      : tr("Remote control taken back from {name}", { name }),
    "info",
  );
  update((s) => {
    const share = s.shares[paneId];
    if (!share) return s;
    return {
      ...s,
      shares: {
        ...s.shares,
        [paneId]: {
          ...share,
          participants: share.participants.map((p) =>
            p.userId === userId ? { ...p, canWrite: enabled } : p,
          ),
        },
      },
    };
  });
}

export function dismissControlHint() {
  update((s) => (s.controlHint ? { ...s, controlHint: null } : s));
}

/** A viewer pane finished joining: pull the initial participant list. */
export async function viewerJoined(paneId: Uuid) {
  try {
    const info = await ipc.multiplayerInfo(paneId);
    if (info) {
      put(info);
      const host = info.participants.find((p) => p.isHost);
      const who = host?.displayName ?? host?.email ?? tr("the host");
      writeSystemLine(
        paneId,
        info.canWrite
          ? tr("Multiplayer: you are watching {who}'s terminal", { who })
          : tr("Multiplayer: you are watching {who}'s terminal (view only)", { who }),
      );
    }
  } catch (e) {
    toast(errorMessage(e), "error");
  }
}

function onLiveEvent(ev: LiveEvent) {
  const s = multiplayerStore.get();
  const share = s.shares[ev.id];
  switch (ev.type) {
    case "participants":
      if (share) put({ ...share, participants: ev.participants });
      break;
    case "control":
      if (share) put({ ...share, canWrite: ev.canWrite });
      if (share?.role === "viewer") {
        update((st) => ({ ...st, controlHint: ev.canWrite ? ev.id : null }));
        if (!ev.canWrite) writeSystemLine(ev.id, tr("Multiplayer: remote control revoked"));
      }
      break;
    case "resize":
      if (terminalStore.get().panes[ev.id]?.target.kind === "live") {
        setPaneFixedSize(ev.id, { cols: ev.cols, rows: ev.rows });
      }
      break;
    case "title":
      break;
    case "typing": {
      // Our own keystrokes need no badge.
      const me = share?.participants.find((p) => p.isMe)?.userId;
      if (share && ev.user_id !== me) noteTyping(ev.id, ev.user_id);
      break;
    }
    case "ended": {
      const role = share?.role;
      forget(ev.id);
      if (role === "host") {
        const pane = terminalStore.get().panes[ev.id];
        if (ev.reason !== "stopped") {
          toast(`Multiplayer for ${pane?.title ?? "terminal"} ended: ${ev.message}`, "warning");
        }
      } else if (role === "viewer") {
        writeSystemLine(ev.id, tr("Multiplayer: {message}", { message: ev.message }));
      }
      break;
    }
  }
}

let started = false;
/** Subscribe to Rust multiplayer events once for the app lifetime. */
export function startMultiplayerEvents() {
  if (started) return;
  started = true;
  void ipc.onLiveEvent(onLiveEvent);
  terminalStore.subscribe(() => {
    const panes = terminalStore.get().panes;
    for (const id of Object.keys(multiplayerStore.get().shares)) {
      if (!panes[id]) forget(id);
    }
  });
}
