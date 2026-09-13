// Keyboard chords: parsing, formatting and matching against `KeyboardEvent`s.
// A chord is stored as `ctrl+shift+k` — modifiers in a fixed order, then one
// key name derived from `KeyboardEvent.code` so layouts don't matter.

export interface Chord {
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  meta: boolean;
  /** `KeyboardEvent.code`, e.g. `KeyK`, `Digit1`, `Period`, `ArrowLeft`, `Tab`. */
  code: string;
}

const SYMBOL_CODES: Record<string, string> = {
  Period: ".",
  Comma: ",",
  Slash: "/",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Backquote: "`",
  BracketLeft: "[",
  BracketRight: "]",
  Minus: "-",
  Equal: "=",
  NumpadAdd: "numadd",
  NumpadSubtract: "numsub",
  Numpad0: "num0",
};
const CODES_BY_SYMBOL = Object.fromEntries(Object.entries(SYMBOL_CODES).map(([c, s]) => [s, c]));

const NAMED_CODES: Record<string, string> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
  Tab: "tab",
  Space: "space",
  Enter: "enter",
  Escape: "escape",
  Backspace: "backspace",
  Delete: "delete",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
  Insert: "insert",
};
const CODES_BY_NAME = Object.fromEntries(Object.entries(NAMED_CODES).map(([c, n]) => [n, c]));

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

export const isModifierCode = (code: string) => MODIFIER_CODES.has(code);

/** Key name (the part after the modifiers) for a `KeyboardEvent.code`, or null if unsupported. */
export function keyName(code: string): string | null {
  const letter = /^Key([A-Z])$/.exec(code)?.[1];
  if (letter) return letter.toLowerCase();
  const digit = /^Digit([0-9])$/.exec(code)?.[1];
  if (digit) return digit;
  const fn = /^F([1-9]|1[0-2])$/.exec(code);
  if (fn) return code.toLowerCase();
  return SYMBOL_CODES[code] ?? NAMED_CODES[code] ?? null;
}

function codeForName(name: string): string | null {
  if (/^[a-z]$/.test(name)) return `Key${name.toUpperCase()}`;
  if (/^[0-9]$/.test(name)) return `Digit${name}`;
  if (/^f([1-9]|1[0-2])$/.test(name)) return name.toUpperCase();
  return CODES_BY_SYMBOL[name] ?? CODES_BY_NAME[name] ?? null;
}

export function parseChord(text: string): Chord | null {
  const parts = text.trim().toLowerCase().split("+");
  if (parts.length === 0) return null;
  const chord: Chord = { ctrl: false, shift: false, alt: false, meta: false, code: "" };
  const key = parts.pop() ?? "";
  for (const p of parts) {
    if (p === "ctrl") chord.ctrl = true;
    else if (p === "shift") chord.shift = true;
    else if (p === "alt") chord.alt = true;
    else if (p === "meta" || p === "cmd" || p === "super") chord.meta = true;
    else return null;
  }
  const code = codeForName(key);
  if (!code) return null;
  chord.code = code;
  return chord;
}

export function chordFromEvent(ev: KeyboardEvent): Chord | null {
  if (isModifierCode(ev.code) || !keyName(ev.code)) return null;
  return {
    ctrl: ev.ctrlKey,
    shift: ev.shiftKey,
    alt: ev.altKey,
    meta: ev.metaKey,
    code: ev.code,
  };
}

/** Canonical storage form, e.g. `ctrl+shift+k`. */
export function serializeChord(c: Chord): string {
  const parts: string[] = [];
  if (c.ctrl) parts.push("ctrl");
  if (c.shift) parts.push("shift");
  if (c.alt) parts.push("alt");
  if (c.meta) parts.push("meta");
  parts.push(keyName(c.code) ?? c.code);
  return parts.join("+");
}

const DISPLAY_NAMES: Record<string, string> = {
  left: "←",
  right: "→",
  up: "↑",
  down: "↓",
  tab: "Tab",
  space: "Space",
  enter: "Enter",
  escape: "Esc",
  backspace: "Backspace",
  delete: "Del",
  home: "Home",
  end: "End",
  pageup: "PgUp",
  pagedown: "PgDn",
  insert: "Ins",
  numadd: "Num +",
  numsub: "Num −",
  num0: "Num 0",
};

/** Human key caps, e.g. `["Ctrl", "Shift", "K"]`; empty for an unbound chord. */
export function chordParts(text: string | null): string[] {
  if (!text) return [];
  const c = parseChord(text);
  if (!c) return [text];
  const parts: string[] = [];
  if (c.ctrl) parts.push("Ctrl");
  if (c.shift) parts.push("Shift");
  if (c.alt) parts.push("Alt");
  if (c.meta) parts.push("Super");
  const name = keyName(c.code) ?? c.code;
  parts.push(DISPLAY_NAMES[name] ?? name.toUpperCase());
  return parts;
}

/** Human form, e.g. `Ctrl+Shift+K`; empty for an unbound chord. */
export function formatChord(text: string | null): string {
  return chordParts(text).join("+");
}

export function chordMatches(chord: string, ev: KeyboardEvent): boolean {
  const c = parseChord(chord);
  if (!c) return false;
  return (
    c.code === ev.code &&
    c.ctrl === ev.ctrlKey &&
    c.shift === ev.shiftKey &&
    c.alt === ev.altKey &&
    c.meta === ev.metaKey
  );
}

/** Chords must include at least one of Ctrl / Alt / Super unless they are F-keys. */
export function isBindable(c: Chord): boolean {
  if (c.ctrl || c.alt || c.meta) return true;
  return /^F([1-9]|1[0-2])$/.test(c.code);
}
