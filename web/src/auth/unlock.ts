import { useSyncExternalStore } from "react";
import { authStore } from "./store";

interface Waiter {
  resolve: (privateKey: string) => void;
  reject: (err: Error) => void;
}

let waiters: Waiter[] = [];
let open = false;
const listeners = new Set<() => void>();

function emit() {
  for (const l of listeners) l();
}

/**
 * Resolves with the account private key, prompting for the password when the key
 * is not available in this tab. Rejects if the user dismisses the prompt.
 */
export function requireUnlocked(): Promise<string> {
  const { privateKey } = authStore.get();
  if (privateKey) return Promise.resolve(privateKey);
  return new Promise<string>((resolve, reject) => {
    waiters.push({ resolve, reject });
    if (!open) {
      open = true;
      emit();
    }
  });
}

export const unlockPrompt = {
  isOpen: () => open,
  subscribe: (l: () => void) => {
    listeners.add(l);
    return () => listeners.delete(l);
  },
  resolve(privateKey: string) {
    const w = waiters;
    waiters = [];
    open = false;
    emit();
    for (const x of w) x.resolve(privateKey);
  },
  cancel() {
    const w = waiters;
    waiters = [];
    open = false;
    emit();
    for (const x of w) x.reject(new UnlockCancelled());
  },
};

export class UnlockCancelled extends Error {
  constructor() {
    super("Unlock cancelled");
    this.name = "UnlockCancelled";
  }
}

export function useUnlockPromptOpen(): boolean {
  return useSyncExternalStore(unlockPrompt.subscribe, unlockPrompt.isOpen, unlockPrompt.isOpen);
}
