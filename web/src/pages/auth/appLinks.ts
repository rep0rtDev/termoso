/**
 * Hand-off from the web landing pages to an installed Termoso client.
 *
 * `https://<server>/invite/<token>` and `https://<server>/join/<id>#<secret>`
 * are what people share. Android opens them in the app directly when App
 * Links are verified for this host; everywhere else the page renders and
 * offers the `termoso://` form of the same link, which any installed client
 * understands. The multiplayer secret lives in the URL fragment and is never
 * sent to the server — this module only ever reads it from `location.hash`.
 */

const SESSION_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const SECRET = /^[A-Za-z0-9_-]{40,}$/;

export const isSessionId = (s: string): boolean => SESSION_ID.test(s);

/** The live-sharing secret carried in `#…`, or `null` when absent/malformed. */
export function secretFromHash(hash: string): string | null {
  const s = hash.startsWith("#") ? hash.slice(1) : hash;
  return SECRET.test(s) ? s : null;
}

export const appInviteLink = (token: string): string =>
  `termoso://invite/${encodeURIComponent(token)}`;

/** `termoso://join/<id>?s=<server>#<secret>`; `server` is this cabinet's origin. */
export function appJoinLink(sessionId: string, server: string, secret: string): string {
  const q = new URLSearchParams({ s: server });
  return `termoso://join/${sessionId}?${q.toString()}#${secret}`;
}

/** Server base for clients: the origin plus the path the cabinet is mounted on. */
export function serverBase(loc: { origin: string; pathname: string }, route: string): string {
  const idx = loc.pathname.indexOf(route);
  const prefix = idx > 0 ? loc.pathname.slice(0, idx) : "";
  return `${loc.origin}${prefix}/`;
}
