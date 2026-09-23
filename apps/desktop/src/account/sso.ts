// Browser-based single sign-on, shared between the sign-in form and the
// deep-link handler. Rust owns the flow and the verified session; this
// store only mirrors what the UI needs to show.

import * as ipc from "@/ipc/commands";
import { errorMessage, type SsoOutcome, type SsoProvider } from "@/ipc/types";
import { createStore } from "@/lib/store";
import { parseSsoLink } from "./ssoLink";
import { tr } from "@/i18n";

export { parseSsoLink } from "./ssoLink";

export type SsoVerified = Extract<SsoOutcome, { step: "loginRequired" | "registrationRequired" }>;

export type SsoState =
  | { phase: "idle" }
  | { phase: "waiting"; serverUrl: string; provider: SsoProvider; flowId: string }
  | { phase: "verified"; serverUrl: string; provider: SsoProvider; outcome: SsoVerified };

export const ssoStore = createStore<SsoState>({ phase: "idle" });

const POLL_MS = 2_000;
/** Give up waiting for the browser after this long; the server flow expires anyway. */
const TIMEOUT_MS = 10 * 60_000;

let timer: ReturnType<typeof setTimeout> | null = null;
let deadline = 0;
let onFailed: ((message: string) => void) | null = null;

function stopPolling() {
  if (timer) clearTimeout(timer);
  timer = null;
}

function settle(outcome: SsoOutcome, flowId: string) {
  const s = ssoStore.get();
  if (s.phase !== "waiting" || s.flowId !== flowId) return;
  switch (outcome.step) {
    case "pending":
      return;
    case "failed":
      stopPolling();
      ssoStore.set({ phase: "idle" });
      onFailed?.(outcome.message);
      return;
    default:
      stopPolling();
      ssoStore.set({ phase: "verified", serverUrl: s.serverUrl, provider: s.provider, outcome });
  }
}

function schedule(flowId: string) {
  stopPolling();
  timer = setTimeout(() => {
    void tick(flowId);
  }, POLL_MS);
}

async function tick(flowId: string) {
  const s = ssoStore.get();
  if (s.phase !== "waiting" || s.flowId !== flowId) return;
  if (Date.now() > deadline) {
    void cancelSso();
    onFailed?.(tr("Sign-in timed out — the browser did not finish in time"));
    return;
  }
  try {
    settle(await ipc.accountSsoPoll(), flowId);
  } catch (e) {
    // Network blips are retried; a flow the backend no longer knows is over.
    const msg = errorMessage(e);
    if (/no single sign-on|expired|unauthori[sz]ed/i.test(msg)) {
      stopPolling();
      ssoStore.set({ phase: "idle" });
      onFailed?.(msg);
      return;
    }
  }
  if (ssoStore.get().phase === "waiting") schedule(flowId);
}

/**
 * Open the provider in the system browser and keep polling the server until
 * the identity is verified, fails or times out. `fail` reports the terminal
 * error to the user (the store just goes back to idle).
 */
export async function startSso(
  serverUrl: string,
  provider: SsoProvider,
  fail: (message: string) => void,
): Promise<void> {
  await cancelSso();
  const flowId = await ipc.accountSsoStart({ serverUrl, provider: provider.id });
  onFailed = fail;
  deadline = Date.now() + TIMEOUT_MS;
  ssoStore.set({ phase: "waiting", serverUrl, provider, flowId });
  schedule(flowId);
}

/** Forget the current attempt on both sides. Safe to call when nothing is running. */
export async function cancelSso(): Promise<void> {
  stopPolling();
  onFailed = null;
  if (ssoStore.get().phase === "idle") return;
  ssoStore.set({ phase: "idle" });
  await ipc.accountSsoCancel().catch(() => undefined);
}

/** After the verified identity was handed to login / registration (or the user backed out). */
export function clearSso() {
  stopPolling();
  onFailed = null;
  ssoStore.set({ phase: "idle" });
}

/** Deep-link entry: resolves to `true` when the link matched a running sign-in. */
export async function handleSsoLink(url: string): Promise<boolean> {
  const flowId = parseSsoLink(url);
  if (!flowId) return false;
  const s = ssoStore.get();
  if (s.phase !== "waiting" || s.flowId !== flowId) {
    throw new Error(tr("This sign-in link does not belong to a sign-in started here"));
  }
  settle(await ipc.accountSsoCallback(flowId), flowId);
  return true;
}
