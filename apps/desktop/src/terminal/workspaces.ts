// Workspace templates and "restore previous session". Templates are named
// layouts of connections saved from the New Tab page or a tab; the previous
// session is a snapshot of the open tabs, written to the local store whenever
// they change and offered for restore on the next start. Only targets and
// layout are stored — never terminal contents or credentials.

import * as ipc from "@/ipc/commands";
import type {
  LayoutTemplate,
  OpenTarget,
  SessionSnapshot,
  SnapshotTab,
  TabViewMode,
  Uuid,
  WorkspaceTemplate,
} from "@/ipc/types";
import { createStore, useStore } from "@/lib/store";
import {
  openLayout,
  openTerminal,
  renameTab,
  setTabTemplate,
  tabLayoutTemplate,
  terminalStore,
  type TerminalState,
  type TerminalTab,
} from "./store";
import { tr, msg } from "@/i18n";

export interface WorkspacesUiState {
  loaded: boolean;
  templates: WorkspaceTemplate[];
  /** Tabs open when the app last ran, until restored or dismissed. */
  previous: SessionSnapshot | null;
  /** Template whose name is being edited inline (just created or "Rename"). */
  editingId: Uuid | null;
}

export const workspacesStore = createStore<WorkspacesUiState>({
  loaded: false,
  templates: [],
  previous: null,
  editingId: null,
});

export const useWorkspaces = <S>(selector: (s: WorkspacesUiState) => S) =>
  useStore(workspacesStore, selector);

export const DEFAULT_WORKSPACE_NAME = msg("New Workspace");
const SAVE_DEBOUNCE_MS = 400;

const uuid = () => crypto.randomUUID();
const now = () => new Date().toISOString();

function update(fn: (s: WorkspacesUiState) => WorkspacesUiState) {
  workspacesStore.set(fn);
}

// ───────────────────────────── layouts ─────────────────────────────

/** Leaves of a layout in reading order. */
function nameOrDefault(name: string | undefined): string {
  const trimmed = name?.trim() ?? "";
  return trimmed === "" ? tr(DEFAULT_WORKSPACE_NAME) : trimmed;
}

export function layoutTargets(node: LayoutTemplate): OpenTarget[] {
  if (node.kind === "leaf") return [node.target];
  return [...layoutTargets(node.first), ...layoutTargets(node.second)];
}

/** Balanced layout for a list of targets: rows of up to three, stacked. */
export function layoutFromTargets(targets: OpenTarget[]): LayoutTemplate | null {
  const leaves: LayoutTemplate[] = targets.map((target) => ({ kind: "leaf", target }));
  if (leaves.length === 0) return null;
  const rows: LayoutTemplate[] = [];
  for (let i = 0; i < leaves.length; i += 3) rows.push(chain(leaves.slice(i, i + 3), "row"));
  return chain(rows, "column");
}

function chain(nodes: LayoutTemplate[], direction: "row" | "column"): LayoutTemplate {
  const [head, ...rest] = nodes;
  if (!head) throw new Error("empty layout");
  if (rest.length === 0) return head;
  return {
    kind: "split",
    direction,
    ratio: 1 / nodes.length,
    first: head,
    second: chain(rest, direction),
  };
}

export const sameTarget = (a: OpenTarget, b: OpenTarget) => JSON.stringify(a) === JSON.stringify(b);

// ───────────────────────────── snapshot ─────────────────────────────

export function snapshotTabs(s: TerminalState): SnapshotTab[] {
  const out: SnapshotTab[] = [];
  for (const tab of s.tabs) {
    const layout = tabLayoutTemplate(tab, s);
    if (layout) {
      out.push({ name: tab.name, viewMode: tab.viewMode, templateId: tab.templateId, layout });
    }
  }
  return out;
}

export const snapshotConnections = (snap: SessionSnapshot) =>
  snap.tabs.reduce((n, t) => n + layoutTargets(t.layout).length, 0);

// ───────────────────────────── persistence ─────────────────────────────

let saveTimer: ReturnType<typeof setTimeout> | null = null;
let lastSaved = "";
let saving: Promise<void> = Promise.resolve();

