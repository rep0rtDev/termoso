import { describe, expect, it } from "vitest";
import type { LayoutTemplate } from "@/ipc/types";
import { leafState, restoreKeystrokes, shellQuote, withLeafState } from "./restore";

const target = { kind: "local" as const };
const leaf = (extra: Partial<Extract<LayoutTemplate, { kind: "leaf" }>> = {}) =>
  ({ kind: "leaf", target, ...extra }) as Extract<LayoutTemplate, { kind: "leaf" }>;

describe("leafState", () => {
  it("reads v1 leaves (no shell state) as nothing to restore", () => {
    expect(leafState(leaf())).toBeNull();
    expect(leafState(leaf({ cwd: null, command: null }))).toBeNull();
    expect(leafState(leaf({ cwd: "", command: "" }))).toBeNull();
  });

  it("keeps whichever of cwd / command was saved", () => {
    expect(leafState(leaf({ cwd: "/srv" }))).toEqual({ cwd: "/srv", command: null });
    expect(leafState(leaf({ command: "htop" }))).toEqual({ cwd: null, command: "htop" });
    expect(leafState(leaf({ cwd: "/srv", command: "htop" }))).toEqual({
      cwd: "/srv",
      command: "htop",
    });
  });
});

describe("withLeafState", () => {
  it("omits empty fields so the JSON stays in the v1 shape", () => {
    expect(withLeafState(leaf(), { cwd: null, command: null })).toEqual({ kind: "leaf", target });
    expect(JSON.stringify(withLeafState(leaf(), { cwd: "", command: null }))).not.toContain("cwd");
  });

  it("writes the fields it was given and drops stale ones from the source leaf", () => {
    const out = withLeafState(leaf({ cwd: "/old", command: "old" }), {
      cwd: "/new",
      command: null,
    });
    expect(out).toEqual({ kind: "leaf", target, cwd: "/new" });
  });
});

describe("shellQuote", () => {
  it("single-quotes and escapes embedded quotes", () => {
    expect(shellQuote("/tmp/a b")).toBe("'/tmp/a b'");
    expect(shellQuote("/tmp/$HOME/*")).toBe("'/tmp/$HOME/*'");
    expect(shellQuote("/tmp/it's")).toBe(String.raw`'/tmp/it'\''s'`);
  });
});

describe("restoreKeystrokes", () => {
  const state = { cwd: "/srv/app", command: "tail -f log" };

  it("changes directory with a history-skipping leading space", () => {
    expect(restoreKeystrokes({ cwd: "/srv/app", command: null }, "run")).toBe(
      " cd -- '/srv/app'\r",
    );
  });

  it("types the command without running it by default", () => {
    expect(restoreKeystrokes(state, "type")).toBe(" cd -- '/srv/app'\rtail -f log");
  });

  it("runs the command when asked to", () => {
    expect(restoreKeystrokes(state, "run")).toBe(" cd -- '/srv/app'\rtail -f log\r");
  });

  it("drops the command in `never` mode and sends nothing for an empty state", () => {
    expect(restoreKeystrokes(state, "never")).toBe(" cd -- '/srv/app'\r");
    expect(restoreKeystrokes({ cwd: null, command: null }, "run")).toBe("");
    expect(restoreKeystrokes({ cwd: null, command: "htop" }, "type")).toBe("htop");
  });
});
