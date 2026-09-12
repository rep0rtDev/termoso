import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type DragEvent,
  type MouseEvent,
} from "react";
import {
  Box,
  Button,
  CircularProgress,
  Divider,
  InputBase,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tooltip,
  Typography,
} from "@mui/material";
import ArrowUpwardRoundedIcon from "@mui/icons-material/ArrowUpwardRounded";
import HomeRoundedIcon from "@mui/icons-material/HomeRounded";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import CreateNewFolderRoundedIcon from "@mui/icons-material/CreateNewFolderRounded";
import VisibilityRoundedIcon from "@mui/icons-material/VisibilityRounded";
import VisibilityOffRoundedIcon from "@mui/icons-material/VisibilityOffRounded";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import InsertDriveFileOutlinedIcon from "@mui/icons-material/InsertDriveFileOutlined";
import LinkRoundedIcon from "@mui/icons-material/LinkRounded";
import DriveFileRenameOutlineRoundedIcon from "@mui/icons-material/DriveFileRenameOutlineRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import UploadRoundedIcon from "@mui/icons-material/UploadRounded";
import DownloadRoundedIcon from "@mui/icons-material/DownloadRounded";
import OpenInNewRoundedIcon from "@mui/icons-material/OpenInNewRounded";
import AppsRoundedIcon from "@mui/icons-material/AppsRounded";
import EditRoundedIcon from "@mui/icons-material/EditRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as ipc from "@/ipc/commands";
import type { FsEntry, Listing, Uuid } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { useSnackbar } from "@/components/Snackbar";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ToolIconButton } from "@/components/ui";
import { monoFontFamily, sizes } from "@/theme/theme";
import { ChmodDialog, NameDialog } from "./dialogs";
import { DROP_DEST_ATTR, DROP_SIDE_ATTR, PANE_MIME, hasOsFiles } from "./drop";
import { formatMode, formatMtime, formatSize, isHidden, joinPath, sortEntries } from "./format";
import { fsQueryKey } from "./store";

export type Side = "local" | "remote";

/** Payload of a drag that started in a pane. */
interface DragPayload {
  side: Side;
  entries: FsEntry[];
}

const mimeFor = (side: Side) => `${PANE_MIME}-${side}`;

interface Props {
  side: Side;
  /** Remote connection; `null` for the local side. */
  sftpId: Uuid | null;
  /** Directory to start in; `null` = home. */
  initialPath: string | null;
  /** Current directory of the opposite pane (transfer destination). */
  oppositePath: string | null;
  onPathChange: (path: string) => void;
  /** Send entries of this pane to the opposite pane's current directory. */
  onTransfer: (entries: FsEntry[]) => void;
  /** Entries dragged over from the opposite pane and dropped on `dest`. */
  onReceive: (entries: FsEntry[], dest: string) => void;
  /** Files dropped from the OS onto `dest` (remote side only). */
  onReceiveFiles?: (dt: DataTransfer, dest: string) => void;
  /** Open a file locally (`with` asks for the application first). */
  onOpen: (entry: FsEntry, mode: "default" | "with") => void;
  /** Paths currently open for editing (remote side). */
  editing?: ReadonlySet<string>;
  disabled?: boolean;
}

interface FsApi {
  list: (path: string | null) => Promise<Listing>;
  mkdir: (path: string) => Promise<null>;
  rename: (from: string, to: string) => Promise<null>;
  remove: (path: string, recursive: boolean) => Promise<null>;
  chmod: ((path: string, mode: number) => Promise<null>) | null;
}

function apiFor(side: Side, sftpId: Uuid | null): FsApi {
  if (side === "remote" && sftpId) {
    return {
      list: (p) => ipc.sftpList(sftpId, p),
      mkdir: (p) => ipc.sftpMkdir(sftpId, p),
      rename: (a, b) => ipc.sftpRename(sftpId, a, b),
      remove: (p, r) => ipc.sftpRemove(sftpId, p, r),
      chmod: (p, m) => ipc.sftpChmod(sftpId, p, m),
    };
  }
  return {
    list: ipc.localList,
    mkdir: ipc.localMkdir,
    rename: ipc.localRename,
    remove: ipc.localRemove,
    chmod: null,
  };
}

