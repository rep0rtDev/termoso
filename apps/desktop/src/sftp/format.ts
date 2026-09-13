import type { FsEntry } from "@/ipc/types";

export function formatSize(n: number | null | undefined): string {
  if (n === null || n === undefined) return "";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export const formatSpeed = (bytesPerSec: number) => `${formatSize(Math.round(bytesPerSec))}/s`;

/** `0:42`, `3:05`, `1:12:09`. */
export function formatDuration(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  return `${h > 0 ? `${h}:` : ""}${mm}:${String(r).padStart(2, "0")}`;
}

export function formatMtime(secs: number | null): string {
  if (secs === null) return "";
  const d = new Date(secs * 1000);
  const now = new Date();
  const sameYear = d.getFullYear() === now.getFullYear();
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    year: sameYear ? undefined : "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatMode(mode: number | null, kind: FsEntry["kind"]): string {
  if (mode === null) return "";
  const t = kind === "dir" ? "d" : kind === "symlink" ? "l" : "-";
  const bits = ["r", "w", "x"];
  let out = t;
  for (let i = 0; i < 9; i++) {
    out += mode & (1 << (8 - i)) ? (bits[i % 3] ?? "-") : "-";
  }
  return out;
}

export function joinPath(dir: string, name: string): string {
  if (dir.endsWith("/") || dir.endsWith("\\")) return dir + name;
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  return `${dir}${sep}${name}`;
}

export function baseName(path: string): string {
  const trimmed = path.replace(/[/\\]+$/, "");
  const i = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  return i >= 0 ? trimmed.slice(i + 1) : trimmed;
}

export const isHidden = (e: FsEntry) => e.name.startsWith(".");

/** A directory, or a symlink that resolves to one. */
export const isDirLike = (e: FsEntry) =>
  e.kind === "dir" || (e.kind === "symlink" && e.target_kind === "dir");

/** Symlink whose target is missing (or unreadable). */
export const isBrokenLink = (e: FsEntry) => e.kind === "symlink" && e.target_kind === null;

/** Lower-case extension without the dot; `""` when there is none. */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** The "Kind" column: `folder`, `link`, the extension (`txt`) or `file`. */
export function kindLabel(e: FsEntry): string {
  if (e.kind === "dir") return "folder";
  if (e.kind === "symlink") return "link";
  if (e.kind === "other") return "special";
  return extensionOf(e.name) || "file";
}

export type SortKey = "name" | "mtime" | "size" | "kind";
export interface Sort {
  key: SortKey;
  dir: "asc" | "desc";
}

const order = (x: string, y: string) => (x < y ? -1 : x > y ? 1 : 0);

/** Case-insensitive (the webview may lack ICU, so no `localeCompare`). */
const byName = (a: FsEntry, b: FsEntry) =>
  order(a.name.toLowerCase(), b.name.toLowerCase()) || order(a.name, b.name);

/** Folders first, then by the chosen column; ties fall back to the name. */
export function sortEntries(
  entries: FsEntry[],
  sort: Sort = { key: "name", dir: "asc" },
): FsEntry[] {
  const sign = sort.dir === "asc" ? 1 : -1;
  const cmp = (a: FsEntry, b: FsEntry): number => {
    switch (sort.key) {
      case "name":
        return byName(a, b);
      case "mtime":
        return (a.mtime ?? 0) - (b.mtime ?? 0);
      case "size":
        return (a.size ?? 0) - (b.size ?? 0);
      case "kind":
        return order(kindLabel(a), kindLabel(b));
    }
  };
  return [...entries].sort((a, b) => {
    const ad = a.kind === "dir" ? 0 : 1;
    const bd = b.kind === "dir" ? 0 : 1;
    if (ad !== bd) return ad - bd;
    return sign * cmp(a, b) || byName(a, b);
  });
}
