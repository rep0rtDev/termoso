import { describe, expect, it } from "vitest";
import type { PresenceEntry, TeamPresence } from "@/ipc/types";
import { connectedFor, distinctPeople, viewersByHost, viewersSummary } from "./presence";

const HOST_A = "aaaaaaaa-0000-4000-8000-000000000001";
const HOST_B = "aaaaaaaa-0000-4000-8000-000000000002";
const VAULT = "bbbbbbbb-0000-4000-8000-000000000001";
const ME = "cccccccc-0000-4000-8000-000000000001";
const BOB = "cccccccc-0000-4000-8000-000000000002";

function entry(p: Partial<PresenceEntry> & { user_id: string; device_id: string }): PresenceEntry {
  return {
    email: `${p.user_id.slice(-1)}@example.com`,
    display_name: null,
    device_name: "dev",
    platform: "linux",
    sessions: [],
    seen_at: "2026-01-01T10:00:00Z",
    ...p,
  };
}

const presence: TeamPresence = {
  enabled: true,
  entries: [
    entry({
      user_id: BOB,
      device_id: "d1",
      display_name: "Bob",
      platform: "android",
      sessions: [
        { vault_id: VAULT, host_id: HOST_A, protocol: "sftp", since: "2026-01-01T09:30:00Z" },
        { vault_id: VAULT, host_id: HOST_A, protocol: "ssh", since: "2026-01-01T09:20:00Z" },
        { vault_id: VAULT, host_id: HOST_A, protocol: "ssh", since: "2026-01-01T09:40:00Z" },
        { vault_id: VAULT, host_id: HOST_B, protocol: "forward", since: "2026-01-01T09:00:00Z" },
      ],
    }),
    entry({
      user_id: BOB,
      device_id: "d2",
      display_name: "Bob",
      sessions: [
        { vault_id: VAULT, host_id: HOST_A, protocol: "ssh", since: "2026-01-01T09:50:00Z" },
      ],
    }),
    entry({
      user_id: ME,
      device_id: "d3",
      sessions: [
        { vault_id: VAULT, host_id: HOST_A, protocol: "mosh", since: "2026-01-01T09:10:00Z" },
      ],
    }),
  ],
};

describe("viewersByHost", () => {
  it("groups devices per host, oldest connection first, with distinct protocols", () => {
    const m = viewersByHost(presence, ME);
    const a = m.get(HOST_A) ?? [];
    expect(a.map((v) => v.deviceId)).toEqual(["d3", "d1", "d2"]);
    expect(a[0]?.me).toBe(true);
    expect(a[1]).toMatchObject({
      userId: BOB,
      displayName: "Bob",
      platform: "android",
      protocols: ["ssh", "sftp"],
      since: "2026-01-01T09:20:00Z",
      me: false,
    });
    expect(m.get(HOST_B)?.map((v) => v.protocols)).toEqual([["forward"]]);
  });

  it("is empty when the team has presence off or nothing loaded", () => {
    expect(viewersByHost({ ...presence, enabled: false }, ME).size).toBe(0);
    expect(viewersByHost(undefined, ME).size).toBe(0);
  });

  it("collapses devices into people for counts and summaries", () => {
    const a = viewersByHost(presence, ME).get(HOST_A) ?? [];
    expect(distinctPeople(a).map((v) => v.userId)).toEqual([ME, BOB]);
    expect(viewersSummary(a)).toBe("You and Bob");
    expect(viewersSummary(a.slice(1))).toBe("Bob");
    const bob = a[1];
    if (!bob) throw new Error("expected Bob");
    const many = [
      ...a,
      { ...bob, userId: "x1", displayName: "Cy" },
      { ...bob, userId: "x2", displayName: null, email: "dee@example.com" },
    ];
    expect(viewersSummary(many)).toBe("You, Bob and 2 others");
  });
});

describe("connectedFor", () => {
  const now = Date.parse("2026-01-01T12:00:00Z");
  it("formats durations", () => {
    expect(connectedFor("2026-01-01T11:59:30Z", now)).toBe("just now");
    expect(connectedFor("2026-01-01T11:45:00Z", now)).toBe("15 min");
    expect(connectedFor("2026-01-01T10:55:00Z", now)).toBe("1 h 05 min");
    expect(connectedFor("2025-12-30T09:00:00Z", now)).toBe("2 d 3 h");
    expect(connectedFor("nope", now)).toBe("");
  });
});
