import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import CloudRoundedIcon from "@mui/icons-material/CloudRounded";
import SwapHorizRoundedIcon from "@mui/icons-material/SwapHorizRounded";
import { Button } from "@mui/material";
import type { MenuAction } from "@/components/ui";
import { goToSftp, requestForwardingRule } from "@/app/navigation";
import { openSftpForHost, openWebDavForHost } from "@/sftp/store";
import { openTerminal } from "@/terminal/store";
import type { ConnectProtocol, HostCard, Uuid } from "@/ipc/types";

/** The bits of a saved host an open needs: its id, the vault it came from, a title. */
export type HostRef = Pick<HostCard, "id" | "vaultId" | "label">;

export const PROTOCOL_NAME: Record<ConnectProtocol, string> = {
  ssh: "SSH",
  mosh: "Mosh",
  telnet: "Telnet",
};

/** What the primary Connect opens: a terminal protocol, or the WebDAV share in Files. */
export type ConnectTarget = ConnectProtocol | "webdav";

/** Opens the host's WebDAV share in the files view. */
export function openWebDav(h: HostRef) {
  openWebDavForHost(h.id, h.label, h.vaultId);
  goToSftp();
}

/** Opens a saved host the way its primary section dictates. */
export function connectTo(h: HostRef, target: ConnectTarget | null) {
  if (target === "webdav") openWebDav(h);
  else openTerminal({ kind: "host", host_id: h.id, vault_id: h.vaultId, protocol: target });
}

/**
 * Termius' `Connect ▸` submenu for a saved host: one entry per protocol
 * section, then SFTP and port forwarding, which need the SSH one, and the
 * WebDAV share when the host has one.
 */
export function connectActions(
  h: HostRef,
  protocols: ConnectProtocol[],
  webdav = false,
): MenuAction[] {
  const ssh = protocols.includes("ssh");
  const needsSsh = ssh ? "" : " (needs an SSH section)";
  return [
    ...protocols.map((p) => ({
      label: `with ${PROTOCOL_NAME[p]}`,
      icon: <TerminalRoundedIcon fontSize="small" />,
      onClick: () =>
        openTerminal({ kind: "host", host_id: h.id, vault_id: h.vaultId, protocol: p }),
    })),
    {
      label: `with SFTP${needsSsh}`,
      icon: <FolderCopyRoundedIcon fontSize="small" />,
      disabled: !ssh,
      divider: protocols.length > 0,
      onClick: () => {
        openSftpForHost(h.id, h.label, h.vaultId);
        goToSftp();
      },
    },
    ...(webdav
      ? [
          {
            label: "with WebDAV",
            icon: <CloudRoundedIcon fontSize="small" />,
            onClick: () => openWebDav(h),
          },
        ]
      : []),
    {
      label: `Port forwarding${needsSsh}`,
      icon: <SwapHorizRoundedIcon fontSize="small" />,
      disabled: !ssh,
      onClick: () => requestForwardingRule(h.id),
    },
  ];
}

/** Full-width primary Connect at the bottom of Host Details, as in Termius. */
export function ConnectButton({
  hostId,
  vaultId,
  label = "",
  target,
  disabled,
  onClick,
}: {
  hostId: Uuid | null;
  vaultId: Uuid;
  label?: string;
  /** Section to open; `null` lets the host pick (SSH, or Mosh when enabled). */
  target?: ConnectTarget | null;
  disabled?: boolean;
  /** Runs instead of opening the saved host (e.g. save first, then connect). */
  onClick?: () => void;
}) {
  const webdav = target === "webdav";
  return (
    <Button
      fullWidth
      variant="contained"
      size="large"
      disabled={disabled}
      startIcon={webdav ? <FolderCopyRoundedIcon /> : <PlayArrowRoundedIcon />}
      onClick={
        onClick ??
        (() => {
          if (hostId) connectTo({ id: hostId, vaultId, label }, target ?? null);
        })
      }
      sx={{ height: 40, borderRadius: 2.5, fontWeight: 600 }}
    >
      {webdav ? "Open Files" : "Connect"}
    </Button>
  );
}
