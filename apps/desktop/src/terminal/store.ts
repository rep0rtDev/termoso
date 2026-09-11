// Terminal tabs and panes. React renders this state; xterm instances live in
// `runtimes` so they survive tab switches. Every session decision (connect,
// auth, prompts, history) is made in Rust — this file only moves bytes
// between the IPC channel and xterm.js.

import { Terminal, type IDisposable } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as ipc from "@/ipc/commands";
import type { OpenTarget, SessionEvent, SessionInfo, Settings, Uuid } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { monoFontFamily } from "@/theme/theme";
import { createStore, omit, useStore } from "@/lib/store";
import { terminalThemes } from "./xtermTheme";

export type PaneStatus = "connecting" | "connected" | "exited" | "error" | "closed";
export type SplitDirection = "row" | "column";

export interface Pane {
  id: Uuid;
  target: OpenTarget;
  title: string;
  subtitle: string;
  protocol: SessionInfo["protocol"] | null;
  hostId: Uuid | null;
  status: PaneStatus;
  message: string | null;
}

export interface TerminalTab {
  id: string;
  paneIds: Uuid[];
  activePaneId: Uuid;
  direction: SplitDirection;
  broadcast: boolean;
  searchOpen: boolean;
}

export const HOME_TAB = "home";

export interface TerminalState {
  tabs: TerminalTab[];
  panes: Record<Uuid, Pane>;
  activeTabId: string;
  /** Multi-line paste waiting for the user's confirmation. */
  pendingPaste: { paneId: Uuid; text: string } | null;
  /** Pane whose close needs confirming. */
  pendingClose: Uuid | null;
}

export const terminalStore = createStore<TerminalState>({
  tabs: [],
  panes: {},
  activeTabId: HOME_TAB,
  pendingPaste: null,
  pendingClose: null,
});

export const useTerminal = <S>(selector: (s: TerminalState) => S) =>
  useStore(terminalStore, selector);

interface Runtime {
  term: Terminal;
  fit: FitAddon;
  search: SearchAddon;
  container: HTMLDivElement;
  webgl: WebglAddon | null;
  opened: boolean;
  disposables: IDisposable[];
}

const runtimes = new Map<Uuid, Runtime>();
export const getRuntime = (paneId: Uuid) => runtimes.get(paneId);

let currentSettings: Settings | null = null;
let currentScheme: "dark" | "light" = "dark";

const uuid = () => crypto.randomUUID();

function update(fn: (s: TerminalState) => TerminalState) {
  terminalStore.set(fn);
}

function patchPane(id: Uuid, patch: Partial<Pane>) {
  update((s) => {
    const pane = s.panes[id];
    if (!pane) return s;
    return { ...s, panes: { ...s.panes, [id]: { ...pane, ...patch } } };
  });
}

function tabOf(s: TerminalState, paneId: Uuid) {
  return s.tabs.find((t) => t.paneIds.includes(paneId));
}

// ───────────────────────────── settings / theme ─────────────────────────────

function fontFamily(settings: Settings | null) {
  const f = settings?.terminalFontFamily.trim();
  return f ? `'${f}', ${monoFontFamily}` : monoFontFamily;
}

export function applyTerminalSettings(settings: Settings) {
  currentSettings = settings;
  for (const rt of runtimes.values()) {
    rt.term.options.fontSize = settings.terminalFontSize;
    rt.term.options.fontFamily = fontFamily(settings);
    rt.term.options.cursorBlink = settings.cursorBlink;
    rt.term.options.scrollback = settings.scrollback;
    if (rt.opened) rt.fit.fit();
  }
}

export function applyTerminalScheme(scheme: "dark" | "light") {
  currentScheme = scheme;
  for (const rt of runtimes.values()) rt.term.options.theme = terminalThemes[scheme];
}

// ───────────────────────────── clipboard ─────────────────────────────

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // clipboard unavailable (no permission / insecure context); selection stays in xterm
  }
}

async function readClipboard(): Promise<string> {
  try {
    return await navigator.clipboard.readText();
  } catch {
    return "";
  }
}

