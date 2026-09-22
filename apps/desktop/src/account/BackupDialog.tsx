import { useEffect, useMemo, useRef, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import { open as openFile, save as saveFile } from "@tauri-apps/plugin-dialog";
import { useQueryClient } from "@tanstack/react-query";
import { useSnackbar } from "@/components/Snackbar";
import { CheckTile, EntityCard, Field, IconTile, Mono } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { useVaults } from "@/ipc/hooks";
import {
  errorMessage,
  type BackupSummary,
  type BackupVaultSummary,
  type LocalVault,
  type RestoreReport,
  type Uuid,
} from "@/ipc/types";
import { vaultHint, vaultIcon } from "@/app/vault";
import { sizes } from "@/theme/theme";
import { invalidateAll } from "./SignIn";

export type BackupMode = "export" | { kind: "restore"; path: string };

const MIN_PASSWORD = 8;
const EXT = "termoso";

const KIND_LABEL: Record<string, string> = {
  host: "hosts",
  group: "groups",
  identity: "identities",
  ssh_key: "keys",
  ssh_certificate: "certificates",
  snippet: "snippets",
  snippet_package: "packages",
  pf_rule: "forwarding rules",
  known_host: "known hosts",
  proxy: "proxies",
  tag: "tags",
  host_chain: "chains",
  ssh_config: "SSH configs",
  telnet_config: "Telnet configs",
  webdav_config: "WebDAV configs",
};

function countsLine(v: BackupVaultSummary) {
  const parts = Object.entries(v.counts)
    .filter(([k]) => k in KIND_LABEL)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([k, n]) => `${n} ${KIND_LABEL[k]}`);
  return parts.length ? parts.join(" · ") : `${v.entities} items`;
}

