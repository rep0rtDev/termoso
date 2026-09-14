import type { HostCard, HostProtocol, OpenTarget, Uuid } from "@/ipc/types";

export type QuickTarget = Extract<OpenTarget, { kind: "quick" }>;

const PORT_MAX = 65535;

/** `[::1]` → `::1`; anything else unchanged. */
const unbracket = (h: string) => h.replace(/^\[|\]$/g, "");

function parsePort(raw: string | undefined): number | null | undefined {
  if (raw === undefined || raw === "") return null;
  const n = Number(raw);
  return Number.isInteger(n) && n >= 1 && n <= PORT_MAX ? n : undefined;
}

/**
 * Parse `user@host:port` (SSH) or the URL forms other tools hand out:
 * `ssh://user@host:port`, `telnet://host:port`, `telnet:host:port`.
 * Credentials embedded in the URL are dropped — passwords never travel in links.
 */
export function parseQuickConnect(input: string): QuickTarget | null {
  const s = input.trim();
  if (!s) return null;
  const scheme = /^(ssh|telnet):(?:\/\/)?/i.exec(s);
  const protocol = scheme?.[1]?.toLowerCase() === "telnet" ? "telnet" : "ssh";
  const rest = scheme ? s.slice(scheme[0].length).replace(/[/?#].*$/, "") : s;
  if (!rest) return null;
  const m = /^(?:(?<user>[^@\s]+)@)?(?<host>\[[^\]\s]+\]|[^:\s@/]+)(?::(?<port>\d{1,5}))?$/.exec(
    rest,
  );
  const groups = m?.groups;
  const host = groups?.host ? unbracket(groups.host) : "";
  if (!groups || !host) return null;
  const port = parsePort(groups.port);
  if (port === undefined) return null;
  const username = groups.user?.replace(/:.*$/, "");
  return {
    kind: "quick",
    address: host,
    username: protocol === "telnet" || !username ? null : username,
    port,
    protocol,
  };
}

/** Rebuild a quick target from a connection-history row (`target` is `user@host:port`). */
export function quickFromHistory(target: string, protocol: string): QuickTarget | null {
  const t = parseQuickConnect(target);
  if (!t) return null;
  return protocol === "telnet" ? { ...t, username: null, protocol: "telnet" } : t;
}

/** `user@host:22` / `telnet host:23` for chips and list rows. */
export function quickLabel(t: QuickTarget) {
  const host = t.address.includes(":") ? `[${t.address}]` : t.address;
  const port = t.port ? `:${t.port}` : "";
  if (t.protocol === "telnet") return `telnet ${host}${port}`;
  return `${t.username ? `${t.username}@` : ""}${host}${port}`;
}

export interface KnownSuggestion {
  address: string;
  /** Non-standard port from a `[host]:port` known_hosts entry; null means default. */
  port: number | null;
}

/** known_hosts names are `host` or `[host]:port`; hashed (`|1|…`) entries stay as-is. */
export function parseKnownHostName(name: string): KnownSuggestion {
  const m = /^\[(?<host>[^\]]+)\]:(?<port>\d{1,5})$/.exec(name.trim());
  if (m?.groups?.host) {
    const port = parsePort(m.groups.port);
    if (port !== undefined) return { address: m.groups.host, port };
  }
  return { address: name.trim(), port: null };
}

/** A search string that looks like something you'd connect to rather than filter by. */
export function looksLikeTarget(s: string) {
  return /[@:.]/.test(s) || /^\d+$/.test(s) || s === "localhost";
}

/** Deep link that other Termoso clients can open (`termoso://host/<id>`). */
export const hostLink = (h: Pick<HostCard, "id">) => `termoso://host/${h.id}`;

/**
 * RFC-style `ssh://user@host:port` for the SSH section, or `telnet://host:port`
 * when asked for Telnet (or the host is Telnet-only). Null when the host has
 * no such section.
 */
export function protocolLink(h: HostCard, protocol: HostProtocol = h.protocol): string | null {
  const host = h.address.includes(":") ? `[${h.address}]` : h.address;
  if (protocol === "telnet") {
    const port = h.protocol === "telnet" ? h.port : h.telnetPort;
    return port === null ? null : `telnet://${host}:${port}`;
  }
  if (h.protocol !== "ssh") return null;
  const user = h.username ? `${encodeURIComponent(h.username)}@` : "";
  return `ssh://${user}${host}${h.port === 22 ? "" : `:${h.port}`}`;
}

/** Everything a link can ask the app to open. */
export type LinkTarget =
  | { kind: "host"; hostId: Uuid }
  | { kind: "quick"; target: QuickTarget }
  /** Multiplayer invitation (`https://<server>/join/<session>#<secret>` or `termoso://join/…`). */
  | { kind: "live"; link: string }
  | { kind: "unsupported"; url: string };

/** Is this a multiplayer invitation link? */
export const isLiveLink = (s: string) =>
  /^(?:termoso:\/\/join\/|https?:\/\/[^\s?#]+\/join\/)[0-9a-f-]{36}(?:[/?][^#]*)?#[A-Za-z0-9_-]{40,}$/i.test(
    s.trim(),
  );

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Resolve an incoming `termoso://`, `ssh://` or `telnet://` URL. */
export function parseLink(url: string): LinkTarget {
  const s = url.trim();
  if (isLiveLink(s)) return { kind: "live", link: s };
  const m = /^termoso:\/\/host\/([^/?#]+)\/?(?:[?#].*)?$/i.exec(s);
  if (m?.[1]) {
    const id = decodeURIComponent(m[1]);
    return UUID.test(id) ? { kind: "host", hostId: id } : { kind: "unsupported", url: s };
  }
  if (/^(ssh|telnet):/i.test(s)) {
    const target = parseQuickConnect(s);
    return target ? { kind: "quick", target } : { kind: "unsupported", url: s };
  }
  return { kind: "unsupported", url: s };
}
