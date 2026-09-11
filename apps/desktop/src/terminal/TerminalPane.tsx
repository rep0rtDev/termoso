import { useEffect, useRef } from "react";
import { Box, Button, CircularProgress, Stack, Typography, alpha } from "@mui/material";
import ReplayRoundedIcon from "@mui/icons-material/ReplayRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import type { Uuid } from "@/ipc/types";
import {
  closePane,
  focusPane,
  mountPane,
  reconnectPane,
  setActivePane,
  useTerminal,
} from "./store";

interface Props {
  paneId: Uuid;
  active: boolean;
  showFrame: boolean;
}

export function TerminalPane({ paneId, active, showFrame }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const pane = useTerminal((s) => s.panes[paneId]);

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
        display: "flex",
        flexDirection: "column",
        bgcolor: "background.default",
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
        </Stack>
      )}
      <Box ref={hostRef} sx={{ flex: 1, minHeight: 0, position: "relative" }} />

      {pane.status === "connecting" && (
        <Overlay>
          <CircularProgress size={22} />
          <Typography variant="body2" color="text.secondary">
            Connecting to {pane.subtitle || pane.title}…
          </Typography>
        </Overlay>
      )}
      {finished && (
        <Overlay dim>
          <Typography variant="body2" color={pane.status === "error" ? "error" : "text.secondary"}>
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

function Overlay({ children, dim }: { children: React.ReactNode; dim?: boolean }) {
  return (
    <Stack
      spacing={1.5}
      sx={(t) => ({
        alignItems: "center",
        justifyContent: "flex-end",
        position: "absolute",
        inset: 0,
        pb: 4,
        pointerEvents: dim ? "auto" : "none",
        background: dim
          ? `linear-gradient(to bottom, transparent 40%, ${alpha(t.palette.background.default, 0.92)})`
          : "transparent",
      })}
    >
      {children}
    </Stack>
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
