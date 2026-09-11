import { useCallback, useMemo, useState } from "react";
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  IconButton,
  List,
  ListItemButton,
  ListItemText,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ArticleRoundedIcon from "@mui/icons-material/ArticleRounded";
import BookmarkAddRoundedIcon from "@mui/icons-material/BookmarkAddRounded";
import BookmarkRoundedIcon from "@mui/icons-material/BookmarkRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import FileDownloadRoundedIcon from "@mui/icons-material/FileDownloadRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { save as saveFile } from "@tauri-apps/plugin-dialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageHeader } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useBookmarks, useLogBody, useLogs, useSettings } from "@/ipc/hooks";
import { errorMessage, type LogCard, type Uuid } from "@/ipc/types";
import { formatSize } from "@/sftp/format";
import { LogViewer, type ViewerHandle } from "./LogViewer";

function duration(secs: number | null): string {
  if (secs === null) return "—";
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m ${secs % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

function BookmarkDialog({
  line,
  busy,
  onCancel,
  onConfirm,
}: {
  line: number;
  busy: boolean;
  onCancel: () => void;
  onConfirm: (note: string) => void;
}) {
  const [note, setNote] = useState("");
  return (
    <Dialog open onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Bookmark line {line + 1}</DialogTitle>
      <DialogContent>
        <TextField
          autoFocus
          fullWidth
          label="Note"
          value={note}
          onChange={(e) => setNote(e.target.value)}
          sx={{ mt: 0.5 }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && note.trim()) onConfirm(note.trim());
          }}
        />
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={busy || note.trim().length === 0}
          onClick={() => onConfirm(note.trim())}
        >
          Add
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function Viewer({
  log,
  onClose,
  onDeleted,
}: {
  log: LogCard;
  onClose: () => void;
  onDeleted: () => void;
}) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const body = useLogBody(log.id);
  const bookmarks = useBookmarks(log.id);
  const settings = useSettings();
  const [handle, setHandle] = useState<ViewerHandle | null>(null);
  const [dialog, setDialog] = useState<
    { kind: "none" } | { kind: "bookmark"; line: number } | { kind: "delete" }
  >({ kind: "none" });
  const onReady = useCallback((h: ViewerHandle) => setHandle(h), []);

  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      void qc.invalidateQueries({ queryKey: keys.bookmarks(log.id) });
      void qc.invalidateQueries({ queryKey: keys.logs });
      setDialog({ kind: "none" });
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const exportLog = async () => {
    const stamp = log.startedAt.replace(/[:.]/g, "-");
    const path = await saveFile({
      title: "Export recording",
      defaultPath: `${log.label.replace(/[^\w.-]+/g, "_")}-${stamp}.log`,
    });
    if (path === null) return null;
    const n = await ipc.logExport(log.id, path);
    return `Exported ${formatSize(n)} to ${path}`;
  };

  return (
    <Box sx={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
      <Box
        sx={{
          px: 2,
          py: 1,
          display: "flex",
          alignItems: "center",
          gap: 1.5,
          borderBottom: 1,
          borderColor: "divider",
        }}
      >
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography variant="subtitle2" noWrap>
            {log.label}
            <Typography
              component="span"
              variant="caption"
              color="text.secondary"
              sx={{ ml: 1, fontFamily: "monospace" }}
            >
              {log.target}
            </Typography>
          </Typography>
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {new Date(log.startedAt).toLocaleString()} · {duration(log.durationSecs)} ·{" "}
            {formatSize(log.sizeBytes)} · {log.cols}×{log.rows}
            {!log.completed && " · in progress"}
          </Typography>
        </Box>
        <Tooltip title="Bookmark the line at the top of the view">
          <IconButton
            size="small"
            disabled={!handle}
            onClick={() => handle && setDialog({ kind: "bookmark", line: handle.topLine() })}
          >
            <BookmarkAddRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <Tooltip title="Export as plain file">
          <IconButton size="small" onClick={() => op.mutate(exportLog)}>
            <FileDownloadRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <Tooltip title="Delete recording">
          <IconButton size="small" onClick={() => setDialog({ kind: "delete" })}>
            <DeleteOutlineRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <IconButton size="small" onClick={onClose}>
          <CloseRoundedIcon fontSize="small" />
        </IconButton>
      </Box>
      <Box sx={{ flex: 1, minHeight: 0, display: "flex" }}>
        {body.isPending || settings.isPending ? (
          <Box sx={{ flex: 1, display: "flex", justifyContent: "center", pt: 8 }}>
            <CircularProgress size={28} />
          </Box>
        ) : body.error ? (
          <Box sx={{ flex: 1 }}>
            <EmptyState title="Could not read recording" description={errorMessage(body.error)} />
          </Box>
        ) : settings.error ? (
          <Box sx={{ flex: 1 }}>
            <EmptyState title="Settings unavailable" description={errorMessage(settings.error)} />
          </Box>
        ) : (
          <LogViewer
            text={body.data.text}
            cols={log.cols}
            settings={settings.data}
            onReady={onReady}
          />
        )}
        {(bookmarks.data ?? []).length > 0 && (
          <Box
            sx={{
              width: 240,
              flexShrink: 0,
              borderLeft: 1,
              borderColor: "divider",
              overflowY: "auto",
            }}
          >
            <Typography variant="overline" color="text.secondary" sx={{ px: 2, pt: 1 }}>
              Bookmarks
            </Typography>
            <List dense disablePadding>
              {(bookmarks.data ?? []).map((b) => (
                <ListItemButton
                  key={b.id}
                  onClick={() => handle?.scrollTo(b.offset)}
                  sx={{ pr: 0.5, "&:hover .bm-del": { opacity: 1 } }}
                >
                  <BookmarkRoundedIcon
                    sx={{ fontSize: 16, mr: 1, color: "primary.main", flexShrink: 0 }}
                  />
                  <ListItemText
                    primary={b.note}
                    secondary={`line ${b.offset + 1}`}
                    slotProps={{ primary: { noWrap: true, variant: "body2" } }}
                  />
                  <IconButton
                    size="small"
                    className="bm-del"
                    sx={{ opacity: 0 }}
                    onClick={(e) => {
                      e.stopPropagation();
                      op.mutate(async () => {
                        await ipc.logBookmarkDelete(b.id);
                        return null;
                      });
                    }}
                  >
                    <CloseRoundedIcon sx={{ fontSize: 14 }} />
                  </IconButton>
                </ListItemButton>
              ))}
            </List>
          </Box>
        )}
      </Box>

      {dialog.kind === "bookmark" && (
        <BookmarkDialog
          line={dialog.line}
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={(note) => {
            const line = dialog.line;
            op.mutate(async () => {
              await ipc.logBookmarkAdd(log.id, line, note);
              return null;
            });
          }}
        />
      )}
      {dialog.kind === "delete" && (
        <ConfirmDialog
          open
          title="Delete recording?"
          confirmLabel="Delete"
          danger
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={() =>
            op.mutate(async () => {
              await ipc.logDelete(log.id);
              onDeleted();
              return "Recording deleted";
            })
          }
        >
          The recording of <b>{log.label}</b> and its bookmarks will be removed from this device.
        </ConfirmDialog>
      )}
    </Box>
  );
}

export function LogsPage() {
  const logs = useLogs();
  const settings = useSettings();
  const [selectedId, setSelectedId] = useState<Uuid | null>(null);
  const selected = useMemo(
    () => (logs.data ?? []).find((l) => l.id === selectedId) ?? null,
    [logs.data, selectedId],
  );

  return (
    <Page>
      <PageHeader
        title="Logs"
        description={
          settings.data && !settings.data.recordSessions
            ? "Session recording is off — enable it in Settings › Sessions to capture new terminals."
            : "Encrypted recordings of your terminal sessions, stored locally."
        }
      />
      <Box sx={{ flex: 1, minHeight: 0, display: "flex" }}>
        <Box
          sx={{
            width: selected ? 320 : "100%",
            flexShrink: 0,
            borderRight: selected ? 1 : 0,
            borderColor: "divider",
            display: "flex",
            flexDirection: "column",
            minHeight: 0,
          }}
        >
          {logs.isPending ? (
            <Box sx={{ display: "flex", justifyContent: "center", pt: 8 }}>
              <CircularProgress size={28} />
            </Box>
          ) : logs.error ? (
            <EmptyState title="Could not load logs" description={errorMessage(logs.error)} />
          ) : logs.data.length === 0 ? (
            <EmptyState
              icon={<ArticleRoundedIcon />}
              title="No recordings"
              description="Turn on session recording in Settings and every terminal session will be captured here."
            />
          ) : (
            <List dense disablePadding sx={{ overflowY: "auto", py: 1 }}>
              {logs.data.map((l) => (
                <ListItemButton
                  key={l.id}
                  selected={l.id === selectedId}
                  onClick={() => setSelectedId(l.id)}
                  sx={{ mx: 1, borderRadius: 1.5, alignItems: "flex-start" }}
                >
                  <ListItemText
                    primary={
                      <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
                        <Typography variant="body2" sx={{ fontWeight: 600 }} noWrap>
                          {l.label}
                        </Typography>
                        <Chip
                          size="small"
                          variant="outlined"
                          label={l.protocol.toUpperCase()}
                          sx={{ height: 18, fontSize: 10 }}
                        />
                        {l.bookmarks > 0 && (
                          <BookmarkRoundedIcon sx={{ fontSize: 14, color: "primary.main" }} />
                        )}
                      </Box>
                    }
                    secondary={`${new Date(l.startedAt).toLocaleString()} · ${duration(
                      l.durationSecs,
                    )} · ${formatSize(l.sizeBytes)}`}
                    slotProps={{ secondary: { noWrap: true } }}
                  />
                </ListItemButton>
              ))}
            </List>
          )}
          {!selected && logs.data && logs.data.length > 0 && (
            <>
              <Divider />
              <Typography variant="caption" color="text.secondary" sx={{ px: 2.5, py: 1 }}>
                Select a recording to replay it.
              </Typography>
            </>
          )}
        </Box>
        {selected && (
          <Viewer
            key={selected.id}
            log={selected}
            onClose={() => setSelectedId(null)}
            onDeleted={() => setSelectedId(null)}
          />
        )}
      </Box>
    </Page>
  );
}
