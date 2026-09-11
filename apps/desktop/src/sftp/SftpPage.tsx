import { useCallback, useState } from "react";
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
import type { FsEntry, Uuid } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { useHosts } from "@/ipc/hooks";
import { useSnackbar } from "@/components/Snackbar";
import { IconTile, Toolbar } from "@/components/ui";
import { HostAvatar } from "@/hosts/HostAvatar";
import { useTerminal } from "@/terminal/store";
import { FilePane } from "./FilePane";
import { TransfersPanel } from "./TransfersPanel";
import { baseName, joinPath } from "./format";
import {
  closeSftp,
  openSftpForHost,
  openSftpForSession,
  reconnectSftp,
  setActiveSftp,
  startTransfer,
  useSftp,
} from "./store";

export function SftpPage() {
  const order = useSftp((s) => s.order);
  const conns = useSftp((s) => s.conns);
  const activeId = useSftp((s) => s.activeId);
  const active = activeId ? conns[activeId] : undefined;
  const [picker, setPicker] = useState(false);
  const [localPath, setLocalPath] = useState<string | null>(null);
  const [remotePath, setRemotePath] = useState<string | null>(null);
  const snack = useSnackbar();

  const onLocalPath = useCallback((p: string) => setLocalPath(p), []);
  const onRemotePath = useCallback((p: string) => setRemotePath(p), []);

  const transfer = (direction: "upload" | "download", entries: FsEntry[]) => {
    if (active?.status !== "open") return;
    const dest = direction === "upload" ? remotePath : localPath;
    if (!dest) return;
    for (const e of entries) {
      const local = direction === "upload" ? e.path : joinPath(dest, baseName(e.path));
      const remote = direction === "upload" ? joinPath(dest, baseName(e.path)) : e.path;
      startTransfer({ sftpId: active.id, direction, local, remote }).catch((err: unknown) =>
        snack.error(errorMessage(err)),
      );
    }
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
