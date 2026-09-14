import { useSyncExternalStore } from "react";
import { isReauthRequired } from "./flows";
import { authStore } from "./store";

interface Waiter {
  resolve: () => void;
  reject: (err: Error) => void;
}

let waiters: Waiter[] = [];
let open = false;
let needsKey = false;
const listeners = new Set<() => void>();

function emit() {
  for (const l of listeners) l();
}

function prompt(forKey: boolean): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    waiters.push({ resolve, reject });
    needsKey = needsKey || forKey;
    if (!open) {
      open = true;
      emit();
    }
  });
}

/**
 * Resolves with the account private key, prompting for the password when the key
 * is not available in this tab. Rejects if the user dismisses the prompt.
 */
export async function requireUnlocked(): Promise<string> {
  const { privateKey } = authStore.get();
  if (privateKey) return privateKey;
  await prompt(true);
  const pk = authStore.get().privateKey;
  if (!pk) throw new Error("Unlock failed");
  return pk;
}

/**
 * Runs a sensitive action; when the server answers `reauth_required`, asks the
 * user to confirm their identity (password / MFA / email code) and retries once.
 */
export async function withStepUp<T>(action: () => Promise<T>): Promise<T> {
  try {
    return await action();
  } catch (e) {
    if (!isReauthRequired(e)) throw e;
    await prompt(false);
    return await action();
  }
}

export const unlockPrompt = {
  isOpen: () => open,
  /** Some waiter needs the private key, which only the password can unwrap. */
  needsKey: () => needsKey,
  subscribe: (l: () => void) => {
    listeners.add(l);
    return () => listeners.delete(l);
  },
  /** The identity was confirmed: the session is stepped up and the tab holds the private key when a password was used. */
  resolve() {
    const w = waiters;
    waiters = [];
    open = false;
    needsKey = false;
    emit();
    for (const x of w) x.resolve();
  },
  cancel() {
    const w = waiters;
    waiters = [];
    open = false;
    needsKey = false;
    emit();
    for (const x of w) x.reject(new UnlockCancelled());
  },
};

export class UnlockCancelled extends Error {
  constructor() {
    super("Cancelled");
    this.name = "UnlockCancelled";
  }
}

export function useUnlockPromptOpen(): boolean {
  return useSyncExternalStore(unlockPrompt.subscribe, unlockPrompt.isOpen, unlockPrompt.isOpen);
}
