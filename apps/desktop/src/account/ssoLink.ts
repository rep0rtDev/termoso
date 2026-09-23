const FLOW_ID = /^[A-Za-z0-9_-]{16,128}$/;

/**
 * `termoso://sso?flow=<id>` — the browser came back. The link carries only the
 * flow id (never a provider token or a session); Rust checks the id against
 * the flow it started and fetches the result from the server itself. Anything
 * else in the URL means it did not come from our server and is refused.
 */
export function parseSsoLink(url: string): string | null {
  const m = /^termoso:\/\/sso\/?(?:\?([^#]*))?$/i.exec(url.trim());
  if (!m) return null;
  const params = new URLSearchParams(m[1] ?? "");
  const keys = [...params.keys()];
  if (keys.length !== 1 || keys[0] !== "flow") return null;
  const flow = params.get("flow") ?? "";
  return FLOW_ID.test(flow) ? flow : null;
}

/** Whether a deep link is addressed to the sign-in callback at all (even if malformed). */
export function isSsoLink(url: string): boolean {
  return /^termoso:\/\/sso(?:[/?#]|$)/i.test(url.trim());
}
