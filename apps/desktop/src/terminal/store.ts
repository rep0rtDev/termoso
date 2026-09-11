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
import {
  readText as readNativeClipboard,
  writeText as writeNativeClipboard,
} from "@tauri-apps/plugin-clipboard-manager";
import * as ipc from "@/ipc/commands";
import type {
  OpenTarget,
  SessionEvent,
  SessionInfo,
  Settings,
  SshAlgorithms,
  Uuid,
} from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { createStore, omit, useStore } from "@/lib/store";
import { terminalFontStack } from "./fonts";
import {
  MAX_PANES,
  leaf,
  leaves,
  neighbor,
  removeLeaf,
  replaceLeaf,
  setRatio,
  splitLeaf,
  type Rect,
  type Side,
  type SplitDirection,
  type SplitNode,
} from "./layout";
import { resolveTerminalTheme, toXtermTheme, type TerminalTheme } from "./themes";

export type { SplitDirection, SplitNode } from "./layout";

export type PaneStatus = "connecting" | "connected" | "exited" | "error" | "closed";

export interface Pane {
  id: Uuid;
  target: OpenTarget;
  title: string;
  subtitle: string;
  protocol: SessionInfo["protocol"] | null;
  hostId: Uuid | null;
  algorithms: SshAlgorithms | null;
  /** Jump hosts the connection went through, outermost first. */
  via: string[];
  /** Colour scheme configured on the host; `null` follows the app setting. */
  hostTheme: string | null;
  startedAt: string | null;
  status: PaneStatus;
  message: string | null;
}

export interface TerminalTab {
  id: string;
  layout: SplitNode;
  /** Leaves of `layout` in reading order (kept in sync by the store). */
  paneIds: Uuid[];
  activePaneId: Uuid;
  broadcast: boolean;
  searchOpen: boolean;
  /** Font scale, 1 = the settings font size. */
  zoom: number;
  /** Colour scheme picked from the side panel for this tab; `null` = host / app default. */
  themeOverride: string | null;
}

export type SidePanelTab = "snippets" | "history" | "themes" | "info";

export const HOME_TAB = "home";

export interface TerminalState {
  tabs: TerminalTab[];
  panes: Record<Uuid, Pane>;
  activeTabId: string;
  /** Multi-line paste waiting for the user's confirmation. */
  pendingPaste: { paneId: Uuid; text: string } | null;
  /** Pane whose close needs confirming. */
  pendingClose: Uuid | null;
  /** Right-click menu for a pane, at viewport coordinates. */
  contextMenu: { paneId: Uuid; left: number; top: number } | null;
  /** Open section of the terminal side panel (shared by all tabs). */
  sidePanel: SidePanelTab | null;
}

