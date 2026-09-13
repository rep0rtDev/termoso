import { describe, expect, it } from "vitest";
import {
  RECONNECT_ATTEMPTS,
  dequeueReconnect,
  disconnectedLabel,
  enqueueReconnect,
  reconnectDelayMs,
} from "./reconnect";

const A = "00000000-0000-0000-0000-00000000000a";
const B = "00000000-0000-0000-0000-00000000000b";

describe("reconnectDelayMs", () => {
  it("starts at 30 s and caps at 60 s", () => {
    expect(reconnectDelayMs(1)).toBe(30_000);
    expect(reconnectDelayMs(2)).toBe(60_000);
    expect(reconnectDelayMs(3)).toBe(60_000);
    expect(reconnectDelayMs(RECONNECT_ATTEMPTS)).toBe(60_000);
  });
});

describe("enqueueReconnect", () => {
  it("opens a countdown for the first dropped pane", () => {
    expect(enqueueReconnect(null, A, 0, 1_000)).toEqual({
      paneIds: [A],
      dueAt: 31_000,
      delayMs: 30_000,
      attemptsLeft: RECONNECT_ATTEMPTS,
    });
  });

  it("uses the longer wait after the first failed attempt", () => {
    const q = enqueueReconnect(null, A, 1, 0);
    expect(q?.delayMs).toBe(60_000);
    expect(q?.attemptsLeft).toBe(RECONNECT_ATTEMPTS - 1);
  });

  it("gives up after the last attempt", () => {
    expect(enqueueReconnect(null, A, RECONNECT_ATTEMPTS, 0)).toBeNull();
    expect(enqueueReconnect(null, A, RECONNECT_ATTEMPTS + 1, 0)).toBeNull();
  });

  it("lets a second pane join the running countdown without resetting it", () => {
    const first = enqueueReconnect(null, A, 0, 1_000);
    const both = enqueueReconnect(first, B, 3, 20_000);
    expect(both).toEqual({
      paneIds: [A, B],
      dueAt: 31_000,
      delayMs: 30_000,
      attemptsLeft: RECONNECT_ATTEMPTS - 3,
    });
  });

  it("is idempotent for a pane already queued", () => {
    const q = enqueueReconnect(null, A, 0, 0);
    expect(enqueueReconnect(q, A, 0, 5_000)).toBe(q);
  });
});

describe("dequeueReconnect", () => {
  it("removes one pane and clears the queue when empty", () => {
    const q = enqueueReconnect(enqueueReconnect(null, A, 0, 0), B, 0, 0);
    const one = dequeueReconnect(q, A);
    expect(one?.paneIds).toEqual([B]);
    expect(dequeueReconnect(one, B)).toBeNull();
  });

  it("leaves unrelated queues untouched", () => {
    const q = enqueueReconnect(null, A, 0, 0);
    expect(dequeueReconnect(q, B)).toBe(q);
    expect(dequeueReconnect(null, A)).toBeNull();
  });
});

describe("disconnectedLabel", () => {
  it("matches the Termius singular / plural wording", () => {
    expect(disconnectedLabel(1)).toBe("1 session is disconnected.");
    expect(disconnectedLabel(2)).toBe("2 sessions are disconnected.");
  });
});
