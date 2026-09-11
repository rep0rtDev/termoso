// Window-level terminal shortcuts. xterm lets these through (see
// `isAppShortcut` in store.ts); they act on the active tab only.

import {
  activeTab,
  clearBuffer,
  cycleTab,
  focusNeighbor,
  requestClosePane,
  resetZoom,
  setSearchOpen,
  splitActivePane,
  toggleSidePanel,
  zoomTab,
} from "./store";
import type { Side } from "./layout";

const ARROWS: Partial<Record<string, Side>> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
};

function onKeyDown(ev: KeyboardEvent) {
  const ctrl = ev.ctrlKey || ev.metaKey;
  if (!ctrl) return;
  const tab = activeTab();
  if (!tab) return;

  const arrow = ev.altKey ? ARROWS[ev.code] : undefined;
  if (arrow) {
    ev.preventDefault();
    focusNeighbor(tab.id, arrow);
    return;
  }

  switch (ev.code) {
    case "Equal":
    case "NumpadAdd":
      ev.preventDefault();
      zoomTab(tab.id, 1);
      return;
    case "Minus":
    case "NumpadSubtract":
      ev.preventDefault();
      zoomTab(tab.id, -1);
      return;
    case "Digit0":
    case "Numpad0":
      ev.preventDefault();
      resetZoom(tab.id);
      return;
    case "Tab":
      ev.preventDefault();
      cycleTab(ev.shiftKey ? -1 : 1);
      return;
    default:
  }

  if (!ev.shiftKey) return;
  switch (ev.code) {
    case "KeyF":
      ev.preventDefault();
      setSearchOpen(tab.id, true);
      break;
    case "KeyD":
      ev.preventDefault();
      splitActivePane(tab.id, ev.altKey ? "column" : "row");
      break;
    case "KeyW":
      ev.preventDefault();
      requestClosePane(tab.activePaneId);
      break;
    case "KeyK":
      ev.preventDefault();
      clearBuffer(tab.activePaneId);
      break;
    case "KeyB":
      ev.preventDefault();
      toggleSidePanel();
      break;
    default:
  }
}

let installed = false;
/** Install the shortcuts once for the app lifetime. */
export function startTerminalHotkeys() {
  if (installed) return;
  installed = true;
  window.addEventListener("keydown", onKeyDown);
}