export const terminalStore = createStore<TerminalState>({
  tabs: [],
  panes: {},
  activeTabId: HOME_TAB,
  pendingPaste: null,
  pendingClose: null,
  contextMenu: null,
  sidePanel: null,
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

function patchTab(id: string, patch: Partial<TerminalTab>) {
  update((s) => ({
    ...s,
    tabs: s.tabs.map((t) => (t.id === id ? { ...t, ...patch } : t)),
  }));
}

function tabOf(s: TerminalState, paneId: Uuid) {
  return s.tabs.find((t) => t.paneIds.includes(paneId));
}

export function activeTab(s: TerminalState = terminalStore.get()): TerminalTab | undefined {
  return s.tabs.find((t) => t.id === s.activeTabId);
}

function withLayout(tab: TerminalTab, layout: SplitNode): TerminalTab {
  return { ...tab, layout, paneIds: leaves(layout) };
}

// ───────────────────────────── settings / theme ─────────────────────────────

const fontFamily = (settings: Settings | null) => terminalFontStack(settings?.terminalFontFamily);

export const ZOOM_STEPS = [0.5, 0.6, 0.7, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3];

const fontSize = (zoom: number) =>
  Math.max(6, Math.round((currentSettings?.terminalFontSize ?? 13) * zoom));

/** Scheme id a pane renders with: tab override → host scheme → app setting. */
export function paneThemeId(pane: Pane | undefined, tab: TerminalTab | undefined): string {
  return tab?.themeOverride ?? pane?.hostTheme ?? currentSettings?.terminalTheme ?? "auto";
}

export function paneTheme(paneId: Uuid, s: TerminalState = terminalStore.get()): TerminalTheme {
  return resolveTerminalTheme(paneThemeId(s.panes[paneId], tabOf(s, paneId)), currentScheme);
}

function applyPaneLook(paneId: Uuid, s: TerminalState = terminalStore.get()) {
  const rt = runtimes.get(paneId);
  if (!rt) return;
  const tab = tabOf(s, paneId);
  rt.term.options.theme = toXtermTheme(paneTheme(paneId, s));
  const size = fontSize(tab?.zoom ?? 1);
  if (rt.term.options.fontSize !== size) {
    rt.term.options.fontSize = size;
    if (rt.opened) fitPane(paneId);
  }
  if (rt.opened) rt.term.refresh(0, rt.term.rows - 1);
}

function applyTabLook(tabId: string) {
  const s = terminalStore.get();
  const tab = s.tabs.find((t) => t.id === tabId);
  for (const id of tab?.paneIds ?? []) applyPaneLook(id, s);
}

export function applyTerminalSettings(settings: Settings) {
  currentSettings = settings;
  const s = terminalStore.get();
  for (const [id, rt] of runtimes) {
    rt.term.options.fontFamily = fontFamily(settings);
    rt.term.options.lineHeight = settings.terminalLineHeight;
    rt.term.options.cursorBlink = settings.cursorBlink;
    rt.term.options.cursorStyle = settings.cursorStyle;
    rt.term.options.scrollback = settings.scrollback;
    rt.term.options.theme = toXtermTheme(paneTheme(id, s));
    rt.term.options.fontSize = fontSize(tabOf(s, id)?.zoom ?? 1);
    if (rt.opened) rt.fit.fit();
  }
}

export function applyTerminalScheme(scheme: "dark" | "light") {
  currentScheme = scheme;
  const s = terminalStore.get();
  for (const [id, rt] of runtimes) rt.term.options.theme = toXtermTheme(paneTheme(id, s));
}

/** Pick a colour scheme for every pane of the tab; `null` returns to the host / app default. */
export function setTabTheme(tabId: string, themeId: string | null) {
  patchTab(tabId, { themeOverride: themeId });
  applyTabLook(tabId);
}

export function zoomTab(tabId: string, step: 1 | -1) {
  const tab = terminalStore.get().tabs.find((t) => t.id === tabId);
  if (!tab) return;
  const i = ZOOM_STEPS.findIndex((z) => Math.abs(z - tab.zoom) < 1e-6);
  const next = ZOOM_STEPS[(i === -1 ? ZOOM_STEPS.indexOf(1) : i) + step];
  if (next === undefined) return;
  patchTab(tabId, { zoom: next });
  applyTabLook(tabId);
}

export function resetZoom(tabId: string) {
  patchTab(tabId, { zoom: 1 });
  applyTabLook(tabId);
}

/** Wipe scrollback and the screen; the shell prompt is redrawn on the next output. */
export function clearBuffer(paneId: Uuid) {
  runtimes.get(paneId)?.term.clear();
}

export function selectAll(paneId: Uuid) {
  runtimes.get(paneId)?.term.selectAll();
}

// ───────────────────────────── clipboard ─────────────────────────────

// Native clipboard first: WebKitGTK only allows `navigator.clipboard.readText()` inside a paste event.
async function copyText(text: string) {
  try {
    await writeNativeClipboard(text);
  } catch {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // clipboard unavailable; selection stays in xterm
    }
  }
}

async function readClipboard(): Promise<string> {
  try {
    return await readNativeClipboard();
  } catch {
    try {
      return await navigator.clipboard.readText();
    } catch {
      return "";
    }
  }
}

export function copySelection(paneId: Uuid) {
  const term = runtimes.get(paneId)?.term;
  if (term?.hasSelection()) {
    void copyText(term.getSelection());
    term.clearSelection();
  }
}

export function pasteClipboard(paneId: Uuid) {
  void readClipboard().then((t) => pasteInto(paneId, t));
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

/**
 * OSC 52 — programs on the remote side (tmux, vim, `osc52.sh`) put text on
 * the local clipboard. Writes only: a query (`?`) is swallowed rather than
 * answered, so no remote program can read the clipboard.
 */
function handleOsc52(data: string): boolean {
  const sep = data.indexOf(";");
  const payload = sep === -1 ? data : data.slice(sep + 1);
  if (payload === "?" || payload === "") return true;
  try {
    const bin = atob(payload.replace(/\s+/g, ""));
    const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
    const text = new TextDecoder().decode(bytes);
    if (text) void copyText(text);
  } catch {
    // malformed base64 — ignore
  }
  return true;
}

// ───────────────────────────── bell ─────────────────────────────

let audio: AudioContext | null = null;

function beep() {
  try {
    audio ??= new AudioContext();
    const osc = audio.createOscillator();
    const gain = audio.createGain();
    osc.type = "sine";
    osc.frequency.value = 880;
    gain.gain.setValueAtTime(0.08, audio.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.0001, audio.currentTime + 0.12);
    osc.connect(gain).connect(audio.destination);
    osc.start();
    osc.stop(audio.currentTime + 0.12);
  } catch {
    // no audio output available
  }
}

// ───────────────────────────── xterm runtime ─────────────────────────────

/** Keys handled by the app (see hotkeys.ts) that xterm must not turn into input. */
function isAppShortcut(ev: KeyboardEvent): boolean {
  const ctrl = ev.ctrlKey || ev.metaKey;
  if (!ctrl) return false;
  if (ev.shiftKey && ["KeyK", "KeyF", "KeyD", "KeyW", "KeyB"].includes(ev.code)) {
    return true;
  }
  if (
    ["Equal", "Minus", "Digit0", "NumpadAdd", "NumpadSubtract", "Numpad0", "Tab"].includes(ev.code)
  ) {
    return true;
  }
  return ev.altKey && ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(ev.code);
}

function createRuntime(paneId: Uuid): Runtime {
  const term = new Terminal({
    allowProposedApi: true,
    cursorBlink: currentSettings?.cursorBlink ?? true,
    cursorStyle: currentSettings?.cursorStyle ?? "bar",
    fontSize: fontSize(1),
    fontFamily: fontFamily(currentSettings),
    fontWeight: "400",
    fontWeightBold: "600",
    lineHeight: currentSettings?.terminalLineHeight ?? 1,
    letterSpacing: 0,
    scrollback: currentSettings?.scrollback ?? 10_000,
    theme: toXtermTheme(resolveTerminalTheme(currentSettings?.terminalTheme, currentScheme)),
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
    term.onBell(() => {
      if (currentSettings?.terminalBell) beep();
    }),
    term.parser.registerOscHandler(52, handleOsc52),
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
    return !isAppShortcut(ev);
  });

  container.addEventListener("contextmenu", (ev) => {
    ev.preventDefault();
    if (!currentSettings?.pasteOnRightClick) {
      update((s) => ({ ...s, contextMenu: { paneId, left: ev.clientX, top: ev.clientY } }));
      return;
    }
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

export function closeContextMenu() {
  update((s) => (s.contextMenu ? { ...s, contextMenu: null } : s));
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

/** Rows × cols of the live grid. */
export function paneSize(paneId: Uuid): { cols: number; rows: number } | null {
  const t = runtimes.get(paneId)?.term;
  return t ? { cols: t.cols, rows: t.rows } : null;
}

export function paneHasSelection(paneId: Uuid): boolean {
  return runtimes.get(paneId)?.term.hasSelection() ?? false;
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
        algorithms: info.algorithms,
        via: info.via,
        hostTheme: info.colorScheme,
        startedAt: info.startedAt,
        status: "connected",
        message: null,
      });
      applyPaneLook(paneId);
      const t = runtimes.get(paneId)?.term;
      if (t) ipc.terminalResize(paneId, t.cols, t.rows).catch(() => undefined);
    })
    .catch((e: unknown) => {
      const pane = terminalStore.get().panes[paneId];
      if (!pane || pane.status === "closed" || pane.status === "error") return;
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
  /** Open the tab without switching to it ("Add to workspace"). */
  background?: boolean;
}

function newPane(paneId: Uuid, target: OpenTarget): Pane {
  const { title, subtitle } = describe(target);
  return {
    id: paneId,
    target,
    title,
    subtitle,
    protocol: null,
    hostId: target.kind === "host" ? target.host_id : null,
    algorithms: null,
    via: [],
    hostTheme: null,
    startedAt: null,
    status: "connecting",
    message: null,
  };
}

/**
 * Open a terminal for `target` in a new tab, or split the target tab's active
 * pane. Returns `null` when the tab already holds `MAX_PANES` panes.
 */
export function openTerminal(target: OpenTarget, opts: OpenOptions = {}): Uuid | null {
  const state = terminalStore.get();
  const existing = opts.intoTab ? state.tabs.find((t) => t.id === opts.intoTab) : undefined;
  if (existing && existing.paneIds.length >= MAX_PANES) return null;

  const paneId = uuid();
  const pane = newPane(paneId, target);
  runtimes.set(paneId, createRuntime(paneId));

  update((s) => {
    const panes = { ...s.panes, [paneId]: pane };
    if (existing) {
      const tabs = s.tabs.map((t) =>
        t.id === existing.id
          ? {
              ...withLayout(
                t,
                splitLeaf(t.layout, t.activePaneId, paneId, opts.direction ?? "row", uuid()),
              ),
              activePaneId: paneId,
            }
          : t,
      );
      return { ...s, panes, tabs, activeTabId: opts.background ? s.activeTabId : existing.id };
    }
    const tab: TerminalTab = {
      id: uuid(),
      layout: leaf(paneId),
      paneIds: [paneId],
      activePaneId: paneId,
      broadcast: false,
      searchOpen: false,
      zoom: 1,
      themeOverride: null,
    };
    return {
      ...s,
      panes,
      tabs: [...s.tabs, tab],
      activeTabId: opts.background ? s.activeTabId : tab.id,
    };
  });
  applyPaneLook(paneId);
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

export function setSplitRatio(tabId: string, splitId: string, ratio: number) {
  update((s) => ({
    ...s,
    tabs: s.tabs.map((t) =>
      t.id === tabId ? withLayout(t, setRatio(t.layout, splitId, ratio)) : t,
    ),
  }));
}

/** Move keyboard focus to the pane next to the active one. */
export function focusNeighbor(tabId: string, side: Side) {
  const s = terminalStore.get();
  const tab = s.tabs.find((t) => t.id === tabId);
  if (!tab || tab.paneIds.length < 2) return;
  const rectOf = (id: Uuid): Rect | null => {
    const el = runtimes.get(id)?.container;
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { left: r.left, top: r.top, width: r.width, height: r.height };
  };
  const from = rectOf(tab.activePaneId);
  if (!from) return;
  const others = new Map<Uuid, Rect>();
  for (const id of tab.paneIds) {
    if (id === tab.activePaneId) continue;
    const r = rectOf(id);
    if (r) others.set(id, r);
  }
  const next = neighbor(from, others, side);
  if (next) {
    setActivePane(next);
    focusPane(next);
  }
}

/** Detach a pane from its split tab into a tab of its own. */
export function movePaneToNewTab(paneId: Uuid) {
  const s = terminalStore.get();
  const tab = tabOf(s, paneId);
  if (!tab || tab.paneIds.length < 2) return;
  update((st) => {
    const tabs: TerminalTab[] = [];
    let fresh: TerminalTab | null = null;
    for (const t of st.tabs) {
      if (t.id !== tab.id) {
        tabs.push(t);
        continue;
      }
      const rest = removeLeaf(t.layout, paneId);
      if (rest) {
        const kept = withLayout(t, rest);
        tabs.push({
          ...kept,
          activePaneId: kept.paneIds.includes(t.activePaneId)
            ? t.activePaneId
            : (kept.paneIds[0] ?? t.activePaneId),
          broadcast: kept.paneIds.length > 1 && t.broadcast,
        });
      }
      fresh = {
        id: uuid(),
        layout: leaf(paneId),
        paneIds: [paneId],
        activePaneId: paneId,
        broadcast: false,
        searchOpen: false,
        zoom: t.zoom,
        themeOverride: t.themeOverride,
      };
      tabs.push(fresh);
    }
    return fresh ? { ...st, tabs, activeTabId: fresh.id } : st;
  });
  requestAnimationFrame(() => fitPane(paneId));
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
            ...withLayout(t, replaceLeaf(t.layout, paneId, newId)),
            activePaneId: t.activePaneId === paneId ? newId : t.activePaneId,
          }
        : t,
    );
    return { ...st, panes, tabs };
  });
  applyPaneLook(newId);
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
      const rest = removeLeaf(t.layout, paneId);
      if (rest === null) {
        if (activeTabId === t.id) {
          const i = s.tabs.indexOf(t);
          const next = s.tabs[i + 1] ?? s.tabs[i - 1];
          activeTabId = next?.id ?? HOME_TAB;
        }
        continue;
      }
      const kept = withLayout(t, rest);
      const last = kept.paneIds[kept.paneIds.length - 1] ?? t.activePaneId;
      tabs.push({
        ...kept,
        activePaneId: t.activePaneId === paneId ? last : t.activePaneId,
        broadcast: kept.paneIds.length > 1 && t.broadcast,
      });
    }
    const contextMenu = s.contextMenu?.paneId === paneId ? null : s.contextMenu;
    return { ...s, panes, tabs, activeTabId, pendingClose: null, contextMenu };
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

/** Switch to the tab `offset` places away, wrapping around. */
export function cycleTab(offset: number) {
  const s = terminalStore.get();
  if (s.tabs.length === 0) return;
  const i = s.tabs.findIndex((t) => t.id === s.activeTabId);
  const next =
    s.tabs[((((i === -1 ? 0 : i) + offset) % s.tabs.length) + s.tabs.length) % s.tabs.length];
  if (next) setActiveTab(next.id);
}

/** Reorder: place `tabId` where `beforeId` is (or at the end when `null`). */
export function moveTab(tabId: string, beforeId: string | null) {
  update((s) => {
    if (tabId === beforeId) return s;
    const moving = s.tabs.find((t) => t.id === tabId);
    if (!moving) return s;
    const rest = s.tabs.filter((t) => t.id !== tabId);
    const at = beforeId === null ? rest.length : rest.findIndex((t) => t.id === beforeId);
    if (at === -1) return s;
    const tabs = [...rest.slice(0, at), moving, ...rest.slice(at)];
    return { ...s, tabs };
  });
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

export function setSidePanel(panel: SidePanelTab | null) {
  update((s) => (s.sidePanel === panel ? s : { ...s, sidePanel: panel }));
}

/** Open the side panel on `panel`, or close it when that section is already showing. */
export function toggleSidePanel(panel: SidePanelTab = "snippets") {
  update((s) => ({ ...s, sidePanel: s.sidePanel === null ? panel : null }));
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
        hostTheme: ev.info.colorScheme,
      });
      applyPaneLook(ev.id);
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
      if (pane.status !== "closed" && pane.status !== "error") {
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
