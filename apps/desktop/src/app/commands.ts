// Every user-invokable app action in one place: the command palette, the app
// menu and the keyboard all dispatch through this list.

import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { createStore, useStore } from "@/lib/store";
import {
  HOME_TAB,
  activeTab,
  clearBuffer,
  closeTab,
  copySelection,
  cycleTab,
  focusNeighbor,
  isWorkspaceTab,
  movePaneToNewTab,
  openTerminal,
  pasteClipboard,
  requestClosePane,
  resetZoom,
  selectAll,
  setActiveTab,
  isSearchOpen,
  setSearchOpen,
  splitActivePane,
  terminalStore,
  toggleBroadcast,
  toggleSidePanel,
  toggleTabViewMode,
  zoomTab,
} from "@/terminal/store";
import { saveTabAsTemplate } from "@/terminal/workspaces";
import { toast } from "@/components/Snackbar";
import {
  goHome,
  goToNewTab,
  goToSerial,
  goToSection,
  goToSettings,
  goToSftp,
  requestCreate,
} from "./navigation";
import { commandForEvent, registerCommands, tabDigit, type Command } from "./shortcuts";

export const DOCS_URL = "https://github.com/rep0rtDev/termoso#readme";

// ───────────────────────────── palette state ─────────────────────────────

export type PaletteMode = "commands" | "jump";

interface PaletteState {
  open: PaletteMode | null;
}

export const paletteStore = createStore<PaletteState>({ open: null });
export const usePalette = <S>(selector: (s: PaletteState) => S) => useStore(paletteStore, selector);

/** Opens the palette in `mode`; the same shortcut again closes it. */
export function openPalette(mode: PaletteMode) {
  paletteStore.set((s) => ({ open: s.open === mode ? null : mode }));
}

export function closePalette() {
  paletteStore.set({ open: null });
}

// ───────────────────────────── command list ─────────────────────────────

const win = getCurrentWindow();

const hasTab = () => activeTab() !== undefined;
const hasPanes = () => (activeTab()?.paneIds.length ?? 0) > 1;
const hasWorkspace = () => {
  const t = activeTab();
  return t !== undefined && isWorkspaceTab(t);
};

const onActivePane = (f: (paneId: string) => void) => () => {
  const t = activeTab();
  if (t) f(t.activePaneId);
};
const onActiveTab = (f: (tabId: string) => void) => () => {
  const t = activeTab();
  if (t) f(t.id);
};