function pasteInto(paneId: Uuid, text: string, force = false) {
  if (!text) return;
  if (!force && currentSettings?.confirmPasteMultiline && /\r|\n/.test(text.trimEnd())) {
    update((s) => ({ ...s, pendingPaste: { paneId, text } }));
    return;
  }
  const rt = runtimes.get(paneId);
  rt?.term.paste(text);
}

export function confirmPendingPaste(accept: boolean) {
  const pending = terminalStore.get().pendingPaste;
  update((s) => ({ ...s, pendingPaste: null }));
  if (pending && accept) pasteInto(pending.paneId, pending.text, true);
}

// ───────────────────────────── xterm runtime ─────────────────────────────

function createRuntime(paneId: Uuid): Runtime {
  const term = new Terminal({
    allowProposedApi: true,
    cursorBlink: currentSettings?.cursorBlink ?? true,
    cursorStyle: "bar",
    fontSize: currentSettings?.terminalFontSize ?? 13,
    fontFamily: fontFamily(currentSettings),
    fontWeight: "400",
    fontWeightBold: "600",
    lineHeight: 1.15,
    letterSpacing: 0,
    scrollback: currentSettings?.scrollback ?? 10_000,
    theme: terminalThemes[currentScheme],
    macOptionIsMeta: true,
    minimumContrastRatio: 1,
    scrollOnUserInput: true,
    smoothScrollDuration: 0,
  });
  const fit = new FitAddon();
  const search = new SearchAddon();
  term.loadAddon(fit);
  term.loadAddon(search);
  term.loadAddon(new Unicode11Addon());
  term.unicode.activeVersion = "11";
  term.loadAddon(
    new WebLinksAddon((_e, uri) => {
      void openUrl(uri);
    }),
  );

  const container = document.createElement("div");
  container.className = "termoso-xterm";

  const disposables: IDisposable[] = [];
  disposables.push(
    term.onData((data) => {
      void sendInput(paneId, data);
    }),
    term.onBinary((data) => {
      void sendInput(paneId, data);
    }),
    term.onResize(({ cols, rows }) => {
      const pane = terminalStore.get().panes[paneId];
      if (pane?.status === "connected") {
        ipc.terminalResize(paneId, cols, rows).catch(() => undefined);
      }
    }),
    term.onTitleChange((title) => {
      const pane = terminalStore.get().panes[paneId];
      if (pane && title.trim()) patchPane(paneId, { title: title.trim() });
    }),
    term.onSelectionChange(() => {
      if (currentSettings?.copyOnSelect && term.hasSelection()) {
        void copyText(term.getSelection());
      }
    }),
  );

  term.attachCustomKeyEventHandler((ev) => {
    if (ev.type !== "keydown") return true;
    const ctrl = ev.ctrlKey || ev.metaKey;
    if (ctrl && ev.shiftKey && ev.code === "KeyC") {
      if (term.hasSelection()) void copyText(term.getSelection());
      return false;
    }
    if (ctrl && ev.code === "KeyC" && !ev.shiftKey && term.hasSelection()) {
      void copyText(term.getSelection());
      term.clearSelection();
      return false;
    }
    if ((ctrl && ev.shiftKey && ev.code === "KeyV") || (ev.shiftKey && ev.code === "Insert")) {
      void readClipboard().then((t) => pasteInto(paneId, t));
      return false;
    }
    if (ctrl && ev.code === "Insert") {
      if (term.hasSelection()) void copyText(term.getSelection());
      return false;
    }
    return true;
  });

  container.addEventListener("contextmenu", (ev) => {
    if (!currentSettings?.pasteOnRightClick) return;
    ev.preventDefault();
    if (term.hasSelection()) {
      void copyText(term.getSelection());
      term.clearSelection();
    } else {
      void readClipboard().then((t) => pasteInto(paneId, t));
    }
  });
  container.addEventListener("mousedown", () => setActivePane(paneId));

  return { term, fit, search, container, webgl: null, opened: false, disposables };
}

