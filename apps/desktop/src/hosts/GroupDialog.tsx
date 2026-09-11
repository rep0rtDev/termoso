import { useState } from "react";
import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  TextField,
} from "@mui/material";
import { useSnackbar } from "@/components/Snackbar";
import { useDeleteGroup, useGroups, useSaveGroup } from "@/ipc/hooks";
import { errorMessage, type GroupNode, type Uuid } from "@/ipc/types";

interface Props {
  open: boolean;
  vaultId: Uuid;
  parentId: Uuid | null;
  group: GroupNode | null;
  onClose: () => void;
  onDeleted: (id: Uuid) => void;
}

export function GroupDialog({ open, ...rest }: Props) {
  const [busy, setBusy] = useState(false);
  return (
    <Dialog open={open} onClose={busy ? undefined : rest.onClose} maxWidth="xs" fullWidth>
      {open && <GroupForm {...rest} onBusy={setBusy} />}
    </Dialog>
  );
}

function GroupForm({
  vaultId,
  parentId,
  group,
  onClose,
  onDeleted,
  onBusy,
}: Omit<Props, "open"> & { onBusy: (b: boolean) => void }) {
  const snackbar = useSnackbar();
  const groups = useGroups(vaultId);
  const save = useSaveGroup();
  const del = useDeleteGroup();
  const [label, setLabel] = useState(group?.label ?? "");
  const [parent, setParent] = useState<Uuid | null>(group ? group.parentId : parentId);

  const busy = save.isPending || del.isPending;
  const run = <T,>(p: Promise<T>, done: () => void) => {
    onBusy(true);
    p.then(done)
      .catch((e: unknown) => snackbar.error(errorMessage(e)))
      .finally(() => onBusy(false));
  };

  const onSave = () =>
    run(
      save.mutateAsync({ vaultId, id: group?.id ?? null, label: label.trim(), parentId: parent }),
      () => {
        snackbar.notify(group ? "Group renamed" : "Group created");
        onClose();
      },
    );

  const onDelete = () => {
    if (!group) return;
    run(del.mutateAsync({ id: group.id, vaultId }), () => {
      snackbar.notify("Group removed; its hosts moved up one level");
      onDeleted(group.id);
      onClose();
    });
  };

  return (
    <>
      <DialogTitle>{group ? "Edit group" : "New group"}</DialogTitle>
      <DialogContent>
        <Box
          component="form"
          onSubmit={(e) => {
            e.preventDefault();
            if (label.trim()) onSave();
          }}
          sx={{ display: "flex", flexDirection: "column", gap: 2, pt: 1 }}
        >
          <TextField
            label="Name"
            autoFocus
            required
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
          <TextField
            select
            label="Parent group"
            value={parent ?? ""}
            onChange={(e) => setParent(e.target.value === "" ? null : e.target.value)}
          >
            <MenuItem value="">
              <em>Top level</em>
            </MenuItem>
            {(groups.data ?? [])
              .filter((g) => g.id !== group?.id)
              .map((g) => (
                <MenuItem key={g.id} value={g.id}>
                  {g.label}
                </MenuItem>
              ))}
          </TextField>
        </Box>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        {group && (
          <Button color="error" onClick={onDelete} disabled={busy} sx={{ mr: "auto" }}>
            Delete
          </Button>
        )}
        <Button color="inherit" onClick={onClose} disabled={busy}>
          Cancel
        </Button>
        <Button variant="contained" onClick={onSave} disabled={busy || !label.trim()}>
          Save
        </Button>
      </DialogActions>
    </>
  );
}
