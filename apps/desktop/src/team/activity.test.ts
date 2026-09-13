import { describe, expect, it } from "vitest";
import type { AuditEvent } from "@/ipc/types";
import { describeEvent, formatWhen } from "./activity";

const ctx = {
  vaultNames: new Map([["v1", "Staging"]]),
  people: new Map([["u2", "Bob"]]),
};

const base = (over: Partial<AuditEvent>): AuditEvent => ({
  id: 1,
  team_id: "t",
  actor_id: "u1",
  actor_email: "alice@example.com",
  actor_name: "Alice",
  action: "team.created",
  details: {},
  created_at: "2026-09-12T10:00:00Z",
  ...over,
});

describe("describeEvent", () => {
  it("names the actor and the vault", () => {
    const l = describeEvent(
      base({
        action: "vault.access_granted",
        vault_id: "v1",
        target_user: "u2",
        details: { role: "editor" },
      }),
      ctx,
    );
    expect(l.actor).toBe("Alice");
    expect(l.text).toBe("gave Bob access (can edit)");
    expect(l.vault).toBe("Staging");
  });

  it("prefers the server-joined target email over the local lookup", () => {
    const l = describeEvent(
      base({
        action: "member.role",
        target_user: "u9",
        target_email: "carol@example.com",
        details: { role: "admin", previous_role: "member" },
      }),
      ctx,
    );
    expect(l.text).toBe("changed carol@example.com's role to Admin");
    expect(l.meta).toBe("was Member");
  });

  it("pluralises grouped entity changes", () => {
    expect(
      describeEvent(
        base({ action: "entity.created", vault_id: "v1", details: { kind: "pf_rule", count: 3 } }),
        ctx,
      ).text,
    ).toBe("added 3 port forwarding rules");
    expect(
      describeEvent(
        base({ action: "entity.deleted", vault_id: "v1", details: { kind: "ssh_key", count: 1 } }),
        ctx,
      ).text,
    ).toBe("removed an SSH key");
  });

  it("survives a deleted actor and unknown actions", () => {
    const l = describeEvent(
      base({
        actor_id: undefined,
        actor_email: undefined,
        actor_name: undefined,
        action: "something.new_thing",
      }),
      ctx,
    );
    expect(l.actor).toBe("Someone");
    expect(l.text).toBe("something: new thing");
    expect(l.vault).toBeNull();
  });

  it("never echoes unknown detail keys", () => {
    const l = describeEvent(
      base({ action: "vault.renamed", details: { name: "Ops", secret: "hunter2" } }),
      ctx,
    );
    expect(JSON.stringify(l)).not.toContain("hunter2");
  });
});

describe("formatWhen", () => {
  it("shows only the time for today", () => {
    const now = new Date(2026, 8, 12, 15, 0);
    const iso = new Date(2026, 8, 12, 9, 5).toISOString();
    expect(formatWhen(iso, now)).not.toMatch(/Sep/);
  });
  it("adds the date for other days and the year for other years", () => {
    const now = new Date(2026, 8, 12, 15, 0);
    expect(formatWhen(new Date(2026, 7, 1, 9, 5).toISOString(), now)).toMatch(/Aug/);
    expect(formatWhen(new Date(2025, 7, 1, 9, 5).toISOString(), now)).toMatch(/2025/);
    expect(formatWhen("garbage", now)).toBe("");
  });
});