/** Attach the pane's DOM to `host` (first mount also opens xterm). */
export function mountPane(paneId: Uuid, host: HTMLElement) {
  const rt = runtimes.get(paneId);
  if (!rt) return () => undefined;
  host.appendChild(rt.container);
  if (!rt.opened) {
    rt.term.open(rt.container);
    rt.opened = true;
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => {
        webgl.dispose();
        rt.webgl = null;
      });
      rt.term.loadAddon(webgl);
      rt.webgl = webgl;
    } catch {
      rt.webgl = null;
    }
  }
  const ro = new ResizeObserver(() => fitPane(paneId));
  ro.observe(host);
  requestAnimationFrame(() => fitPane(paneId));
  return () => {
    ro.disconnect();
    if (rt.container.parentElement === host) host.removeChild(rt.container);
  };
}

export function fitPane(paneId: Uuid) {
  const rt = runtimes.get(paneId);
  if (!rt?.opened || !rt.container.isConnected) return;
  const { width, height } = rt.container.getBoundingClientRect();
  if (width < 20 || height < 20) return;
  try {
    rt.fit.fit();
  } catch {
    // container not laid out yet
  }
}

export function focusPane(paneId: Uuid) {
  runtimes.get(paneId)?.term.focus();
}

function disposeRuntime(paneId: Uuid) {
  const rt = runtimes.get(paneId);
  if (!rt) return;
  runtimes.delete(paneId);
  for (const d of rt.disposables) d.dispose();
  rt.webgl?.dispose();
  rt.term.dispose();
  rt.container.remove();
}

function writeSystemLine(paneId: Uuid, text: string, color = "90") {
  const rt = runtimes.get(paneId);
  rt?.term.write(`\r\n\x1b[${color}m${text}\x1b[0m\r\n`);
}

// ───────────────────────────── input ─────────────────────────────

async function sendInput(paneId: Uuid, data: string) {
  const s = terminalStore.get();
  const tab = tabOf(s, paneId);
  const targets =
    tab?.broadcast && tab.paneIds.length > 1
      ? tab.paneIds.filter((id) => s.panes[id]?.status === "connected")
      : [paneId];
  await Promise.all(
    targets.map((id) =>
      s.panes[id]?.status === "connected"
        ? ipc.terminalWrite(id, data).catch(() => undefined)
        : Promise.resolve(),
    ),
  );
}

// ───────────────────────────── lifecycle ─────────────────────────────

function describe(target: OpenTarget): { title: string; subtitle: string } {
  switch (target.kind) {
    case "local":
      return { title: "Local", subtitle: "local shell" };
    case "quick":
      return { title: target.address, subtitle: target.address };
    case "host":
      return { title: "Connecting…", subtitle: "" };
  }
}

function startSession(paneId: Uuid, target: OpenTarget) {
  const rt = runtimes.get(paneId);
  const cols = rt?.term.cols ?? 80;
  const rows = rt?.term.rows ?? 24;
  ipc
    .terminalOpen(paneId, target, cols, rows, (bytes) => {
      runtimes.get(paneId)?.term.write(bytes);
    })
    .then((info) => {
      patchPane(paneId, {
        title: info.title,
        subtitle: info.target,
        protocol: info.protocol,
        hostId: info.hostId,
        status: "connected",
        message: null,
      });
      const t = runtimes.get(paneId)?.term;
      if (t) ipc.terminalResize(paneId, t.cols, t.rows).catch(() => undefined);
    })
    .catch((e: unknown) => {
      const pane = terminalStore.get().panes[paneId];
      if (!pane || pane.status === "closed") return;
      const message = errorMessage(e);
      if (message.startsWith("cancelled") || /cancel/i.test(message)) {
        patchPane(paneId, { status: "closed", message: "Cancelled" });
      } else {
        patchPane(paneId, { status: "error", message });
        writeSystemLine(paneId, message, "31");
      }
    });
}

export interface OpenOptions {
  /** Add as a split pane to this tab instead of opening a new tab. */
  intoTab?: string;
  direction?: SplitDirection;
}

