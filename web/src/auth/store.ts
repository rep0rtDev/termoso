import { useSyncExternalStore } from "react";
import type { AccountKeys, DeviceInfo, Session, UserProfile } from "@/api/types";

const SESSION_KEY = "termoso.session";
const DEVICE_KEY = "termoso.device";
const PRIVATE_KEY_KEY = "termoso.unlocked";

export const APP_VERSION = "0.1.0";

export interface AuthState {
  session: Session | null;
  /** Account X25519 private key (base64), present only after an OPAQUE login in this tab. */
  privateKey: string | null;
}

type Listener = () => void;
const listeners = new Set<Listener>();

function readJson(storage: Storage, key: string): unknown {
  const raw = storage.getItem(key);
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    storage.removeItem(key);
    return null;
  }
}

function loadInitial(): AuthState {
  const session = readJson(localStorage, SESSION_KEY) as Session | null;
  if (session && Date.parse(session.expires_at) < Date.now()) {
    localStorage.removeItem(SESSION_KEY);
    sessionStorage.removeItem(PRIVATE_KEY_KEY);
    return { session: null, privateKey: null };
  }
  return { session, privateKey: session ? sessionStorage.getItem(PRIVATE_KEY_KEY) : null };
}

let state: AuthState = loadInitial();

function emit() {
  for (const l of listeners) l();
}

function set(next: AuthState) {
  state = next;
  emit();
}

export const authStore = {
  get: () => state,
  subscribe: (l: Listener) => {
    listeners.add(l);
    return () => listeners.delete(l);
  },
  token: () => state.session?.token ?? null,

  signIn(session: Session, privateKey: string | null) {
    localStorage.setItem(SESSION_KEY, JSON.stringify(session));
    if (privateKey) sessionStorage.setItem(PRIVATE_KEY_KEY, privateKey);
    else sessionStorage.removeItem(PRIVATE_KEY_KEY);
    set({ session, privateKey });
  },

  unlock(privateKey: string) {
    sessionStorage.setItem(PRIVATE_KEY_KEY, privateKey);
    set({ ...state, privateKey });
  },

  updateUser(user: UserProfile) {
    if (!state.session) return;
    const session = { ...state.session, user };
    localStorage.setItem(SESSION_KEY, JSON.stringify(session));
    set({ ...state, session });
  },

  updateKeys(keys: AccountKeys) {
    if (!state.session) return;
    const session = { ...state.session, keys };
    localStorage.setItem(SESSION_KEY, JSON.stringify(session));
    set({ ...state, session });
  },

  signOut() {
    localStorage.removeItem(SESSION_KEY);
    sessionStorage.removeItem(PRIVATE_KEY_KEY);
    set({ session: null, privateKey: null });
  },
};

export function useAuthState(): AuthState {
  return useSyncExternalStore(authStore.subscribe, authStore.get, authStore.get);
}

/** Stable per-browser device id so re-logins reuse the same device record. */
export function deviceId(): string {
  let id = localStorage.getItem(DEVICE_KEY);
  if (!id) {
    id = crypto.randomUUID();
    localStorage.setItem(DEVICE_KEY, id);
  }
  return id;
}

export function describeBrowser(): string {
  const ua = navigator.userAgent;
  const first = (pairs: [string, string][], fallback: string) =>
    pairs.find(([needle]) => ua.includes(needle))?.[1] ?? fallback;
  const browser = first(
    [
      ["Firefox/", "Firefox"],
      ["Edg/", "Edge"],
      ["OPR/", "Opera"],
      ["Chrome/", "Chrome"],
      ["Safari/", "Safari"],
    ],
    "Browser",
  );
  const os = first(
    [
      ["Windows", "Windows"],
      ["Android", "Android"],
      ["iPhone", "iOS"],
      ["iPad", "iOS"],
      ["Mac OS", "macOS"],
      ["Linux", "Linux"],
    ],
    "",
  );
  return os ? `${browser} on ${os}` : browser;
}

export function deviceInfo(): DeviceInfo {
  return {
    name: describeBrowser(),
    platform: "web",
    app_version: APP_VERSION,
    client_device_id: deviceId(),
  };
}
