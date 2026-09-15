import { describe, expect, it } from "vitest";
import type { LocalVault, LogAuthor, LogCard } from "@/ipc/types";
import { authorName, authorsOf, recordingState, visibleLogs } from "./team";

const ann: LogAuthor = { userId: "u-ann", email: "ann@x.io", displayName: "Ann", avatar: "t1" };
const bob: LogAuthor = { userId: "u-bob", email: "bob@x.io", displayName: null, avatar: null };

function log(id: string, vaultId: string, author: LogAuthor | null, mine = false): LogCard {
  return {
    id,
    vaultId,
    hostId: null,
    label: id,
    target: "h",
    protocol: "ssh",
    startedAt: "2026-01-01T00:00:00Z",
    endedAt: null,
    durationSecs: 1,
    cols: 80,
    rows: 24,
    sizeBytes: 1,
    cached: true,
    uploaded: true,
    completed: true,
    createdAt: "2026-01-01T00:00:00Z",
    bookmarks: 0,
    mine,
    team: author !== null,
    author,
    pinned: false,
    note: "",
    noteBy: null,
    canAnnotate: true,
    canDelete: mine,
  };
}

function vault(kind: LocalVault["kind"], sessionLogging: boolean): LocalVault {
  return {
    id: "v",
    kind,
    name: "V",
    team_id: kind === "team" ? "t" : null,
    role: "editor",
    unlocked: true,
    key_version: 1,
    cursor: 0,
    session_logging: sessionLogging,
    logs_cursor: 0,
  };
}

describe("authorsOf", () => {
  it("lists each author once, most recordings first, skipping local recordings", () => {
    const list = [log("1", "v", bob), log("2", "v", ann), log("3", "v", bob), log("4", "v", null)];
    expect(authorsOf(list).map((a) => a.userId)).toEqual(["u-bob", "u-ann"]);
  });

  it("names people by display name, falling back to e-mail", () => {
    expect(authorName(ann)).toBe("Ann");
    expect(authorName(bob)).toBe("bob@x.io");
    expect(authorName({ ...ann, displayName: "  " })).toBe("ann@x.io");
  });
});

describe("visibleLogs", () => {
  const list = [log("1", "a", ann), log("2", "b", bob), log("3", "a", bob), log("4", "a", null)];

  it("scopes to the active vault and then to one author", () => {
    expect(visibleLogs(list, "a", null).map((l) => l.id)).toEqual(["1", "3", "4"]);
    expect(visibleLogs(list, "a", "u-bob").map((l) => l.id)).toEqual(["3"]);
    expect(visibleLogs(list, null, null)).toHaveLength(4);
  });

  it("hides local recordings when an author is picked", () => {
    expect(visibleLogs(list, "a", "u-ann").map((l) => l.id)).toEqual(["1"]);
  });
});

describe("recordingState", () => {
  it("follows the user's switch outside team policy", () => {
    expect(recordingState(null, true)).toBe("mine");
    expect(recordingState(vault("local", false), false)).toBe("off");
    expect(recordingState(vault("personal", false), true)).toBe("mine");
  });

  it("records for the team when the vault manager turned logging on, whatever the switch", () => {
    expect(recordingState(vault("team", true), false)).toBe("team");
    expect(recordingState(vault("team", true), true)).toBe("team");
    expect(recordingState(vault("team", false), false)).toBe("off");
    expect(recordingState(vault("team", false), true)).toBe("mine");
  });

  it("ignores session_logging on non-team vaults", () => {
    expect(recordingState(vault("personal", true), false)).toBe("off");
  });
});