/** Open a terminal for `target` in a new tab (or split into an existing one). */
export function openTerminal(target: OpenTarget, opts: OpenOptions = {}): Uuid {
  const paneId = uuid();
  const { title, subtitle } = describe(target);
  const pane: Pane = {
    id: paneId,
    target,
    title,
    subtitle,
    protocol: null,
    hostId: target.kind === "host" ? target.host_id : null,
    status: "connecting",
    message: null,
  };
  runtimes.set(paneId, createRuntime(paneId));

  update((s) => {
    const panes = { ...s.panes, [paneId]: pane };
    const existing = opts.intoTab ? s.tabs.find((t) => t.id === opts.intoTab) : undefined;
    if (existing) {
      const tabs = s.tabs.map((t) =>
        t.id === existing.id
          ? {
              ...t,
              paneIds: [...t.paneIds, paneId],
              activePaneId: paneId,
              direction: opts.direction ?? t.direction,
            }
          : t,
      );
      return { ...s, panes, tabs, activeTabId: existing.id };
    }
    const tab: TerminalTab = {
      id: uuid(),
      paneIds: [paneId],
      activePaneId: paneId,
      direction: "row",
      broadcast: false,
      searchOpen: false,
    };
    return { ...s, panes, tabs: [...s.tabs, tab], activeTabId: tab.id };
  });
  startSession(paneId, target);
  return paneId;
}

/** Split the tab's active pane with a fresh session to the same target. */
export function splitActivePane(tabId: string, direction: SplitDirection) {
  const s = terminalStore.get();
  const tab = s.tabs.find((t) => t.id === tabId);
  const pane = tab ? s.panes[tab.activePaneId] : undefined;
  if (!tab || !pane) return;
  openTerminal(pane.target, { intoTab: tabId, direction });
}

/** Replace a finished pane with a new session to the same target. */
export function reconnectPane(paneId: Uuid) {
  const s = terminalStore.get();
  const old = s.panes[paneId];
  const tab = tabOf(s, paneId);
  if (!old || !tab) return;
  const newId = uuid();
  const pane: Pane = { ...old, id: newId, status: "connecting", message: null };
  runtimes.set(newId, createRuntime(newId));
  disposeRuntime(paneId);
  update((st) => {
    const panes = { ...omit(st.panes, paneId), [newId]: pane };
    const tabs = st.tabs.map((t) =>
      t.id === tab.id
        ? {
            ...t,
            paneIds: t.paneIds.map((id) => (id === paneId ? newId : id)),
            activePaneId: t.activePaneId === paneId ? newId : t.activePaneId,
          }
        : t,
    );
    return { ...st, panes, tabs };
  });
  startSession(newId, pane.target);
}

function removePane(paneId: Uuid) {
  disposeRuntime(paneId);
  update((s) => {
    const panes = omit(s.panes, paneId);
    let activeTabId = s.activeTabId;
    const tabs: TerminalTab[] = [];
    for (const t of s.tabs) {
      if (!t.paneIds.includes(paneId)) {
        tabs.push(t);
        continue;
      }
      const paneIds = t.paneIds.filter((id) => id !== paneId);
      const last = paneIds[paneIds.length - 1];
      if (last === undefined) {
        if (activeTabId === t.id) {
          const i = s.tabs.indexOf(t);
          const next = s.tabs[i + 1] ?? s.tabs[i - 1];
          activeTabId = next?.id ?? HOME_TAB;
        }
        continue;
      }
      tabs.push({
        ...t,
        paneIds,
        activePaneId: t.activePaneId === paneId ? last : t.activePaneId,
        broadcast: paneIds.length > 1 && t.broadcast,
      });
    }
    return { ...s, panes, tabs, activeTabId, pendingClose: null };
  });
}

/** Close a pane, asking first when it is still connected and the setting says so. */
export function requestClosePane(paneId: Uuid) {
  const pane = terminalStore.get().panes[paneId];
  if (!pane) return;
  if (pane.status === "connected" && currentSettings?.confirmCloseTab) {
    update((s) => ({ ...s, pendingClose: paneId }));
    return;
  }
  void closePane(paneId);
}

