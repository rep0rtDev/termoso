import { describe, expect, it } from "vitest";
import type { LocalVault } from "@/ipc/types";
import {
  type NotifySettings,
  commandLabel,
  commandLongEnough,
  commandNotice,
  droppedNotice,
  newParticipants,
  newSharedVaults,
  revokedByServer,
  transferNotice,
  wantsNotice,
} from "./notices";

const ALL: NotifySettings = {
  notifications: true,
  notifyCommands: true,
  notifyCommandSeconds: 5,
  notifyTransfers: true,
  notifySessions: true,
  notifyAccount: true,
};

const vault = (id: string, role: LocalVault["role"], kind: LocalVault["kind"] = "team") =>
  ({
    id,
    kind,
    role,
    name: id,
    team_id: null,
    unlocked: true,
    key_version: 1,
    cursor: 0,
  }) as LocalVault;

describe("wantsNotice", () => {
  it("master switch wins over every kind", () => {
    const off = { ...ALL, notifications: false };
    for (const k of ["commands", "transfers", "sessions", "account"] as const) {
      expect(wantsNotice(off, k)).toBe(false);
      expect(wantsNotice(ALL, k)).toBe(true);
    }
    expect(wantsNotice(null, "commands")).toBe(false);
  });

  it("per-kind switches are independent", () => {
    const s = { ...ALL, notifyCommands: false, notifyAccount: false };
    expect(wantsNotice(s, "commands")).toBe(false);
    expect(wantsNotice(s, "transfers")).toBe(true);
    expect(wantsNotice(s, "sessions")).toBe(true);
    expect(wantsNotice(s, "account")).toBe(false);
  });
});

describe("commandLongEnough", () => {
  it("respects the minimum duration and unknown starts", () => {
    expect(commandLongEnough(ALL, 1_000, 5_999)).toBe(false);
    expect(commandLongEnough(ALL, 1_000, 6_000)).toBe(true);
    expect(commandLongEnough(ALL, undefined, 6_000)).toBe(false);
    expect(commandLongEnough({ ...ALL, notifyCommandSeconds: 0 }, 1_000, 1_000)).toBe(true);
  });
});

describe("commandNotice", () => {
  it("keeps only the program name, never the arguments", () => {
    expect(commandLabel("mysql -u root -pS3cret db")).toBe("mysql");
    expect(commandLabel("  /usr/local/bin/restic backup --password-file x ")).toBe("restic");
    expect(commandLabel("")).toBeNull();
    expect(commandLabel(null)).toBeNull();
    expect(commandLabel(`${"a".repeat(40)} x`)).toBe(`${"a".repeat(31)}…`);
  });

  it("reports success and failure without echoing the command line", () => {
    const ok = commandNotice(
      { title: "web-1", command: "curl -H 'Authorization: Bearer t0k' u" },
      0,
    );
    expect(ok.title).toBe("Command finished");
    expect(ok.body).toBe("curl · web-1");
    expect(ok.body).not.toContain("t0k");
    const bad = commandNotice({ title: "web-1", command: "make" }, 2);
    expect(bad.title).toBe("Command failed (exit 2)");
    expect(commandNotice({ title: "web-1", command: null }, null).body).toBe("web-1");
  });
});

describe("transferNotice", () => {
  it("names the file, not its full path, and reports failures", () => {
    const up = transferNotice({
      direction: "upload",
      status: "done",
      local: "/home/me/secret-project/dump.sql",
      remote: "/srv/dump.sql",
    });
    expect(up).toEqual({ title: "Upload finished", body: "dump.sql" });
    const down = transferNotice({
      direction: "download",
      status: "done",
      local: "C:\\Users\\me\\logs\\",
      remote: "/var/log/app/",
    });
    expect(down).toEqual({ title: "Download finished", body: "app" });
    expect(
      transferNotice({ direction: "download", status: "failed", local: "/x", remote: "/y/z.bin" }),
    ).toEqual({ title: "Transfer failed", body: "z.bin" });
  });
});

describe("droppedNotice", () => {
  it("names the pane and nothing else", () => {
    expect(droppedNotice({ title: "db" })).toEqual({ title: "Connection lost", body: "db" });
  });
});

describe("newSharedVaults", () => {
  it("reports only team vaults that appeared and that this user does not manage", () => {
    const known = new Set(["t1"]);
    const out = newSharedVaults(known, [
      vault("t1", "viewer"),
      vault("t2", "editor"),
      vault("t3", "manager"),
      vault("p1", "manager", "personal"),
    ]);
    expect(out.map((v) => v.id)).toEqual(["t2"]);
  });
});

describe("newParticipants", () => {
  const me = { userId: "me", isMe: true };
  const ann = { userId: "ann", isMe: false };
  const bob = { userId: "bob", isMe: false };

  it("is quiet on the first observation and for the host themself", () => {
    expect(newParticipants(undefined, [me, ann])).toEqual([]);
    expect(newParticipants(new Set(["me"]), [me])).toEqual([]);
  });

  it("reports people that were not there before", () => {
    expect(newParticipants(new Set(["me", "ann"]), [me, ann, bob])).toEqual([bob]);
    expect(newParticipants(new Set(["me", "ann", "bob"]), [me, bob])).toEqual([]);
  });
});

describe("revokedByServer", () => {
  it("tells a server revocation from the user's own sign-out", () => {
    expect(revokedByServer({ state: "error", lastError: "signed out by the server" })).toBe(true);
    expect(revokedByServer({ state: "idle", lastError: null })).toBe(false);
    expect(revokedByServer({ state: "error", lastError: null })).toBe(false);
  });
});
