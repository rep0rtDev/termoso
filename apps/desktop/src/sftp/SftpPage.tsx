import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Dialog,
  DialogContent,
  DialogTitle,
  InputAdornment,
  List,
  ListItemButton,
  ListItemText,
  ListSubheader,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import ReplayRoundedIcon from "@mui/icons-material/ReplayRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import * as ipc from "@/ipc/commands";
import type { Conflict, Direction, FsEntry, Uuid } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { useHosts, useSettings } from "@/ipc/hooks";
import { useSnackbar } from "@/components/Snackbar";
import { IconTile, Toolbar } from "@/components/ui";
import { HostAvatar } from "@/hosts/HostAvatar";
import { useTerminal } from "@/terminal/store";
import { ConflictDialog, type ConflictDecision, type ConflictPrompt } from "./ConflictDialog";
import { FilePane, type Side } from "./FilePane";
import { OpenWithDialog, extensionOf } from "./OpenWithDialog";
import { TransfersPanel } from "./TransfersPanel";
import { dropTargetAt, stageDrop, statPaths } from "./drop";
import { joinPath } from "./format";
import {
  closeSftp,
  editingPaths,
  openEdit,
  openSftpForHost,
  openSftpForSession,
  reconnectSftp,
  setActiveSftp,
  startTransfer,
  useSftp,
} from "./store";

interface Planned {
  entry: FsEntry;
  local: string;
  remote: string;
  existing: FsEntry | null;
}

