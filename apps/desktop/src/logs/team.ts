// Team recordings: pure helpers behind the Logs page (author filter, why a
// vault is / is not being recorded).

import type { LocalVault, LogAuthor, LogCard, Uuid } from "@/ipc/types";

export function authorName(a: LogAuthor): string {
  const name = a.displayName?.trim() ?? "";
  return name.length > 0 ? name : a.email;
}

/** Author filter of a team vault: everyone who recorded something, most recordings first. */
export function authorsOf(list: LogCard[]): LogAuthor[] {
  const seen = new Map<Uuid, { author: LogAuthor; n: number }>();
  for (const l of list) {
    if (!l.author) continue;
    const cur = seen.get(l.author.userId);
    if (cur) cur.n += 1;
    else seen.set(l.author.userId, { author: l.author, n: 1 });
  }
  return [...seen.values()].sort((a, b) => b.n - a.n).map((x) => x.author);
}

export type RecordingState = "off" | "mine" | "team";

/**
 * Why sessions in `vault` are (not) being captured on this device: the
 * vault manager's team policy wins; otherwise it is the user's own switch.
 */
export function recordingState(vault: LocalVault | null, recordSessions: boolean): RecordingState {
  if (vault?.kind === "team" && vault.session_logging) return "team";
  return recordSessions ? "mine" : "off";
}

/** Recordings of `vaultId` (all of them when no vault is active), narrowed to one author. */
export function visibleLogs(list: LogCard[], vaultId: Uuid | null, authorId: Uuid | null) {
  return list.filter(
    (l) =>
      (vaultId === null || l.vaultId === vaultId) &&
      (authorId === null || l.author?.userId === authorId),
  );
}
