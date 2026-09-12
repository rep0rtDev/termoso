import { useState } from "react";
import { Box, Button, LinearProgress, Stack, Typography } from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import UploadRoundedIcon from "@mui/icons-material/UploadRounded";
import DownloadRoundedIcon from "@mui/icons-material/DownloadRounded";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import BlockRoundedIcon from "@mui/icons-material/BlockRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import ExpandLessRoundedIcon from "@mui/icons-material/ExpandLessRounded";
import EditRoundedIcon from "@mui/icons-material/EditRounded";
import CloudUploadRoundedIcon from "@mui/icons-material/CloudUploadRounded";
import OpenInNewRoundedIcon from "@mui/icons-material/OpenInNewRounded";
import CircularProgress from "@mui/material/CircularProgress";
import * as ipc from "@/ipc/commands";
import { ToolIconButton } from "@/components/ui";
import { baseName, formatDuration, formatSize, formatSpeed } from "./format";
import {
  cancelTransfer,
  clearFinishedTransfers,
  closeEdit,
  uploadEditNow,
  useSftp,
  type Edit,
  type Transfer,
} from "./store";

export function TransfersPanel() {
  const order = useSftp((s) => s.transferOrder);
  const transfers = useSftp((s) => s.transfers);
  const editOrder = useSftp((s) => s.editOrder);
  const edits = useSftp((s) => s.edits);
  const staging = useSftp((s) => s.staging);
  const [collapsed, setCollapsed] = useState(false);
  const editing = editOrder.map((id) => edits[id]).filter((e): e is Edit => e !== undefined);
  if (order.length === 0 && editing.length === 0 && staging === null) return null;

  const all = order.map((id) => transfers[id]).filter((t): t is Transfer => t !== undefined);
  const running = all.filter((t) => t.status === "running");
  const failed = all.filter((t) => t.status === "failed").length;
  const done = running.reduce((n, t) => n + t.done, 0);
  const total = running.reduce((n, t) => n + (t.total ?? 0), 0);
  const speed = running.reduce((n, t) => n + (t.speed ?? 0), 0);
  const known = running.every((t) => t.total !== null);
  const pct = known && total > 0 ? Math.min(100, (done / total) * 100) : undefined;

  const summary =
    staging !== null
      ? `Preparing dropped files… ${staging}`
      : running.length > 0
        ? [
            `${running.length} active`,
            pct !== undefined ? `${Math.round(pct)}%` : null,
            speed > 0 ? formatSpeed(speed) : null,
            pct !== undefined && speed > 0
              ? `${formatDuration((total - done) / speed)} left`
              : null,
          ]
            .filter(Boolean)
            .join(" · ")
        : all.length > 0
          ? `${all.length} finished${failed > 0 ? ` · ${failed} failed` : ""}`
          : `${editing.length} ${editing.length === 1 ? "file" : "files"} open for editing`;

  return (
    <Box
      sx={{
        borderTop: 1,
        borderColor: "border.light",
        bgcolor: "surface.base",
        display: "flex",
        flexDirection: "column",
        flexShrink: 0,
        position: "relative",
      }}
    >
      {(running.length > 0 || staging !== null) && (
        <LinearProgress
          variant={pct === undefined || staging !== null ? "indeterminate" : "determinate"}
          value={pct}
          sx={{
            position: "absolute",
            top: -1,
            left: 0,
            right: 0,
            height: 2,
            bgcolor: "transparent",
          }}
        />
      )}
      <Stack
        direction="row"
        spacing={1}
        sx={{ alignItems: "center", pl: 1.5, pr: 0.75, height: 36, flexShrink: 0 }}
      >
        <Typography variant="subtitle2" color="text.secondary">
          Transfers
        </Typography>
        <Typography variant="caption" color="text.disabled" noWrap sx={{ flex: 1 }}>
          {summary}
        </Typography>
        <Button
          color="inherit"
          onClick={clearFinishedTransfers}
          disabled={running.length === all.length}
          sx={{ visibility: all.length > 0 ? "visible" : "hidden" }}
        >
          Clear finished
        </Button>
        <ToolIconButton
          title={collapsed ? "Show transfers" : "Hide transfers"}
          onClick={() => setCollapsed((v) => !v)}
        >
          {collapsed ? (
            <ExpandLessRoundedIcon fontSize="small" />
          ) : (
            <ExpandMoreRoundedIcon fontSize="small" />
          )}
        </ToolIconButton>
      </Stack>
      {!collapsed && (
        <Box sx={{ overflow: "auto", maxHeight: 176, px: 1.5, pb: 1 }}>
          {editing.map((e) => (
            <EditRow key={e.info.id} e={e} />
          ))}
          {all.map((t) => (
            <TransferRow key={t.id} t={t} />
          ))}
        </Box>
      )}
    </Box>
  );
}

function detail(t: Transfer): string {
  switch (t.status) {
    case "failed":
      return t.message ?? "Failed";
    case "cancelled":
      return `Cancelled · ${formatSize(t.done)}`;
    case "done": {
      const secs = ((t.finishedAt ?? Date.now()) - t.startedAt) / 1000;
      const avg = secs > 0.5 ? ` · ${formatSpeed(t.done / secs)}` : "";
      const skipped = t.filesSkipped > 0 ? ` · ${t.filesSkipped} skipped` : "";
      if (t.filesSkipped > 0 && t.filesSkipped === t.filesTotal) return "Skipped · already exists";
      return `${formatSize(t.done)} · ${formatDuration(secs)}${avg}${skipped}`;
    }
    case "running": {
      if (t.cancelling) return "Cancelling…";
      const parts = [`${formatSize(t.done)}${t.total !== null ? ` / ${formatSize(t.total)}` : ""}`];
      if (t.speed !== null && t.speed > 0) {
        parts.push(formatSpeed(t.speed));
        if (t.total !== null && t.total > t.done) {
          parts.push(`${formatDuration((t.total - t.done) / t.speed)} left`);
        }
      }
      return parts.join(" · ");
    }
  }
}

