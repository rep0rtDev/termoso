import { useEffect, useState } from "react";
import {
  Box,
  Button,
  Checkbox,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  IconButton,
  MenuItem,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import SwapHorizRoundedIcon from "@mui/icons-material/SwapHorizRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import StopRoundedIcon from "@mui/icons-material/StopRounded";
import EditRoundedIcon from "@mui/icons-material/EditRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useDefaultVault, useHosts, usePfRules } from "@/ipc/hooks";
import {
  errorMessage,
  type HostCard,
  type PfKind,
  type PfRuleCard,
  type PfRuleForm,
  type Uuid,
} from "@/ipc/types";
import { formatSize } from "@/sftp/format";

const KIND_LABEL: Record<PfKind, string> = {
  local: "Local",
  remote: "Remote",
  dynamic: "Dynamic (SOCKS5)",
};

function describe(r: PfRuleCard): string {
  const bind = `${r.boundAddress}:${r.localPort}`;
  switch (r.kind) {
    case "local":
      return `${bind} → ${r.hostLabel} → ${r.remoteHost}:${r.remotePort}`;
    case "remote":
      return `${r.hostLabel}:${r.remotePort} → ${r.remoteHost}:${r.localPort}`;
    case "dynamic":
      return `socks5://${bind} via ${r.hostLabel}`;
  }
}

function StateChip({ r }: { r: PfRuleCard }) {
  const rt = r.runtime;
  if (rt.state === "running")
    return <Chip size="small" color="success" label={rt.bound ? `on ${rt.bound}` : "running"} />;
  if (rt.state === "starting") return <Chip size="small" color="info" label="starting…" />;
  if (rt.lastError)
    return (
      <Tooltip title={rt.lastError}>
        <Chip size="small" color="error" variant="outlined" label="failed" />
      </Tooltip>
    );
  return <Chip size="small" variant="outlined" label="stopped" />;
}

function RuleDialog({
  vaultId,
  hosts,
  initial,
  busy,
  onCancel,
  onConfirm,
}: {
  vaultId: Uuid;
  hosts: HostCard[];
  initial: PfRuleCard | null;
  busy: boolean;
  onCancel: () => void;
  onConfirm: (form: PfRuleForm) => void;
}) {
  const [f, setF] = useState<PfRuleForm>(
    initial
      ? {
          id: initial.id,
          vaultId: initial.vaultId,
          label: initial.label,
          hostId: initial.hostId,
          kind: initial.kind,
          boundAddress: initial.boundAddress,
          localPort: initial.localPort,
          remoteHost: initial.remoteHost,
          remotePort: initial.remotePort,
          autoStart: initial.autoStart,
        }
      : {
          id: null,
          vaultId,
          label: "",
          hostId: hosts[0]?.id ?? "",
          kind: "local",
          boundAddress: "127.0.0.1",
          localPort: 8080,
          remoteHost: "localhost",
          remotePort: 80,
          autoStart: false,
        },
  );
  const set = <K extends keyof PfRuleForm>(k: K, v: PfRuleForm[K]) => setF({ ...f, [k]: v });
  const port = (v: string) => Math.min(65535, Math.max(0, Number(v) || 0));
  const sshHosts = hosts.filter((h) => h.protocol === "ssh");
  const valid =
    f.hostId !== "" &&
    f.localPort > 0 &&
    (f.kind === "dynamic" || (f.remoteHost.trim().length > 0 && f.remotePort > 0));

  return (
    <Dialog open onClose={busy ? undefined : onCancel} maxWidth="sm" fullWidth>
      <DialogTitle>{initial ? "Edit rule" : "New forwarding rule"}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Stack direction="row" spacing={2}>
            <TextField
              autoFocus
              label="Label (optional)"
              value={f.label}
              onChange={(e) => set("label", e.target.value)}
              sx={{ flex: 1 }}
            />
            <TextField
              select
              label="Type"
              value={f.kind}
              onChange={(e) => set("kind", e.target.value as PfKind)}
              sx={{ width: 200 }}
            >
              {(Object.keys(KIND_LABEL) as PfKind[]).map((k) => (
                <MenuItem key={k} value={k}>
                  {KIND_LABEL[k]}
                </MenuItem>
              ))}
            </TextField>
          </Stack>
          <TextField
            select
            label="Through host"
            value={f.hostId}
            onChange={(e) => set("hostId", e.target.value)}
            helperText={sshHosts.length === 0 ? "Add an SSH host first" : undefined}
          >
            {sshHosts.map((h) => (
              <MenuItem key={h.id} value={h.id}>
                {h.label}
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ ml: 1 }}
                >
                  {h.address}
                </Typography>
              </MenuItem>
            ))}
          </TextField>
          <Stack direction="row" spacing={2}>
            <TextField
              label={f.kind === "remote" ? "Remote bind address" : "Bind address"}
              value={f.boundAddress}
              onChange={(e) => set("boundAddress", e.target.value)}
              sx={{ flex: 1 }}
            />
            <TextField
              label={f.kind === "remote" ? "Remote port" : "Local port"}
              type="number"
              value={f.kind === "remote" ? f.remotePort : f.localPort}
              onChange={(e) =>
                set(f.kind === "remote" ? "remotePort" : "localPort", port(e.target.value))
              }
              sx={{ width: 140 }}
            />
          </Stack>
          {f.kind !== "dynamic" && (
            <Stack direction="row" spacing={2}>
              <TextField
                label={f.kind === "remote" ? "Forward to local host" : "Destination host"}
                value={f.remoteHost}
                onChange={(e) => set("remoteHost", e.target.value)}
                sx={{ flex: 1 }}
              />
              <TextField
                label={f.kind === "remote" ? "Local port" : "Destination port"}
                type="number"
                value={f.kind === "remote" ? f.localPort : f.remotePort}
                onChange={(e) =>
                  set(f.kind === "remote" ? "localPort" : "remotePort", port(e.target.value))
                }
                sx={{ width: 140 }}
              />
            </Stack>
          )}
          <FormControlLabel
            control={
              <Checkbox
                checked={f.autoStart}
                onChange={(e) => set("autoStart", e.target.checked)}
              />
            }
            label="Start automatically when Termoso opens"
          />
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button variant="contained" disabled={!valid || busy} onClick={() => onConfirm(f)}>
          Save
        </Button>
      </DialogActions>
    </Dialog>
  );
}

