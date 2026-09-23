import { useState } from "react";
import {
  Box,
  Button,
  IconButton,
  InputAdornment,
  Popover,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import EditOutlinedIcon from "@mui/icons-material/EditOutlined";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import TuneRoundedIcon from "@mui/icons-material/TuneRounded";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import { useCreateTag, useDeleteTag, useTags, useUpdateTag } from "@/ipc/hooks";
import { errorMessage, type TagInfo, type Uuid } from "@/ipc/types";
import { tr } from "@/i18n";

const hostsLabel = (n: number) => `${n} host${n === 1 ? "" : "s"}`;

/** Termius' round mark: grey ring, filled blue with a white check when on. */
function RoundCheck({ on, label, onToggle }: { on: boolean; label: string; onToggle: () => void }) {
  return (
    <Box
      component="button"
      type="button"
      role="checkbox"
      aria-checked={on}
      aria-label={label}
      onClick={onToggle}
      sx={{
        all: "unset",
        boxSizing: "border-box",
        width: 28,
        height: 28,
        flexShrink: 0,
        display: "grid",
        placeItems: "center",
        cursor: "pointer",
        borderRadius: "50%",
        "&:focus-visible": { outline: "2px solid", outlineColor: "info.main", outlineOffset: -4 },
      }}
    >
      <Box
        sx={{
          width: 18,
          height: 18,
          borderRadius: "50%",
          display: "grid",
          placeItems: "center",
          boxSizing: "border-box",
          border: on ? "none" : "1.5px solid",
          borderColor: "text.disabled",
          bgcolor: on ? "info.main" : "transparent",
          color: "#fff",
          transition: "background-color 80ms",
        }}
      >
        {on && <CheckRoundedIcon sx={{ fontSize: 14 }} />}
      </Box>
    </Box>
  );
}

/**
 * Termius' tag popover: search on top, one checkable row per tag with its
 * host count, rename / delete on hover. Used both as the Hosts toolbar filter
 * and as the tag picker in Host Details (`allowCreate` adds "Create “…”").
 */
export function TagsPopover({
  anchor,
  vaultId,
  selected,
  onToggle,
  onClose,
  onManage,
  onRenamed,
  onDeleted,
  allowCreate = false,
  onCreated,
  title,
}: {
  anchor: HTMLElement | null;
  vaultId: Uuid | null;
  selected: ReadonlySet<Uuid>;
  onToggle: (tag: TagInfo, on: boolean) => void;
  onClose: () => void;
  onManage?: () => void;
  onRenamed?: (tag: TagInfo, label: string) => void;
  onDeleted?: (tag: TagInfo) => void;
  allowCreate?: boolean;
  onCreated?: (id: Uuid) => void;
  title?: string;
}) {
  const snackbar = useSnackbar();
  const open = anchor !== null;
  const tags = useTags(open ? vaultId : null);
  const update = useUpdateTag();
  const remove = useDeleteTag();
  const create = useCreateTag();
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<{ id: Uuid; label: string } | null>(null);
  const [confirm, setConfirm] = useState<TagInfo | null>(null);

  const all = [...(tags.data ?? [])].sort((a, b) => a.label.localeCompare(b.label));
  const q = query.trim().toLowerCase();
  const list = q ? all.filter((t) => t.label.toLowerCase().includes(q)) : all;
  const exact = all.find((t) => t.label.toLowerCase() === q);
  const canCreate = allowCreate && vaultId !== null && q.length > 0 && !exact && !create.isPending;

  const close = () => {
    setQuery("");
    setEditing(null);
    onClose();
  };

  const createTag = () => {
    if (!canCreate || !vaultId) return;
    create.mutate(
      { vaultId, label: query.trim() },
      {
        onSuccess: (e) => {
          onCreated?.(e.id);
          setQuery("");
        },
        onError: (err) => snackbar.error(errorMessage(err)),
      },
    );
  };

  const commitRename = () => {
    if (!editing) return;
    const tag = all.find((t) => t.id === editing.id);
    const label = editing.label.trim();
    setEditing(null);
    if (!tag || !label || label === tag.label) return;
    update.mutate(
      { id: tag.id, label, color: tag.color },
      {
        onSuccess: (t) => {
          onRenamed?.(tag, t.label);
          if (t.id !== tag.id) snackbar.notify(tr("Merged into “{label}”", { label: t.label }));
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );
  };

  const runDelete = () => {
    if (!confirm) return;
    const tag = confirm;
    remove.mutate(tag.id, {
      onSuccess: () => {
        onDeleted?.(tag);
        setConfirm(null);
        snackbar.notify(tr("Removed “{label}”", { label: tag.label }), "info");
      },
      onError: (e) => snackbar.error(errorMessage(e)),
    });
  };

  return (
    <>
      <Popover
        open={open}
        anchorEl={anchor}
        onClose={close}
        anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
        transformOrigin={{ vertical: "top", horizontal: "right" }}
        slotProps={{ paper: { sx: { width: 300, display: "flex", flexDirection: "column" } } }}
      >
        <Box sx={{ px: 1.5, pt: 1.5, pb: 1 }}>
          {title && (
            <Typography
              variant="caption"
              color="text.secondary"
              sx={{ display: "block", mb: 0.75 }}
            >
              {title}
            </Typography>
          )}
          <TextField
            autoFocus
            size="small"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={allowCreate ? tr("Search or add a tag") : tr("Search tags")}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                if (canCreate) createTag();
                else if (list.length === 1 && list[0]) onToggle(list[0], !selected.has(list[0].id));
              }
              if (e.key === "Escape") close();
            }}
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
        <Box sx={{ maxHeight: 320, overflowY: "auto", px: 0.75, pb: 0.75 }}>
          {canCreate && (
            <Box
              component="button"
              type="button"
              onClick={createTag}
              sx={{
                all: "unset",
                boxSizing: "border-box",
                display: "flex",
                alignItems: "center",
                gap: 1,
                width: "100%",
                height: 36,
                px: 1,
                borderRadius: 1.5,
                cursor: "pointer",
                color: "primary.main",
                "&:hover": { bgcolor: "surface.highest" },
              }}
            >
              <AddRoundedIcon fontSize="small" />
              <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
                {tr("Create “{tag}”", { tag: query.trim() })}
              </Typography>
            </Box>
          )}
          {list.length === 0 && !canCreate ? (
            <Typography variant="body2" color="text.secondary" sx={{ px: 1, py: 1.5 }}>
              {all.length === 0
                ? allowCreate
                  ? tr("No tags yet — type a name and press Enter.")
                  : tr("No tags yet — add them in Host Details.")
                : tr("Nothing matches.")}
            </Typography>
          ) : (
            list.map((t) => {
              const on = selected.has(t.id);
              const isEditing = editing?.id === t.id;
              return (
                <Box
                  key={t.id}
                  sx={{
                    display: "flex",
                    alignItems: "center",
                    height: 36,
                    pl: 0.5,
                    pr: 0.5,
                    gap: 0.5,
                    borderRadius: 1.5,
                    bgcolor: on ? "surface.highest" : undefined,
                    "&:hover": { bgcolor: on ? "surface.strong" : "surface.highest" },
                    "& .tag-actions": { opacity: 0 },
                    "&:hover .tag-actions, &:has(.tag-actions :focus-visible) .tag-actions": {
                      opacity: 1,
                    },
                    "&:hover .tag-count, &:has(.tag-actions :focus-visible) .tag-count": {
                      opacity: 0,
                    },
                  }}
                >
                  <RoundCheck on={on} label={t.label} onToggle={() => onToggle(t, !on)} />
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
                        e.stopPropagation();
                      }}
                      sx={{ flex: 1, "& .MuiInputBase-root": { height: 28 } }}
                    />
                  ) : (
                    <Typography
                      variant="body2"
                      noWrap
                      onClick={() => onToggle(t, !on)}
                      sx={{ flex: 1, cursor: "pointer", fontWeight: 500 }}
                    >
                      {t.label}
                    </Typography>
                  )}
                  <Box sx={{ position: "relative", flexShrink: 0, minWidth: 56, height: 28 }}>
                    <Typography
                      className="tag-count"
                      variant="caption"
                      color="text.secondary"
                      sx={{
                        position: "absolute",
                        right: 6,
                        top: 0,
                        lineHeight: "28px",
                        transition: "opacity 80ms",
                      }}
                    >
                      {hostsLabel(t.hosts)}
                    </Typography>
                    <Box
                      className="tag-actions"
                      sx={{
                        position: "absolute",
                        right: 0,
                        top: 0,
                        display: "flex",
                        transition: "opacity 80ms",
                      }}
                    >
                      <Tooltip title={tr("Rename")}>
                        <IconButton
                          size="small"
                          aria-label={tr("Rename {label}", { label: t.label })}
                          onClick={() => setEditing({ id: t.id, label: t.label })}
                          sx={{ width: 28, height: 28 }}
                        >
                          <EditOutlinedIcon sx={{ fontSize: 16 }} />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title={tr("Delete")}>
                        <IconButton
                          size="small"
                          aria-label={tr("Delete {label}", { label: t.label })}
                          onClick={() => setConfirm(t)}
                          sx={{ width: 28, height: 28 }}
                        >
                          <DeleteOutlineRoundedIcon sx={{ fontSize: 16 }} />
                        </IconButton>
                      </Tooltip>
                    </Box>
                  </Box>
                </Box>
              );
            })
          )}
        </Box>
        {onManage && (
          <Box sx={{ borderTop: 1, borderColor: "border.light", px: 0.75, py: 0.5 }}>
            <Button
              fullWidth
              size="small"
              variant="text"
              color="inherit"
              startIcon={<TuneRoundedIcon />}
              onClick={() => {
                close();
                onManage();
              }}
              sx={{ justifyContent: "flex-start", color: "text.secondary" }}
            >
              {tr("Manage tags")}
            </Button>
          </Box>
        )}
      </Popover>

      <ConfirmDialog
        open={confirm !== null}
        title={confirm ? tr("Delete tag “{label}”?", { label: confirm.label }) : ""}
        danger
        confirmLabel={tr("Delete")}
        busy={remove.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={runDelete}
      >
        {confirm && confirm.hosts > 0
          ? tr("The tag is removed from {hostsLabel}. The hosts themselves stay.", {
              hostsLabel: hostsLabel(confirm.hosts),
            })
          : tr("No host carries this tag.")}
      </ConfirmDialog>
    </>
  );
}
