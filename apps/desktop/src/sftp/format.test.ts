import { describe, expect, it } from "vitest";
import type { FsEntry } from "@/ipc/types";
import { isBrokenLink, isDirLike, kindLabel, sortEntries } from "./format";

const entry = (name: string, patch: Partial<FsEntry> = {}): FsEntry => ({
  name,
  path: `/x/${name}`,
  kind: "file",
  size: 0,
  mode: null,
  uid: null,
  gid: null,
  user: null,
  group: null,
  mtime: null,
  atime: null,
  link_target: null,
  target_kind: null,
  ...patch,
});

const names = (xs: FsEntry[]) => xs.map((e) => e.name);

describe("symlink classification", () => {
  it("treats only directory targets as folders", () => {
    expect(isDirLike(entry("d", { kind: "dir" }))).toBe(true);
    expect(isDirLike(entry("l", { kind: "symlink", link_target: "d", target_kind: "dir" }))).toBe(
      true,
    );
    expect(isDirLike(entry("l", { kind: "symlink", link_target: "f", target_kind: "file" }))).toBe(
      false,
    );
    expect(isDirLike(entry("l", { kind: "symlink", link_target: "gone" }))).toBe(false);
  });

  it("flags dangling links", () => {
    expect(isBrokenLink(entry("l", { kind: "symlink", link_target: "gone" }))).toBe(true);
    expect(isBrokenLink(entry("l", { kind: "symlink", target_kind: "file" }))).toBe(false);
    expect(isBrokenLink(entry("f"))).toBe(false);
  });
});

describe("kindLabel", () => {
  it("derives the kind from type and extension", () => {
    expect(kindLabel(entry("d", { kind: "dir" }))).toBe("folder");
    expect(kindLabel(entry("l", { kind: "symlink" }))).toBe("link");
    expect(kindLabel(entry("notes.TXT"))).toBe("txt");
    expect(kindLabel(entry("Makefile"))).toBe("file");
    expect(kindLabel(entry(".bashrc"))).toBe("file");
  });
});

describe("sortEntries", () => {
  const list = [
    entry("zeta.txt", { size: 10, mtime: 300 }),
    entry("Alpha.md", { size: 30, mtime: 100 }),
    entry("beta", { kind: "dir", mtime: 200 }),
    entry("link", { kind: "symlink", link_target: "beta", target_kind: "dir", mtime: 50 }),
    entry("mid.bin", { size: 20, mtime: 200 }),
  ];

  it("keeps folders first (links sort with files), case-insensitive by name", () => {
    expect(names(sortEntries(list))).toEqual(["beta", "Alpha.md", "link", "mid.bin", "zeta.txt"]);
    expect(names(sortEntries(list, { key: "name", dir: "desc" }))).toEqual([
      "beta",
      "zeta.txt",
      "mid.bin",
      "link",
      "Alpha.md",
    ]);
  });

  it("sorts by size and date within the file group", () => {
    expect(names(sortEntries(list, { key: "size", dir: "desc" }))).toEqual([
      "beta",
      "Alpha.md",
      "mid.bin",
      "zeta.txt",
      "link",
    ]);
    expect(names(sortEntries(list, { key: "mtime", dir: "asc" }))).toEqual([
      "beta",
      "link",
      "Alpha.md",
      "mid.bin",
      "zeta.txt",
    ]);
  });

  it("sorts by kind with the name as a tie-breaker", () => {
    expect(names(sortEntries(list, { key: "kind", dir: "asc" }))).toEqual([
      "beta",
      "mid.bin",
      "link",
      "Alpha.md",
      "zeta.txt",
    ]);
  });
});
