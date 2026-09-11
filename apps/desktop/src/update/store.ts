// Updater UI state. Rust owns the check/verify/install pipeline; this only
// mirrors its events so Settings and the shell banner render the same thing.

import * as ipc from "@/ipc/commands";
import type { UpdateInfo } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { createStore, useStore } from "@/lib/store";

export type UpdatePhase =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate"; checkedAt: number }
  | { kind: "available"; info: UpdateInfo }
  | { kind: "downloading"; info: UpdateInfo; downloaded: number; total: number | null }
  | { kind: "installed"; version: string }
  | { kind: "error"; message: string };

export interface UpdateState {
  phase: UpdatePhase;
  /** Set when the startup check found a release and the user has not dismissed it. */
  banner: boolean;
}

export const updateStore = createStore<UpdateState>({ phase: { kind: "idle" }, banner: false });

export const useUpdate = <S>(selector: (s: UpdateState) => S) => useStore(updateStore, selector);

const setPhase = (phase: UpdatePhase) => updateStore.set((s) => ({ ...s, phase }));

export async function checkForUpdates(): Promise<void> {
  const cur = updateStore.get().phase;
  if (cur.kind === "checking" || cur.kind === "downloading") return;
  setPhase({ kind: "checking" });
  try {
    const info = await ipc.updateCheck();
    setPhase(info ? { kind: "available", info } : { kind: "upToDate", checkedAt: Date.now() });
  } catch (e) {
    setPhase({ kind: "error", message: errorMessage(e) });
  }
}

export async function installUpdate(): Promise<void> {
  const cur = updateStore.get().phase;
  if (cur.kind !== "available") return;
  setPhase({ kind: "downloading", info: cur.info, downloaded: 0, total: null });
  try {
    const info = await ipc.updateInstall();
    updateStore.set({ phase: { kind: "installed", version: info.version }, banner: false });
  } catch (e) {
    setPhase({ kind: "error", message: errorMessage(e) });
  }
}

export const restartToUpdate = () => ipc.updateRestart();

export const dismissBanner = () => updateStore.set((s) => ({ ...s, banner: false }));

let started = false;
export function startUpdateEvents() {
  if (started) return;
  started = true;
  void ipc.onUpdateEvent((ev) => {
    switch (ev.type) {
      case "available":
        updateStore.set({ phase: { kind: "available", info: ev.info }, banner: true });
        break;
      case "progress":
        updateStore.set((s) =>
          s.phase.kind === "downloading"
            ? { ...s, phase: { ...s.phase, downloaded: ev.downloaded, total: ev.total } }
            : s,
        );
        break;
      case "installed":
        updateStore.set({ phase: { kind: "installed", version: ev.version }, banner: false });
        break;
      case "failed":
        setPhase({ kind: "error", message: ev.message });
        break;
    }
  });
}