export function SftpPage() {
  const order = useSftp((s) => s.order);
  const conns = useSftp((s) => s.conns);
  const activeId = useSftp((s) => s.activeId);
  const active = activeId ? conns[activeId] : undefined;
  const [picker, setPicker] = useState(false);
  const [localPath, setLocalPath] = useState<string | null>(null);
  const [remotePath, setRemotePath] = useState<string | null>(null);
  const [prompt, setPrompt] = useState<ConflictPrompt | null>(null);
  const [openWith, setOpenWith] = useState<{ side: Side; entry: FsEntry } | null>(null);
  const snack = useSnackbar();
  const settings = useSettings();
  const edits = useSftp((s) => s.edits);
  const editing = useMemo(
    () => (activeId ? editingPaths(Object.values(edits), activeId) : undefined),
    [edits, activeId],
  );
  /** Batches wait for each other so only one conflict dialog is up at a time. */
  const queue = useRef(Promise.resolve());

  const onLocalPath = useCallback((p: string) => setLocalPath(p), []);
  const onRemotePath = useCallback((p: string) => setRemotePath(p), []);

  const ask = (p: Omit<ConflictPrompt, "resolve">) =>
    new Promise<ConflictDecision | null>((resolve) => {
      setPrompt({
        ...p,
        resolve: (d) => {
          setPrompt(null);
          resolve(d);
        },
      });
    });

  const runBatch = async (
    sftpId: Uuid,
    direction: Direction,
    entries: FsEntry[],
    dest: string,
    temp: boolean,
  ) => {
    const items: Planned[] = await Promise.all(
      entries.map(async (entry) => {
        const local = direction === "upload" ? entry.path : joinPath(dest, entry.name);
        const remote = direction === "upload" ? joinPath(dest, entry.name) : entry.path;
        const existing = await ipc
          .transferProbe({ sftpId, direction, local, remote })
          .catch(() => null);
        return { entry, local, remote, existing };
      }),
    );
    const conflicts = items.flatMap((i) =>
      i.existing ? [{ entry: i.entry, existing: i.existing }] : [],
    );
    const decisions = new Map<string, Conflict>();
    let forAll: Conflict | null = null;
    for (const [i, c] of conflicts.entries()) {
      if (forAll) {
        decisions.set(c.entry.path, forAll);
        continue;
      }
      const d = await ask({
        direction,
        incoming: c.entry,
        existing: c.existing,
        dest,
        remaining: conflicts.length - i - 1,
      });
      if (!d) return false;
      decisions.set(c.entry.path, d.conflict);
      if (d.all) forAll = d.conflict;
    }
    for (const it of items) {
      await startTransfer({
        sftpId,
        direction,
        local: it.local,
        remote: it.remote,
        conflict: decisions.get(it.entry.path) ?? "replace",
        temp,
      });
    }
    return true;
  };

  const transfer = (direction: Direction, entries: FsEntry[], dest?: string, staging?: string) => {
    const target = dest ?? (direction === "upload" ? remotePath : localPath);
    if (active?.status !== "open" || !target || entries.length === 0) {
      if (staging) void ipc.dropAbort(staging).catch(() => undefined);
      return;
    }
    const sftpId = active.id;
    queue.current = queue.current
      .then(() => runBatch(sftpId, direction, entries, target, staging !== undefined))
      .then(async (started) => {
        if (!started && staging) await ipc.dropAbort(staging);
      })
      .catch((err: unknown) => {
        snack.error(errorMessage(err));
        if (staging) void ipc.dropAbort(staging).catch(() => undefined);
      });
  };

  const receiveFiles = (dt: DataTransfer, dest: string) => {
    stageDrop(dt)
      .then((staged) => {
        if (staged) transfer("upload", staged.entries, dest, staged.dir ?? undefined);
      })
      .catch((err: unknown) => snack.error(errorMessage(err)));
  };

  // Native drops (Tauri drag-drop handler) carry real paths and land wherever
  // the cursor is; the pane marks its drop zones with data attributes.
  const onPaths = (paths: string[], dest: string) => {
    statPaths(paths)
      .then((entries) => transfer("upload", entries, dest))
      .catch((err: unknown) => snack.error(errorMessage(err)));
  };
  const receivePaths = useRef(onPaths);
  useEffect(() => {
    receivePaths.current = onPaths;
  });
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    void getCurrentWebview()
      .onDragDropEvent((e) => {
        if (e.payload.type !== "drop" || e.payload.paths.length === 0) return;
        const { x, y } = e.payload.position.toLogical(window.devicePixelRatio);
        const target = dropTargetAt(x, y);
        if (target?.side === "remote") receivePaths.current(e.payload.paths, target.dest);
      })
      .then((off) => {
        if (alive) unlisten = off;
        else off();
      })
      .catch(() => undefined);
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  const assoc = useMemo(() => settings.data?.sftpOpenWith ?? {}, [settings.data]);
  const openEntry = (side: Side, entry: FsEntry, app: string | null) => {
    const run =
      side === "local"
        ? ipc.localOpen(entry.path, app)
        : active?.status === "open"
          ? openEdit(active.id, entry.path, app)
          : Promise.resolve();
    run.catch((err: unknown) => snack.error(errorMessage(err)));
  };
  const onOpen = (side: Side) => (entry: FsEntry, mode: "default" | "with") => {
    if (mode === "with") setOpenWith({ side, entry });
    else openEntry(side, entry, assoc[extensionOf(entry.name)] ?? null);
  };

  const remoteReady = active?.status === "open";

  return (
    <Box sx={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
      <Toolbar
        trailing={
          order.length > 0 && (
            <Stack direction="row" spacing={0.75} sx={{ overflowX: "auto", py: 0.5 }}>
              {order.map((id) => {
                const c = conns[id];
                if (!c) return null;
                return (
                  <Chip
                    key={id}
                    label={c.title}
                    size="small"
                    variant={id === activeId ? "filled" : "outlined"}
                    color={c.status === "error" ? "error" : id === activeId ? "primary" : "default"}
                    icon={
                      c.status === "connecting" ? (
                        <CircularProgress size={12} sx={{ ml: 0.75 }} />
                      ) : undefined
                    }
                    onClick={() => setActiveSftp(id)}
                    onDelete={() => void closeSftp(id)}
                  />
                );
              })}
            </Stack>
          )
        }
      >
        <Button variant="tonal" startIcon={<AddRoundedIcon />} onClick={() => setPicker(true)}>
          Connect
        </Button>
      </Toolbar>

      <Stack direction="row" sx={{ flex: 1, minHeight: 0 }}>
        <PaneFrame title="Local">
          <FilePane
            side="local"
            sftpId={null}
            initialPath={null}
            oppositePath={remoteReady ? remotePath : null}
            onPathChange={onLocalPath}
            onTransfer={(entries) => transfer("upload", entries)}
            onReceive={(entries, dest) => transfer("download", entries, dest)}
            onOpen={onOpen("local")}
          />
        </PaneFrame>
        <Box sx={{ width: "1px", bgcolor: "border.light", flexShrink: 0 }} />
        <PaneFrame title={active ? active.title : "Remote"}>
          {active && remoteReady ? (
            <FilePane
              key={active.id}
              side="remote"
              sftpId={active.id}
              initialPath={active.info?.home ?? null}
              oppositePath={localPath}
              onPathChange={onRemotePath}
              onTransfer={(entries) => transfer("download", entries)}
              onReceive={(entries, dest) => transfer("upload", entries, dest)}
              onReceiveFiles={receiveFiles}
              onOpen={onOpen("remote")}
              editing={editing}
            />
          ) : (
            <RemotePlaceholder
              status={active?.status ?? null}
              message={active?.message ?? null}
              onConnect={() => setPicker(true)}
              onRetry={active ? () => reconnectSftp(active.id) : undefined}
            />
          )}
        </PaneFrame>
      </Stack>

      <TransfersPanel />
      <ConnectPicker open={picker} onClose={() => setPicker(false)} />
      <ConflictDialog prompt={prompt} />
      <OpenWithDialog
        key={openWith ? `${openWith.side}:${openWith.entry.path}` : ""}
        entry={openWith?.entry ?? null}
        onCancel={() => setOpenWith(null)}
        onConfirm={(app) => {
          if (openWith) openEntry(openWith.side, openWith.entry, app);
          setOpenWith(null);
        }}
      />
    </Box>
  );
}

function PaneFrame({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
      <Typography
        variant="subtitle2"
        color="text.secondary"
        sx={{ px: 1.5, height: 32, display: "flex", alignItems: "center", flexShrink: 0 }}
        noWrap
      >
        {title}
      </Typography>
      <Box sx={{ flex: 1, minHeight: 0 }}>{children}</Box>
    </Box>
  );
}

function RemotePlaceholder({
  status,
  message,
  onConnect,
  onRetry,
}: {
  status: "connecting" | "open" | "error" | "closed" | null;
  message: string | null;
  onConnect: () => void;
  onRetry?: () => void;
}) {
  return (
    <Stack
      spacing={1.5}
      sx={{ alignItems: "center", justifyContent: "center", height: "100%", p: 3 }}
    >
      {status === "connecting" ? (
        <>
          <CircularProgress size={24} />
          <Typography variant="body2" color="text.secondary">
            Opening SFTP channel…
          </Typography>
        </>
      ) : (
        <>
          <IconTile size={48}>
            <FolderCopyRoundedIcon />
          </IconTile>
          <Typography
            variant="body2"
            color={status === "error" ? "error" : "text.secondary"}
            align="center"
          >
            {message ?? "Pick a host or an open SSH session to browse its files."}
          </Typography>
          <Stack direction="row" spacing={1}>
            {onRetry && (status === "error" || status === "closed") && (
              <Button startIcon={<ReplayRoundedIcon />} onClick={onRetry}>
                Retry
              </Button>
            )}
            <Button variant="contained" onClick={onConnect}>
              Connect
            </Button>
          </Stack>
        </>
      )}
    </Stack>
  );
}

function ConnectPicker({ open, onClose }: { open: boolean; onClose: () => void }) {
  const hosts = useHosts(null);
  const panes = useTerminal((s) => s.panes);
  const [filter, setFilter] = useState("");
  const q = filter.trim().toLowerCase();
  const sshHosts = (hosts.data ?? []).filter(
    (h) =>
      h.protocol === "ssh" &&
      (!q || h.label.toLowerCase().includes(q) || h.address.toLowerCase().includes(q)),
  );
  const sessions = Object.values(panes).filter(
    (p) => p.protocol === "ssh" && p.status === "connected",
  );
  const pick = (id: Uuid, kind: "host" | "session", title: string, hostId: Uuid | null) => {
    if (kind === "host") openSftpForHost(id, title);
    else openSftpForSession(id, title, hostId);
    onClose();
  };
  return (
    <Dialog open={open} onClose={onClose} maxWidth="xs" fullWidth>
      <DialogTitle>Open SFTP</DialogTitle>
      <DialogContent sx={{ p: 0 }}>
        <Box sx={{ px: 3, pb: 1 }}>
          <TextField
            autoFocus
            placeholder="Search hosts"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            slotProps={{
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchRoundedIcon fontSize="small" />
                  </InputAdornment>
                ),
              },
            }}
          />
        </Box>
        <List dense sx={{ maxHeight: 360, overflow: "auto" }}>
          {sessions.length > 0 && !q && (
            <>
              <ListSubheader disableSticky>Open sessions</ListSubheader>
              {sessions.map((p) => (
                <ListItemButton key={p.id} onClick={() => pick(p.id, "session", p.title, p.hostId)}>
                  <ListItemText primary={p.title} secondary={p.subtitle} />
                </ListItemButton>
              ))}
            </>
          )}
          <ListSubheader disableSticky>Hosts</ListSubheader>
          {sshHosts.map((h) => (
            <ListItemButton key={h.id} onClick={() => pick(h.id, "host", h.label, h.id)}>
              <Box sx={{ mr: 1.5 }}>
                <HostAvatar host={h} size={28} />
              </Box>
              <ListItemText primary={h.label} secondary={`${h.username}@${h.address}:${h.port}`} />
            </ListItemButton>
          ))}
          {sshHosts.length === 0 && (
            <Typography variant="body2" color="text.disabled" sx={{ px: 3, py: 2 }}>
              No SSH hosts saved yet.
            </Typography>
          )}
        </List>
      </DialogContent>
    </Dialog>
  );
}
