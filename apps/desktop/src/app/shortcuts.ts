// Command registry + effective key bindings. Commands are registered by
// `commands.ts`; overrides come from `Settings.shortcuts`. Kept free of app
// imports so the terminal store can ask "is this an app shortcut?" without a
// dependency cycle.

import { createStore, useStore } from "@/lib/store";
import { chordMatches, formatChord } from "./keymap";

export type CommandGroup =
  "Navigation" | "Tabs" | "Panes" | "Terminal" | "Workspace" | "Create" | "Window";

export interface Command {
  id: string;
  title: string;
  group: CommandGroup;
  /** Extra search words for the palette. */
  keywords?: string;
  /** Default chords (`ctrl+shift+k`); an override replaces the whole list. */
  keys: string[];
  /** Hidden from the palette and ignored by shortcuts while false. */
  enabled?: () => boolean;
  run: () => void;
}

interface ShortcutsState {
  commands: Command[];
  /** Overrides from settings: id → chord, `""` = unbound. */
  overrides: Record<string, string>;
  /** The shortcut editor is capturing keys: nothing else may react to them. */
  recording: boolean;
}

export const shortcutsStore = createStore<ShortcutsState>({
  commands: [],
  overrides: {},
  recording: false,
});

export const useShortcuts = <S>(selector: (s: ShortcutsState) => S) =>
  useStore(shortcutsStore, selector);

export function registerCommands(commands: Command[]) {
  shortcutsStore.set((s) => ({ ...s, commands }));
}

export function applyShortcutOverrides(overrides: Record<string, string>) {
  shortcutsStore.set((s) => ({ ...s, overrides }));
}

export function setRecording(recording: boolean) {
  shortcutsStore.set((s) => ({ ...s, recording }));
}

/** Chords currently bound to a command (after overrides). */
export function bindingsOf(cmd: Command, overrides = shortcutsStore.get().overrides): string[] {
  const o = overrides[cmd.id];
  if (o === undefined) return cmd.keys;
  return o === "" ? [] : [o];
}

export function commandById(id: string): Command | undefined {
  return shortcutsStore.get().commands.find((c) => c.id === id);
}

/** Human-readable first binding, e.g. for tooltips: `Ctrl+Shift+D`. */
export function hint(id: string): string {
  const cmd = commandById(id);
  return cmd ? formatChord(bindingsOf(cmd)[0] ?? null) : "";
}

/** Tooltip label with the shortcut appended when bound. */
export function withHint(label: string, id: string): string {
  const h = hint(id);
  return h ? `${label} (${h})` : label;
}

/** The enabled command bound to this key event, if any. */
export function commandForEvent(ev: KeyboardEvent): Command | null {
  const { commands, overrides, recording } = shortcutsStore.get();
  if (recording) return null;
  for (const cmd of commands) {
    if (cmd.enabled && !cmd.enabled()) continue;
    if (bindingsOf(cmd, overrides).some((k) => chordMatches(k, ev))) return cmd;
  }
  return null;
}

/** Ctrl+1 … Ctrl+9 select a session tab by position; not rebindable. */
export function tabDigit(ev: KeyboardEvent): number | null {
  if (shortcutsStore.get().recording) return null;
  if (!(ev.ctrlKey || ev.metaKey) || ev.shiftKey || ev.altKey) return null;
  const m = /^Digit([1-9])$/.exec(ev.code);
  return m ? Number(m[1]) : null;
}

/** Other commands (any state) already using `chord`, excluding `exceptId`. */
export function conflictsFor(
  chord: string,
  exceptId: string,
  overrides = shortcutsStore.get().overrides,
): Command[] {
  return shortcutsStore
    .get()
    .commands.filter((c) => c.id !== exceptId && bindingsOf(c, overrides).includes(chord));
}