export const COMMANDS: Command[] = [
  // Navigation
  {
    id: "palette.commands",
    title: "Command palette",
    group: "Navigation",
    keys: ["ctrl+k"],
    run: () => openPalette("commands"),
  },
  {
    id: "palette.jump",
    title: "Jump to host or tab…",
    group: "Navigation",
    keywords: "quick open switch",
    keys: ["ctrl+j"],
    run: () => openPalette("jump"),
  },
  {
    id: "nav.hosts",
    title: "Go to Hosts",
    group: "Navigation",
    keywords: "vaults home",
    keys: ["ctrl+shift+h"],
    run: () => goToSection("hosts"),
  },
  {
    id: "nav.sftp",
    title: "Go to SFTP",
    group: "Navigation",
    keywords: "files transfer",
    keys: ["ctrl+shift+e"],
    run: goToSftp,
  },
  {
    id: "nav.keychain",
    title: "Go to Keychain",
    group: "Navigation",
    keywords: "keys identities",
    keys: [],
    run: () => goToSection("keychain"),
  },
  {
    id: "nav.forwarding",
    title: "Go to Port Forwarding",
    group: "Navigation",
    keywords: "tunnel",
    keys: ["ctrl+p"],
    run: () => goToSection("forwarding"),
  },
  {
    id: "nav.snippets",
    title: "Go to Snippets",
    group: "Navigation",
    keys: ["ctrl+shift+s"],
    run: () => goToSection("snippets"),
  },
  {
    id: "nav.knownHosts",
    title: "Go to Known Hosts",
    group: "Navigation",
    keywords: "fingerprints",
    keys: [],
    run: () => goToSection("knownHosts"),
  },
  {
    id: "nav.logs",
    title: "Go to Session logs",
    group: "Navigation",
    keywords: "recordings",
    keys: [],
    run: () => goToSection("logs"),
  },
  {
    id: "nav.settings",
    title: "Open Settings",
    group: "Navigation",
    keywords: "preferences",
    keys: ["ctrl+,"],
    run: () => goToSettings(),
  },
  {
    id: "nav.keyboard",
    title: "Keyboard shortcuts",
    group: "Navigation",
    keywords: "keys bindings hotkeys",
    keys: [],
    run: () => goToSettings("keyboard"),
  },
  {
    id: "nav.themes",
    title: "Terminal themes & fonts",
    group: "Navigation",
    keywords: "colors appearance",
    keys: [],
    run: () => goToSettings("terminal"),
  },
  {
    id: "nav.docs",
    title: "Open documentation",
    group: "Navigation",
    keywords: "help readme",
    keys: ["f1"],
    run: () => void openUrl(DOCS_URL),
  },
  {
    id: "nav.about",
    title: "About Termoso",
    group: "Navigation",
    keywords: "version",
    keys: [],
    run: () => goToSettings("about"),
  },

  // Tabs
  {
    id: "tab.new",
    title: "New tab",
    group: "Tabs",
    keys: ["ctrl+t"],
    run: goToNewTab,
  },
  {
    id: "tab.local",
    title: "New local terminal",
    group: "Tabs",
    keywords: "shell",
    keys: ["ctrl+l"],
    run: () => openTerminal({ kind: "local" }),
  },
  {
    id: "tab.serial",
    title: "New serial connection",
    group: "Tabs",
    keywords: "serial port com tty usb console",
    keys: ["ctrl+alt+s"],
    run: goToSerial,
  },
  {
    id: "tab.close",
    title: "Close tab",
    group: "Tabs",
    keys: ["ctrl+shift+w"],
    enabled: () => hasTab() || terminalStore.get().activeTabId !== HOME_TAB,
    run: () => {
      const t = activeTab();
      if (t) closeTab(t.id);
      else goHome();
    },
  },
  {
    id: "tab.next",
    title: "Next tab",
    group: "Tabs",
    keys: ["alt+right", "ctrl+tab", "ctrl+pagedown"],
    enabled: () => terminalStore.get().tabs.length > 0,
    run: () => cycleTab(1),
  },
  {
    id: "tab.prev",
    title: "Previous tab",
    group: "Tabs",
    keys: ["alt+left", "ctrl+shift+tab", "ctrl+pageup"],
    enabled: () => terminalStore.get().tabs.length > 0,
    run: () => cycleTab(-1),
  },
  {
    id: "tab.duplicate",
    title: "Duplicate session",
    group: "Tabs",
    keys: [],
    enabled: hasTab,
    run: () => {
      const t = activeTab();
      const pane = t ? terminalStore.get().panes[t.activePaneId] : undefined;
      if (pane) openTerminal(pane.target);
    },
  },

  // Panes
  {
    id: "pane.splitRight",
    title: "Split right",
    group: "Panes",
    keys: ["ctrl+shift+d"],
    enabled: hasTab,
    run: onActiveTab((id) => splitActivePane(id, "row")),
  },
  {
    id: "pane.splitDown",
    title: "Split down",
    group: "Panes",
    keys: ["ctrl+shift+alt+d"],
    enabled: hasTab,
    run: onActiveTab((id) => splitActivePane(id, "column")),
  },
  {
    id: "pane.close",
    title: "Close pane",
    group: "Panes",
    keys: ["ctrl+shift+q"],
    enabled: hasPanes,
    run: onActivePane(requestClosePane),
  },
  {
    id: "pane.detach",
    title: "Move pane to new tab",
    group: "Panes",
    keys: [],
    enabled: hasPanes,
    run: onActivePane(movePaneToNewTab),
  },
  {
    id: "pane.focusLeft",
    title: "Focus pane on the left",
    group: "Panes",
    keys: ["ctrl+alt+left"],
    enabled: hasPanes,
    run: onActivePane((id) => focusNeighbor(id, "left")),
  },
  {
    id: "pane.focusRight",
    title: "Focus pane on the right",
    group: "Panes",
    keys: ["ctrl+alt+right"],
    enabled: hasPanes,
    run: onActivePane((id) => focusNeighbor(id, "right")),
  },
  {
    id: "pane.focusUp",
    title: "Focus pane above",
    group: "Panes",
    keys: ["ctrl+alt+up"],
    enabled: hasPanes,
    run: onActivePane((id) => focusNeighbor(id, "up")),
  },
  {
    id: "pane.focusDown",
    title: "Focus pane below",
    group: "Panes",
    keys: ["ctrl+alt+down"],
    enabled: hasPanes,
    run: onActivePane((id) => focusNeighbor(id, "down")),
  },
  {
    id: "pane.broadcast",
    title: "Toggle broadcast input",
    group: "Panes",
    keywords: "all panes",
    keys: ["ctrl+alt+b"],
    enabled: hasPanes,
    run: onActiveTab(toggleBroadcast),
  },

  // Terminal
  {
    id: "term.copy",
    title: "Copy",
    group: "Terminal",
    keys: ["ctrl+shift+c"],
    enabled: hasTab,
    run: onActivePane(copySelection),
  },
  {
    id: "term.paste",
    title: "Paste",
    group: "Terminal",
    keys: ["ctrl+shift+v"],
    enabled: hasTab,
    run: onActivePane(pasteClipboard),
  },
  {
    id: "term.selectAll",
    title: "Select all",
    group: "Terminal",
    keys: ["ctrl+shift+a"],
    enabled: hasTab,
    run: onActivePane(selectAll),
  },
  {
    id: "term.find",
    title: "Find in terminal",
    group: "Terminal",
    keywords: "search",
    keys: ["ctrl+shift+f"],
    enabled: hasTab,
    run: onActiveTab(() => setSearchOpen(!isSearchOpen(terminalStore.get()))),
  },
  {
    id: "term.clear",
    title: "Clear buffer",
    group: "Terminal",
    keywords: "scrollback",
    keys: ["ctrl+shift+k"],
    enabled: hasTab,
    run: onActivePane(clearBuffer),
  },
  {
    id: "term.zoomIn",
    title: "Zoom in",
    group: "Terminal",
    keys: ["ctrl+=", "ctrl+numadd"],
    enabled: hasTab,
    run: onActiveTab((id) => zoomTab(id, 1)),
  },
  {
    id: "term.zoomOut",
    title: "Zoom out",
    group: "Terminal",
    keys: ["ctrl+-", "ctrl+numsub"],
    enabled: hasTab,
    run: onActiveTab((id) => zoomTab(id, -1)),
  },
  {
    id: "term.zoomReset",
    title: "Reset zoom",
    group: "Terminal",
    keys: ["ctrl+0", "ctrl+num0"],
    enabled: hasTab,
    run: onActiveTab(resetZoom),
  },
  {
    id: "term.sidePanel",
    title: "Toggle side panel",
    group: "Terminal",
    keywords: "snippets history themes info",
    keys: ["ctrl+."],
    enabled: hasTab,
    run: () => toggleSidePanel(),
  },

  // Workspace
  {
    id: "ws.viewMode",
    title: "Toggle split / list view",
    group: "Workspace",
    keys: ["ctrl+alt+m"],
    enabled: hasTab,
    run: onActiveTab(toggleTabViewMode),
  },
  {
    id: "ws.saveTemplate",
    title: "Save as workspace template",
    group: "Workspace",
    keys: ["ctrl+s"],
    enabled: hasTab,
    run: onActiveTab((id) => {
      const tpl = saveTabAsTemplate(id);
      if (tpl) toast(`Workspace “${tpl.name}” saved`);
    }),
  },
  {
    id: "ws.close",
    title: "Close workspace",
    group: "Workspace",
    keys: [],
    enabled: hasWorkspace,
    run: onActiveTab(closeTab),
  },

  // Create
  {
    id: "new.host",
    title: "New host",
    group: "Create",
    keys: ["ctrl+shift+n"],
    run: () => requestCreate("host"),
  },
  {
    id: "new.group",
    title: "New group",
    group: "Create",
    keys: [],
    run: () => requestCreate("group"),
  },
  {
    id: "new.snippet",
    title: "New snippet",
    group: "Create",
    keys: [],
    run: () => requestCreate("snippet"),
  },

  // Window
  {
    id: "window.fullscreen",
    title: "Toggle full screen",
    group: "Window",
    keys: ["f11"],
    run: () => {
      void win.isFullscreen().then((f) => win.setFullscreen(!f));
    },
  },
  {
    id: "window.maximize",
    title: "Maximize / restore window",
    group: "Window",
    keys: [],
    run: () => void win.toggleMaximize(),
  },
  {
    id: "window.minimize",
    title: "Minimize window",
    group: "Window",
    keys: [],
    run: () => void win.minimize(),
  },
  {
    id: "window.quit",
    title: "Quit Termoso",
    group: "Window",
    keywords: "exit close",
    keys: ["ctrl+q"],
    run: () => void win.close(),
  },
];

