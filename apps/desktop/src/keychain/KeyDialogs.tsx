import { useState } from "react";
import {
  Alert,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import type { GenerateKeyForm, ImportKeyForm, KeyAlgorithm, KeyCard, Uuid } from "@/ipc/types";

interface Base {
  open: boolean;
  busy: boolean;
  onCancel: () => void;
}

type Algo = "ed25519" | "rsa2048" | "rsa3072" | "rsa4096";
const algoOf = (a: Algo): KeyAlgorithm =>
  a === "ed25519" ? "ed25519" : { rsa: { bits: Number(a.slice(3)) } };

export function GenerateKeyDialog({
  open,
  vaultId,
  busy,
  onCancel,
  onConfirm,
}: Base & { vaultId: Uuid; onConfirm: (form: GenerateKeyForm) => void }) {
  const [label, setLabel] = useState("");
  const [algo, setAlgo] = useState<Algo>("ed25519");
  const [comment, setComment] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [remember, setRemember] = useState(true);
  const mismatch = passphrase.length > 0 && passphrase !== confirm;
  const valid = label.trim().length > 0 && !mismatch;

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="sm" fullWidth>
      <DialogTitle>Generate key</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <TextField
            autoFocus
            label="Label"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            fullWidth
          />
          <Stack direction="row" spacing={2}>
            <TextField
              select
              label="Algorithm"
              value={algo}
              onChange={(e) => setAlgo(e.target.value as Algo)}
              sx={{ width: 220 }}
            >
              <MenuItem value="ed25519">Ed25519 (recommended)</MenuItem>
              <MenuItem value="rsa2048">RSA 2048</MenuItem>
              <MenuItem value="rsa3072">RSA 3072</MenuItem>
              <MenuItem value="rsa4096">RSA 4096</MenuItem>
            </TextField>
            <TextField
              label="Comment"
              value={comment}
              onChange={(e) => setComment(e.target.value)}
              placeholder="you@laptop"
              sx={{ flex: 1 }}
            />
          </Stack>
          <Stack direction="row" spacing={2}>
            <TextField
              label="Passphrase (optional)"
              type="password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              sx={{ flex: 1 }}
            />
            <TextField
              label="Confirm"
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              error={mismatch}
              helperText={mismatch ? "Passphrases differ" : undefined}
              sx={{ flex: 1 }}
            />
          </Stack>
          {passphrase.length > 0 && (
            <FormControlLabel
              control={
                <Checkbox checked={remember} onChange={(e) => setRemember(e.target.checked)} />
              }
              label="Remember passphrase in the encrypted vault"
            />
          )}
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
              vaultId,
              label: label.trim(),
              algorithm: algoOf(algo),
              comment: comment.trim(),
              passphrase: passphrase.length > 0 ? passphrase : null,
              rememberPassphrase: passphrase.length > 0 && remember,
            })
          }
        >
          Generate
        </Button>
      </DialogActions>
    </Dialog>
  );
}

