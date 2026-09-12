/**
 * Keyword highlighting: colours error / warning / ok / info / debug words and
 * IP / MAC addresses in terminal output before it reaches xterm. Works on the
 * byte stream, leaves every escape sequence untouched and restores the
 * foreground colour the program had set, so coloured output keeps its look.
 */

export type KeywordKind = "error" | "warning" | "ok" | "info" | "debug" | "address";

export interface KeywordCategory {
  kind: KeywordKind;
  label: string;
  /** SGR foreground parameter (ANSI palette, so it follows the theme). */
  sgr: number;
  pattern: RegExp;
}

const word = (alts: string) => new RegExp(`\\b(?:${alts})\\b`, "gi");

export const KEYWORD_CATEGORIES: readonly KeywordCategory[] = [
  {
    kind: "error",
    label: "Error",
    sgr: 31,
    pattern: word("errors?|err|fail(?:ed|ure|s)?|fatal|critical|crit|panic|exception|denied"),
  },
  {
    kind: "warning",
    label: "Warning",
    sgr: 33,
    pattern: word("warnings?|warn|deprecated|caution"),
  },
  { kind: "ok", label: "OK", sgr: 32, pattern: word("ok|okay|success(?:ful|fully)?|passed|done") },
  { kind: "info", label: "Info", sgr: 34, pattern: word("info|information|notice") },
  { kind: "debug", label: "Debug", sgr: 35, pattern: word("debug|trace|verbose") },
  {
    kind: "address",
    label: "IP address & MAC",
    sgr: 95,
    pattern:
      /\b(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)(?:\.(?:25[0-5]|2[0-4]\d|1?\d?\d)){3}(?::\d{1,5})?|(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2})\b/gi,
  },
];

const ESC = "\x1b";
const BEL = "\x07";
/** Longest escape sequence held back waiting for its terminator. */
const MAX_PENDING = 4096;

/** Foreground-setting SGR parameters, or "" when the sequence does not touch fg. */
function foregroundOf(params: string): string | null {
  const parts = params === "" ? ["0"] : params.split(";");
  let fg: string | null = null;
  for (let i = 0; i < parts.length; i++) {
    const p = parts[i] === "" ? 0 : Number(parts[i]);
    if (p === 0 || p === 39) fg = "";
    else if ((p >= 30 && p <= 37) || (p >= 90 && p <= 97)) fg = String(p);
    else if (p === 38) {
      const mode = Number(parts[i + 1]);
      const len = mode === 5 ? 3 : mode === 2 ? 5 : 1;
      fg = parts.slice(i, i + len).join(";");
      i += len - 1;
    }
  }
  return fg;
}

/**
 * Per-session transformer. Feed each output chunk through `push`; text that
 * arrives split across chunks inside an escape sequence is held back until
 * the sequence completes.
 */
export class KeywordHighlighter {
  private readonly decoder = new TextDecoder("utf-8");
  /** Incomplete escape sequence carried over from the previous chunk. */
  private pending = "";
  /** Foreground the program currently has active (SGR params, "" = default). */
  private fg = "";

  constructor(private readonly categories: readonly KeywordCategory[] = KEYWORD_CATEGORIES) {}

  push(chunk: Uint8Array | string): string {
    const text =
      this.pending +
      (typeof chunk === "string" ? chunk : this.decoder.decode(chunk, { stream: true }));
    this.pending = "";
    let out = "";
    let i = 0;
    while (i < text.length) {
      const esc = text.indexOf(ESC, i);
      if (esc === -1) {
        out += this.colorize(text.slice(i));
        break;
      }
      out += this.colorize(text.slice(i, esc));
      const end = sequenceEnd(text, esc);
      if (end === -1) {
        const rest = text.slice(esc);
        if (rest.length > MAX_PENDING) out += rest;
        else this.pending = rest;
        break;
      }
      const seq = text.slice(esc, end);
      if (seq.startsWith(`${ESC}[`) && seq.endsWith("m")) {
        const fg = foregroundOf(seq.slice(2, -1));
        if (fg !== null) this.fg = fg;
      }
      out += seq;
      i = end;
    }
    return out;
  }

  private colorize(text: string): string {
    if (text === "") return text;
    const restore = this.fg === "" ? `${ESC}[39m` : `${ESC}[${this.fg}m`;
    const spans: { start: number; end: number; sgr: number }[] = [];
    for (const cat of this.categories) {
      cat.pattern.lastIndex = 0;
      for (const m of text.matchAll(cat.pattern)) {
        const start = m.index;
        const end = start + m[0].length;
        if (!spans.some((s) => start < s.end && end > s.start))
          spans.push({ start, end, sgr: cat.sgr });
      }
    }
    if (spans.length === 0) return text;
    spans.sort((a, b) => a.start - b.start);
    let out = "";
    let pos = 0;
    for (const s of spans) {
      out += text.slice(pos, s.start) + `${ESC}[${s.sgr}m` + text.slice(s.start, s.end) + restore;
      pos = s.end;
    }
    return out + text.slice(pos);
  }
}

/** Index just past the escape sequence starting at `start`, or -1 if incomplete. */
function sequenceEnd(text: string, start: number): number {
  const kind = text[start + 1];
  if (kind === undefined) return -1;
  if (kind === "[") {
    for (let i = start + 2; i < text.length; i++) {
      const c = text.charCodeAt(i);
      if (c >= 0x40 && c <= 0x7e) return i + 1;
    }
    return -1;
  }
  if (kind === "]" || kind === "P" || kind === "^" || kind === "_") {
    for (let i = start + 2; i < text.length; i++) {
      if (text[i] === BEL) return i + 1;
      if (text[i] === ESC) {
        if (text[i + 1] === "\\") return i + 2;
        if (text[i + 1] === undefined) return -1;
      }
    }
    return -1;
  }
  if (kind === "(" || kind === ")" || kind === "*" || kind === "+" || kind === "#") {
    return start + 3 <= text.length ? start + 3 : -1;
  }
  return start + 2;
}