type Dialog =
  | { kind: "mkdir" }
  | { kind: "rename"; entry: FsEntry }
  | { kind: "delete"; entries: FsEntry[] }
  | { kind: "chmod"; entry: FsEntry }
  | null;

export function FilePane(props: Props) {
  const {
    side,
    sftpId,
    initialPath,
    oppositePath,
    onPathChange,
    onTransfer,
    onReceive,
    onReceiveFiles,
    onOpen,
    editing,
    disabled = false,
  } = props;
  const api = useMemo(() => apiFor(side, sftpId), [side, sftpId]);
  const [path, setPath] = useState<string | null>(initialPath);
  const [pathDraft, setPathDraft] = useState<string | null>(null);
  const [showHidden, setShowHidden] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [menu, setMenu] = useState<{ x: number; y: number; entry: FsEntry | null } | null>(null);
  const [dialog, setDialog] = useState<Dialog>(null);
  /** Directory a drag is currently hovering (pane root or a folder row). */
  const [dropOver, setDropOver] = useState<string | null>(null);
  const dragDepth = useRef(0);
  const qc = useQueryClient();
  const snack = useSnackbar();

  const listing = useQuery({
    queryKey: fsQueryKey(side, sftpId, path),
    queryFn: () => api.list(path),
    enabled: !disabled,
    staleTime: 5_000,
  });

  const resolvedPath = listing.data?.path ?? null;
  useEffect(() => {
    if (resolvedPath !== null) onPathChange(resolvedPath);
  }, [resolvedPath, onPathChange]);
  const pathInput = pathDraft ?? resolvedPath ?? "";

  const entries = useMemo(() => {
    const all = listing.data?.entries ?? [];
    return sortEntries(showHidden ? all : all.filter((e) => !isHidden(e)));
  }, [listing.data, showHidden]);

  const navigate = useCallback((next: string | null) => {
    setPath(next);
    setPathDraft(null);
    setSelected(new Set());
  }, []);

  const refresh = () => void qc.invalidateQueries({ queryKey: fsQueryKey(side, sftpId, path) });

  const mutate = useMutation({
    mutationFn: (fn: () => Promise<unknown>) => fn(),
    onSuccess: () => {
      setDialog(null);
      setSelected(new Set());
      refresh();
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  const selectedEntries = entries.filter((e) => selected.has(e.path));

  const onRowClick = (e: MouseEvent, entry: FsEntry) => {
    setSelected((prev) => {
      if (e.ctrlKey || e.metaKey) {
        const next = new Set(prev);
        if (next.has(entry.path)) next.delete(entry.path);
        else next.add(entry.path);
        return next;
      }
      if (e.shiftKey && prev.size > 0) {
        const idx = entries.findIndex((x) => x.path === entry.path);
        const anchor = entries.findIndex((x) => prev.has(x.path));
        const [a, b] = idx < anchor ? [idx, anchor] : [anchor, idx];
        return new Set(entries.slice(a, b + 1).map((x) => x.path));
      }
      return new Set([entry.path]);
    });
  };

  const isDirLike = (entry: FsEntry) =>
    entry.kind === "dir" || (entry.kind === "symlink" && entry.link_target !== null);

  const openEntry = (entry: FsEntry) => {
    if (isDirLike(entry)) navigate(entry.path);
    else onOpen(entry, "default");
  };

  // ── drag & drop ──
  const canReceive = !disabled && oppositePath !== null;
  const accepts = (dt: DataTransfer) =>
    canReceive &&
    (Array.from(dt.types).includes(mimeFor(side === "local" ? "remote" : "local")) ||
      (onReceiveFiles !== undefined && hasOsFiles(dt)));

  const osZone = canReceive && onReceiveFiles !== undefined;
  const dropZoneAttrs = (dest: string) => ({
    [DROP_SIDE_ATTR]: side,
    [DROP_DEST_ATTR]: dest,
  });

  const onRowDragStart = (e: DragEvent, entry: FsEntry) => {
    const targets = selected.has(entry.path) ? selectedEntries : [entry];
    if (!selected.has(entry.path)) setSelected(new Set([entry.path]));
    const payload: DragPayload = { side, entries: targets };
    e.dataTransfer.setData(mimeFor(side), JSON.stringify(payload));
    e.dataTransfer.effectAllowed = "copy";
  };

  const onDragOverZone = (e: DragEvent, dest: string | null) => {
    if (!accepts(e.dataTransfer) || dest === null) return;
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = "copy";
    if (dropOver !== dest) setDropOver(dest);
  };

  const onDragEnterPane = (e: DragEvent) => {
    if (!accepts(e.dataTransfer)) return;
    dragDepth.current += 1;
  };

  const onDragLeavePane = (e: DragEvent) => {
    if (!accepts(e.dataTransfer)) return;
    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (dragDepth.current === 0) setDropOver(null);
  };

  const onDropZone = (e: DragEvent, dest: string | null) => {
    dragDepth.current = 0;
    setDropOver(null);
    if (!accepts(e.dataTransfer) || dest === null) return;
    e.preventDefault();
    e.stopPropagation();
    const raw = e.dataTransfer.getData(mimeFor(side === "local" ? "remote" : "local"));
    if (raw) {
      try {
        const payload = JSON.parse(raw) as DragPayload;
        if (payload.entries.length > 0) onReceive(payload.entries, dest);
      } catch {
        // not ours
      }
      return;
    }
    if (onReceiveFiles && hasOsFiles(e.dataTransfer)) onReceiveFiles(e.dataTransfer, dest);
  };

  const onContext = (e: MouseEvent, entry: FsEntry | null) => {
    e.preventDefault();
    if (entry && !selected.has(entry.path)) setSelected(new Set([entry.path]));
    setMenu({ x: e.clientX, y: e.clientY, entry });
  };

  const transferLabel = side === "local" ? "Upload" : "Download";
  const TransferIcon = side === "local" ? UploadRoundedIcon : DownloadRoundedIcon;
  const menuTargets = menu?.entry ? (selected.size > 1 ? selectedEntries : [menu.entry]) : [];
  const menuSingle = menuTargets.length === 1 ? menuTargets[0] : undefined;
  const menuFile = menuSingle && !isDirLike(menuSingle) ? menuSingle : undefined;
  const chmod = api.chmod;
  const currentDir = listing.data?.path ?? null;
  const paneOver = dropOver !== null && dropOver === currentDir;

  return (
    <Box
      sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", height: "100%" }}
      onContextMenu={(e) => {
        if (e.target === e.currentTarget) onContext(e, null);
      }}
    >
      <Stack
        direction="row"

        spacing={0.25}
        sx={{
          alignItems: "center",
          px: 0.75,
          height: 44,
          borderBottom: 1,
          borderColor: "border.light",
          flexShrink: 0,
        }}
      >
        <ToolIconButton
          title="Parent directory"
          disabled={disabled || !listing.data?.parent}
          onClick={() => navigate(listing.data?.parent ?? null)}
        >
          <ArrowUpwardRoundedIcon fontSize="small" />
        </ToolIconButton>
        <ToolIconButton title="Home" disabled={disabled} onClick={() => navigate(null)}>
          <HomeRoundedIcon fontSize="small" />
        </ToolIconButton>
        <InputBase
          value={pathInput}
          disabled={disabled}
          onChange={(e) => setPathDraft(e.target.value)}
          onBlur={() => setPathDraft(null)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              const trimmed = pathInput.trim();
              navigate(trimmed.length > 0 ? trimmed : null);
            }
            if (e.key === "Escape") setPathDraft(null);
          }}
          sx={{
            flex: 1,
            mx: 0.5,
            px: 1.25,
            height: sizes.control,
            fontSize: 12.5,
            fontFamily: monoFontFamily,
            borderRadius: 1.5,
            bgcolor: "surface.high",
            "&.Mui-focused": { outline: "1px solid", outlineColor: "primary.main" },
          }}
        />
        <ToolIconButton title="Refresh" disabled={disabled} onClick={refresh}>
          <RefreshRoundedIcon fontSize="small" />
        </ToolIconButton>
        <ToolIconButton
          title="New folder"
          disabled={disabled}
          onClick={() => setDialog({ kind: "mkdir" })}
        >
          <CreateNewFolderRoundedIcon fontSize="small" />
        </ToolIconButton>
        <ToolIconButton
          title={showHidden ? "Hide dotfiles" : "Show dotfiles"}
          active={showHidden}
          onClick={() => setShowHidden((v) => !v)}
        >
          {showHidden ? (
            <VisibilityRoundedIcon fontSize="small" />
          ) : (
            <VisibilityOffRoundedIcon fontSize="small" />
          )}
        </ToolIconButton>
        <Divider orientation="vertical" flexItem sx={{ my: 1.25, mx: 0.5 }} />
        <Tooltip title={`${transferLabel} selected to ${oppositePath ?? "…"}`}>
          <span>
            <Button
              variant="tonal"
              startIcon={<TransferIcon />}
              disabled={disabled || selectedEntries.length === 0 || oppositePath === null}
              onClick={() => onTransfer(selectedEntries)}
              sx={{ minWidth: 0, px: 1.25 }}
            >
              {transferLabel}
            </Button>
          </span>
        </Tooltip>
      </Stack>

      <Box
        sx={{
          flex: 1,
          minHeight: 0,
          overflow: "auto",
          position: "relative",
          outline: paneOver ? "2px solid" : "2px solid transparent",
          outlineColor: paneOver ? "primary.main" : "transparent",
          outlineOffset: -2,
          bgcolor: paneOver ? "action.hover" : undefined,
          transition: "background-color 120ms",
        }}
        onContextMenu={(e) => {
          if (e.target === e.currentTarget) onContext(e, null);
        }}
        onClick={(e) => {
          if (e.target === e.currentTarget) setSelected(new Set());
        }}
        onDragEnter={onDragEnterPane}
        onDragLeave={onDragLeavePane}
        onDragOver={(e) => onDragOverZone(e, currentDir)}
        onDrop={(e) => onDropZone(e, currentDir)}
        {...(osZone && currentDir ? dropZoneAttrs(currentDir) : {})}
      >
        {listing.isPending && !disabled && (
          <Stack sx={{ alignItems: "center", pt: 6 }}>
            <CircularProgress size={22} />
          </Stack>
        )}
        {listing.error && (
          <Typography variant="body2" color="error" sx={{ p: 2, userSelect: "text" }}>
            {errorMessage(listing.error)}
          </Typography>
        )}
        {listing.data && (
          <Table size="small" stickyHeader sx={{ tableLayout: "fixed" }}>
            <TableHead>
              <TableRow
                sx={{
                  "& th": {
                    py: 0.5,
                    fontSize: 12,
                    color: "text.secondary",
                    bgcolor: "surface.base",
                    borderColor: "border.light",
                  },
                }}
              >
                <TableCell>Name</TableCell>
                <TableCell align="right" sx={{ width: 84 }}>
                  Size
                </TableCell>
                <TableCell sx={{ width: 140 }}>Modified</TableCell>
                <TableCell sx={{ width: 104 }}>Permissions</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {entries.map((e) => {
                const sel = selected.has(e.path);
                const dir = isDirLike(e);
                const over = dir && dropOver === e.path;
                const inEdit = editing?.has(e.path) ?? false;
                return (
                  <TableRow
                    key={e.path}
                    hover
                    selected={sel}
                    draggable={!disabled}
                    onDragStart={(ev) => onRowDragStart(ev, e)}
                    onDragOver={dir ? (ev) => onDragOverZone(ev, e.path) : undefined}
                    onDrop={dir ? (ev) => onDropZone(ev, e.path) : undefined}
                    {...(osZone && dir ? dropZoneAttrs(e.path) : {})}
                    onClick={(ev) => onRowClick(ev, e)}
                    onDoubleClick={() => openEntry(e)}
                    onContextMenu={(ev) => onContext(ev, e)}
                    sx={{
                      cursor: "default",
                      "& td": {
                        py: 0.4,
                        fontSize: 13,
                        borderColor: "border.light",
                        ...(over && { bgcolor: "action.selected" }),
                      },
                      ...(over && {
                        "& td:first-of-type": { boxShadow: "inset 2px 0 0", color: "primary.main" },
                      }),
                    }}
                  >
                    <TableCell sx={{ overflow: "hidden" }}>
                      <Stack direction="row" spacing={1} sx={{ alignItems: "center", minWidth: 0 }}>
                        <EntryIcon entry={e} />
                        <Typography variant="body2" noWrap sx={{ fontSize: 13 }}>
                          {e.name}
                        </Typography>
                        {inEdit && (
                          <Tooltip title="Open in a local application — saves are uploaded back">
                            <EditRoundedIcon sx={{ fontSize: 14, color: "primary.main" }} />
                          </Tooltip>
                        )}
                        {e.link_target && (
                          <Typography variant="caption" color="text.disabled" noWrap>
                            → {e.link_target}
                          </Typography>
                        )}
                      </Stack>
                    </TableCell>
                    <TableCell
                      align="right"
                      sx={{ color: "text.secondary", fontVariantNumeric: "tabular-nums" }}
                    >
                      {e.kind === "dir" ? "" : formatSize(e.size)}
                    </TableCell>
                    <TableCell sx={{ color: "text.secondary", whiteSpace: "nowrap" }}>
                      {formatMtime(e.mtime)}
                    </TableCell>
                    <TableCell
                      sx={{ color: "text.secondary", fontFamily: monoFontFamily, fontSize: 12 }}
                    >
                      {formatMode(e.mode, e.kind)}
                    </TableCell>
                  </TableRow>
                );
              })}
              {entries.length === 0 && (
                <TableRow>
                  <TableCell
                    colSpan={4}
                    sx={{ color: "text.disabled", textAlign: "center", py: 4 }}
                  >
                    {canReceive ? "Empty directory — drop files here" : "Empty directory"}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        )}
      </Box>

      <Stack
        direction="row"
        sx={{
          px: 1.5,
          height: 24,
          alignItems: "center",
          borderTop: 1,
          borderColor: "border.light",
        }}
      >
        <Typography variant="caption" color="text.secondary">
          {entries.length} {entries.length === 1 ? "item" : "items"}
          {selected.size > 0 ? ` · ${selected.size} selected` : ""}
        </Typography>
      </Stack>

      <Menu
        open={menu !== null}
        onClose={() => setMenu(null)}
        anchorReference="anchorPosition"
        anchorPosition={menu ? { top: menu.y, left: menu.x } : undefined}
        onClick={() => setMenu(null)}
        slotProps={{ paper: { sx: { minWidth: 200 } } }}
      >
        {menuFile && (
          <MenuItem onClick={() => onOpen(menuFile, "default")}>
            <ListItemIcon>
              <OpenInNewRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Open</ListItemText>
          </MenuItem>
        )}
        {menuFile && (
          <MenuItem onClick={() => onOpen(menuFile, "with")}>
            <ListItemIcon>
              <AppsRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Open with…</ListItemText>
          </MenuItem>
        )}
        {menuFile && <Divider />}
        {menu?.entry && (
          <MenuItem disabled={oppositePath === null} onClick={() => onTransfer(menuTargets)}>
            <ListItemIcon>
              <TransferIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>
              {transferLabel}
              {menuTargets.length > 1 ? ` ${menuTargets.length} items` : ""}
            </ListItemText>
          </MenuItem>
        )}
        {menuSingle && (
          <MenuItem onClick={() => setDialog({ kind: "rename", entry: menuSingle })}>
            <ListItemIcon>
              <DriveFileRenameOutlineRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Rename</ListItemText>
          </MenuItem>
        )}
        {menuSingle && chmod && (
          <MenuItem onClick={() => setDialog({ kind: "chmod", entry: menuSingle })}>
            <ListItemIcon>
              <LockOutlinedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Permissions…</ListItemText>
          </MenuItem>
        )}
        {menuSingle && (
          <MenuItem
            onClick={() => {
              void navigator.clipboard.writeText(menuSingle.path).catch(() => undefined);
            }}
          >
            <ListItemIcon>
              <ContentCopyRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Copy path</ListItemText>
          </MenuItem>
        )}
        {menu?.entry && <Divider />}
        {menu?.entry && (
          <MenuItem
            onClick={() => setDialog({ kind: "delete", entries: menuTargets })}
            sx={{ color: "error.main" }}
          >
            <ListItemIcon>
              <DeleteOutlineRoundedIcon fontSize="small" color="error" />
            </ListItemIcon>
            <ListItemText>
              Delete{menuTargets.length > 1 ? ` ${menuTargets.length} items` : ""}
            </ListItemText>
          </MenuItem>
        )}
        {!menu?.entry && (
          <MenuItem onClick={() => setDialog({ kind: "mkdir" })}>
            <ListItemIcon>
              <CreateNewFolderRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>New folder</ListItemText>
          </MenuItem>
        )}
        {!menu?.entry && (
          <MenuItem onClick={refresh}>
            <ListItemIcon>
              <RefreshRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Refresh</ListItemText>
          </MenuItem>
        )}
      </Menu>

      {dialog?.kind === "mkdir" && listing.data && (
        <NameDialog
          open
          title="New folder"
          label="Folder name"
          confirmLabel="Create"
          busy={mutate.isPending}
          onCancel={() => setDialog(null)}
          onConfirm={(name) => mutate.mutate(() => api.mkdir(joinPath(listing.data.path, name)))}
        />
      )}
      {dialog?.kind === "rename" && listing.data && (
        <NameDialog
          open
          title="Rename"
          label="New name"
          initial={dialog.entry.name}
          confirmLabel="Rename"
          busy={mutate.isPending}
          onCancel={() => setDialog(null)}
          onConfirm={(name) =>
            mutate.mutate(() => api.rename(dialog.entry.path, joinPath(listing.data.path, name)))
          }
        />
      )}
      {dialog?.kind === "chmod" && chmod && (
        <ChmodDialog
          open
          name={dialog.entry.name}
          mode={dialog.entry.mode ?? 0o644}
          busy={mutate.isPending}
          onCancel={() => setDialog(null)}
          onConfirm={(mode) => mutate.mutate(() => chmod(dialog.entry.path, mode))}
        />
      )}
      <ConfirmDialog
        open={dialog?.kind === "delete"}
        title={
          dialog?.kind === "delete" && dialog.entries.length > 1
            ? `Delete ${dialog.entries.length} items?`
            : "Delete?"
        }
        confirmLabel="Delete"
        danger
        busy={mutate.isPending}
        onCancel={() => setDialog(null)}
        onConfirm={() => {
          if (dialog?.kind !== "delete") return;
          mutate.mutate(async () => {
            for (const e of dialog.entries) await api.remove(e.path, e.kind === "dir");
          });
        }}
      >
        {dialog?.kind === "delete" && (
          <Box component="ul" sx={{ m: 0, pl: 2.5, maxHeight: 160, overflow: "auto" }}>
            {dialog.entries.map((e) => (
              <li key={e.path}>
                {e.name}
                {e.kind === "dir" ? "/ (and everything inside)" : ""}
              </li>
            ))}
          </Box>
        )}
        <Typography variant="body2" sx={{ mt: 1 }}>
          This cannot be undone.
        </Typography>
      </ConfirmDialog>
    </Box>
  );
}

function EntryIcon({ entry }: { entry: FsEntry }) {
  if (entry.kind === "dir")
    return <FolderRoundedIcon fontSize="small" sx={{ color: "secondary.main" }} />;
  if (entry.kind === "symlink")
    return <LinkRoundedIcon fontSize="small" sx={{ color: "text.disabled" }} />;
  return <InsertDriveFileOutlinedIcon fontSize="small" sx={{ color: "text.disabled" }} />;
}
