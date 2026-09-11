import { Box, Button, IconButton, LinearProgress, Stack, Tooltip, Typography } from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import UploadRoundedIcon from "@mui/icons-material/UploadRounded";
import DownloadRoundedIcon from "@mui/icons-material/DownloadRounded";
import CheckCircleRoundedIcon from "@mui/icons-material/CheckCircleRounded";
import ErrorRoundedIcon from "@mui/icons-material/ErrorRounded";
import BlockRoundedIcon from "@mui/icons-material/BlockRounded";
import { baseName, formatSize } from "./format";
import { cancelTransfer, clearFinishedTransfers, useSftp, type Transfer } from "./store";

export function TransfersPanel() {
  const order = useSftp((s) => s.transferOrder);
  const transfers = useSftp((s) => s.transfers);
  if (order.length === 0) return null;
  const running = order.filter((id) => transfers[id]?.status === "running").length;

  return (
    <Box
      sx={{
        borderTop: 1,
        borderColor: "divider",
        bgcolor: "background.paper",
        maxHeight: 200,
        display: "flex",
        flexDirection: "column",
        flexShrink: 0,
      }}
    >
      <Stack direction="row" sx={{ alignItems: "center", px: 1.5, height: 32, flexShrink: 0 }}>
        <Typography variant="caption" sx={{ fontWeight: 600, flex: 1 }}>
          Transfers{running > 0 ? ` · ${running} active` : ""}
        </Typography>
        <Button
          size="small"
          color="inherit"
          onClick={clearFinishedTransfers}
          disabled={running === order.length}
        >
          Clear finished
        </Button>
      </Stack>
      <Box sx={{ overflow: "auto", px: 1.5, pb: 1 }}>
        {order.map((id) => {
          const t = transfers[id];
          return t ? <TransferRow key={id} t={t} /> : null;
        })}
      </Box>
    </Box>
  );
}

function TransferRow({ t }: { t: Transfer }) {
  const Icon = t.direction === "upload" ? UploadRoundedIcon : DownloadRoundedIcon;
  const src = t.direction === "upload" ? t.local : t.remote;
  const pct = t.total ? Math.min(100, (t.done / t.total) * 100) : undefined;
  const multi = t.filesTotal > 1;
  return (
    <Stack direction="row" spacing={1.25} sx={{ alignItems: "center", py: 0.5 }}>
      <Icon fontSize="small" color={t.status === "running" ? "primary" : "disabled"} />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Stack sx={{ alignItems: "baseline" }} direction="row" spacing={1}>
          <Typography variant="body2" noWrap sx={{ fontSize: 13, flex: 1 }}>
            {baseName(src)}
            {multi ? ` (${t.filesDone}/${t.filesTotal})` : ""}
          </Typography>
          <Typography variant="caption" color="text.secondary" noWrap>
            {t.status === "failed"
              ? t.message
              : t.status === "cancelled"
                ? "Cancelled"
                : `${formatSize(t.done)}${t.total ? ` / ${formatSize(t.total)}` : ""}`}
          </Typography>
        </Stack>
        {t.status === "running" && (
          <LinearProgress
            variant={pct === undefined ? "indeterminate" : "determinate"}
            value={pct}
            sx={{ mt: 0.5, height: 4, borderRadius: 2 }}
          />
        )}
        {t.status === "running" && multi && t.current && (
          <Typography sx={{ display: "block" }} variant="caption" color="text.disabled" noWrap>
            {t.current}
          </Typography>
        )}
      </Box>
      {t.status === "running" ? (
        <Tooltip title="Cancel">
          <IconButton size="small" onClick={() => void cancelTransfer(t.id)}>
            <CloseRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      ) : t.status === "done" ? (
        <CheckCircleRoundedIcon fontSize="small" color="success" />
      ) : t.status === "failed" ? (
        <ErrorRoundedIcon fontSize="small" color="error" />
      ) : (
        <BlockRoundedIcon fontSize="small" color="disabled" />
      )}
    </Stack>
  );
}
