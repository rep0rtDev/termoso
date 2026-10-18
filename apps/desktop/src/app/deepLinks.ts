import { listen } from "@tauri-apps/api/event";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { bringToFront } from "@/app/tray";
import { toast } from "@/components/Snackbar";
import { hostsList, launchFiles } from "@/ipc/commands";
import { errorMessage } from "@/ipc/types";
import { parseLink, quickLabel } from "@/hosts/links";
import { handleSsoLink } from "@/account/sso";
import { isSsoLink, parseSsoLink } from "@/account/ssoLink";
import { openTerminal } from "@/terminal/store";
import {
  goToSection,
  goToSettings,
  goToSftp,
  requestCreate,
  requestOpenFile,
} from "@/app/navigation";
import { tr } from "@/i18n";

/** URLs already handled — the launch batch can be reported twice on some platforms. */
const seen = new Set<string>();

async function openLink(url: string) {
  if (seen.has(url)) return;
  seen.add(url);
  setTimeout(() => seen.delete(url), 2_000);

  if (isSsoLink(url)) {
    void bringToFront();
    if (!parseSsoLink(url)) {
      toast(tr("Ignored a malformed sign-in link"), "warning");
      return;
    }
    try {
      await handleSsoLink(url);
    } catch (e) {
      toast(errorMessage(e), "error");
    }
    return;
  }
  const link = parseLink(url);
  if (link.kind === "unsupported") {
    toast(tr("Can't open link: {url}", { url }), "warning");
    return;
  }
  void bringToFront();
  if (link.kind === "action") {
    if (link.action === "new") requestCreate("host");
    else if (link.action === "local") openTerminal({ kind: "local" });
    else goToSftp();
    return;
  }
  if (link.kind === "quick") {
    openTerminal(link.target);
    toast(tr("Connecting to {quickLabel}", { quickLabel: quickLabel(link.target) }), "info");
    return;
  }
  if (link.kind === "live") {
    openTerminal({ kind: "live", link: link.link });
    toast(tr("Joining multiplayer session"), "info");
    return;
  }
  try {
    const host = (await hostsList(null)).find((h) => h.id === link.hostId);
    if (!host) {
      toast(tr("This link points to a host that isn't in your vaults"), "warning");
      return;
    }
    openTerminal({ kind: "host", host_id: host.id, vault_id: host.vaultId });
  } catch (e) {
    toast(errorMessage(e), "error");
  }
}

const fileName = (path: string) => (path.split(/[\\/]/).pop() ?? path).toLowerCase();

/** Colour schemes: iTerm2, Windows Terminal, Alacritty, Kitty, Ghostty, Gogh. */
export const isThemeFile = (path: string) =>
  /\.(itermcolors|json|toml|ya?ml|conf)$/.test(fileName(path));
/** Private keys the keychain can import. */
export const isKeyFile = (path: string) =>
  /\.(pem|ppk|key)$/.test(fileName(path)) || /^id_[a-z0-9]+$/.test(fileName(path));
/** OpenSSH client configuration. */
export const isSshConfigFile = (path: string) => /^(ssh_)?config$/.test(fileName(path));

/**
 * A file the OS handed to the app (file association, Open with…, drop on the
 * Dock icon): routed to the page that imports that kind of file.
 */
export function openFilePath(path: string) {
  void bringToFront();
  if (isThemeFile(path)) goToSettings("terminal");
  else if (isKeyFile(path)) goToSection("keychain");
  else if (isSshConfigFile(path)) goToSection("hosts");
  else {
    toast(tr("Can't open file: {name}", { name: fileName(path) }), "warning");
    return;
  }
  requestOpenFile(path);
}

/**
 * Open `termoso://host/<id>`, `termoso://sso?flow=…`, `ssh://…` and `telnet://…` links handed to the app —
 * at launch (`getCurrent`) and while running (second instance forwards its argv).
 */
export function startDeepLinks(): () => void {
  let stopped = false;
  let unlisten: (() => void) | null = null;
  void getCurrent()
    .then((urls) => {
      if (!stopped) urls?.forEach((u) => void openLink(u));
    })
    .catch(() => undefined);
  void onOpenUrl((urls) => urls.forEach((u) => void openLink(u)))
    .then((un) => {
      if (stopped) un();
      else unlisten = un;
    })
    .catch(() => undefined);
  void launchFiles()
    .then((paths) => {
      if (!stopped) paths.forEach(openFilePath);
    })
    .catch(() => undefined);
  const stops: (() => void)[] = [];
  void listen<string[]>("files-opened", (e) => e.payload.forEach(openFilePath))
    .then((un) => (stopped ? un() : stops.push(un)))
    .catch(() => undefined);
  // macOS Dock menu picks arrive as termoso:// links from Rust.
  void listen<string>("quick-action", (e) => void openLink(e.payload))
    .then((un) => (stopped ? un() : stops.push(un)))
    .catch(() => undefined);
  return () => {
    stopped = true;
    unlisten?.();
    stops.forEach((un) => un());
  };
}
