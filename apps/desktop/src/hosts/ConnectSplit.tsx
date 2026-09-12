import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import SwapHorizRoundedIcon from "@mui/icons-material/SwapHorizRounded";
import { Button } from "@mui/material";
import type { MenuAction } from "@/components/ui";
import { goToSftp, requestForwardingRule } from "@/app/navigation";
import { openSftpForHost } from "@/sftp/store";
import { openTerminal } from "@/terminal/store";
import type { HostProtocol, Uuid } from "@/ipc/types";

export const PROTOCOL_NAME: Record<HostProtocol, string> = { ssh: "SSH", telnet: "Telnet" };

/**
 * Termius' `Connect ▸` submenu for a saved host: one entry per protocol
 * section, then SFTP and port forwarding, which need the SSH one.
 */
export function connectActions(
  hostId: Uuid,
  label: string,
  protocols: HostProtocol[],
): MenuAction[] {
  const ssh = protocols.includes("ssh");
  const needsSsh = ssh ? "" : " (needs an SSH section)";
  return [
    ...protocols.map((p) => ({
      label: `with ${PROTOCOL_NAME[p]}`,
      icon: <TerminalRoundedIcon fontSize="small" />,
      onClick: () => openTerminal({ kind: "host", host_id: hostId, protocol: p }),
    })),
    {
      label: `with SFTP${needsSsh}`,
      icon: <FolderCopyRoundedIcon fontSize="small" />,
      disabled: !ssh,
      divider: true,
      onClick: () => {
        openSftpForHost(hostId, label);
        goToSftp();
      },
    },
    {
      label: `Port forwarding${needsSsh}`,
      icon: <SwapHorizRoundedIcon fontSize="small" />,
      disabled: !ssh,
      onClick: () => requestForwardingRule(hostId),
    },
  ];
}

/** Full-width primary Connect at the bottom of Host Details, as in Termius. */
export function ConnectButton({
  hostId,
  protocol,
  disabled,
  onClick,
}: {
  hostId: Uuid | null;
  /** Section to open; `null` lets the host pick (SSH when it has one). */
  protocol?: HostProtocol | null;
  disabled?: boolean;
  /** Runs instead of opening the saved host (e.g. save first, then connect). */
  onClick?: () => void;
}) {
  return (
    <Button
      fullWidth
      variant="contained"
      size="large"
      disabled={disabled}
      startIcon={<PlayArrowRoundedIcon />}
      onClick={
        onClick ??
        (() => {
          if (hostId) openTerminal({ kind: "host", host_id: hostId, protocol: protocol ?? null });
        })
      }
      sx={{ height: 40, borderRadius: 2.5, fontWeight: 600 }}
    >
      Connect
    </Button>
  );
}
