import { afterEach, describe, expect, it, vi } from "vitest";

// Minimal stand-in: keymap only reads modifier flags and `code`.
function key(code: string, mods: Partial<Record<"ctrl" | "shift" | "alt" | "meta", boolean>> = {}) {
  return {
    code,
    ctrlKey: mods.ctrl ?? false,
    shiftKey: mods.shift ?? false,
    altKey: mods.alt ?? false,
    metaKey: mods.meta ?? false,
  } as unknown as KeyboardEvent;
}

type Keymap = typeof import("./keymap");

/** What the shortcut editor would store for this key press. */
function record(km: Keymap, ev: KeyboardEvent): string | null {
  const chord = km.chordFromEvent(ev);
  return chord ? km.serializeChord(chord) : null;
}

afterEach(() => {
  vi.resetModules();
  vi.doUnmock("@/lib/platform");
});

describe("keymap on Linux / Windows", () => {
  it("binds `ctrl` to the Control key and `meta` to Super", async () => {
    vi.doMock("@/lib/platform", () => ({ IS_MAC: false, MAC_TRAFFIC_LIGHTS_WIDTH: 0 }));
    const km = await import("./keymap");
    expect(km.chordMatches("ctrl+shift+k", key("KeyK", { ctrl: true, shift: true }))).toBe(true);
    expect(km.chordMatches("ctrl+shift+k", key("KeyK", { meta: true, shift: true }))).toBe(false);
    expect(km.chordMatches("meta+k", key("KeyK", { meta: true }))).toBe(true);
    expect(record(km, key("KeyK", { ctrl: true }))).toBe("ctrl+k");
    expect(km.formatChord("ctrl+alt+meta+k")).toBe("Ctrl+Alt+Super+K");
  });
});

describe("keymap on macOS", () => {
  it("binds `ctrl` to ⌘ so Control stays free for terminal control characters", async () => {
    vi.doMock("@/lib/platform", () => ({ IS_MAC: true, MAC_TRAFFIC_LIGHTS_WIDTH: 78 }));
    const km = await import("./keymap");
    expect(km.chordMatches("ctrl+shift+k", key("KeyK", { meta: true, shift: true }))).toBe(true);
    expect(km.chordMatches("ctrl+shift+k", key("KeyK", { ctrl: true, shift: true }))).toBe(false);
    expect(km.chordMatches("ctrl+c", key("KeyC", { ctrl: true }))).toBe(false);
    expect(km.chordMatches("ctrl+c", key("KeyC", { meta: true }))).toBe(true);
    // The stored chord stays portable: ⌘+K records as `ctrl+k`, Control+K as `meta+k`.
    expect(record(km, key("KeyK", { meta: true }))).toBe("ctrl+k");
    expect(record(km, key("KeyK", { ctrl: true }))).toBe("meta+k");
    expect(km.formatChord("ctrl+alt+meta+k")).toBe("Cmd+Option+Ctrl+K");
  });
});
