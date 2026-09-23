import { describe, expect, it } from "vitest";
import type { LocalVault, VaultConnection } from "@/ipc/types";
import { inVault, scopedTo } from "./scope";

const LOCAL = "bbbbbbbb-0000-4000-8000-000000000001";
const PERSONAL = "bbbbbbbb-0000-4000-8000-000000000002";
const TEAM = "bbbbbbbb-0000-4000-8000-000000000003";

const vault = (id: string, kind: LocalVault["kind"]): Pick<LocalVault, "id" | "kind"> => ({
  id,
  kind,
});
const local = vault(LOCAL, "local");
const personal = vault(PERSONAL, "personal");
const team = vault(TEAM, "team");

let n = 0;
function conn(vault_id: string | null, host_id: string | null = vault_id): VaultConnection {
  n += 1;
  return {
    id: `cccccccc-0000-4000-8000-${String(n).padStart(12, "0")}`,
    created_at: new Date(2026, 0, n).toISOString(),
    vault_id,
    data: { host_id, label: "h", target: "t", protocol: "ssh", duration_secs: 1, error: null },
  };
}

describe("history scope", () => {
  it("shows a saved host only in the vault it lives in", () => {
    const item = conn(PERSONAL);
    expect(inVault(item, personal)).toBe(true);
    expect(inVault(item, local)).toBe(false);
    expect(inVault(item, team)).toBe(false);
  });

  it("keeps quick connects, local shells and deleted hosts in the local vault", () => {
    for (const item of [conn(null, null), conn(null, "aaaaaaaa-0000-4000-8000-000000000009")]) {
      expect(inVault(item, local)).toBe(true);
      expect(inVault(item, personal)).toBe(false);
      expect(inVault(item, team)).toBe(false);
    }
  });

  it("belongs nowhere without an active vault", () => {
    expect(inVault(conn(PERSONAL), null)).toBe(false);
    expect(inVault(conn(null), null)).toBe(false);
  });

  it("filters a mixed list per vault, preserving order", () => {
    const items = [conn(PERSONAL), conn(null), conn(TEAM), conn(PERSONAL), conn(LOCAL)];
    expect(scopedTo(items, personal)).toEqual([items[0], items[3]]);
    expect(scopedTo(items, team)).toEqual([items[2]]);
    expect(scopedTo(items, local)).toEqual([items[1], items[4]]);
    expect(scopedTo(items, null)).toEqual([]);
  });
});
