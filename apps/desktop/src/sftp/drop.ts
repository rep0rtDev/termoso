// Files dropped from the OS arrive either with real paths (Tauri's native
// drag-drop handler on Linux/macOS, or `file://` URIs), which are uploaded
// straight from disk, or as blobs without paths (WebView2), which are copied
// into a private staging directory owned by Rust and uploaded from there; the
// staging copy is removed once the upload ends.

import * as ipc from "@/ipc/commands";
import type { FsEntry } from "@/ipc/types";
import { joinPath } from "./format";
import { setStaging } from "./store";

const CHUNK = 1 << 20;

export interface Staged {
  /** Staging directory to remove after the upload, if the files were copied. */
  dir: string | null;
  entries: FsEntry[];
}

/** Internal MIME type carried by drags that start inside a file pane. */
export const PANE_MIME = "application/x-termoso-entries";

/** Data attributes that mark pane drop zones for native (OS) drops. */
export const DROP_SIDE_ATTR = "data-drop-side";
export const DROP_DEST_ATTR = "data-drop-dest";

export interface DropTarget {
  side: string;
  dest: string;
}

/** Drop zone under a point in CSS pixels, if any. */
export function dropTargetAt(x: number, y: number): DropTarget | null {
  const zone = document.elementFromPoint(x, y)?.closest<HTMLElement>(`[${DROP_DEST_ATTR}]`);
  const side = zone?.getAttribute(DROP_SIDE_ATTR);
  const dest = zone?.getAttribute(DROP_DEST_ATTR);
  return side && dest ? { side, dest } : null;
}

export const statPaths = (paths: string[]) => Promise.all(paths.map((p) => ipc.localStat(p)));

const URI_LIST = "text/uri-list";

export const hasOsFiles = (dt: DataTransfer) => {
  const types = Array.from(dt.types);
  return types.includes("Files") || types.includes(URI_LIST);
};

/** Local paths of `file://` URIs carried by the drop, if any. */
export function droppedPaths(dt: DataTransfer): string[] {
  const out: string[] = [];
  for (const line of dt.getData(URI_LIST).split(/\r?\n/)) {
    const raw = line.trim();
    if (!raw || raw.startsWith("#")) continue;
    let url: URL;
    try {
      url = new URL(raw);
    } catch {
      continue;
    }
    if (url.protocol !== "file:" || (url.hostname && url.hostname !== "localhost")) continue;
    const path = decodeURIComponent(url.pathname);
    out.push(/^\/[A-Za-z]:/.test(path) ? path.slice(1).replace(/\//g, "\\") : path);
  }
  return out;
}

function fsEntry(path: string, name: string, kind: "dir" | "file", file: File | null): FsEntry {
  return {
    name,
    path,
    kind,
    size: file?.size ?? null,
    mode: null,
    uid: null,
    gid: null,
    user: null,
    group: null,
    mtime: file ? Math.floor(file.lastModified / 1000) : null,
    atime: null,
    link_target: null,
    target_kind: null,
  };
}

async function writeFile(dir: string, rel: string, file: File, tick: () => void) {
  if (file.size === 0) {
    await ipc.dropWrite(dir, rel, new Uint8Array(), false);
  }
  for (let off = 0; off < file.size; off += CHUNK) {
    const buf = await file.slice(off, Math.min(file.size, off + CHUNK)).arrayBuffer();
    await ipc.dropWrite(dir, rel, new Uint8Array(buf), off > 0);
  }
  tick();
}

const readAll = (reader: FileSystemDirectoryReader) =>
  new Promise<FileSystemEntry[]>((resolve, reject) => {
    const out: FileSystemEntry[] = [];
    const step = () =>
      reader.readEntries((batch) => {
        if (batch.length === 0) resolve(out);
        else {
          out.push(...batch);
          step();
        }
      }, reject);
    step();
  });

const fileOf = (entry: FileSystemFileEntry) =>
  new Promise<File>((resolve, reject) => entry.file(resolve, reject));

async function stageEntry(dir: string, rel: string, entry: FileSystemEntry, tick: () => void) {
  if (entry.isDirectory) {
    await ipc.dropMkdir(dir, rel);
    for (const child of await readAll((entry as FileSystemDirectoryEntry).createReader())) {
      await stageEntry(dir, `${rel}/${child.name}`, child, tick);
    }
  } else if (entry.isFile) {
    await writeFile(dir, rel, await fileOf(entry as FileSystemFileEntry), tick);
  }
}

/**
 * Copy everything in the drop into a fresh staging directory. Resolves with
 * the top-level items as pane entries; the caller uploads them with
 * `temp: true` or aborts the staging directory.
 */
export async function stageDrop(dt: DataTransfer): Promise<Staged | null> {
  const paths = droppedPaths(dt);
  if (paths.length > 0) return { dir: null, entries: await statPaths(paths) };

  const items = Array.from(dt.items);
  const tree = items
    .filter((i) => i.kind === "file")
    .map((i) => i.webkitGetAsEntry())
    .filter((e): e is FileSystemEntry => e !== null);
  const flat = tree.length === 0 ? Array.from(dt.files) : [];
  if (tree.length === 0 && flat.length === 0) return null;

  const dir = await ipc.dropBegin();
  let count = 0;
  const tick = () => setStaging(++count);
  setStaging(0);
  try {
    const entries: FsEntry[] = [];
    for (const e of tree) {
      await stageEntry(dir, e.name, e, tick);
      entries.push(fsEntry(joinPath(dir, e.name), e.name, e.isDirectory ? "dir" : "file", null));
    }
    for (const f of flat) {
      await writeFile(dir, f.name, f, tick);
      entries.push(fsEntry(joinPath(dir, f.name), f.name, "file", f));
    }
    return { dir, entries };
  } catch (e) {
    await ipc.dropAbort(dir).catch(() => undefined);
    throw e;
  } finally {
    setStaging(null);
  }
}
