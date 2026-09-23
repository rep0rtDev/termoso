import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toast } from "@/components/Snackbar";
import { hostsList } from "@/ipc/commands";
import { errorMessage } from "@/ipc/types";
import { parseLink, quickLabel } from "@/hosts/links";
import { handleSsoLink } from "@/account/sso";
import { isSsoLink, parseSsoLink } from "@/account/ssoLink";
import { openTerminal } from "@/terminal/store";

/** URLs already handled — the launch batch can be reported twice on some platforms. */
const seen = new Set<string>();

async function openLink(url: string) {
  if (seen.has(url)) return;
  seen.add(url);
  setTimeout(() => seen.delete(url), 2_000);

  if (isSsoLink(url)) {
    const win = getCurrentWindow();
    void win.unminimize().then(() => win.setFocus());
    if (!parseSsoLink(url)) {
      toast("Ignored a malformed sign-in link", "warning");
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
    toast(`Can't open link: ${url}`, "warning");
    return;
  }
  const win = getCurrentWindow();
  void win.unminimize().then(() => win.setFocus());
  if (link.kind === "quick") {
    openTerminal(link.target);
    toast(`Connecting to ${quickLabel(link.target)}`, "info");
    return;
  }
  if (link.kind === "live") {
    openTerminal({ kind: "live", link: link.link });
    toast("Joining multiplayer session", "info");
    return;
  }
  try {
    const host = (await hostsList(null)).find((h) => h.id === link.hostId);
    if (!host) {
      toast("This link points to a host that isn't in your vaults", "warning");
      return;
    }
    openTerminal({ kind: "host", host_id: host.id, vault_id: host.vaultId });
  } catch (e) {
    toast(errorMessage(e), "error");
  }
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
  return () => {
    stopped = true;
    unlisten?.();
  };
}
