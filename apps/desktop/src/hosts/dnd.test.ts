import { describe, expect, it } from "vitest";
import type { DragEvent } from "react";
import { MIME, droppedHostIds, isHostDrag, movedIds, parseDragData } from "./dnd";

const A = "3f2b7c9e-1d4a-4e8b-9c6f-0a1b2c3d4e5f";
const B = "9d8c7b6a-5f4e-4d3c-8b2a-1f0e9d8c7b6a";
const G = "11111111-2222-4333-8444-555555555555";

describe("parseDragData", () => {
  it("accepts a well-formed payload and drops duplicate ids", () => {
    expect(parseDragData(JSON.stringify({ ids: [A, B, A], fromGroups: [G, null, G] }))).toEqual({
      ids: [A, B],
      fromGroups: [G, null],
    });
  });

  it("rejects foreign or malformed payloads", () => {
    expect(parseDragData("not json")).toBeNull();
    expect(parseDragData("null")).toBeNull();
    expect(parseDragData(JSON.stringify({ ids: [] }))).toBeNull();
    expect(parseDragData(JSON.stringify({ ids: [A], fromGroups: [] }))).toBeNull();
    expect(parseDragData(JSON.stringify({ ids: [42], fromGroups: [null] }))).toBeNull();
    expect(parseDragData(JSON.stringify({ ids: [A], fromGroups: [7] }))).toBeNull();
  });
});

describe("droppedHostIds", () => {
  const drop = (types: string[], data: string) =>
    ({
      dataTransfer: { types, getData: (t: string) => (t === MIME ? data : "") },
    }) as unknown as DragEvent<HTMLElement>;

  it("yields the dragged ids for a host drop and nothing for other drags", () => {
    const ev = drop([MIME], JSON.stringify({ ids: [A, B], fromGroups: [G, null] }));
    expect(isHostDrag(ev)).toBe(true);
    expect(droppedHostIds(ev)).toEqual([A, B]);
    const tab = drop(["application/x-termoso-tab"], "tab-id");
    expect(isHostDrag(tab)).toBe(false);
    expect(droppedHostIds(tab)).toEqual([]);
    expect(droppedHostIds(drop([MIME], "garbage"))).toEqual([]);
  });
});

describe("movedIds", () => {
  it("skips hosts already in the target group", () => {
    const data = { ids: [A, B], fromGroups: [G, null] };
    expect(movedIds(data, G)).toEqual([B]);
    expect(movedIds(data, null)).toEqual([A]);
    expect(movedIds(data, "other")).toEqual([A, B]);
  });
});
