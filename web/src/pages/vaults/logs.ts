import type { LogAuthor, LogMeta, SessionLog } from "@/api/types";

/** Distinct authors, most recordings first. */
export function authorsOf(list: SessionLog[]): LogAuthor[] {
  const seen = new Map<string, { author: LogAuthor; n: number }>();
  for (const l of list) {
    if (!l.author) continue;
    const cur = seen.get(l.author.user_id);
    if (cur) cur.n += 1;
    else seen.set(l.author.user_id, { author: l.author, n: 1 });
  }
  return [...seen.values()].sort((a, b) => b.n - a.n).map((x) => x.author);
}

/** Pinned first, then newest first. */
export function sortLogs(list: SessionLog[]): SessionLog[] {
  return [...list].sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    return b.created_at.localeCompare(a.created_at);
  });
}

/** `1h 02m` / `3m 10s` / `recording` from a decrypted meta. */
export function durationOf(meta: LogMeta): string {
  if (!meta.ended_at) return "recording";
  const secs = Math.max(
    0,
    Math.round((new Date(meta.ended_at).getTime() - new Date(meta.started_at).getTime()) / 1000),
  );
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  if (m > 0) return `${m}m ${String(s).padStart(2, "0")}s`;
  return `${s}s`;
}
