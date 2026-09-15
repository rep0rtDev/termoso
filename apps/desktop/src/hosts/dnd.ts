import { type DragEvent, useCallback, useRef, useState } from "react";
import type { DragHandlers } from "@/components/ui";
import type { GroupNode, HostCard, Uuid } from "@/ipc/types";

/** Private MIME type so foreign drags (files, text) never look like hosts. */
export const MIME = "application/x-termoso-hosts";

export const isHostDrag = (e: DragEvent<HTMLElement>) =>
  Array.from(e.dataTransfer.types).includes(MIME);

/** Host drag payload: the ids being moved, plus their groups so no-op drops can be skipped. */
export interface HostDragData {
  ids: Uuid[];
  fromGroups: (Uuid | null)[];
}

const isId = (v: unknown): v is Uuid => typeof v === "string" && v.length > 0;

/** Parse a `dataTransfer` payload; anything malformed (or foreign) yields `null`. */
export function parseDragData(raw: string): HostDragData | null {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof value !== "object" || value === null) return null;
  const { ids, fromGroups } = value as { ids?: unknown; fromGroups?: unknown };
  if (!Array.isArray(ids) || !ids.every(isId) || ids.length === 0) return null;
  const isGroup = (g: unknown): g is Uuid | null => g === null || isId(g);
  if (!Array.isArray(fromGroups) || !fromGroups.every(isGroup)) return null;
  if (fromGroups.length !== ids.length) return null;
  const keep = ids.map((id, i) => ids.indexOf(id) === i);
  return {
    ids: ids.filter((_, i) => keep[i]),
    fromGroups: fromGroups.filter((_, i) => keep[i]),
  };
}

/** Ids carried by a host drop (any drop target: groups, breadcrumbs, terminal tabs). */
export function droppedHostIds(e: DragEvent<HTMLElement>): Uuid[] {
  if (!isHostDrag(e)) return [];
  return parseDragData(e.dataTransfer.getData(MIME))?.ids ?? [];
}

/** Which of the dragged hosts actually change group when dropped on `target`. */
export const movedIds = (data: HostDragData, target: Uuid | null) =>
  data.ids.filter((_, i) => data.fromGroups[i] !== target);

export interface HostDnd {
  dragHost: (h: HostCard) => DragHandlers;
  dropGroup: (g: GroupNode) => DragHandlers;
  /** Drop target for a group (or the root when `null`) rendered outside the grid — breadcrumbs. */
  dropInto: (groupId: Uuid | null) => DragHandlers;
  /** Group id currently hovered by a host drag (`"root"` for the top level). */
  dropping: string | null;
  /** Ids in flight; used to dim the source cards. */
  dragging: ReadonlySet<Uuid>;
}

const rootKey = (id: Uuid | null) => id ?? "root";

/**
 * Drag hosts onto groups. `selection` is the current multi-selection: dragging a
 * checked host moves the whole selection, dragging an unchecked one moves just it.
 */
export function useHostDnd({
  selection,
  hosts,
  onMove,
}: {
  selection: ReadonlySet<Uuid>;
  hosts: readonly HostCard[];
  onMove: (ids: Uuid[], groupId: Uuid | null) => void;
}): HostDnd {
  const [dropping, setDropping] = useState<string | null>(null);
  const [dragging, setDragging] = useState<ReadonlySet<Uuid>>(() => new Set());
  const payload = useRef<HostDragData | null>(null);

  const dragHost = useCallback(
    (h: HostCard): DragHandlers => ({
      draggable: true,
      onDragStart: (e) => {
        const ids = selection.has(h.id) ? Array.from(selection) : [h.id];
        const data: HostDragData = {
          ids,
          fromGroups: ids.map((id) => hosts.find((x) => x.id === id)?.groupId ?? null),
        };
        payload.current = data;
        e.dataTransfer.setData(MIME, JSON.stringify(data));
        e.dataTransfer.effectAllowed = "move";
        setDragging(new Set(ids));
      },
      onDragEnd: () => {
        payload.current = null;
        setDragging(new Set());
        setDropping(null);
      },
    }),
    [selection, hosts],
  );

  const dropInto = useCallback(
    (groupId: Uuid | null): DragHandlers => {
      const key = rootKey(groupId);
      return {
        onDragEnter: (e) => {
          if (!isHostDrag(e)) return;
          e.preventDefault();
          setDropping(key);
        },
        onDragOver: (e) => {
          if (!isHostDrag(e)) return;
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
          if (dropping !== key) setDropping(key);
        },
        onDragLeave: (e) => {
          if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
          setDropping((d) => (d === key ? null : d));
        },
        onDrop: (e) => {
          if (!isHostDrag(e)) return;
          e.preventDefault();
          setDropping(null);
          const raw = e.dataTransfer.getData(MIME);
          const data = (raw ? parseDragData(raw) : null) ?? payload.current;
          payload.current = null;
          setDragging(new Set());
          if (!data) return;
          const ids = movedIds(data, groupId);
          if (ids.length > 0) onMove(ids, groupId);
        },
      };
    },
    [dropping, onMove],
  );

  const dropGroup = useCallback((g: GroupNode) => dropInto(g.id), [dropInto]);

  return { dragHost, dropGroup, dropInto, dropping, dragging };
}