// ───────────────────────────── keyboard dispatch ─────────────────────────────

/** Ctrl+1 … Ctrl+9: select a session tab by position. */
function selectTabByIndex(n: number): boolean {
  const { tabs } = terminalStore.get();
  const tab = tabs[n - 1];
  if (!tab) return false;
  setActiveTab(tab.id);
  return true;
}

/** Typing in a text field outside the terminal: leave edit shortcuts to the field. */
function inTextField(ev: KeyboardEvent): boolean {
  const el = ev.target;
  if (!(el instanceof HTMLElement) || el.closest(".xterm")) return false;
  return (
    el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el.isContentEditable
  );
}

function inModal(ev: KeyboardEvent): boolean {
  return ev.target instanceof Element && ev.target.closest(".MuiDialog-root") !== null;
}

function onKeyDown(ev: KeyboardEvent) {
  if (ev.defaultPrevented) return;
  if (inModal(ev) && !paletteStore.get().open) return;
  const n = tabDigit(ev);
  if (n !== null) {
    if (selectTabByIndex(n)) ev.preventDefault();
    return;
  }
  const cmd = commandForEvent(ev);
  if (!cmd || (cmd.group === "Terminal" && inTextField(ev))) return;
  if (paletteStore.get().open && !cmd.id.startsWith("palette.")) return;
  ev.preventDefault();
  ev.stopPropagation();
  cmd.run();
}

/** Register the command list and install the window-level key handler. */
export function startCommands(): () => void {
  registerCommands(COMMANDS);
  window.addEventListener("keydown", onKeyDown, true);
  return () => window.removeEventListener("keydown", onKeyDown, true);
}