function persist(immediate = false) {
  const st = workspacesStore.get();
  if (!st.loaded) return;
  const tabs = snapshotTabs(terminalStore.get());
  const lastSession: SessionSnapshot | null =
    tabs.length > 0 ? { savedAt: now(), tabs } : st.previous;
  const body = { templates: st.templates, lastSession };
  const key = JSON.stringify({ templates: st.templates, tabs: lastSession?.tabs ?? [] });
  if (key === lastSaved) return;
  const write = () => {
    lastSaved = key;
    saving = saving
      .then(() => ipc.workspacesSet(body).then(() => undefined))
      .catch(() => undefined);
  };
  if (saveTimer) clearTimeout(saveTimer);
  if (immediate) {
    saveTimer = null;
    write();
  } else {
    saveTimer = setTimeout(() => {
      saveTimer = null;
      write();
    }, SAVE_DEBOUNCE_MS);
  }
}

let started = false;

/** Load templates and the previous session, then mirror open tabs to the store. */
export function startWorkspaces() {
  if (started) return;
  started = true;
  void ipc
    .workspacesGet()
    .then((st) => {
      const previous = st.lastSession && st.lastSession.tabs.length > 0 ? st.lastSession : null;
      lastSaved = JSON.stringify({ templates: st.templates, tabs: previous?.tabs ?? [] });
      update((w) => ({ ...w, loaded: true, templates: st.templates, previous }));
    })
    .catch(() => update((w) => ({ ...w, loaded: true })))
    .finally(() => persist());
  // Panes are watched too: the shell's cwd / running command live there.
  let seen = terminalStore.get();
  terminalStore.subscribe(() => {
    const s = terminalStore.get();
    if (s.tabs === seen.tabs && s.panes === seen.panes) return;
    seen = s;
    persist();
  });
  window.addEventListener("beforeunload", () => persist(true));
}

// ───────────────────────────── templates ─────────────────────────────

export function templateById(id: Uuid | null, s = workspacesStore.get()) {
  return id ? s.templates.find((t) => t.id === id) : undefined;
}

/** Save a new template and open its name for editing. */
export function createTemplate(
  layout: LayoutTemplate,
  opts: { name?: string; viewMode?: TabViewMode; edit?: boolean } = {},
): WorkspaceTemplate {
  const at = now();
  const tpl: WorkspaceTemplate = {
    id: uuid(),
    name: nameOrDefault(opts.name),
    viewMode: opts.viewMode ?? "split",
    layout,
    createdAt: at,
    updatedAt: at,
  };
  update((s) => ({
    ...s,
    templates: [...s.templates, tpl],
    editingId: opts.edit === false ? s.editingId : tpl.id,
  }));
  persist();
  return tpl;
}

export function renameTemplate(id: Uuid, name: string) {
  const trimmed = name.trim();
  update((s) => ({
    ...s,
    editingId: s.editingId === id ? null : s.editingId,
    templates: trimmed
      ? s.templates.map((t) => (t.id === id ? { ...t, name: trimmed, updatedAt: now() } : t))
      : s.templates,
  }));
  if (trimmed) {
    terminalStore.set((ts) => ({
      ...ts,
      tabs: ts.tabs.map((t) => (t.templateId === id ? { ...t, name: trimmed } : t)),
    }));
  }
  persist();
}

export function deleteTemplate(id: Uuid) {
  update((s) => ({
    ...s,
    editingId: s.editingId === id ? null : s.editingId,
    templates: s.templates.filter((t) => t.id !== id),
  }));
  terminalStore.set((ts) => ({
    ...ts,
    tabs: ts.tabs.map((t) => (t.templateId === id ? { ...t, templateId: null } : t)),
  }));
  persist();
}

export function setEditingTemplate(id: Uuid | null) {
  update((s) => (s.editingId === id ? s : { ...s, editingId: id }));
}

/** Open the template as a new workspace tab connecting every saved target. */
export function openTemplate(id: Uuid, background = false): string | null {
  const tpl = templateById(id);
  if (!tpl) return null;
  return openLayout(tpl.layout, {
    name: tpl.name,
    viewMode: tpl.viewMode,
    templateId: tpl.id,
    background,
  });
}

