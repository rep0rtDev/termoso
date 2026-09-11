import { useCallback, useMemo, useState } from "react";
import {
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  List,
  ListItemButton,
  ListItemText,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import ArticleRoundedIcon from "@mui/icons-material/ArticleRounded";
import BookmarkAddRoundedIcon from "@mui/icons-material/BookmarkAddRounded";
import BookmarkRoundedIcon from "@mui/icons-material/BookmarkRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import FileDownloadRoundedIcon from "@mui/icons-material/FileDownloadRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import FiberManualRecordRoundedIcon from "@mui/icons-material/FiberManualRecordRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { save as saveFile } from "@tauri-apps/plugin-dialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { goToSettings } from "@/app/navigation";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import { EntityCard, Field, IconTile, InfoBar, Loading, ToolIconButton } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useBookmarks, useLogBody, useLogs, useSettings } from "@/ipc/hooks";
import { errorMessage, type LogCard, type Uuid } from "@/ipc/types";
import { formatSize } from "@/sftp/format";
import { sizes } from "@/theme/theme";
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
        <Field label="Note">
          <TextField
            autoFocus
            fullWidth
            value={note}
            onChange={(e) => setNote(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && note.trim()) onConfirm(note.trim());
            }}
            sx={{ mt: 0.5 }}
          />
        </Field>
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
          borderColor: "border.light",
          minHeight: 48,
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
        <ToolIconButton
          title="Bookmark the line at the top of the view"
          disabled={!handle}
          onClick={() => handle && setDialog({ kind: "bookmark", line: handle.topLine() })}
        >
          <BookmarkAddRoundedIcon fontSize="small" />
        </ToolIconButton>
        <ToolIconButton title="Export as plain file" onClick={() => op.mutate(exportLog)}>
          <FileDownloadRoundedIcon fontSize="small" />
        </ToolIconButton>
        <ToolIconButton title="Delete recording" onClick={() => setDialog({ kind: "delete" })}>
          <DeleteOutlineRoundedIcon fontSize="small" />
        </ToolIconButton>
        <ToolIconButton title="Close" onClick={onClose}>
          <CloseRoundedIcon fontSize="small" />
        </ToolIconButton>
      </Box>
      <Box sx={{ flex: 1, minHeight: 0, display: "flex" }}>
        {body.isPending || settings.isPending ? (
          <Box sx={{ flex: 1 }}>
            <Loading />
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
              borderColor: "border.light",
              overflowY: "auto",
            }}
          >
            <Typography variant="subtitle2" sx={{ px: 2, pt: 1.5, pb: 0.5, display: "block" }}>
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

  const recordingOff = settings.data !== undefined && !settings.data.recordSessions;
  const list = logs.data ?? [];

  return (
    <Page>
      <PageHeader
        actions={
          recordingOff ? (
            <Button
              variant="tonal"
              startIcon={<FiberManualRecordRoundedIcon color="error" />}
              onClick={() => goToSettings("logs")}
            >
              Enable recording
            </Button>
          ) : (
            <Chip
              size="small"
              icon={<FiberManualRecordRoundedIcon />}
              label="Recording new sessions"
              sx={{ "& .MuiChip-icon": { color: "error.main", fontSize: 12 } }}
            />
          )
        }
        trailing={
          list.length > 0 && (
            <Typography variant="body2" color="text.secondary" sx={{ px: 1 }}>
              {list.length} {list.length === 1 ? "recording" : "recordings"} ·{" "}
              {formatSize(list.reduce((n, l) => n + l.sizeBytes, 0))}
            </Typography>
          )
        }
      />
      <Box sx={{ flex: 1, minHeight: 0, display: "flex" }}>
        <Box
          sx={{
            width: selected ? 320 : "100%",
            flexShrink: 0,
            borderRight: selected ? 1 : 0,
            borderColor: "border.light",
            display: "flex",
            flexDirection: "column",
            minHeight: 0,
          }}
        >
          <PageBody>
            {recordingOff && list.length > 0 && (
              <InfoBar
                action={
                  <Button size="small" onClick={() => goToSettings("logs")}>
                    Settings
                  </Button>
                }
              >
                Session recording is off — new terminals are not captured.
              </InfoBar>
            )}
            {logs.isPending ? (
              <Loading />
            ) : logs.error ? (
              <EmptyState title="Could not load logs" description={errorMessage(logs.error)} />
            ) : list.length === 0 ? (
              <EmptyState
                icon={<ArticleRoundedIcon />}
                title="No recordings"
                description="Turn on session recording in Settings and every terminal session will be captured here, encrypted and stored locally."
                action={
                  recordingOff && (
                    <Button variant="contained" onClick={() => goToSettings("logs")}>
                      Open settings
                    </Button>
                  )
                }
              />
            ) : (
              <Stack spacing={1}>
                {list.map((l) => (
                  <EntityCard
                    key={l.id}
                    dense
                    selected={l.id === selectedId}
                    onClick={() => setSelectedId(l.id)}
                    tile={
                      <IconTile size={sizes.tileSmall} tone={l.completed ? "neutral" : "danger"}>
                        <ArticleRoundedIcon />
                      </IconTile>
                    }
                    title={
                      <>
                        {l.label}
                        <Typography
                          component="span"
                          variant="caption"
                          color="text.secondary"
                          sx={{ ml: 1 }}
                        >
                          {l.protocol.toUpperCase()}
                        </Typography>
                        {!l.completed && (
                          <Typography
                            component="span"
                            variant="caption"
                            color="error.main"
                            sx={{ ml: 1 }}
                          >
                            recording
                          </Typography>
                        )}
                      </>
                    }
                    subtitle={`${new Date(l.startedAt).toLocaleString()} · ${duration(
                      l.durationSecs,
                    )} · ${formatSize(l.sizeBytes)}`}
                    trailing={
                      l.bookmarks > 0 && (
                        <Chip
                          size="small"
                          variant="outlined"
                          icon={<BookmarkRoundedIcon />}
                          label={l.bookmarks}
                        />
                      )
                    }
                  />
                ))}
              </Stack>
            )}
          </PageBody>
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
