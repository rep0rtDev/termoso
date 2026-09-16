// Offline autocomplete over the line the user has typed so far. Pure: this
// module never talks to the network — sources are the bundled command
// catalogue, the (decrypted) command history, snippets and directory
// listings the caller fetches through the session itself.

import { COMMANDS, WRAPPERS, commandSpec } from "./commands";
import { optionsFor, subcommandsFor } from "./commandFlags";

export type SuggestionKind =
  "command" | "option" | "subcommand" | "path" | "history" | "snippet" | "identity";

export interface Suggestion {
  kind: SuggestionKind;
  /** Full token / line shown in the popup. */
  label: string;
  desc: string;
  /** Text to type to turn what is on the line into `label`. */
  insert: string;
}

export interface SnippetSource {
  label: string;
  script: string;
}

export interface CompletionContext {
  /** Input between the prompt and the cursor. */
  line: string;
  /** Distinct command lines, most recent first. */
  history: readonly string[];
  snippets: readonly SnippetSource[];
}

/** A directory the caller should list to finish the path part of the completion. */
export interface PathQuery {
  /** Directory as typed (`src/`, `~/`, `/etc/`); empty = working directory. */
  dir: string;
  /** Unescaped file-name prefix typed after the last slash. */
  prefix: string;
  dirsOnly: boolean;
}

export interface Completion {
  items: Suggestion[];
  path: PathQuery | null;
}

export const MAX_SUGGESTIONS = 12;
const MAX_HISTORY = 4;
const MAX_SNIPPETS = 3;

const SEPARATORS = /(\|\||&&|;|\||&)/;

/** Last simple command of the line (after `|`, `&&`, `;` …). */
function lastSegment(line: string): string {
  const parts = line.split(SEPARATORS);
  return parts[parts.length - 1] ?? line;
}

/** Whitespace split that keeps quoted / escaped spaces inside a token. */
function tokenize(segment: string): { words: string[]; current: string } {
  const words: string[] = [];
  let cur = "";
  let quote: string | null = null;
  for (let i = 0; i < segment.length; i++) {
    const ch = segment[i] ?? "";
    if (quote) {
      cur += ch;
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "\\" && i + 1 < segment.length) {
      cur += ch + (segment[i + 1] ?? "");
      i++;
      continue;
    }
    if (ch === "'" || ch === '"') {
      quote = ch;
      cur += ch;
      continue;
    }
    if (ch === " " || ch === "\t") {
      if (cur) words.push(cur);
      cur = "";
      continue;
    }
    cur += ch;
  }
  return { words, current: cur };
}

const ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*=/;

/** Index of the real command word, skipping `VAR=x`, `sudo`, `time`… and their flags. */
function commandIndex(words: string[]): number {
  let i = 0;
  while (i < words.length && ASSIGNMENT.test(words[i] ?? "")) i++;
  while (i < words.length && WRAPPERS.has(words[i] ?? "")) {
    i++;
    while (
      i < words.length &&
      ((words[i] ?? "").startsWith("-") || ASSIGNMENT.test(words[i] ?? ""))
    )
      i++;
  }
  return i;
}

const unescape = (s: string) => s.replace(/\\(.)/g, "$1");

