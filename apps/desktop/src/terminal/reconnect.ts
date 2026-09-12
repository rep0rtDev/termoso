import type { Uuid } from "@/ipc/types";

/** Automatic attempts per drop before giving up (Termius counts down from 6). */
export const RECONNECT_ATTEMPTS = 6;
const RECONNECT_STEP_MS = 30_000;
const RECONNECT_MAX_MS = 60_000;

/** Panes waiting for the next automatic attempt; one countdown is shared. */
export interface ReconnectQueue {
  paneIds: Uuid[];
  dueAt: number;
  delayMs: number;
  attemptsLeft: number;
}

/** Wait before attempt number `attempt` (1-based): 30 s, then 60 s. */
export function reconnectDelayMs(attempt: number): number {
  return Math.min(RECONNECT_STEP_MS * attempt, RECONNECT_MAX_MS);
}

/**
 * Put a pane that has already made `attempts` retries on the queue. Returns
 * `null` when the pane is out of attempts, the unchanged queue when it is
 * already listed, and otherwise a queue that keeps the running countdown.
 */
export function enqueueReconnect(
  queue: ReconnectQueue | null,
  paneId: Uuid,
  attempts: number,
  now: number,
): ReconnectQueue | null {
  if (attempts >= RECONNECT_ATTEMPTS) return null;
  const attemptsLeft = RECONNECT_ATTEMPTS - attempts;
  if (queue) {
    if (queue.paneIds.includes(paneId)) return queue;
    return {
      ...queue,
      paneIds: [...queue.paneIds, paneId],
      attemptsLeft: Math.min(queue.attemptsLeft, attemptsLeft),
    };
  }
  const delayMs = reconnectDelayMs(attempts + 1);
  return { paneIds: [paneId], dueAt: now + delayMs, delayMs, attemptsLeft };
}

/** Drop a pane from the queue; an emptied queue becomes `null`. */
export function dequeueReconnect(
  queue: ReconnectQueue | null,
  paneId: Uuid,
): ReconnectQueue | null {
  if (!queue?.paneIds.includes(paneId)) return queue;
  const paneIds = queue.paneIds.filter((id) => id !== paneId);
  return paneIds.length ? { ...queue, paneIds } : null;
}

/** Snackbar headline, singular/plural like Termius. */
export function disconnectedLabel(count: number): string {
  return count === 1 ? "1 session is disconnected." : `${count} sessions are disconnected.`;
}
