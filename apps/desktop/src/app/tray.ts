// Tray icon + close-to-tray. Sessions and port forwards live in Rust, so a
// hidden window costs nothing and keeps them running; the tray is the way
// back (and the way out). No tray → closing the window quits as before.
import { defaultWindowIcon } from "@tauri-apps/api/app";
import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { TrayIcon } from "@tauri-apps/api/tray";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { appQuit } from "@/ipc/commands";
import type { Settings } from "@/ipc/types";
import { onLanguageChange, tr } from "@/i18n";

const TRAY_ID = "termoso";

let wanted = false;
let tray: TrayIcon | null = null;
let showItem: MenuItem | null = null;
let quitItem: MenuItem | null = null;
let pending: Promise<void> = Promise.resolve();

/** Un-hide, un-minimise and focus the main window (tray, deep links, second launch). */
export async function bringToFront(): Promise<void> {
  const win = getCurrentWindow();
  try {
    await win.show();
    await win.unminimize();
    await win.setFocus();
  } catch {
    // Window already gone (quitting).
  }
}

export function quitApp(): void {
  void appQuit();
}

async function create(): Promise<void> {
  const show = await MenuItem.new({
    id: "tray.show",
    text: tr("Show Termoso"),
    action: () => void bringToFront(),
  });
  const quit = await MenuItem.new({
    id: "tray.quit",
    text: tr("Quit Termoso"),
    action: quitApp,
  });
  const menu = await Menu.new({
    items: [show, await PredefinedMenuItem.new({ item: "Separator" }), quit],
  });
  const icon = await defaultWindowIcon();
  const created = await TrayIcon.new({
    id: TRAY_ID,
    icon: icon ?? undefined,
    menu,
    tooltip: "Termoso",
    showMenuOnLeftClick: false,
    action: (ev) => {
      if (ev.type === "Click" && ev.button === "Left" && ev.buttonState === "Up") {
        void bringToFront();
      }
    },
  });
  showItem = show;
  quitItem = quit;
  tray = created;
}

async function destroy(): Promise<void> {
  const t = tray;
  tray = null;
  showItem = null;
  quitItem = null;
  await t?.close();
}

async function reconcile(): Promise<void> {
  if (wanted && !tray) {
    try {
      await create();
    } catch (e) {
      // No tray host on this desktop: closing the window keeps quitting.
      console.warn("tray unavailable", e);
      await destroy();
    }
  } else if (!wanted && tray) {
    await destroy();
  }
}

export function applyTraySettings(s: Pick<Settings, "minimizeToTray">): void {
  wanted = s.minimizeToTray;
  pending = pending.then(reconcile, reconcile);
}

/** Whether closing the window should hide it instead of quitting. */
export function closeHidesToTray(): boolean {
  return wanted && tray !== null;
}

export function startTray(): () => void {
  const win = getCurrentWindow();
  let stopped = false;
  let unlistenClose: (() => void) | null = null;
  void win
    .onCloseRequested(async (ev) => {
      if (!closeHidesToTray()) return;
      ev.preventDefault();
      await win.hide();
    })
    .then((un) => {
      if (stopped) un();
      else unlistenClose = un;
    });
  const unlistenLanguage = onLanguageChange(() => {
    void showItem?.setText(tr("Show Termoso"));
    void quitItem?.setText(tr("Quit Termoso"));
  });
  return () => {
    stopped = true;
    unlistenClose?.();
    unlistenLanguage();
    wanted = false;
    pending = pending.then(destroy, destroy);
  };
}