type DialogState =
  | { kind: "none" }
  | { kind: "edit"; rule: PfRuleCard | null }
  | { kind: "delete"; rule: PfRuleCard };

export function ForwardingPage() {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vault = useDefaultVault();
  const vaultId = vault.data?.id ?? null;
  const rules = usePfRules(vaultId);
  const hosts = useHosts(vaultId);
  const [dialog, setDialog] = useState<DialogState>({ kind: "none" });
  const [busyId, setBusyId] = useState<Uuid | null>(null);

  useEffect(() => {
    let active = true;
    const un = ipc.onForwardEvent((ev) => {
      if (!active) return;
      qc.setQueriesData<PfRuleCard[]>({ queryKey: ["pfRules"] }, (old) =>
        old?.map((r) => (r.id === ev.id ? { ...r, runtime: ev.runtime } : r)),
      );
    });
    return () => {
      active = false;
      void un.then((f) => f());
    };
  }, [qc]);

  const invalidate = () => void qc.invalidateQueries({ queryKey: keys.pfRules(vaultId) });
  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      invalidate();
      setDialog({ kind: "none" });
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
    onSettled: () => setBusyId(null),
  });

  const toggle = (r: PfRuleCard) => {
    setBusyId(r.id);
    op.mutate(async () => {
      if (r.runtime.state === "stopped") {
        await ipc.pfStart(r.id);
        return null;
      }
      await ipc.pfStop(r.id);
      return null;
    });
  };

  const loading = vault.isPending || rules.isPending || hosts.isPending;
  const loadError = vault.error ?? rules.error ?? hosts.error;

  return (
    <Page>
      <PageHeader
        title="Port Forwarding"
        description="Local, remote and dynamic (SOCKS5) tunnels over your SSH hosts. Rules run in Rust; the UI only observes them."
        actions={
          <Button
            variant="contained"
            startIcon={<AddRoundedIcon />}
            disabled={!vaultId}
            onClick={() => setDialog({ kind: "edit", rule: null })}
          >
            New rule
          </Button>
        }
      />
      <PageBody>
        {loading ? (
          <Box sx={{ display: "flex", justifyContent: "center", pt: 8 }}>
            <CircularProgress size={28} />
          </Box>
        ) : loadError ? (
          <EmptyState title="Could not load rules" description={errorMessage(loadError)} />
        ) : (rules.data ?? []).length === 0 ? (
          <EmptyState
            icon={<SwapHorizRoundedIcon />}
            title="No forwarding rules"
            description="Expose a remote database locally, publish a local port on a server, or open a SOCKS proxy."
            action={
              <Button variant="contained" onClick={() => setDialog({ kind: "edit", rule: null })}>
                New rule
              </Button>
            }
          />
        ) : (
          <Table size="small" sx={{ mt: 1 }}>
            <TableHead>
              <TableRow sx={{ "& th": { color: "text.secondary", fontWeight: 600 } }}>
                <TableCell padding="checkbox" />
                <TableCell>Rule</TableCell>
                <TableCell>Type</TableCell>
                <TableCell>State</TableCell>
                <TableCell align="right">Conns</TableCell>
                <TableCell align="right">Traffic</TableCell>
                <TableCell padding="checkbox" />
                <TableCell padding="checkbox" />
              </TableRow>
            </TableHead>
            <TableBody>
              {(rules.data ?? []).map((r) => {
                const running = r.runtime.state !== "stopped";
                return (
                  <TableRow key={r.id} hover>
                    <TableCell padding="checkbox">
                      <Tooltip title={running ? "Stop" : "Start"}>
                        <IconButton
                          size="small"
                          color={running ? "default" : "primary"}
                          disabled={busyId === r.id}
                          onClick={() => toggle(r)}
                        >
                          {busyId === r.id ? (
                            <CircularProgress size={16} />
                          ) : running ? (
                            <StopRoundedIcon fontSize="small" />
                          ) : (
                            <PlayArrowRoundedIcon fontSize="small" />
                          )}
                        </IconButton>
                      </Tooltip>
                    </TableCell>
                    <TableCell>
                      <Typography variant="body2" sx={{ fontWeight: 600 }} noWrap>
                        {r.label || describe(r)}
                        {r.autoStart && (
                          <Chip
                            label="auto"
                            size="small"
                            variant="outlined"
                            sx={{ ml: 1, height: 18, fontSize: 10 }}
                          />
                        )}
                      </Typography>
                      <Typography
                        variant="caption"
                        color="text.secondary"
                        sx={{ fontFamily: "monospace" }}
                        noWrap
                      >
                        {describe(r)}
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Chip size="small" variant="outlined" label={KIND_LABEL[r.kind]} />
                    </TableCell>
                    <TableCell>
                      <StateChip r={r} />
                    </TableCell>
                    <TableCell align="right">
                      <Typography variant="body2" color="text.secondary">
                        {r.runtime.active}/{r.runtime.connections}
                      </Typography>
                    </TableCell>
                    <TableCell align="right">
                      <Typography variant="body2" color="text.secondary" noWrap>
                        ↓{formatSize(r.runtime.bytesIn)} ↑{formatSize(r.runtime.bytesOut)}
                      </Typography>
                    </TableCell>
                    <TableCell padding="checkbox">
                      <IconButton
                        size="small"
                        disabled={running}
                        onClick={() => setDialog({ kind: "edit", rule: r })}
                      >
                        <EditRoundedIcon fontSize="small" />
                      </IconButton>
                    </TableCell>
                    <TableCell padding="checkbox">
                      <IconButton
                        size="small"
                        onClick={() => setDialog({ kind: "delete", rule: r })}
                      >
                        <DeleteOutlineRoundedIcon fontSize="small" />
                      </IconButton>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        )}
      </PageBody>

      {vaultId && dialog.kind === "edit" && (
        <RuleDialog
          vaultId={vaultId}
          hosts={hosts.data ?? []}
          initial={dialog.rule}
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={(form) =>
            op.mutate(async () => {
              await ipc.pfSave(form);
              return null;
            })
          }
        />
      )}
      {dialog.kind === "delete" && (
        <ConfirmDialog
          open
          title="Delete rule?"
          confirmLabel="Delete"
          danger
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={() => {
            const id = dialog.rule.id;
            op.mutate(async () => {
              await ipc.pfDelete(id);
              return "Rule deleted";
            });
          }}
        >
          {dialog.rule.runtime.state !== "stopped" && "The tunnel will be stopped. "}
          <b>{dialog.rule.label || describe(dialog.rule)}</b> will be removed.
        </ConfirmDialog>
      )}
    </Page>
  );
}
