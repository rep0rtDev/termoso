import { useEffect, useState } from "react";
import { Box, Button, IconButton, Paper, Stack, Typography } from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import KeyboardReturnRoundedIcon from "@mui/icons-material/KeyboardReturnRounded";
import { closeDisconnected, dismissReconnect, reconnectNow, useTerminal } from "./store";
import { useAppInfo } from "@/ipc/hooks";
import { disconnectedLabel } from "./reconnect";

const RING = 34;
const STROKE = 3;

/**
 * Bottom-left card shown while dropped sessions wait for the next automatic
 * attempt: countdown ring with the attempts left, Reconnect (Enter) and Close
 * terminal, like Termius.
 */
export function ReconnectSnackbar() {
  const queue = useTerminal((s) => s.reconnect);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!queue) return;
    const timer = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(timer);
  }, [queue]);

  useEffect(() => {
    if (!queue) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Enter" || e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;
      const t = e.target;
      if (t instanceof HTMLElement && t.closest("input, textarea, [role=dialog]")) return;
      e.preventDefault();
      reconnectNow();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [queue]);

  if (!queue) return null;
  const remaining = Math.max(0, queue.dueAt - now);
  const fraction = queue.delayMs > 0 ? remaining / queue.delayMs : 0;
  const n = queue.paneIds.length;
  const r = (RING - STROKE) / 2;
  const circumference = 2 * Math.PI * r;

  return (
    <Paper
      elevation={0}
      role="status"
      sx={{
        position: "fixed",
        left: 20,
        bottom: 20,
        zIndex: (t) => t.zIndex.snackbar,
        p: 1.5,
        pr: 1,
        borderRadius: 2,
        bgcolor: "background.paper",
        border: 1,
        borderColor: "divider",
        boxShadow: "0 8px 24px rgba(0,0,0,.28)",
        minWidth: 280,
      }}
    >
      <Stack direction="row" spacing={1.25} sx={{ alignItems: "flex-start" }}>
        <Box sx={{ position: "relative", width: RING, height: RING, flexShrink: 0 }}>
          <Box component="svg" viewBox={`0 0 ${RING} ${RING}`} sx={{ width: RING, height: RING }}>
            <circle
              cx={RING / 2}
              cy={RING / 2}
              r={r}
              fill="none"
              stroke="currentColor"
              strokeOpacity={0.15}
              strokeWidth={STROKE}
            />
            <Box
              component="circle"
              cx={RING / 2}
              cy={RING / 2}
              r={r}
              fill="none"
              strokeWidth={STROKE}
              strokeLinecap="round"
              strokeDasharray={circumference}
              strokeDashoffset={circumference * (1 - fraction)}
              transform={`rotate(-90 ${RING / 2} ${RING / 2})`}
              sx={{ stroke: "success.main", transition: "stroke-dashoffset .25s linear" }}
            />
          </Box>
          <Typography
            variant="body2"
            sx={{
              position: "absolute",
              inset: 0,
              display: "grid",
              placeItems: "center",
              fontWeight: 700,
              fontSize: 13,
            }}
          >
            {queue.attemptsLeft}
          </Typography>
        </Box>
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography variant="body2" sx={{ fontSize: 12, lineHeight: 1.45 }}>
            {disconnectedLabel(n)}
            <br />
            Initiating reconnection…
          </Typography>
          <Stack direction="row" spacing={1} sx={{ mt: 1.25 }}>
            <Button
              size="small"
              variant="contained"
              onClick={reconnectNow}
              endIcon={<KeyboardReturnRoundedIcon sx={{ fontSize: 14 }} />}
            >
              Reconnect
            </Button>
            <Button size="small" variant="outlined" color="inherit" onClick={closeDisconnected}>
              Close terminal
            </Button>
          </Stack>
        </Box>
        <IconButton size="small" aria-label="Dismiss" onClick={dismissReconnect} sx={{ mt: -0.5 }}>
          <CloseRoundedIcon sx={{ fontSize: 16 }} />
        </IconButton>
      </Stack>
    </Paper>
  );
}

/** `[Ctrl] + [Click] to open the link` tooltip next to the hovered URL. */
export function LinkHoverHint() {
  const hover = useTerminal((s) => s.linkHover);
  const app = useAppInfo();
  if (!hover) return null;
  const mod = app.data?.platform === "macos" ? "Cmd" : "Ctrl";
  return (
    <Paper
      elevation={0}
      sx={{
        position: "fixed",
        left: hover.left + 12,
        top: hover.top + 18,
        zIndex: (t) => t.zIndex.tooltip,
        px: 1,
        py: 0.5,
        borderRadius: 1,
        border: 1,
        borderColor: "divider",
        pointerEvents: "none",
        fontSize: 11,
        color: "text.secondary",
        whiteSpace: "nowrap",
      }}
    >
      <Box component="kbd" sx={kbdSx}>
        {mod}
      </Box>{" "}
      +{" "}
      <Box component="kbd" sx={kbdSx}>
        Click
      </Box>{" "}
      to open the link
    </Paper>
  );
}

const kbdSx = {
  fontFamily: "inherit",
  fontSize: 11,
  px: 0.5,
  borderRadius: 0.5,
  bgcolor: "action.hover",
} as const;
