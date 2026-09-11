import { useMemo, useState } from "react";
import {
  Alert,
  Autocomplete,
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
import type {
  GenerateKeyForm,
  HostCard,
  ImportKeyForm,
  KeyAlgorithm,
  KeyCard,
  Uuid,
} from "@/ipc/types";
import { Field, Mono } from "@/components/ui";
import { HostAvatar } from "@/hosts/HostAvatar";

interface Base {
  open: boolean;
  busy: boolean;
  onCancel: () => void;
}

type Algo =
  "ed25519" | "rsa2048" | "rsa3072" | "rsa4096" | "ecdsa_p256" | "ecdsa_p384" | "ecdsa_p521";
const ALGOS: Record<Algo, KeyAlgorithm> = {
  ed25519: "ed25519",
  rsa2048: { rsa: { bits: 2048 } },
  rsa3072: { rsa: { bits: 3072 } },
  rsa4096: { rsa: { bits: 4096 } },
  ecdsa_p256: "ecdsa_p256",
  ecdsa_p384: "ecdsa_p384",
  ecdsa_p521: "ecdsa_p521",
};
const algoOf = (a: Algo): KeyAlgorithm => ALGOS[a];

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
          <Field label="Label">
            <TextField
              autoFocus
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              fullWidth
            />
          </Field>
          <Stack direction="row" spacing={2}>
            <Field label="Algorithm" sx={{ width: 220 }}>
              <TextField select value={algo} onChange={(e) => setAlgo(e.target.value as Algo)}>
                <MenuItem value="ed25519">Ed25519 (recommended)</MenuItem>
                <MenuItem value="rsa2048">RSA 2048</MenuItem>
                <MenuItem value="rsa3072">RSA 3072</MenuItem>
                <MenuItem value="rsa4096">RSA 4096</MenuItem>
                <MenuItem value="ecdsa_p256">ECDSA P-256</MenuItem>
                <MenuItem value="ecdsa_p384">ECDSA P-384</MenuItem>
                <MenuItem value="ecdsa_p521">ECDSA P-521</MenuItem>
              </TextField>
            </Field>
            <Field label="Comment" sx={{ flex: 1 }}>
              <TextField
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                placeholder="you@laptop"
              />
            </Field>
          </Stack>
          <Stack direction="row" spacing={2}>
            <Field label="Passphrase (optional)" sx={{ flex: 1 }}>
              <TextField
                type="password"
                value={passphrase}
                onChange={(e) => setPassphrase(e.target.value)}
              />
            </Field>
            <Field label="Confirm" sx={{ flex: 1 }}>
              <TextField
                type="password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                error={mismatch}
                helperText={mismatch ? "Passphrases differ" : undefined}
              />
            </Field>
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
          <Field label="Label">
            <TextField
              autoFocus
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              fullWidth
            />
          </Field>
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
            <Field label="Private key (OpenSSH / PEM / PKCS#8)">
              <TextField
                value={text}
                onChange={(e) => setText(e.target.value)}
                multiline
                minRows={6}
                maxRows={12}
                slotProps={{ htmlInput: { spellCheck: false, style: { fontFamily: "monospace" } } }}
              />
            </Field>
          )}
          <Field label="Passphrase (if the key is encrypted)">
            <TextField
              type="password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
            />
          </Field>
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
            <Field
              label={needCurrent ? "Current passphrase" : "Current passphrase (stored, optional)"}
            >
              <TextField
                autoFocus
                type="password"
                value={current}
                onChange={(e) => setCurrent(e.target.value)}
              />
            </Field>
          )}
          <Field label="New passphrase (empty removes it)">
            <TextField
              autoFocus={!card.encrypted}
              type="password"
              value={next}
              onChange={(e) => setNext(e.target.value)}
            />
          </Field>
          <Field label="Confirm">
            <TextField
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              error={mismatch}
              helperText={mismatch ? "Passphrases differ" : undefined}
            />
          </Field>
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
            <Field label="Current passphrase">
              <TextField
                autoFocus
                type="password"
                value={current}
                onChange={(e) => setCurrent(e.target.value)}
              />
            </Field>
          )}
          <Field label="Passphrase for the exported copy">
            <TextField
              type="password"
              value={exportPass}
              onChange={(e) => setExportPass(e.target.value)}
            />
          </Field>
          <Field label="Confirm">
            <TextField
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
          </Field>
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

/** ssh-copy-id: pick a saved SSH host and confirm before its
 *  `~/.ssh/authorized_keys` is touched. */
export function ExportToHostDialog({
  open,
  card,
  hosts,
  busy,
  onCancel,
  onConfirm,
}: Base & {
  card: KeyCard;
  hosts: HostCard[];
  onConfirm: (host: HostCard) => void;
}) {
  const [host, setHost] = useState<HostCard | null>(null);
  const sshHosts = useMemo(
    () => hosts.filter((h) => h.protocol === "ssh").sort((a, b) => a.label.localeCompare(b.label)),
    [hosts],
  );
  const where = host ? `${host.username || "?"}@${host.address}:${host.port}` : null;

  return (
    <Dialog open={open} onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Export key to host</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Typography variant="body2" color="text.secondary">
            Adds the public half of <b>{card.label}</b> to <Mono>~/.ssh/authorized_keys</Mono> on
            the selected host, connecting with the host&apos;s current credentials. Nothing else is
            changed; an already present key is left as is.
          </Typography>
          <Field label="Host">
            <Autocomplete
              autoFocus
              options={sshHosts}
              value={host}
              onChange={(_, v) => setHost(v)}
              getOptionLabel={(h) => h.label}
              isOptionEqualToValue={(a, b) => a.id === b.id}
              noOptionsText="No SSH hosts"
              renderOption={(props, h) => {
                const { key, ...rest } = props;
                return (
                  <li key={key} {...rest}>
                    <Stack
                      direction="row"
                      spacing={1.25}
                      sx={{ alignItems: "center", minWidth: 0 }}
                    >
                      <HostAvatar host={h} size={24} />
                      <Stack sx={{ minWidth: 0 }}>
                        <Typography variant="body2" noWrap>
                          {h.label}
                        </Typography>
                        <Mono secondary>
                          {h.username ? `${h.username}@` : ""}
                          {h.address}
                          {h.port !== 22 ? `:${h.port}` : ""}
                        </Mono>
                      </Stack>
                    </Stack>
                  </li>
                );
              }}
              renderInput={(params) => <TextField {...params} placeholder="Choose a host" />}
            />
          </Field>
          {where && (
            <Alert severity="info" variant="outlined">
              You may be asked for the password or to trust the host key of <Mono>{where}</Mono>.
            </Alert>
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={host === null || busy}
          onClick={() => host && onConfirm(host)}
        >
          {busy ? "Connecting…" : "Export"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