/** Shell-escape the characters that would otherwise split or expand a file name. */
export function escapePath(name: string): string {
  return name.replace(/([ '"\\$&|;<>()*?[\]#~!{}`])/g, "\\$1");
}

function looksLikePath(token: string): boolean {
  return (
    token.startsWith("/") || token.startsWith("~") || token.startsWith(".") || token.includes("/")
  );
}

function pathQuery(current: string, dirsOnly: boolean): PathQuery | null {
  if (current.startsWith("'") || current.startsWith('"')) return null;
  if (current.startsWith("-") || current.startsWith("$")) return null;
  const slash = current.lastIndexOf("/");
  const dir = slash === -1 ? "" : current.slice(0, slash + 1);
  const prefix = unescape(slash === -1 ? current : current.slice(slash + 1));
  if (dir === "" && prefix.startsWith("~")) return null;
  return { dir, prefix, dirsOnly };
}

function historyItems(line: string, history: readonly string[]): Suggestion[] {
  const out: Suggestion[] = [];
  const typed = line.trimStart();
  if (!typed) return out;
  for (const h of history) {
    if (h.length > typed.length && h.startsWith(typed) && !h.includes("\n")) {
      out.push({ kind: "history", label: h, desc: "history", insert: h.slice(typed.length) });
      if (out.length >= MAX_HISTORY) break;
    }
  }
  return out;
}

function snippetItems(current: string, snippets: readonly SnippetSource[]): Suggestion[] {
  const out: Suggestion[] = [];
  if (current.length < 2) return out;
  for (const s of snippets) {
    const lines = s.script.split("\n").filter((l) => l.trim());
    const first = lines[0]?.trim() ?? "";
    if (lines.length !== 1 || !first.startsWith(current) || first === current) continue;
    out.push({ kind: "snippet", label: first, desc: s.label, insert: first.slice(current.length) });
    if (out.length >= MAX_SNIPPETS) break;
  }
  return out;
}

/**
 * Suggestions for `ctx.line`. Everything here resolves synchronously; when
 * the token could be a file name, `path` tells the caller which directory
 * to list (see `mergePaths`).
 */
export function complete(ctx: CompletionContext): Completion {
  const empty: Completion = { items: [], path: null };
  const line = ctx.line;
  if (!line.trim()) return empty;

  const segment = lastSegment(line);
  const { words, current } = tokenize(segment);
  const cmdIdx = commandIndex(words);
  const items: Suggestion[] = historyItems(line, ctx.history);
  const seen = new Set(items.map((i) => i.label));
  const push = (s: Suggestion) => {
    if (seen.has(s.label)) return;
    seen.add(s.label);
    items.push(s);
  };
  let path: PathQuery | null = null;

  if (cmdIdx >= words.length) {
    // Typing the command itself.
    if (current.length === 0) return { items: items.slice(0, MAX_SUGGESTIONS), path: null };
    if (looksLikePath(current)) {
      path = pathQuery(current, false);
    } else if (!current.startsWith("-") && !current.startsWith("$")) {
      let n = 0;
      for (const c of COMMANDS) {
        if (c.name.startsWith(current) && c.name !== current) {
          push({
            kind: "command",
            label: c.name,
            desc: c.desc,
            insert: c.name.slice(current.length),
          });
          if (++n >= 8) break;
        }
      }
    }
    for (const s of snippetItems(segment.trimStart(), ctx.snippets)) push(s);
    return { items: items.slice(0, MAX_SUGGESTIONS), path };
  }

  for (const s of snippetItems(segment.trimStart(), ctx.snippets)) push(s);
  const cmd = words[cmdIdx] ?? "";
  const spec = commandSpec(cmd);
  const argWords = words.slice(cmdIdx + 1);

  if (current.startsWith("-")) {
    const flags = optionsFor(cmd);
    for (const f of flags) {
      if (f.name.startsWith(current) && f.name !== current) {
        push({ kind: "option", label: f.name, desc: f.desc, insert: f.name.slice(current.length) });
      }
    }
    return { items: items.slice(0, MAX_SUGGESTIONS), path: null };
  }

  const subs = subcommandsFor(cmd);
  const firstArg = argWords.every((w) => w.startsWith("-"));
  if (subs.length > 0 && firstArg && !looksLikePath(current)) {
    for (const s of subs) {
      if (s.name.startsWith(current) && s.name !== current) {
        push({
          kind: "subcommand",
          label: s.name,
          desc: s.desc,
          insert: s.name.slice(current.length),
        });
      }
    }
  }

  const wantsPath =
    looksLikePath(current) ||
    (spec ? spec.paths !== "none" : true) ||
    (subs.length > 0 && !firstArg);
  if (wantsPath && !(subs.length > 0 && firstArg && current.length === 0)) {
    path = pathQuery(current, spec?.paths === "dir");
  }

  return { items: items.slice(0, MAX_SUGGESTIONS), path };
}

export interface DirEntryLike {
  name: string;
  dir: boolean;
}

/** Turn a directory listing into suggestions for `query`, appended after `items`. */
export function mergePaths(
  items: Suggestion[],
  query: PathQuery,
  entries: readonly DirEntryLike[],
): Suggestion[] {
  const out = [...items];
  const seen = new Set(out.map((i) => i.label));
  const showHidden = query.prefix.startsWith(".");
  const matches = entries
    .filter((e) => (query.dirsOnly ? e.dir : true))
    .filter((e) => e.name.startsWith(query.prefix) && e.name !== query.prefix)
    .filter((e) => showHidden || !e.name.startsWith("."))
    .sort((a, b) => Number(b.dir) - Number(a.dir) || a.name.localeCompare(b.name));
  for (const e of matches) {
    if (out.length >= MAX_SUGGESTIONS) break;
    const label = query.dir + e.name + (e.dir ? "/" : "");
    if (seen.has(label)) continue;
    seen.add(label);
    const rest = e.name.slice(query.prefix.length);
    out.push({
      kind: "path",
      label,
      desc: e.dir ? "directory" : "file",
      insert: escapePath(rest) + (e.dir ? "/" : " "),
    });
  }
  return out;
}

/** What to type after a non-path suggestion is accepted. */
export function suffixFor(kind: SuggestionKind): string {
  switch (kind) {
    case "command":
    case "subcommand":
    case "option":
      return " ";
    default:
      return "";
  }
}
