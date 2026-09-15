import type { LayoutTemplate, RestoreCommands } from "@/ipc/types";

/** Shell state a workspace leaf carries so a reopened pane can pick up where it was. */
export interface LeafState {
  cwd: string | null;
  command: string | null;
}

type Leaf = Extract<LayoutTemplate, { kind: "leaf" }>;

/** `cwd` / `command` of a saved leaf; `null` when it carries nothing to restore. */
export function leafState(leaf: Leaf): LeafState | null {
  const cwd = leaf.cwd ?? null;
  const command = leaf.command ?? null;
  return cwd || command ? { cwd, command } : null;
}

/** Copy `state` onto a leaf, omitting empty fields so old readers see the v1 shape. */
export function withLeafState(leaf: Leaf, state: LeafState): Leaf {
  const out: Leaf = { kind: "leaf", target: leaf.target };
  if (state.cwd) out.cwd = state.cwd;
  if (state.command) out.command = state.command;
  return out;
}

/** POSIX single-quote so the path survives spaces, globs and `$`. */
export const shellQuote = (s: string) => `'${s.replace(/'/g, String.raw`'\''`)}'`;

/**
 * Keystrokes that bring a fresh shell back to `state`: a `cd` with a leading
 * space (so `HISTCONTROL=ignorespace` shells skip it too), then the command —
 * typed for the user to confirm, executed, or left out per `mode`.
 */
export function restoreKeystrokes(state: LeafState, mode: RestoreCommands): string {
  let text = "";
  if (state.cwd) text += ` cd -- ${shellQuote(state.cwd)}\r`;
  if (state.command && mode !== "never") {
    text += mode === "run" ? `${state.command}\r` : state.command;
  }
  return text;
}
