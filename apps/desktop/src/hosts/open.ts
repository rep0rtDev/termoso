import { goToSftp } from "@/app/navigation";
import { hostProtocols, type HostCard } from "@/ipc/types";
import { openWebDavForHost } from "@/sftp/store";
import { openTerminal } from "@/terminal/store";

/** Connect a saved host the way its sections allow: a terminal, or Files for WebDAV-only hosts. */
export function openHost(h: Pick<HostCard, "id" | "label" | "protocol" | "telnetPort">) {
  if (hostProtocols(h).length === 0) {
    openWebDavForHost(h.id, h.label);
    goToSftp();
    return;
  }
  openTerminal({ kind: "host", host_id: h.id });
}