export function confirmPendingClose(accept: boolean) {
  const id = terminalStore.get().pendingClose;
  update((s) => ({ ...s, pendingClose: null }));
  if (id && accept) void closePane(id);
}

export async function closePane(paneId: Uuid) {
  const pane = terminalStore.get().panes[paneId];
  if (!pane) return;
  patchPane(paneId, { status: "closed" });
  removePane(paneId);
  await ipc.terminalClose(paneId).catch(() => undefined);
}

export function closeTab(tabId: string) {
  const tab = terminalStore.get().tabs.find((t) => t.id === tabId);
  if (!tab) return;
  const live = tab.paneIds.some((id) => terminalStore.get().panes[id]?.status === "connected");
  if (live && currentSettings?.confirmCloseTab) {
    update((s) => ({ ...s, pendingClose: tab.activePaneId }));
    return;
  }
  for (const id of tab.paneIds) void closePane(id);
}

export function setActiveTab(tabId: string) {
  update((s) => (s.activeTabId === tabId ? s : { ...s, activeTabId: tabId }));
}

export function setActivePane(paneId: Uuid) {
  update((s) => {
    const tab = tabOf(s, paneId);
    if (!tab || tab.activePaneId === paneId) return s;
    return {
      ...s,
      tabs: s.tabs.map((t) => (t.id === tab.id ? { ...t, activePaneId: paneId } : t)),
    };
  });
}

export function toggleBroadcast(tabId: string) {
  update((s) => ({
    ...s,
    tabs: s.tabs.map((t) => (t.id === tabId ? { ...t, broadcast: !t.broadcast } : t)),
  }));
}

export function setSearchOpen(tabId: string, open: boolean) {
  update((s) => ({
    ...s,
    tabs: s.tabs.map((t) =>
      t.id === tabId && t.searchOpen !== open ? { ...t, searchOpen: open } : t,
    ),
  }));
}

export function setTabDirection(tabId: string, direction: SplitDirection) {
  update((s) => ({
    ...s,
    tabs: s.tabs.map((t) => (t.id === tabId ? { ...t, direction } : t)),
  }));
}

/** Send text to a pane (e.g. a snippet); goes through the same broadcast rules. */
export function sendText(paneId: Uuid, text: string) {
  void sendInput(paneId, text);
}

export function activeSshPane(): Pane | null {
  const s = terminalStore.get();
  const tab = s.tabs.find((t) => t.id === s.activeTabId);
  const pane = tab ? s.panes[tab.activePaneId] : undefined;
  return pane?.protocol === "ssh" && pane.status === "connected" ? pane : null;
}

// ───────────────────────────── events from Rust ─────────────────────────────

function onSessionEvent(ev: SessionEvent) {
  const pane = terminalStore.get().panes[ev.id];
  if (!pane) return;
  switch (ev.type) {
    case "connecting":
      patchPane(ev.id, {
        title: ev.info.title,
        subtitle: ev.info.target,
        protocol: ev.info.protocol,
        hostId: ev.info.hostId,
      });
      break;
    case "connected":
      break;
    case "notice":
      writeSystemLine(ev.id, ev.message);
      break;
    case "exit": {
      const detail =
        ev.signal !== null
          ? `signal ${ev.signal}`
          : ev.code !== null
            ? `exit code ${ev.code}`
            : "session ended";
      patchPane(ev.id, { status: "exited", message: detail });
      writeSystemLine(ev.id, `[${detail}]`);
      break;
    }
    case "error":
      if (pane.status !== "closed") {
        patchPane(ev.id, { status: "error", message: ev.message });
        writeSystemLine(ev.id, ev.message, "31");
      }
      break;
    case "closed":
      if (pane.status === "connected" || pane.status === "connecting") {
        patchPane(ev.id, { status: "exited", message: "Connection closed" });
        writeSystemLine(ev.id, "[connection closed]");
      }
      break;
  }
}

let started = false;
/** Subscribe to Rust session events once for the app lifetime. */
export function startTerminalEvents() {
  if (started) return;
  started = true;
  void ipc.onSessionEvent(onSessionEvent);
}
