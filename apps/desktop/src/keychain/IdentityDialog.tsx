import { useState } from "react";
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Stack,
  TextField,
} from "@mui/material";
import type { IdentityCard, IdentityForm, KeyCard, Uuid } from "@/ipc/types";
import { Field } from "@/components/ui";

export function IdentityDialog({
  open,
  vaultId,
  initial,
  keys,
  busy,
  onCancel,
  onConfirm,
}: {
  open: boolean;
  vaultId: Uuid;
  initial: IdentityCard | null;
  keys: KeyCard[];
  busy?: boolean;
  onCancel: () => void;
  onConfirm: (form: IdentityForm) => void;
}) {
  const [label, setLabel] = useState(initial?.label ?? "");
  const [username, setUsername] = useState(initial?.username ?? "");
  const [password, setPassword] = useState<string | null>(null);
  const [keyId, setKeyId] = useState<Uuid | null>(initial?.sshKeyId ?? null);
  const valid = label.trim().length > 0 && username.trim().length > 0;

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{initial ? "Edit identity" : "New identity"}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Field label="Label">
            <TextField autoFocus value={label} onChange={(e) => setLabel(e.target.value)} />
          </Field>
          <Field label="Username">
            <TextField value={username} onChange={(e) => setUsername(e.target.value)} />
          </Field>
          <Field label="Password">
            <TextField
              type="password"
              value={password ?? ""}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={initial?.hasPassword ? "•••••••• (stored)" : ""}
              helperText={
                initial?.hasPassword && password === null
                  ? "Leave untouched to keep the stored password; clear to remove it."
                  : undefined
              }
            />
          </Field>
          <Field label="SSH key">
            <TextField
              select
              value={keyId ?? ""}
              onChange={(e) => setKeyId(e.target.value === "" ? null : e.target.value)}
            >
              <MenuItem value="">
                <em>None</em>
              </MenuItem>
              {keys.map((k) => (
                <MenuItem key={k.id} value={k.id}>
                  {k.label}
                </MenuItem>
              ))}
            </TextField>
          </Field>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={!valid || busy}
          onClick={() =>
            onConfirm({
              id: initial?.id ?? null,
              vaultId,
              label: label.trim(),
              username: username.trim(),
              password,
              sshKeyId: keyId,
            })
          }
        >
          Save
        </Button>
      </DialogActions>
    </Dialog>
  );
}
