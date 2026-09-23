import { useEffect, useRef } from "react";
import { Box, Button, Stack, Tooltip, Typography, alpha } from "@mui/material";
import ReplayRoundedIcon from "@mui/icons-material/ReplayRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import ShieldRoundedIcon from "@mui/icons-material/ShieldRounded";
import KeyboardAltRoundedIcon from "@mui/icons-material/KeyboardAltRounded";
import type { SshAlgorithms, Uuid } from "@/ipc/types";
import { isPostQuantumKex } from "@/ipc/types";
import {
  closePane,
  focusPane,
  mountPane,
  reconnectPane,
  setActivePane,
  useTerminal,
} from "./store";
import { AutocompletePopup } from "./AutocompletePopup";
import { ConnectionView } from "./ConnectionView";
import { usePaneTheme } from "./useTerminalTheme";
import { dismissControlHint, useMultiplayer } from "./multiplayer";
import type { TerminalTheme } from "./themes";
import { tr } from "@/i18n";

interface Props {
  paneId: Uuid;
  active: boolean;
  showFrame: boolean;
}

export function TerminalPane({ paneId, active, showFrame }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const pane = useTerminal((s) => s.panes[paneId]);
  const queued = useTerminal((s) => s.reconnect?.paneIds.includes(paneId) ?? false);
  const theme = usePaneTheme(paneId);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    return mountPane(paneId, host);
  }, [paneId]);

  const status = pane?.status;
  useEffect(() => {
    if (active && status !== "connecting") focusPane(paneId);
  }, [active, paneId, status]);

  if (!pane) return null;
  const finished = pane.status === "exited" || pane.status === "error" || pane.status === "closed";
  // Never got a session: show the connection view with the failure instead of
  // an empty terminal. A dropped session keeps its buffer while it reconnects.
  const failedToConnect =
    finished && pane.startedAt === null && pane.status !== "exited" && !pane.reconnecting;
  const connecting = pane.status === "connecting" && !pane.reconnecting;

  return (
    <Box
      onMouseDown={() => setActivePane(paneId)}
      sx={{
        position: "relative",
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        bgcolor: theme.background,
        borderRadius: 2,
        overflow: "hidden",
      }}
    >
      {showFrame && (
        <Stack
          direction="row"
          spacing={1}
          sx={{
            alignItems: "center",
            px: 1.25,
            height: 26,
            flexShrink: 0,
            color: theme.foreground,
            bgcolor: alpha(theme.foreground, 0.05),
            opacity: active ? 1 : 0.7,
          }}
        >
          <StatusDot status={pane.status} />
          <Typography variant="caption" noWrap sx={{ flex: 1, fontWeight: 600 }}>
            {pane.title}
          </Typography>
          <Typography variant="caption" noWrap sx={{ opacity: 0.6 }}>
            {pane.subtitle}
          </Typography>
          <PqBadge algorithms={pane.algorithms} size={13} />
        </Stack>
      )}
      <Box sx={{ flex: 1, minHeight: 0, position: "relative", display: "flex" }}>
        <Box
          ref={hostRef}
          sx={{
            flex: 1,
            minWidth: 0,
            minHeight: 0,
            position: "relative",
            visibility: connecting || failedToConnect ? "hidden" : "visible",
          }}
        />
        <AutocompletePopup paneId={paneId} />
        <RemoteControlHint paneId={paneId} />
        {(connecting || failedToConnect) && <ConnectionView pane={pane} />}
      </Box>

      {finished && !failedToConnect && !queued && (
        <Overlay theme={theme} dim>
          <Typography
            variant="body2"
            color={pane.status === "error" ? "error" : "inherit"}
            sx={{ opacity: pane.status === "error" ? 1 : 0.8 }}
          >
            {pane.message ?? tr("Session ended")}
          </Typography>
          <Stack direction="row" spacing={1}>
            {pane.target.kind !== "live" && (
              <Button
                size="small"
                variant="contained"
                startIcon={<ReplayRoundedIcon />}
                onClick={() => void reconnectPane(paneId)}
              >
                {tr("Reconnect")}
              </Button>
            )}
            <Button
              size="small"
              color="inherit"
              startIcon={<CloseRoundedIcon />}
              onClick={() => void closePane(paneId)}
            >
              {tr("Close")}
            </Button>
          </Stack>
        </Overlay>
      )}
    </Box>
  );
}

/** Termius-style "You've got remote control" hint on a multiplayer viewer pane. */
function RemoteControlHint({ paneId }: { paneId: Uuid }) {
  const shown = useMultiplayer((s) => s.controlHint === paneId);
  useEffect(() => {
    if (!shown) return;
    const t = setTimeout(dismissControlHint, 8_000);
    return () => clearTimeout(t);
  }, [shown]);
  if (!shown) return null;
  return (
    <Stack
      direction="row"
      spacing={1}
      onClick={dismissControlHint}
      sx={{
        position: "absolute",
        top: 12,
        left: 16,
        zIndex: 2,
        alignItems: "center",
        px: 1.5,
        height: 36,
        borderRadius: 1.5,
        cursor: "pointer",
        color: "info.main",
        bgcolor: (t) => alpha(t.palette.info.main, 0.12),
        border: "1px solid",
        borderColor: "info.main",
        backdropFilter: "blur(6px)",
      }}
    >
      <KeyboardAltRoundedIcon sx={{ fontSize: 16 }} />
      <Typography variant="body2" sx={{ fontWeight: 500 }}>
        {tr("You've got remote control. Start typing.")}
      </Typography>
    </Stack>
  );
}

function Overlay({
  children,
  theme,
  dim,
}: {
  children: React.ReactNode;
  theme: TerminalTheme;
  dim?: boolean;
}) {
  return (
    <Stack
      spacing={1.5}
      sx={{
        alignItems: "center",
        justifyContent: "flex-end",
        position: "absolute",
        inset: 0,
        zIndex: 6,
        pb: 4,
        color: theme.foreground,
        pointerEvents: dim ? "auto" : "none",
        background: dim
          ? `linear-gradient(to bottom, transparent 40%, ${alpha(theme.background, 0.92)})`
          : "transparent",
      }}
    >
      {children}
    </Stack>
  );
}

export function algorithmsSummary(a: SshAlgorithms): string {
  return tr("KEX {kex}\nHost key {hostKey}\nCipher {cipher}\nMAC {mac}", {
    kex: a.kex,
    hostKey: a.hostKey,
    cipher: a.cipher,
    mac: a.mac,
  });
}

/** Shield shown when the session negotiated a post-quantum key exchange. */
export function PqBadge({ algorithms, size }: { algorithms: SshAlgorithms | null; size: number }) {
  if (!algorithms || !isPostQuantumKex(algorithms)) return null;
  return (
    <Tooltip
      title={
        <Box sx={{ whiteSpace: "pre-line" }}>
          {tr("Quantum-safe key exchange") + "\n" + algorithmsSummary(algorithms)}
        </Box>
      }
    >
      <ShieldRoundedIcon sx={{ fontSize: size, color: "primary.main" }} />
    </Tooltip>
  );
}

export function StatusDot({ status }: { status: string }) {
  const color =
    status === "connected"
      ? "success.main"
      : status === "connecting"
        ? "warning.main"
        : status === "error"
          ? "error.main"
          : "text.disabled";
  return <Box sx={{ width: 8, height: 8, borderRadius: "50%", bgcolor: color, flexShrink: 0 }} />;
}