export function ImportKeyDialog({
  open,
  vaultId,
  busy,
  onCancel,
  onConfirm,
  onConfirmFile,
}: Base & {
  vaultId: Uuid;
  onConfirm: (form: ImportKeyForm) => void;
  onConfirmFile: (args: {
    vaultId: Uuid;
    label: string;
    path: string;
    passphrase: string | null;
    rememberPassphrase: boolean;
  }) => void;
}) {
  const [label, setLabel] = useState("");
  const [text, setText] = useState("");
  const [path, setPath] = useState<string | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [remember, setRemember] = useState(true);
  const valid = label.trim().length > 0 && (text.trim().length > 0 || path !== null);

  const pick = async () => {
    const picked = await openFile({ multiple: false, directory: false, title: "Private key" });
    if (typeof picked === "string") {
      setPath(picked);
      setText("");
      if (label.trim().length === 0) setLabel(picked.split(/[\\/]/).pop() ?? "");
    }
  };

  const submit = () => {
    const pass = passphrase.length > 0 ? passphrase : null;
    if (path !== null) {
      onConfirmFile({
        vaultId,
        label: label.trim(),
        path,
        passphrase: pass,
        rememberPassphrase: pass !== null && remember,
      });
    } else {
      onConfirm({
        vaultId,
        label: label.trim(),
        privateKey: text,
        passphrase: pass,
        rememberPassphrase: pass !== null && remember,
      });
    }
  };

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="sm" fullWidth>
      <DialogTitle>Import key</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <TextField
            autoFocus
            label="Label"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            fullWidth
          />
          <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
            <Button variant="outlined" onClick={() => void pick()} disabled={busy}>
              Choose file…
            </Button>
            <Typography variant="body2" color="text.secondary" noWrap sx={{ flex: 1 }}>
              {path ?? "or paste the private key below"}
            </Typography>
            {path !== null && (
              <Button size="small" color="inherit" onClick={() => setPath(null)}>
                Clear
              </Button>
            )}
          </Stack>
          {path === null && (
            <TextField
              label="Private key (OpenSSH / PEM / PKCS#8)"
              value={text}
              onChange={(e) => setText(e.target.value)}
              multiline
              minRows={6}
              maxRows={12}
              slotProps={{ htmlInput: { spellCheck: false, style: { fontFamily: "monospace" } } }}
            />
          )}
          <TextField
            label="Passphrase (if the key is encrypted)"
            type="password"
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
          />
          {passphrase.length > 0 && (
            <FormControlLabel
              control={
                <Checkbox checked={remember} onChange={(e) => setRemember(e.target.checked)} />
              }
              label="Remember passphrase in the encrypted vault"
            />
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button variant="contained" disabled={!valid || busy} onClick={submit}>
          Import
        </Button>
      </DialogActions>
    </Dialog>
  );
}

export function PassphraseDialog({
  open,
  card,
  busy,
  onCancel,
  onConfirm,
}: Base & {
  card: KeyCard;
  onConfirm: (args: { current: string | null; next: string | null; remember: boolean }) => void;
}) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [remember, setRemember] = useState(card.hasPassphrase);
  const mismatch = next !== confirm;
  const needCurrent = card.encrypted && !card.hasPassphrase;

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Change passphrase</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          {card.encrypted && (
            <TextField
              autoFocus
              label={needCurrent ? "Current passphrase" : "Current passphrase (stored, optional)"}
              type="password"
              value={current}
              onChange={(e) => setCurrent(e.target.value)}
            />
          )}
          <TextField
            autoFocus={!card.encrypted}
            label="New passphrase (empty removes it)"
            type="password"
            value={next}
            onChange={(e) => setNext(e.target.value)}
          />
          <TextField
            label="Confirm"
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            error={mismatch}
            helperText={mismatch ? "Passphrases differ" : undefined}
          />
          {next.length > 0 && (
            <FormControlLabel
              control={
                <Checkbox checked={remember} onChange={(e) => setRemember(e.target.checked)} />
              }
              label="Remember the new passphrase"
            />
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={mismatch || busy || (needCurrent && current.length === 0)}
          onClick={() =>
            onConfirm({
              current: current.length > 0 ? current : null,
              next: next.length > 0 ? next : null,
              remember: next.length > 0 && remember,
            })
          }
        >
          Save
        </Button>
      </DialogActions>
    </Dialog>
  );
}

export function ExportKeyDialog({
  open,
  card,
  busy,
  onCancel,
  onConfirm,
}: Base & {
  card: KeyCard;
  onConfirm: (args: {
    mode: "file" | "clipboard";
    passphrase: string | null;
    exportPassphrase: string | null;
  }) => void;
}) {
  const [current, setCurrent] = useState("");
  const [exportPass, setExportPass] = useState("");
  const [confirm, setConfirm] = useState("");
  const mismatch = exportPass !== confirm;
  const needCurrent = card.encrypted && !card.hasPassphrase;
  const args = (mode: "file" | "clipboard") => ({
    mode,
    passphrase: current.length > 0 ? current : null,
    exportPassphrase: exportPass.length > 0 ? exportPass : null,
  });

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Export private key</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Alert severity="warning" variant="outlined">
            The private key leaves the encrypted vault. Protect the exported copy with a passphrase
            unless you have a good reason not to.
          </Alert>
          {needCurrent && (
            <TextField
              autoFocus
              label="Current passphrase"
              type="password"
              value={current}
              onChange={(e) => setCurrent(e.target.value)}
            />
          )}
          <TextField
            label="Passphrase for the exported copy"
            type="password"
            value={exportPass}
            onChange={(e) => setExportPass(e.target.value)}
          />
          <TextField
            label="Confirm"
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            error={mismatch}
            helperText={
              mismatch
                ? "Passphrases differ"
                : exportPass.length === 0
                  ? "Empty = unencrypted export"
                  : undefined
            }
          />
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button
          disabled={mismatch || busy || (needCurrent && current.length === 0)}
          onClick={() => onConfirm(args("clipboard"))}
        >
          Copy
        </Button>
        <Button
          variant="contained"
          disabled={mismatch || busy || (needCurrent && current.length === 0)}
          onClick={() => onConfirm(args("file"))}
        >
          Save to file…
        </Button>
      </DialogActions>
    </Dialog>
  );
}
