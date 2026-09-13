import { useState } from "react";
import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  Menu,
  MenuItem,
  ListItemIcon,
  ListItemText,
  Popover,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import EditOutlinedIcon from "@mui/icons-material/EditOutlined";
import MergeRoundedIcon from "@mui/icons-material/MergeRounded";
import SellOutlinedIcon from "@mui/icons-material/SellOutlined";
import BlockRoundedIcon from "@mui/icons-material/BlockRounded";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import { useDeleteTag, useMergeTags, useTags, useUpdateTag } from "@/ipc/hooks";
import { errorMessage, type TagInfo, type Uuid } from "@/ipc/types";
import { TAG_COLORS, TagDot } from "./TagChip";

const hostsLabel = (n: number) => `${n} host${n === 1 ? "" : "s"}`;

type Pending =
  | { kind: "none" }
  | { kind: "delete"; tag: TagInfo }
  | { kind: "merge"; source: TagInfo; target: TagInfo };

/**
 * Rename / recolour / merge / delete tags of the active vault. Every mutation
 * goes through the backend, which keeps host references consistent (renaming
 * onto an existing label merges; deleting unlinks the tag from its hosts).
 */
export function TagManagerDialog({
  open,
  vaultId,
  onClose,
}: {
  open: boolean;
  vaultId: Uuid | null;
  onClose: () => void;
}) {
  const snackbar = useSnackbar();
  const tags = useTags(open ? vaultId : null);
  const update = useUpdateTag();
  const remove = useDeleteTag();
  const merge = useMergeTags();

  const [editing, setEditing] = useState<{ id: Uuid; label: string } | null>(null);
  const [colorFor, setColorFor] = useState<{ tag: TagInfo; anchor: HTMLElement } | null>(null);
  const [mergeFor, setMergeFor] = useState<{ tag: TagInfo; anchor: HTMLElement } | null>(null);
  const [pending, setPending] = useState<Pending>({ kind: "none" });

  const list = [...(tags.data ?? [])].sort((a, b) => a.label.localeCompare(b.label));
  const busy = update.isPending || remove.isPending || merge.isPending;

  const commitRename = () => {
    if (!editing) return;
    const tag = list.find((t) => t.id === editing.id);
    const label = editing.label.trim();
    setEditing(null);
    if (!tag || !label || label === tag.label) return;
    const clash = list.find(
      (t) => t.id !== tag.id && t.label.toLowerCase() === label.toLowerCase(),
    );
    if (clash) {
      setPending({ kind: "merge", source: tag, target: clash });
      return;
    }
    update.mutate(
      { id: tag.id, label, color: tag.color },
      {
        onSuccess: () => snackbar.notify(`Renamed to “${label}”`),
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );
  };

  const setColor = (tag: TagInfo, color: string | null) => {
    setColorFor(null);
    if (color === tag.color) return;
    update.mutate(
      { id: tag.id, label: tag.label, color },
      { onError: (e) => snackbar.error(errorMessage(e)) },
    );
  };

  const runPending = () => {
    if (pending.kind === "delete") {
      const { tag } = pending;
      remove.mutate(tag.id, {
        onSuccess: () => {
          snackbar.notify(`Removed “${tag.label}”`, "info");
          setPending({ kind: "none" });
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      });
    } else if (pending.kind === "merge") {
      const { source, target } = pending;
      merge.mutate(
        { sources: [source.id], target: target.id },
        {
          onSuccess: (t) => {
            snackbar.notify(`Merged “${source.label}” into “${t.label}”`);
            setPending({ kind: "none" });
          },
          onError: (e) => snackbar.error(errorMessage(e)),
        },
      );
    }
  };

  return (
    <>
      <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
        <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1 }}>
          <SellOutlinedIcon fontSize="small" />
          Tags
          <Box sx={{ flex: 1 }} />
          <IconButton size="small" aria-label="Close" onClick={onClose}>
            <CloseRoundedIcon fontSize="small" />
          </IconButton>
        </DialogTitle>
        <DialogContent sx={{ pt: 0 }}>
          {list.length === 0 ? (
            <Typography variant="body2" color="text.secondary" sx={{ py: 2 }}>
              No tags yet — add them in the host editor.
            </Typography>
          ) : (
            <Box sx={{ display: "flex", flexDirection: "column", gap: 0.5 }}>
              {list.map((t) => {
                const isEditing = editing?.id === t.id;
                return (
                  <Box
                    key={t.id}
                    sx={{
                      display: "flex",
                      alignItems: "center",
                      gap: 1,
                      px: 1,
                      py: 0.5,
                      minHeight: 40,
                      borderRadius: 1.5,
                      bgcolor: "surface.high",
                      "& .tag-actions": { opacity: 0 },
                      "&:hover .tag-actions, &:focus-within .tag-actions": { opacity: 1 },
                    }}
                  >
                    <Tooltip title="Colour">
                      <IconButton
                        size="small"
                        aria-label={`Colour of ${t.label}`}
                        onClick={(e) => setColorFor({ tag: t, anchor: e.currentTarget })}
                      >
                        {t.color ? (
                          <TagDot color={t.color} size={14} />
                        ) : (
                          <Box
                            sx={{
                              width: 14,
                              height: 14,
                              borderRadius: "50%",
                              border: "1.5px dashed",
                              borderColor: "text.disabled",
                            }}
                          />
                        )}
                      </IconButton>
                    </Tooltip>
                    {isEditing ? (
                      <TextField
                        autoFocus
                        size="small"
                        value={editing.label}
                        onChange={(e) => setEditing({ id: t.id, label: e.target.value })}
                        onBlur={commitRename}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") commitRename();
                          if (e.key === "Escape") setEditing(null);
                        }}
                        sx={{ flex: 1 }}
                      />
                    ) : (
                      <Typography
                        variant="body1"
                        noWrap
                        sx={{ flex: 1, fontWeight: 500, cursor: "text" }}
                        onDoubleClick={() => setEditing({ id: t.id, label: t.label })}
                      >
                        {t.label}
                      </Typography>
                    )}
                    <Typography variant="caption" color="text.secondary" sx={{ flexShrink: 0 }}>
                      {hostsLabel(t.hosts)}
                    </Typography>
                    <Box className="tag-actions" sx={{ display: "flex", gap: 0.25, flexShrink: 0 }}>
                      <Tooltip title="Rename">
                        <IconButton
                          size="small"
                          aria-label={`Rename ${t.label}`}
                          onClick={() => setEditing({ id: t.id, label: t.label })}
                        >
                          <EditOutlinedIcon fontSize="small" />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title="Merge into…">
                        <span>
                          <IconButton
                            size="small"
                            aria-label={`Merge ${t.label}`}
                            disabled={list.length < 2}
                            onClick={(e) => setMergeFor({ tag: t, anchor: e.currentTarget })}
                          >
                            <MergeRoundedIcon fontSize="small" />
                          </IconButton>
                        </span>
                      </Tooltip>
                      <Tooltip title="Delete">
                        <IconButton
                          size="small"
                          aria-label={`Delete ${t.label}`}
                          onClick={() => setPending({ kind: "delete", tag: t })}
                        >
                          <DeleteOutlineRoundedIcon fontSize="small" />
                        </IconButton>
                      </Tooltip>
                    </Box>
                  </Box>
                );
              })}
            </Box>
          )}
          <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 1.5 }}>
            Double-click a tag to rename it. Renaming onto an existing tag merges the two.
          </Typography>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={onClose} color="inherit">
            Done
          </Button>
        </DialogActions>
      </Dialog>

      <Popover
        open={Boolean(colorFor)}
        anchorEl={colorFor?.anchor}
        onClose={() => setColorFor(null)}
        anchorOrigin={{ vertical: "bottom", horizontal: "left" }}
      >
        {colorFor && (
          <Box sx={{ p: 1, display: "flex", gap: 0.5, alignItems: "center" }}>
            <Tooltip title="No colour">
              <IconButton
                size="small"
                aria-label="No colour"
                onClick={() => setColor(colorFor.tag, null)}
              >
                <BlockRoundedIcon fontSize="small" />
              </IconButton>
            </Tooltip>
            {TAG_COLORS.map((c) => {
              const on = colorFor.tag.color?.toLowerCase() === c;
              return (
                <IconButton
                  key={c}
                  size="small"
                  aria-label={c}
                  onClick={() => setColor(colorFor.tag, c)}
                  sx={{ p: 0.5 }}
                >
                  <Box
                    sx={{
                      width: 20,
                      height: 20,
                      borderRadius: "50%",
                      bgcolor: c,
                      display: "grid",
                      placeItems: "center",
                      color: "#fff",
                    }}
                  >
                    {on && <CheckRoundedIcon sx={{ fontSize: 14 }} />}
                  </Box>
                </IconButton>
              );
            })}
          </Box>
        )}
      </Popover>

      <Menu
        open={Boolean(mergeFor)}
        anchorEl={mergeFor?.anchor}
        onClose={() => setMergeFor(null)}
        slotProps={{ paper: { sx: { minWidth: 220 } } }}
      >
        <MenuItem disabled dense>
          <ListItemText
            primary={`Merge “${mergeFor?.tag.label ?? ""}” into…`}
            slotProps={{ primary: { variant: "caption" } }}
          />
        </MenuItem>
        {list
          .filter((t) => t.id !== mergeFor?.tag.id)
          .map((t) => (
            <MenuItem
              key={t.id}
              onClick={() => {
                if (mergeFor) setPending({ kind: "merge", source: mergeFor.tag, target: t });
                setMergeFor(null);
              }}
            >
              <ListItemIcon>
                <TagDot color={t.color ?? "transparent"} size={10} />
              </ListItemIcon>
              <ListItemText primary={t.label} secondary={hostsLabel(t.hosts)} />
            </MenuItem>
          ))}
      </Menu>

      <ConfirmDialog
        open={pending.kind === "delete"}
        title={pending.kind === "delete" ? `Delete tag “${pending.tag.label}”?` : ""}
        danger
        confirmLabel="Delete"
        busy={busy}
        onCancel={() => setPending({ kind: "none" })}
        onConfirm={runPending}
      >
        {pending.kind === "delete" && pending.tag.hosts > 0
          ? `The tag is removed from ${hostsLabel(pending.tag.hosts)}. The hosts themselves stay.`
          : "No host carries this tag."}
      </ConfirmDialog>

      <ConfirmDialog
        open={pending.kind === "merge"}
        title={
          pending.kind === "merge"
            ? `Merge “${pending.source.label}” into “${pending.target.label}”?`
            : ""
        }
        confirmLabel="Merge"
        busy={busy}
        onCancel={() => setPending({ kind: "none" })}
        onConfirm={runPending}
      >
        {pending.kind === "merge" &&
          `${hostsLabel(pending.source.hosts)} tagged “${pending.source.label}” ${pending.source.hosts === 1 ? "gets" : "get"} “${pending.target.label}” instead; the old tag is removed.`}
      </ConfirmDialog>
    </>
  );
}
