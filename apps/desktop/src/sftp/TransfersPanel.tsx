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
import PauseRoundedIcon from "@mui/icons-material/PauseRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import ReplayRoundedIcon from "@mui/icons-material/ReplayRounded";
import ScheduleRoundedIcon from "@mui/icons-material/ScheduleRounded";
import CircularProgress from "@mui/material/CircularProgress";
import * as ipc from "@/ipc/commands";
import { ToolIconButton } from "@/components/ui";
import { baseName, formatDuration, formatSize, formatSpeed } from "./format";
import {
  cancelTransfer,
  clearFinishedTransfers,
  closeEdit,
  discardTransfer,
  isActive,
  pauseTransfer,
  resumeTransfer,
  uploadEditNow,
  useSftp,
  type Edit,
  type Transfer,
} from "./store";
import { tr } from "@/i18n";

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
  const queued = all.filter((t) => t.status === "queued").length;
  const paused = all.filter((t) => t.status === "paused").length;
  const failed = all.filter((t) => t.status === "failed").length;
  const pending = all.filter((t) => isActive(t.status) || t.status === "paused").length;
  const done = running.reduce((n, t) => n + t.done, 0);
  const total = running.reduce((n, t) => n + (t.total ?? 0), 0);
  const speed = running.reduce((n, t) => n + (t.speed ?? 0), 0);
  const known = running.every((t) => t.total !== null);
  const pct = known && total > 0 ? Math.min(100, (done / total) * 100) : undefined;

  const summary =
    staging !== null
      ? tr("Preparing dropped files… {staging}", { staging })
      : running.length > 0
        ? [
            `${running.length} active`,
            queued > 0 ? `${queued} queued` : null,
            paused > 0 ? `${paused} paused` : null,
            pct !== undefined ? `${Math.round(pct)}%` : null,
            speed > 0 ? formatSpeed(speed) : null,
            pct !== undefined && speed > 0
              ? `${formatDuration((total - done) / speed)} left`
              : null,
          ]
            .filter(Boolean)
            .join(" · ")
        : paused > 0 || queued > 0
          ? [
              paused > 0 ? `${paused} paused` : null,
              queued > 0 ? `${queued} queued` : null,
              failed > 0 ? `${failed} failed` : null,
            ]
              .filter(Boolean)
              .join(" · ")
          : all.length > 0
            ? [
                all.length - failed > 0 ? `${all.length - failed} finished` : null,
                failed > 0 ? `${failed} failed` : null,
              ]
                .filter(Boolean)
                .join(" · ")
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
          {tr("Transfers")}
        </Typography>
        <Typography variant="caption" color="text.disabled" noWrap sx={{ flex: 1 }}>
          {summary}
        </Typography>
        <Button
          color="inherit"
          onClick={clearFinishedTransfers}
          disabled={pending === all.length}
          sx={{ visibility: all.length > 0 ? "visible" : "hidden" }}
        >
          {tr("Clear finished")}
        </Button>
        <ToolIconButton
          title={collapsed ? tr("Show transfers") : tr("Hide transfers")}
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
      return tr("Cancelled · {formatSize}", { formatSize: formatSize(t.done) });
    case "queued":
      return t.cancelling ? tr("Cancelling…") : tr("Waiting…");
    case "paused":
      return `Paused · ${formatSize(t.done)}${t.total !== null ? ` / ${formatSize(t.total)}` : ""}`;
    case "done": {
      const secs = ((t.finishedAt ?? Date.now()) - t.startedAt) / 1000;
      const avg = secs > 0.5 ? ` · ${formatSpeed(t.done / secs)}` : "";
      const skipped =
        t.filesSkipped > 0
          ? " · " + tr("{filesSkipped} skipped", { filesSkipped: t.filesSkipped })
          : "";
      if (t.filesSkipped > 0 && t.filesSkipped === t.filesTotal)
        return tr("Skipped · already exists");
      return `${formatSize(t.done)} · ${formatDuration(secs)}${avg}${skipped}`;
    }
    case "running": {
      if (t.cancelling) return tr("Cancelling…");
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
  const active = isActive(t.status);
  const resumable = t.status === "paused" || t.status === "failed";
  const skippedAll = t.status === "done" && t.filesSkipped > 0 && t.filesSkipped === t.filesTotal;
  return (
    <Stack direction="row" spacing={1.25} sx={{ alignItems: "center", py: 0.625 }}>
      {t.status === "queued" ? (
        <ScheduleRoundedIcon sx={{ fontSize: 18, color: "text.disabled", flexShrink: 0 }} />
      ) : (
        <Icon
          sx={{
            fontSize: 18,
            color: running ? "text.secondary" : "text.disabled",
            flexShrink: 0,
          }}
        />
      )}
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Stack sx={{ alignItems: "baseline" }} direction="row" spacing={1}>
          <Typography variant="body2" noWrap sx={{ fontSize: 13, flex: 1 }}>
            {baseName(src)}
            {multi ? ` (${t.filesDone}/${t.filesTotal})` : ""}
          </Typography>
          {(running || t.status === "paused") && pct !== undefined && (
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
        {(running || t.status === "paused") && (
          <LinearProgress
            variant={pct === undefined && running ? "indeterminate" : "determinate"}
            value={pct ?? 0}
            color={t.cancelling || t.status === "paused" ? "inherit" : "primary"}
            sx={{
              mt: 0.5,
              height: 3,
              borderRadius: 2,
              opacity: t.cancelling || t.status === "paused" ? 0.5 : 1,
            }}
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
      <Stack direction="row" sx={{ flexShrink: 0, alignItems: "center", minWidth: 28 }}>
        {t.status === "failed" && (
          <ErrorOutlineRoundedIcon sx={{ fontSize: 18, color: "error.main", mr: 0.5 }} />
        )}
        {running && (
          <ToolIconButton
            title={tr("Pause")}
            disabled={t.cancelling}
            onClick={() => void pauseTransfer(t.id)}
          >
            <PauseRoundedIcon fontSize="small" />
          </ToolIconButton>
        )}
        {resumable && (
          <ToolIconButton
            title={t.status === "paused" ? tr("Resume") : tr("Retry")}
            onClick={() => void resumeTransfer(t.id).catch(() => undefined)}
          >
            {t.status === "paused" ? (
              <PlayArrowRoundedIcon fontSize="small" />
            ) : (
              <ReplayRoundedIcon fontSize="small" />
            )}
          </ToolIconButton>
        )}
        {active || resumable ? (
          <ToolIconButton
            title={active ? tr("Cancel") : tr("Discard")}
            disabled={t.cancelling}
            onClick={() =>
              void (active ? cancelTransfer(t.id) : discardTransfer(t.id)).catch(() => undefined)
            }
          >
            <CloseRoundedIcon fontSize="small" />
          </ToolIconButton>
        ) : (
          <Box sx={{ width: 28, display: "flex", justifyContent: "center" }}>
            {skippedAll ? (
              <BlockRoundedIcon sx={{ fontSize: 18, color: "text.disabled" }} />
            ) : t.status === "done" ? (
              <CheckRoundedIcon sx={{ fontSize: 18, color: "success.main" }} />
            ) : (
              <BlockRoundedIcon sx={{ fontSize: 18, color: "text.disabled" }} />
            )}
          </Box>
        )}
      </Stack>
    </Stack>
  );
}

function editDetail(e: Edit): string {
  switch (e.status) {
    case "uploading":
      return tr("Uploading changes…");
    case "failed":
      return e.message ?? tr("Upload failed");
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
          <ToolIconButton title={tr("Upload now")} onClick={() => void uploadEditNow(e.info.id)}>
            <CloudUploadRoundedIcon fontSize="small" />
          </ToolIconButton>
        )}
        <ToolIconButton
          title={tr("Open again")}
          onClick={() => void ipc.localOpen(e.info.local, e.info.app)}
        >
          <OpenInNewRoundedIcon fontSize="small" />
        </ToolIconButton>
        <ToolIconButton title={tr("Stop watching")} onClick={() => void closeEdit(e.info.id)}>
          <CloseRoundedIcon fontSize="small" />
        </ToolIconButton>
      </Stack>
    </Stack>
  );
}
