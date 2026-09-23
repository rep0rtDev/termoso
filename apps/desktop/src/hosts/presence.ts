import type { PresenceEntry, PresenceSession, TeamPresence, Uuid } from "@/ipc/types";
import { tr, msg } from "@/i18n";

/** One teammate's device on one host: the sessions it has open there. */
export interface HostViewer {
  userId: Uuid;
  email: string;
  displayName: string | null;
  avatar: string | null;
  deviceId: Uuid;
  deviceName: string;
  platform: string;
  /** Distinct protocols, in the order they were opened. */
  protocols: string[];
  /** Earliest `since` across the device's sessions on this host. */
  since: string;
  /** This is the signed-in account (any of its devices). */
  me: boolean;
}

/** Who is on which host, keyed by host id. Devices are listed oldest connection first. */
export function viewersByHost(
  presence: TeamPresence | undefined,
  myUserId: Uuid | null,
): Map<Uuid, HostViewer[]> {
  const out = new Map<Uuid, HostViewer[]>();
  if (!presence?.enabled) return out;
  for (const e of presence.entries) {
    for (const [hostId, sessions] of byHost(e.sessions)) {
      const list = out.get(hostId) ?? [];
      list.push(viewer(e, sessions, myUserId));
      out.set(hostId, list);
    }
  }
  for (const list of out.values()) list.sort((a, b) => a.since.localeCompare(b.since));
  return out;
}

function byHost(sessions: PresenceSession[]): Map<Uuid, PresenceSession[]> {
  const m = new Map<Uuid, PresenceSession[]>();
  for (const s of sessions) {
    const list = m.get(s.host_id) ?? [];
    list.push(s);
    m.set(s.host_id, list);
  }
  return m;
}

function viewer(e: PresenceEntry, sessions: PresenceSession[], myUserId: Uuid | null): HostViewer {
  const sorted = [...sessions].sort((a, b) => a.since.localeCompare(b.since));
  const protocols: string[] = [];
  for (const s of sorted) if (!protocols.includes(s.protocol)) protocols.push(s.protocol);
  return {
    userId: e.user_id,
    email: e.email,
    displayName: e.display_name ?? null,
    avatar: e.avatar ?? null,
    deviceId: e.device_id,
    deviceName: e.device_name,
    platform: e.platform,
    protocols,
    since: sorted[0]?.since ?? e.seen_at,
    me: myUserId !== null && e.user_id === myUserId,
  };
}

/** Display name when set, else the email. */
export const viewerName = (v: HostViewer) =>
  v.displayName !== null && v.displayName.trim() !== "" ? v.displayName : v.email;

/** Distinct people (not devices) among the viewers, in first-seen order. */
export function distinctPeople(viewers: HostViewer[]): HostViewer[] {
  const seen = new Set<Uuid>();
  return viewers.filter((v) => (seen.has(v.userId) ? false : (seen.add(v.userId), true)));
}

/** `2 min`, `1 h 05 min`, `3 d 2 h` — how long a connection has been open. */
export function connectedFor(sinceIso: string, now = Date.now()): string {
  const t = Date.parse(sinceIso);
  if (Number.isNaN(t)) return "";
  const s = Math.max(0, Math.floor((now - t) / 1000));
  if (s < 60) return "just now";
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h} h ${String(m % 60).padStart(2, "0")} min`;
  const d = Math.floor(h / 24);
  return `${d} d ${h % 24} h`;
}

const PROTOCOL_LABEL: Record<string, string> = {
  ssh: "SSH",
  mosh: "Mosh",
  telnet: "Telnet",
  sftp: "SFTP",
  forward: msg("Port forwarding"),
  serial: "Serial",
};

export const protocolLabel = (p: string) => tr(PROTOCOL_LABEL[p] ?? p.toUpperCase());

const PLATFORM_LABEL: Record<string, string> = {
  windows: "Windows",
  linux: "Linux",
  macos: "macOS",
  android: "Android",
  ios: "iOS",
  web: "Web",
  cli: "CLI",
};

export const platformLabel = (p: string) => PLATFORM_LABEL[p] ?? p;

/** `Alice`, `Alice and Bob`, `Alice, Bob and 2 others` — for a card tooltip. */
export function viewersSummary(viewers: HostViewer[]): string {
  const names = distinctPeople(viewers).map((v) => (v.me ? "You" : viewerName(v)));
  if (names.length === 0) return "";
  if (names.length === 1) return names[0] ?? "";
  if (names.length === 2) return `${names[0]} and ${names[1]}`;
  const rest = names.length - 2;
  return `${names[0]}, ${names[1]} and ${rest} other${rest === 1 ? "" : "s"}`;
}
