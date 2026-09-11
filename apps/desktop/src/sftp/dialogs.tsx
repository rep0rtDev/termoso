import { useState } from "react";
import {
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from "@mui/material";
import { Field } from "@/components/ui";

interface NameDialogProps {
  open: boolean;
  title: string;
  label: string;
  initial?: string;
  confirmLabel?: string;
  busy?: boolean;
  onCancel: () => void;
  onConfirm: (value: string) => void;
}

export function NameDialog({
  open,
  title,
  label,
  initial = "",
  confirmLabel = "OK",
  busy,
  onCancel,
  onConfirm,
}: NameDialogProps) {
  const [value, setValue] = useState(initial);
  const valid = value.trim().length > 0 && !/[/\\]/.test(value);
  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
        <Stack
          component="form"
          onSubmit={(e) => {
            e.preventDefault();
            if (valid) onConfirm(value.trim());
          }}
        >
          <Field label={label}>
            <TextField
              autoFocus
              value={value}
              onChange={(e) => setValue(e.target.value)}
              fullWidth
              margin="dense"
              onFocus={(e) => {
                const dot = initial.lastIndexOf(".");
                e.target.setSelectionRange(0, dot > 0 ? dot : initial.length);
              }}
            />
          </Field>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={!valid || busy}
          onClick={() => onConfirm(value.trim())}
        >
          {confirmLabel}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

interface ChmodProps {
  open: boolean;
  name: string;
  mode: number;
  busy?: boolean;
  onCancel: () => void;
  onConfirm: (mode: number) => void;
}

const rows = [
  { label: "Owner", shift: 6 },
  { label: "Group", shift: 3 },
  { label: "Others", shift: 0 },
];
const cols = [
  { label: "Read", bit: 4 },
  { label: "Write", bit: 2 },
  { label: "Execute", bit: 1 },
];

export function ChmodDialog({ open, name, mode: initial, busy, onCancel, onConfirm }: ChmodProps) {
  const [mode, setMode] = useState(initial & 0o7777);
  const [octal, setOctal] = useState((initial & 0o777).toString(8).padStart(3, "0"));
  const setBits = (m: number) => {
    setMode(m);
    setOctal((m & 0o777).toString(8).padStart(3, "0"));
  };
  const onOctal = (v: string) => {
    setOctal(v);
    if (/^[0-7]{3,4}$/.test(v)) setMode(parseInt(v, 8));
  };
  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Permissions</DialogTitle>
      <DialogContent>
        <Typography variant="body2" color="text.secondary" noWrap sx={{ mb: 1.5 }}>
          {name}
        </Typography>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell />
              {cols.map((c) => (
                <TableCell key={c.label} align="center">
                  {c.label}
                </TableCell>
              ))}
            </TableRow>
          </TableHead>
          <TableBody>
            {rows.map((r) => (
              <TableRow key={r.label}>
                <TableCell>{r.label}</TableCell>
                {cols.map((c) => {
                  const bit = c.bit << r.shift;
                  return (
                    <TableCell key={c.label} align="center" padding="checkbox">
                      <Checkbox
                        size="small"
                        checked={(mode & bit) !== 0}
                        onChange={(e) => setBits(e.target.checked ? mode | bit : mode & ~bit)}
                      />
                    </TableCell>
                  );
                })}
              </TableRow>
            ))}
          </TableBody>
        </Table>
        <Field label="Octal">
          <TextField
            value={octal}
            onChange={(e) => onOctal(e.target.value)}
            size="small"
            error={!/^[0-7]{3,4}$/.test(octal)}
            slotProps={{ input: { sx: { fontFamily: "monospace" } } }}
            sx={{ mt: 2, width: 120 }}
          />
        </Field>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button variant="contained" disabled={busy} onClick={() => onConfirm(mode)}>
          Apply
        </Button>
      </DialogActions>
    </Dialog>
  );
}
