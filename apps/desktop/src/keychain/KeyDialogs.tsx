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
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import type { HostCard, KeyCard } from "@/ipc/types";
import { Field, Mono } from "@/components/ui";
import { HostAvatar } from "@/hosts/HostAvatar";
import { tr, trx } from "@/i18n";

interface Base {
  open: boolean;
  busy: boolean;
  onCancel: () => void;
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
      <DialogTitle>{tr("Change passphrase")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          {card.encrypted && (
            <Field
              label={
                needCurrent ? tr("Current passphrase") : tr("Current passphrase (stored, optional)")
              }
            >
              <TextField
                autoFocus
                type="password"
                value={current}
                onChange={(e) => setCurrent(e.target.value)}
              />
            </Field>
          )}
          <Field label={tr("New passphrase (empty removes it)")}>
            <TextField
              autoFocus={!card.encrypted}
              type="password"
              value={next}
              onChange={(e) => setNext(e.target.value)}
            />
          </Field>
          <Field label={tr("Confirm")}>
            <TextField
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              error={mismatch}
              helperText={mismatch ? tr("Passphrases differ") : undefined}
            />
          </Field>
          {next.length > 0 && (
            <FormControlLabel
              control={
                <Checkbox checked={remember} onChange={(e) => setRemember(e.target.checked)} />
              }
              label={tr("Remember the new passphrase")}
            />
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          {tr("Cancel")}
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
          {tr("Save")}
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
      <DialogTitle>{tr("Export private key")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Alert severity="warning" variant="outlined">
            {tr(
              "The private key leaves the encrypted vault. Protect the exported copy with a passphrase unless you have a good reason not to.",
            )}
          </Alert>
          {needCurrent && (
            <Field label={tr("Current passphrase")}>
              <TextField
                autoFocus
                type="password"
                value={current}
                onChange={(e) => setCurrent(e.target.value)}
              />
            </Field>
          )}
          <Field label={tr("Passphrase for the exported copy")}>
            <TextField
              type="password"
              value={exportPass}
              onChange={(e) => setExportPass(e.target.value)}
            />
          </Field>
          <Field label={tr("Confirm")}>
            <TextField
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              error={mismatch}
              helperText={
                mismatch
                  ? tr("Passphrases differ")
                  : exportPass.length === 0
                    ? tr("Empty = unencrypted export")
                    : undefined
              }
            />
          </Field>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          {tr("Cancel")}
        </Button>
        <Button
          disabled={mismatch || busy || (needCurrent && current.length === 0)}
          onClick={() => onConfirm(args("clipboard"))}
        >
          {tr("Copy")}
        </Button>
        <Button
          variant="contained"
          disabled={mismatch || busy || (needCurrent && current.length === 0)}
          onClick={() => onConfirm(args("file"))}
        >
          {tr("Save to file…")}
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
      <DialogTitle>{tr("Export key to host")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Typography variant="body2" color="text.secondary">
            {trx(
              "Adds the public half of {key} to {file} on the selected host, connecting with the host's current credentials. Nothing else is changed; an already present key is left as is.",
              { key: <b>{card.label}</b>, file: <Mono>~/.ssh/authorized_keys</Mono> },
            )}
          </Typography>
          <Field label={tr("Host")}>
            <Autocomplete
              autoFocus
              options={sshHosts}
              value={host}
              onChange={(_, v) => setHost(v)}
              getOptionLabel={(h) => h.label}
              isOptionEqualToValue={(a, b) => a.id === b.id}
              noOptionsText={tr("No SSH hosts")}
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
              renderInput={(params) => <TextField {...params} placeholder={tr("Choose a host")} />}
            />
          </Field>
          {where && (
            <Alert severity="info" variant="outlined">
              {trx("You may be asked for the password or to trust the host key of {host}.", {
                host: <Mono>{where}</Mono>,
              })}
            </Alert>
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          {tr("Cancel")}
        </Button>
        <Button
          variant="contained"
          disabled={host === null || busy}
          onClick={() => host && onConfirm(host)}
        >
          {busy ? tr("Connecting…") : tr("Export")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