export interface WorkspaceChoice {
  /** Open workspace tab, or a saved template that is not open right now. */
  kind: "tab" | "template";
  id: string;
  name: string;
}

/** Targets for the "Add to Workspace" submenu: open workspaces, then saved ones. */
export function workspaceChoices(
  tabs: TerminalTab[] = terminalStore.get().tabs,
  templates: WorkspaceTemplate[] = workspacesStore.get().templates,
): WorkspaceChoice[] {
  const open = tabs.filter((t) => t.name !== null);
  const openTemplates = new Set(open.map((t) => t.templateId));
  return [
    ...open.map((t): WorkspaceChoice => ({ kind: "tab", id: t.id, name: t.name ?? "" })),
    ...templates
      .filter((t) => !openTemplates.has(t.id))
      .map((t): WorkspaceChoice => ({ kind: "template", id: t.id, name: t.name })),
  ];
}

/**
 * Add connections to a workspace: `null` starts a new one holding just these
 * targets; a template that is not open is opened first. Alternates split
 * direction so several hosts land in a grid rather than a strip.
 */
export function addToWorkspace(
  choice: WorkspaceChoice | null,
  targets: OpenTarget[],
  background = false,
): string | null {
  if (targets.length === 0) return null;
  if (choice === null) {
    const layout = layoutFromTargets(targets);
    return layout ? openLayout(layout, { name: tr(DEFAULT_WORKSPACE_NAME), background }) : null;
  }
  let tabId: string | null = choice.id;
  if (choice.kind === "template") tabId = openTemplate(choice.id, background);
  if (!tabId) return null;
  let direction: "row" | "column" = "row";
  for (const target of targets) {
    if (openTerminal(target, { intoTab: tabId, direction, background }) === null) break;
    direction = direction === "row" ? "column" : "row";
  }
  return tabId;
}

/**
 * Store the tab's current layout as a template: updates the one it was
 * opened from, otherwise creates a new template named after the tab.
 */
export function saveTabAsTemplate(tabId: string): WorkspaceTemplate | null {
  const s = terminalStore.get();
  const tab = s.tabs.find((t) => t.id === tabId);
  const layout = tab ? tabLayoutTemplate(tab, s) : null;
  if (!tab || !layout) return null;
  const existing = templateById(tab.templateId);
  if (existing) {
    const updated = { ...existing, layout, viewMode: tab.viewMode, updatedAt: now() };
    update((w) => ({
      ...w,
      templates: w.templates.map((t) => (t.id === existing.id ? updated : t)),
    }));
    persist();
    return updated;
  }
  const tpl = createTemplate(layout, {
    name: tab.name ?? undefined,
    viewMode: tab.viewMode,
    edit: false,
  });
  setTabTemplate(tabId, tpl.id);
  if (tab.name === null) renameTab(tabId, tpl.name);
  return tpl;
}

/** The open tab differs from the template it came from ("unsaved changes" dot). */
export function tabDiffersFromTemplate(tab: TerminalTab, s: TerminalState): boolean {
  const tpl = templateById(tab.templateId);
  if (!tpl) return false;
  const layout = tabLayoutTemplate(tab, s);
  return (
    JSON.stringify(layout && layoutTargets(layout)) !== JSON.stringify(layoutTargets(tpl.layout))
  );
}

// ───────────────────────────── previous session ─────────────────────────────

/** Reopen every tab from the previous session; the first one becomes active. */
export function restorePrevious(): number {
  const snap = workspacesStore.get().previous;
  if (!snap) return 0;
  update((s) => ({ ...s, previous: null }));
  let first: string | null = null;
  for (const tab of snap.tabs) {
    const id = openLayout(tab.layout, {
      name: tab.name,
      viewMode: tab.viewMode,
      templateId: templateById(tab.templateId) ? tab.templateId : null,
      background: first !== null,
    });
    first ??= id;
  }
  return snapshotConnections(snap);
}

export function dismissPrevious() {
  update((s) => ({ ...s, previous: null }));
  persist();
}