function fileStamp() {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}`;
}

/** Native picker for a `.termoso` file; `null` when the user cancels. */
export async function pickBackupFile(): Promise<string | null> {
  const path = await openFile({
    title: "Open Termoso backup",
    multiple: false,
    filters: [{ name: "Termoso backup", extensions: [EXT] }],
  });
  return typeof path === "string" ? path : null;
}

/** Encrypted `.termoso` backup: export unlocked vaults or restore one into a vault of this device. */
export function BackupDialog({ mode, onClose }: { mode: BackupMode | null; onClose: () => void }) {
  return (
    <Dialog open={mode !== null} onClose={onClose} maxWidth="sm" fullWidth>
      {mode === "export" && <ExportBody onClose={onClose} />}
      {mode !== null && mode !== "export" && <RestoreBody path={mode.path} onClose={onClose} />}
    </Dialog>
  );
}

// ───────────────────────────── export ─────────────────────────────

function PasswordFields({
  password,
  confirm,
  onPassword,
  onConfirm,
  withConfirm,
  autoFocus,
}: {
  password: string;
  confirm: string;
  onPassword: (v: string) => void;
  onConfirm: (v: string) => void;
  withConfirm: boolean;
  autoFocus?: boolean;
}) {
  const short = password.length > 0 && password.length < MIN_PASSWORD;
  const mismatch = withConfirm && confirm.length > 0 && confirm !== password;
  return (
    <Stack spacing={1.25}>
      <Field label="Backup password" hint={`At least ${MIN_PASSWORD} characters.`}>
        <TextField
          type="password"
          value={password}
          onChange={(e) => onPassword(e.target.value)}
          autoFocus={autoFocus}
          autoComplete="new-password"
          error={short}
          fullWidth
        />
      </Field>
      {withConfirm && (
        <Field label="Repeat password">
          <TextField
            type="password"
            value={confirm}
            onChange={(e) => onConfirm(e.target.value)}
            autoComplete="new-password"
            error={mismatch}
            helperText={mismatch ? "Passwords do not match" : undefined}
            fullWidth
          />
        </Field>
      )}
    </Stack>
  );
}

function VaultPick({
  vault,
  checked,
  onToggle,
}: {
  vault: LocalVault;
  checked: boolean;
  onToggle: () => void;
}) {
  return (
    <EntityCard
      dense
      selected={checked}
      onClick={onToggle}
      tile={
        <CheckTile
          checked={checked}
          hoverHint
          size={sizes.tileSmall}
          tile={<IconTile size={sizes.tileSmall}>{vaultIcon(vault)}</IconTile>}
        />
      }
      title={vault.name}
      subtitle={vaultHint(vault)}
    />
  );
}

function ExportBody({ onClose }: { onClose: () => void }) {
  const snackbar = useSnackbar();
  const vaults = useVaults();
  const unlocked = useMemo(() => (vaults.data ?? []).filter((v) => v.unlocked), [vaults.data]);
  const [picked, setPicked] = useState<Set<Uuid> | null>(null);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<BackupSummary | null>(null);

  const selected = picked ?? new Set(unlocked.map((v) => v.id));
  const toggle = (id: Uuid) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setPicked(next);
  };

  const ready =
    selected.size > 0 && password.length >= MIN_PASSWORD && confirm === password && !busy;

  const run = async () => {
    const path = await saveFile({
      title: "Save encrypted backup",
      defaultPath: `termoso-backup-${fileStamp()}.${EXT}`,
      filters: [{ name: "Termoso backup", extensions: [EXT] }],
    });
    if (!path) return;
    setBusy(true);
    try {
      setDone(await ipc.backupExport([...selected], password, path));
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  if (done) {
    return (
      <>
        <DialogTitle>Backup saved</DialogTitle>
        <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}>
          <Alert severity="success" variant="outlined">
            Encrypted with your password. Nothing in the file is readable without it — keep the
            password somewhere safe, it cannot be recovered.
          </Alert>
          <Summary summary={done} />
          {done.path && <Mono secondary>{done.path}</Mono>}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button variant="contained" onClick={onClose}>
            Done
          </Button>
        </DialogActions>
      </>
    );
  }

  return (
    <>
      <DialogTitle>Export encrypted backup</DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
        <Typography variant="body2" color="text.secondary">
          Everything in the chosen vaults — hosts, groups, identities, keys, certificates, snippets,
          forwarding rules, known hosts — is written to one <Mono>.{EXT}</Mono> file encrypted with
          a password of your choice (Argon2id + XChaCha20-Poly1305). The file works offline and
          without an account.
        </Typography>
        <Field label="Vaults">
          {unlocked.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              No unlocked vault on this device.
            </Typography>
          ) : (
            <Stack spacing={0.75}>
              {unlocked.map((v) => (
                <VaultPick
                  key={v.id}
                  vault={v}
                  checked={selected.has(v.id)}
                  onToggle={() => toggle(v.id)}
                />
              ))}
            </Stack>
          )}
        </Field>
        <PasswordFields
          password={password}
          confirm={confirm}
          onPassword={setPassword}
          onConfirm={setConfirm}
          withConfirm
        />
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onClose}>Cancel</Button>
        <Button
          variant="contained"
          startIcon={<LockOutlinedIcon />}
          disabled={!ready}
          onClick={run}
        >
          Save backup…
        </Button>
      </DialogActions>
    </>
  );
}

function Summary({ summary }: { summary: BackupSummary }) {
  return (
    <Stack spacing={0.75}>
      <Typography variant="caption" color="text.secondary">
        Created {new Date(summary.createdAt).toLocaleString()} · Termoso {summary.appVersion}
      </Typography>
      {summary.vaults.map((v) => (
        <EntityCard
          key={v.id}
          dense
          sx={{ bgcolor: "surface.highest", "&:hover": { bgcolor: "surface.highest" } }}
          tile={<IconTile size={sizes.tileSmall}>{vaultIcon(fakeVault(v))}</IconTile>}
          title={v.name}
          subtitle={countsLine(v)}
          trailing={<Chip size="small" label={v.kind} />}
        />
      ))}
    </Stack>
  );
}

const fakeVault = (v: BackupVaultSummary): LocalVault => ({
  id: v.id,
  kind: v.kind,
  name: v.name,
  team_id: null,
  role: "manager",
  unlocked: true,
  key_version: 0,
  cursor: 0,
  session_logging: false,
  logs_cursor: 0,
  is_default: false,
});

// ───────────────────────────── restore ─────────────────────────────

type RestoreStep =
  | { kind: "password"; path: string }
  | { kind: "preview"; summary: BackupSummary }
  | { kind: "done"; reports: { name: string; report: RestoreReport }[] };

function RestoreBody({ path, onClose }: { path: string; onClose: () => void }) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vaults = useVaults();
  const writable = useMemo(
    () => (vaults.data ?? []).filter((v) => v.unlocked && v.role !== "viewer"),
    [vaults.data],
  );
  const [step, setStep] = useState<RestoreStep>({ kind: "password", path });
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [targets, setTargets] = useState<Record<number, string>>({});

  const previewId = step.kind === "preview" ? step.summary.previewId : null;
  const previewRef = useRef<Uuid | null>(null);
  useEffect(() => {
    previewRef.current = previewId;
  }, [previewId]);
  useEffect(
    () => () => {
      if (previewRef.current) void ipc.backupDiscard(previewRef.current);
    },
    [],
  );

  const inspect = async () => {
    if (step.kind !== "password") return;
    setBusy(true);
    try {
      const summary = await ipc.backupInspect(step.path, password);
      const initial: Record<number, string> = {};
      summary.vaults.forEach((v, i) => {
        const same = writable.find((w) => w.id === v.id);
        const sameKind = writable.find((w) => w.kind === v.kind);
        initial[i] = same?.id ?? sameKind?.id ?? writable[0]?.id ?? "";
      });
      setTargets(initial);
      setStep({ kind: "preview", summary });
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const restore = async () => {
    if (step.kind !== "preview") return;
    setBusy(true);
    const reports: { name: string; report: RestoreReport }[] = [];
    try {
      for (const [i, v] of step.summary.vaults.entries()) {
        const target = targets[i];
        if (!target) continue;
        reports.push({
          name: v.name,
          report: await ipc.backupRestore(step.summary.previewId, i, target),
        });
      }
      invalidateAll(qc);
      void ipc.backupDiscard(step.summary.previewId);
      setStep({ kind: "done", reports });
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  if (step.kind === "password") {
    return (
      <>
        <DialogTitle>Restore from backup</DialogTitle>
        <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
          <Mono secondary>{step.path}</Mono>
          <Box
            component="form"
            onSubmit={(e) => {
              e.preventDefault();
              void inspect();
            }}
          >
            <PasswordFields
              password={password}
              confirm=""
              onPassword={setPassword}
              onConfirm={() => undefined}
              withConfirm={false}
              autoFocus
            />
          </Box>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="contained"
            disabled={password.length === 0 || busy}
            onClick={() => void inspect()}
          >
            {busy ? "Decrypting…" : "Unlock"}
          </Button>
        </DialogActions>
      </>
    );
  }

  if (step.kind === "preview") {
    const anyTarget = Object.values(targets).some((t) => t !== "");
    return (
      <>
        <DialogTitle>Restore from backup</DialogTitle>
        <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
          <Typography variant="caption" color="text.secondary">
            Created {new Date(step.summary.createdAt).toLocaleString()} · Termoso{" "}
            {step.summary.appVersion}
          </Typography>
          {writable.length === 0 && (
            <Alert severity="warning">No unlocked, writable vault to restore into.</Alert>
          )}
          <Stack spacing={1.25}>
            {step.summary.vaults.map((v, i) => (
              <Box
                key={v.id}
                sx={{
                  display: "flex",
                  alignItems: "center",
                  gap: 1.5,
                  p: 1,
                  borderRadius: 2,
                  bgcolor: "surface.high",
                }}
              >
                <IconTile size={sizes.tileSmall}>{vaultIcon(fakeVault(v))}</IconTile>
                <Box sx={{ flex: 1, minWidth: 0 }}>
                  <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
                    {v.name}
                  </Typography>
                  <Typography variant="caption" color="text.secondary" noWrap>
                    {countsLine(v)}
                  </Typography>
                </Box>
                <TextField
                  select
                  value={targets[i] ?? ""}
                  onChange={(e) => setTargets((t) => ({ ...t, [i]: e.target.value }))}
                  sx={{ width: 180, flexShrink: 0 }}
                  disabled={writable.length === 0}
                >
                  <MenuItem value="">Skip</MenuItem>
                  {writable.map((w) => (
                    <MenuItem key={w.id} value={w.id}>
                      Into {w.name}
                    </MenuItem>
                  ))}
                </TextField>
              </Box>
            ))}
          </Stack>
          <Typography variant="body2" color="text.secondary">
            Items keep their identity: one already in the target vault is replaced by the backup
            copy, everything else is added. Nothing is deleted.
          </Typography>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="contained" disabled={!anyTarget || busy} onClick={() => void restore()}>
            {busy ? "Restoring…" : "Restore"}
          </Button>
        </DialogActions>
      </>
    );
  }

  const total = step.reports.reduce(
    (acc, r) => ({
      added: acc.added + r.report.added,
      replaced: acc.replaced + r.report.replaced,
      skipped: acc.skipped + r.report.skipped,
    }),
    { added: 0, replaced: 0, skipped: 0 },
  );
  const warnings = step.reports.flatMap((r) => r.report.warnings);
  return (
    <>
      <DialogTitle>Restore complete</DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}>
        <Stack direction="row" spacing={1}>
          <Chip size="small" color="success" label={`${total.added} added`} />
          <Chip size="small" label={`${total.replaced} replaced`} />
          {total.skipped > 0 && <Chip size="small" label={`${total.skipped} skipped`} />}
        </Stack>
        {warnings.length > 0 && (
          <Alert severity="warning" variant="outlined">
            <Stack spacing={0.25}>
              {warnings.map((w, i) => (
                <span key={i}>{w}</span>
              ))}
            </Stack>
          </Alert>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button variant="contained" onClick={onClose}>
          Done
        </Button>
      </DialogActions>
    </>
  );
}
