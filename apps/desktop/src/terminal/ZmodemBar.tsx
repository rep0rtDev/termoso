import { Box, IconButton, LinearProgress, Tooltip, Typography } from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import type { Uuid } from "@/ipc/types";
import { formatSize } from "@/sftp/format";
import { tr } from "@/i18n";
import { cancelZmodem, useTerminal } from "./store";

/** `rz` / `sz` progress over the terminal: file, bytes, cancel. */
export function ZmodemBar({ paneId }: { paneId: Uuid }) {
  const z = useTerminal((s) => s.panes[paneId]?.zmodem ?? null);
  if (!z) return null;
  const pct = z.total && z.total > 0 ? Math.min(100, (z.done / z.total) * 100) : null;
  const label =
    z.name === ""
      ? z.kind === "receive"
        ? tr("ZMODEM: the remote wants to send files — choose a folder")
        : tr("ZMODEM: the remote wants files — choose which")
      : `${z.kind === "receive" ? "↓" : "↑"} ${z.name} · ${formatSize(z.done)}${z.total ? ` / ${formatSize(z.total)}` : ""}${z.files > 0 ? ` · ${z.files}` : ""}`;
  return (
    <Box
      sx={{
        position: "absolute",
        left: 8,
        right: 8,
        bottom: 8,
        zIndex: 5,
        px: 1.25,
        py: 0.75,
        borderRadius: 1,
        bgcolor: "background.paper",
        border: 1,
        borderColor: "border.light",
        boxShadow: 3,
        display: "flex",
        alignItems: "center",
        gap: 1,
      }}
    >
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body2" noWrap title={label}>
          {label}
        </Typography>
        <LinearProgress
          variant={pct === null ? "indeterminate" : "determinate"}
          value={pct ?? 0}
          sx={{ mt: 0.5, height: 4, borderRadius: 2 }}
        />
      </Box>
      <Tooltip title={tr("Cancel transfer")}>
        <IconButton
          size="small"
          aria-label={tr("Cancel transfer")}
          onClick={() => cancelZmodem(paneId)}
        >
          <CloseRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
    </Box>
  );
}
