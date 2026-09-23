import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import * as ipc from "@/ipc/commands";
import type { LocalVault, Uuid } from "@/ipc/types";
import { isSftpTab } from "@/app/navigation";
import { onTransferDone, sftpStore } from "@/sftp/store";
import { multiplayerStore } from "@/terminal/multiplayer";
import { onCommandEvent, onSessionDropped, terminalStore } from "@/terminal/store";
import {
  type Notice,
  type NotifySettings,
  commandLongEnough,
  commandNotice,
  droppedNotice,
  joinedNotice,
  newParticipants,
  newSharedVaults,
  revokedByServer,
  sharedVaultNotice,
  signedOutNotice,
  transferNotice,
  wantsNotice,
} from "./notices";

type Deliver = (n: Notice) => void;

let current: NotifySettings | null = null;

export function applyNotificationSettings(s: NotifySettings) {
  current = s;
}

/** The window is not what the user is looking at right now. */
function windowInBackground(): boolean {
  return document.visibilityState === "hidden" || !document.hasFocus();
}

/** The pane's tab is not the one in front (or the window itself is not). */
function paneInBackground(paneId: Uuid): boolean {
  if (windowInBackground()) return true;
  const s = terminalStore.get();
  const tab = s.tabs.find((t) => t.paneIds.includes(paneId));
  return tab?.id !== s.activeTabId;
}

async function deliverSystem(n: Notice) {
  try {
    let ok = await isPermissionGranted();
    if (!ok) ok = (await requestPermission()) === "granted";
    if (ok) sendNotification({ title: n.title, body: n.body });
  } catch {
    // No notification daemon (headless Linux, restricted sandboxes): nothing to do.
  }
}

/**
 * `signedOut` follows both the user's own sign-out and a revocation by the
 * server; only the latter leaves the sync engine in an error state.
 */
async function noticeRevoked(deliver: Deliver) {
  try {
    const status = await ipc.accountStatus();
    if (revokedByServer(status.sync)) deliver(signedOutNotice());
  } catch {
    // Status unavailable: stay quiet rather than guess.
  }
}

/**
 * Wire every source to the OS notification centre. Returns the teardown.
 */
export function startNotifications(deliver: Deliver = (n) => void deliverSystem(n)): () => void {
  const startedAt = new Map<Uuid, number>();
  const unCommand = onCommandEvent((paneId, ev) => {
    if (ev.kind === "started") {
      startedAt.set(paneId, Date.now());
      return;
    }
    const began = startedAt.get(paneId);
    startedAt.delete(paneId);
    if (!wantsNotice(current, "commands")) return;
    if (!commandLongEnough(current, began, Date.now())) return;
    if (!paneInBackground(paneId)) return;
    const pane = terminalStore.get().panes[paneId];
    if (pane) deliver(commandNotice(pane, ev.exit));
  });

  const unDrop = onSessionDropped((paneId) => {
    if (!wantsNotice(current, "sessions")) return;
    const pane = terminalStore.get().panes[paneId];
    if (pane) deliver(droppedNotice(pane));
  });

  const unTransfer = onTransferDone((t) => {
    if (!wantsNotice(current, "transfers")) return;
    const inFront =
      !windowInBackground() &&
      isSftpTab(terminalStore.get().activeTabId) &&
      sftpStore.get().activeId === t.sftpId;
    if (inFront) return;
    deliver(transferNotice(t));
  });

  const knownVaults = new Set<Uuid>();
  let vaultsPrimed = false;
  const lookAtVaults = async () => {
    let vaults: LocalVault[];
    try {
      vaults = await ipc.vaultsList();
    } catch {
      return;
    }
    if (vaultsPrimed && wantsNotice(current, "account")) {
      for (const v of newSharedVaults(knownVaults, vaults)) deliver(sharedVaultNotice(v));
    }
    knownVaults.clear();
    for (const v of vaults) knownVaults.add(v.id);
    vaultsPrimed = true;
  };
  void lookAtVaults();

  let unSync: (() => void) | null = null;
  let stopped = false;
  void ipc
    .onSyncNotice((n) => {
      if (n.kind === "vaultsChanged") {
        void lookAtVaults();
      } else if (n.kind === "signedOut") {
        knownVaults.clear();
        vaultsPrimed = false;
        if (wantsNotice(current, "account")) void noticeRevoked(deliver);
      }
    })
    .then((un) => {
      if (stopped) un();
      else unSync = un;
    });

  const seenParticipants = new Map<Uuid, Set<Uuid>>();
  const unShares = multiplayerStore.subscribe(() => {
    const shares = multiplayerStore.get().shares;
    for (const paneId of [...seenParticipants.keys()]) {
      if (!shares[paneId]) seenParticipants.delete(paneId);
    }
    for (const share of Object.values(shares)) {
      if (share.role !== "host") continue;
      const seen = seenParticipants.get(share.id);
      if (wantsNotice(current, "account")) {
        for (const p of newParticipants(seen, share.participants)) {
          deliver(joinedNotice(p, terminalStore.get().panes[share.id]));
        }
      }
      seenParticipants.set(share.id, new Set(share.participants.map((p) => p.userId)));
    }
  });

  return () => {
    stopped = true;
    unCommand();
    unDrop();
    unTransfer();
    unShares();
    unSync?.();
  };
}
