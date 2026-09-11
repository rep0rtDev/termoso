import { useEffect, useRef } from "react";
import { Box, Button, CircularProgress, Stack, Tooltip, Typography, alpha } from "@mui/material";
import ReplayRoundedIcon from "@mui/icons-material/ReplayRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import ShieldRoundedIcon from "@mui/icons-material/ShieldRounded";
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
import { usePaneTheme } from "./useTerminalTheme";
import type { TerminalTheme } from "./themes";

interface Props {
  paneId: Uuid;
  active: boolean;
  showFrame: boolean;
}

export function TerminalPane({ paneId, active, showFrame }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const pane = useTerminal((s) => s.panes[paneId]);
  const theme = usePaneTheme(paneId);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    return mountPane(paneId, host);
  }, [paneId]);

  useEffect(() => {
    if (active) focusPane(paneId);
  }, [active, paneId]);

  if (!pane) return null;
  const finished = pane.status === "exited" || pane.status === "error" || pane.status === "closed";

  return (
    <Box
      onMouseDown={() => setActivePane(paneId)}
      sx={(t) => ({
        position: "relative",
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        bgcolor: theme.background,
        outline: showFrame
          ? `1px solid ${active ? t.palette.primary.main : t.palette.divider}`
          : "none",
        outlineOffset: -1,
      })}
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
            borderBottom: 1,
            borderColor: "divider",
            bgcolor: "background.paper",
          }}
        >
          <StatusDot status={pane.status} />
          <Typography variant="caption" noWrap sx={{ flex: 1, fontWeight: 600 }}>
            {pane.title}
          </Typography>
          <Typography variant="caption" color="text.secondary" noWrap>
            {pane.subtitle}
          </Typography>
          <PqBadge algorithms={pane.algorithms} size={13} />
        </Stack>
      )}
      <Box ref={hostRef} sx={{ flex: 1, minHeight: 0, position: "relative" }} />

      {pane.status === "connecting" && (
        <Overlay theme={theme}>
          <CircularProgress size={22} />
          <Typography variant="body2" sx={{ opacity: 0.8 }}>
            Connecting to {pane.subtitle || pane.title}…
          </Typography>
        </Overlay>
      )}
      {finished && (
        <Overlay theme={theme} dim>
          <Typography
            variant="body2"
            color={pane.status === "error" ? "error" : "inherit"}
            sx={{ opacity: pane.status === "error" ? 1 : 0.8 }}
          >
            {pane.message ?? "Session ended"}
          </Typography>
          <Stack direction="row" spacing={1}>
            <Button
              size="small"
              variant="contained"
              startIcon={<ReplayRoundedIcon />}
              onClick={() => reconnectPane(paneId)}
            >
              Reconnect
            </Button>
            <Button
              size="small"
              color="inherit"
              startIcon={<CloseRoundedIcon />}
              onClick={() => void closePane(paneId)}
            >
              Close
            </Button>
          </Stack>
        </Overlay>
      )}
    </Box>
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
  return `KEX ${a.kex}\nHost key ${a.hostKey}\nCipher ${a.cipher}\nMAC ${a.mac}`;
}

/** Shield shown when the session negotiated a post-quantum key exchange. */
export function PqBadge({ algorithms, size }: { algorithms: SshAlgorithms | null; size: number }) {
  if (!algorithms || !isPostQuantumKex(algorithms)) return null;
  return (
    <Tooltip
      title={
        <Box sx={{ whiteSpace: "pre-line" }}>
          {"Quantum-safe key exchange\n" + algorithmsSummary(algorithms)}
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
