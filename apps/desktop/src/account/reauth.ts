// Step-up ("sudo mode") orchestration for sensitive account changes.
//
// Guarded server routes (device revoke, SSH ID changes, security keys…)
// answer `reauth_required` unless the session re-proved the password (and
// second factor) within the last few minutes. `withReauth` runs the action,
// and on that error asks the user to confirm through `ReauthDialog`, then
// retries once.

import { createStore } from "@/lib/store";
import { isReauthRequired } from "@/ipc/types";

interface Request {
  resolve: (confirmed: boolean) => void;
}

interface State {
  request: Request | null;
}

export const reauthStore = createStore<State>({ request: null });

/** Thrown by `withReauth` when the user dismisses the confirmation. */
export class ReauthCancelled extends Error {
  constructor() {
    super("Re-authentication cancelled");
    this.name = "ReauthCancelled";
  }
}

export function isReauthCancelled(e: unknown): e is ReauthCancelled {
  return e instanceof ReauthCancelled;
}

let inFlight: Promise<boolean> | null = null;

/** Show the dialog (or join the one already open); `true` once the session is confirmed. */
export function requestReauth(): Promise<boolean> {
  if (inFlight) return inFlight;
  inFlight = new Promise<boolean>((resolve) => {
    reauthStore.set({
      request: {
        resolve: (confirmed) => {
          inFlight = null;
          reauthStore.set({ request: null });
          resolve(confirmed);
        },
      },
    });
  });
  return inFlight;
}

/**
 * Run `action`; when the server asks for a fresh re-authentication, confirm
 * it with the user and run `action` again.
 */
export async function withReauth<T>(action: () => Promise<T>): Promise<T> {
  try {
    return await action();
  } catch (e) {
    if (!isReauthRequired(e)) throw e;
    const confirmed = await requestReauth();
    if (!confirmed) throw new ReauthCancelled();
    return await action();
  }
}