function TransferRow({ t }: { t: Transfer }) {
  const Icon = t.direction === "upload" ? UploadRoundedIcon : DownloadRoundedIcon;
  const src = t.direction === "upload" ? t.local : t.remote;
  const dst = t.direction === "upload" ? t.remote : t.local;
  const pct = t.total ? Math.min(100, (t.done / t.total) * 100) : undefined;
  const multi = t.filesTotal > 1;
  const running = t.status === "running";
  const skippedAll = t.status === "done" && t.filesSkipped > 0 && t.filesSkipped === t.filesTotal;
  return (
    <Stack direction="row" spacing={1.25} sx={{ alignItems: "center", py: 0.625 }}>
      <Icon
        sx={{
          fontSize: 18,
          color: running ? "text.secondary" : "text.disabled",
          flexShrink: 0,
        }}
      />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Stack sx={{ alignItems: "baseline" }} direction="row" spacing={1}>
          <Typography variant="body2" noWrap sx={{ fontSize: 13, flex: 1 }}>
            {baseName(src)}
            {multi ? ` (${t.filesDone}/${t.filesTotal})` : ""}
          </Typography>
          {running && pct !== undefined && (
            <Typography
              variant="caption"
              color="text.secondary"
              sx={{ fontVariantNumeric: "tabular-nums" }}
            >
              {Math.round(pct)}%
            </Typography>
          )}
          <Typography
            variant="caption"
            color={t.status === "failed" ? "error" : "text.secondary"}
            noWrap
            sx={{ fontVariantNumeric: "tabular-nums" }}
          >
            {detail(t)}
          </Typography>
        </Stack>
        {running && (
          <LinearProgress
            variant={pct === undefined ? "indeterminate" : "determinate"}
            value={pct}
            color={t.cancelling ? "inherit" : "primary"}
            sx={{ mt: 0.5, height: 3, borderRadius: 2, opacity: t.cancelling ? 0.5 : 1 }}
          />
        )}
        <Typography
          sx={{ display: "block", mt: 0.25 }}
          variant="caption"
          color="text.disabled"
          noWrap
        >
          {running && multi && t.current ? t.current : `→ ${dst}`}
        </Typography>
      </Box>
      <Box sx={{ width: 28, display: "flex", justifyContent: "center", flexShrink: 0 }}>
        {running ? (
          <ToolIconButton
            title="Cancel"
            disabled={t.cancelling}
            onClick={() => void cancelTransfer(t.id)}
          >
            <CloseRoundedIcon fontSize="small" />
          </ToolIconButton>
        ) : skippedAll ? (
          <BlockRoundedIcon sx={{ fontSize: 18, color: "text.disabled" }} />
        ) : t.status === "done" ? (
          <CheckRoundedIcon sx={{ fontSize: 18, color: "success.main" }} />
        ) : t.status === "failed" ? (
          <ErrorOutlineRoundedIcon sx={{ fontSize: 18, color: "error.main" }} />
        ) : (
          <BlockRoundedIcon sx={{ fontSize: 18, color: "text.disabled" }} />
        )}
      </Box>
    </Stack>
  );
}

function editDetail(e: Edit): string {
  switch (e.status) {
    case "uploading":
      return "Uploading changes…";
    case "failed":
      return e.message ?? "Upload failed";
    case "watching":
      return e.uploadedAt !== null
        ? `Saved ${formatSize(e.uploadedBytes)} · uploaded ${new Date(e.uploadedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
        : `${formatSize(e.info.size)} · waiting for changes${e.info.app ? ` in ${e.info.app}` : ""}`;
  }
}

function EditRow({ e }: { e: Edit }) {
  const busy = e.status === "uploading";
  return (
    <Stack direction="row" spacing={1.25} sx={{ alignItems: "center", py: 0.625 }}>
      <EditRoundedIcon sx={{ fontSize: 18, color: "primary.main", flexShrink: 0 }} />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Stack sx={{ alignItems: "baseline" }} direction="row" spacing={1}>
          <Typography variant="body2" noWrap sx={{ fontSize: 13, flex: 1 }}>
            {e.info.name}
          </Typography>
          <Typography
            variant="caption"
            color={e.status === "failed" ? "error" : "text.secondary"}
            noWrap
          >
            {editDetail(e)}
          </Typography>
        </Stack>
        <Typography
          sx={{ display: "block", mt: 0.25 }}
          variant="caption"
          color="text.disabled"
          noWrap
        >
          {e.info.remote}
        </Typography>
      </Box>
      <Stack direction="row" sx={{ flexShrink: 0, alignItems: "center" }}>
        {busy ? (
          <Box sx={{ width: 28, display: "flex", justifyContent: "center" }}>
            <CircularProgress size={14} />
          </Box>
        ) : (
          <ToolIconButton title="Upload now" onClick={() => void uploadEditNow(e.info.id)}>
            <CloudUploadRoundedIcon fontSize="small" />
          </ToolIconButton>
        )}
        <ToolIconButton
          title="Open again"
          onClick={() => void ipc.localOpen(e.info.local, e.info.app)}
        >
          <OpenInNewRoundedIcon fontSize="small" />
        </ToolIconButton>
        <ToolIconButton title="Stop watching" onClick={() => void closeEdit(e.info.id)}>
          <CloseRoundedIcon fontSize="small" />
        </ToolIconButton>
      </Stack>
    </Stack>
  );
}
