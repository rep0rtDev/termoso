import { describe, expect, it } from "vitest";
import { movedIds, parseDragData } from "./dnd";

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

describe("movedIds", () => {
  it("skips hosts already in the target group", () => {
    const data = { ids: [A, B], fromGroups: [G, null] };
    expect(movedIds(data, G)).toEqual([B]);
    expect(movedIds(data, null)).toEqual([A]);
    expect(movedIds(data, "other")).toEqual([A, B]);
  });
});
