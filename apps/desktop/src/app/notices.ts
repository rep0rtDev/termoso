import { tr } from "@/i18n";
import type { LocalVault, Settings, SyncStatus, Uuid } from "@/ipc/types";

/**
 * Text and decisions behind system notifications. Kept generic on purpose:
 * notification centres keep a history outside the app, so no command lines,
 * paths or addresses go in — only what the pane / transfer is called plus a
 * status.
 */

export interface Notice {
  title: string;
  body: string;
}

/** The slice of {@link Settings} the notifier reads. */
export type NotifySettings = Pick<
  Settings,
  | "notifications"
  | "notifyCommands"
  | "notifyCommandSeconds"
  | "notifyTransfers"
  | "notifySessions"
  | "notifyAccount"
>;

export type NoticeKind = "commands" | "transfers" | "sessions" | "account";

export function wantsNotice(s: NotifySettings | null, kind: NoticeKind): boolean {
  if (!s?.notifications) return false;
  switch (kind) {
    case "commands":
      return s.notifyCommands;
    case "transfers":
      return s.notifyTransfers;
    case "sessions":
      return s.notifySessions;
    case "account":
      return s.notifyAccount;
  }
}

/**
 * Whether a command that started at `startedAt` and ended at `now` ran long
 * enough to be worth a notification.
 */
export function commandLongEnough(
  s: NotifySettings | null,
  startedAt: number | undefined,
  now: number,
): boolean {
  if (startedAt === undefined) return false;
  return now - startedAt >= (s?.notifyCommandSeconds ?? 0) * 1000;
}

/** Program name only — arguments are where passwords and tokens end up. */
export function commandLabel(command: string | null): string | null {
  const word = command?.trim().split(/\s+/)[0] ?? "";
  if (!word) return null;
  const base = word.slice(word.lastIndexOf("/") + 1);
  return base.length > 32 ? `${base.slice(0, 31)}…` : base;
}

export function commandNotice(
  pane: { title: string; command: string | null },
  exit: number | null,
): Notice {
  const what = commandLabel(pane.command);
  const title =
    exit === null || exit === 0
      ? tr("Command finished")
      : tr("Command failed (exit {code})", { code: exit });
  return { title, body: what ? `${what} · ${pane.title}` : pane.title };
}

function baseName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const i = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  return trimmed.slice(i + 1) || trimmed;
}

export function transferNotice(t: {
  direction: "upload" | "download";
  status: string;
  local: string;
  remote: string;
}): Notice {
  const name = baseName(t.direction === "upload" ? t.local : t.remote);
  if (t.status === "failed") return { title: tr("Transfer failed"), body: name };
  return {
    title: t.direction === "upload" ? tr("Upload finished") : tr("Download finished"),
    body: name,
  };
}

/** Only the pane title: the backend's reason text can quote addresses or server output. */
export function droppedNotice(pane: { title: string }): Notice {
  return { title: tr("Connection lost"), body: pane.title };
}

export function sharedVaultNotice(v: { name: string }): Notice {
  return { title: tr("Vault shared with you"), body: v.name };
}

/** After `signedOut`: a revocation leaves the engine in error, the user's own sign-out resets it. */
export function revokedByServer(sync: Pick<SyncStatus, "state" | "lastError">): boolean {
  return sync.state === "error" && sync.lastError !== null;
}

export function signedOutNotice(): Notice {
  return { title: tr("Signed out"), body: tr("This device was signed out of the account.") };
}

export function joinedNotice(
  p: { displayName: string | null; email: string },
  pane: { title: string } | undefined,
): Notice {
  return {
    title: tr("{name} joined your shared terminal", {
      name: p.displayName?.trim() ? p.displayName : p.email,
    }),
    body: pane?.title ?? "",
  };
}

/** Team vaults that appeared since the last look, excluding ones this user manages (self-created). */
export function newSharedVaults(known: ReadonlySet<Uuid>, vaults: LocalVault[]): LocalVault[] {
  return vaults.filter((v) => v.kind === "team" && v.role !== "manager" && !known.has(v.id));
}

/**
 * Participants of a hosted share that were not there before. `seen` is
 * `undefined` the first time a share is observed — nobody is "new" then,
 * so a share restored with viewers already attached stays quiet.
 */
export function newParticipants<P extends { userId: Uuid; isMe: boolean }>(
  seen: ReadonlySet<Uuid> | undefined,
  participants: P[],
): P[] {
  if (!seen) return [];
  return participants.filter((p) => !p.isMe && !seen.has(p.userId));
}
