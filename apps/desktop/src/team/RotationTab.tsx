import { useState } from "react";
import {
  Alert,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { tr } from "@/i18n";
import * as ipc from "@/ipc/commands";
import { useVaults } from "@/ipc/hooks";
import { errorMessage, type Team } from "@/ipc/types";
import { Loading } from "@/components/ui";

import { EmptyState } from "@/components/EmptyState";
import { useSnackbar } from "@/components/Snackbar";
import { Section } from "./Section";

import { useMemo } from "react";
import { Checkbox } from "@mui/material";
import { useHosts, useSshKeys, useIdentities } from "@/ipc/hooks";
import type { RotationDue, RotationJob } from "@/ipc/types";

const KIND: Record<string, () => string> = {
  key: () => tr("SSH key"),
  certificate: () => tr("Certificate"),
  identity: () => tr("Identity"),
  host: () => tr("Host password"),
};
const JOB: Record<
  RotationJob["status"],
  { label: () => string; color: "default" | "warning" | "info" | "success" | "error" }
> = {
  pending: { label: () => tr("Queued"), color: "warning" },
  running: { label: () => tr("Running"), color: "info" },
  done: { label: () => tr("Done"), color: "success" },
  failed: { label: () => tr("Failed"), color: "error" },
  cancelled: { label: () => tr("Cancelled"), color: "default" },
};

export function RotationTab({ team }: { team: Team }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const key = ["team", team.id, "rotation"];
  const q = useQuery({ queryKey: key, queryFn: () => ipc.teamRotation(team.id) });
  const vaults = useVaults();
  const hosts = useHosts(null);
  const keys = useSshKeys(null);
  const identities = useIdentities(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [queueing, setQueueing] = useState(false);
  const [note, setNote] = useState("");
  const refresh = () => qc.invalidateQueries({ queryKey: key });
  const vaultName = (id: string) => vaults.data?.find((v) => v.id === id)?.name ?? id.slice(0, 8);
  const entityName = (id: string) =>
    hosts.data?.find((h) => h.id === id)?.label ??
    (keys.data as { id: string; label?: string; name?: string }[] | undefined)?.find(
      (k) => k.id === id,
    )?.label ??
    (identities.data as { id: string; label?: string; name?: string }[] | undefined)?.find(
      (k) => k.id === id,
    )?.label ??
    id.slice(0, 8);
  const create = useMutation({
    mutationFn: async () => {
      const byVault = new Map<string, string[]>();
      for (const d of q.data?.due ?? [])
        if (selected.has(d.entity_id))
          byVault.set(d.vault_id, [...(byVault.get(d.vault_id) ?? []), d.entity_id]);
      for (const [vault_id, entity_ids] of byVault)
        await ipc.teamCreateRotationJobs(team.id, { vault_id, entity_ids, note: note.trim() });
    },
    onSuccess: async () => {
      await refresh();
      setSelected(new Set());
      setQueueing(false);
      setNote("");
      snack.notify(
        tr("Rotation jobs queued; a client with write access to the vault will perform them"),
      );
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const cancel = useMutation({
    mutationFn: (j: RotationJob) => ipc.rotationCancelJob(j.id),
    onSuccess: refresh,
    onError: (e) => snack.error(errorMessage(e)),
  });
  const due = useMemo(
    () => [...(q.data?.due ?? [])].sort((a, b) => b.age_days - a.age_days),
    [q.data],
  );
  if (q.isPending) return <Loading />;
  if (q.isError) return <Alert severity="error">{errorMessage(q.error)}</Alert>;
  const o = q.data;
  const toggle = (d: RotationDue) => {
    const next = new Set(selected);
    if (next.has(d.entity_id)) next.delete(d.entity_id);
    else next.add(d.entity_id);
    setSelected(next);
  };
  const selectable = due.filter((d) => !d.job_id);
  const all = selectable.length > 0 && selectable.every((d) => selected.has(d.entity_id));
  return (
    <Stack spacing={2}>
      {o.key_max_age_days === 0 && o.password_max_age_days === 0 && (
        <Alert severity="info">
          {tr(
            "Rotation ages are not set in the team policies, so nothing is reported as due. Set “Key rotation age” or “Password rotation age” to start tracking.",
          )}
        </Alert>
      )}
      <Section
        title={tr("Due for rotation")}
        description={tr(
          "Secrets older than the policy allows. The server only sees when a secret was last rotated — never the secret itself. Queue a job and a client with write access to the vault generates the new key, deploys it to the host and replaces it in the vault.",
        )}
        actions={
          selected.size > 0 && (
            <Button variant="contained" onClick={() => setQueueing(true)}>
              {tr("Queue rotation ({v1})", { v1: selected.size })}
            </Button>
          )
        }
      >
        {due.length === 0 ? (
          <EmptyState title={tr("Nothing is due")} compact />
        ) : (
          <TableContainer>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell padding="checkbox">
                    <Checkbox
                      size="small"
                      checked={all}
                      indeterminate={selected.size > 0 && !all}
                      onChange={() =>
                        setSelected(all ? new Set() : new Set(selectable.map((d) => d.entity_id)))
                      }
                    />
                  </TableCell>
                  <TableCell>{tr("Entity")}</TableCell>
                  <TableCell>{tr("Vault")}</TableCell>
                  <TableCell align="right">{tr("Age")}</TableCell>
                  <TableCell>{tr("Last rotated")}</TableCell>
                  <TableCell>{tr("Status")}</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {due.map((d) => (
                  <TableRow key={d.entity_id} hover>
                    <TableCell padding="checkbox">
                      <Checkbox
                        size="small"
                        disabled={!!d.job_id}
                        checked={selected.has(d.entity_id)}
                        onChange={() => toggle(d)}
                      />
                    </TableCell>
                    <TableCell>
                      <Typography variant="body2">{entityName(d.entity_id)}</Typography>
                      <Typography variant="caption" color="text.secondary">
                        {(KIND[d.kind] ?? (() => d.kind))()}
                      </Typography>
                    </TableCell>
                    <TableCell>{vaultName(d.vault_id)}</TableCell>
                    <TableCell align="right">{tr("{v1} days", { v1: d.age_days })}</TableCell>
                    <TableCell>
                      {d.secret_rotated_at
                        ? new Date(d.secret_rotated_at).toLocaleDateString()
                        : tr("Never")}
                    </TableCell>
                    <TableCell>
                      {d.job_id ? (
                        <Chip size="small" color="warning" label={tr("Job queued")} />
                      ) : (
                        <Chip size="small" color="error" variant="outlined" label={tr("Overdue")} />
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        )}
      </Section>
      <Section title={tr("Rotation jobs")}>
        {o.jobs.length === 0 ? (
          <EmptyState title={tr("No jobs yet")} compact />
        ) : (
          <TableContainer>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>{tr("Entity")}</TableCell>
                  <TableCell>{tr("Vault")}</TableCell>
                  <TableCell>{tr("Status")}</TableCell>
                  <TableCell>{tr("Requested")}</TableCell>
                  <TableCell align="right" />
                </TableRow>
              </TableHead>
              <TableBody>
                {o.jobs.map((j) => {
                  const st = JOB[j.status];
                  return (
                    <TableRow key={j.id} hover>
                      <TableCell>
                        <Typography variant="body2">{entityName(j.entity_id)}</Typography>
                        <Typography variant="caption" color="text.secondary">
                          {(KIND[j.entity_kind] ?? (() => j.entity_kind))()}
                          {j.note ? ` · ${j.note}` : ""}
                        </Typography>
                      </TableCell>
                      <TableCell>{vaultName(j.vault_id)}</TableCell>
                      <TableCell>
                        <Chip
                          size="small"
                          color={st.color}
                          variant={st.color === "default" ? "outlined" : "filled"}
                          label={st.label()}
                        />
                        {j.error && (
                          <Typography variant="caption" color="error" sx={{ display: "block" }}>
                            {j.error}
                          </Typography>
                        )}
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2">{j.requested_by_email}</Typography>
                        <Typography variant="caption" color="text.secondary">
                          {new Date(j.created_at).toLocaleString()}
                        </Typography>
                      </TableCell>
                      <TableCell align="right">
                        {(j.status === "pending" || j.status === "running") && (
                          <Button
                            size="small"
                            color="error"
                            disabled={cancel.isPending}
                            onClick={() => cancel.mutate(j)}
                          >
                            {tr("Cancel")}
                          </Button>
                        )}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </TableContainer>
        )}
      </Section>
      <Dialog open={queueing} onClose={() => setQueueing(false)} maxWidth="xs" fullWidth>
        <DialogTitle>{tr("Queue rotation")}</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            <Typography variant="body2" color="text.secondary">
              {tr(
                "{v1} secrets will be rotated by the next client with write access to their vaults. Members are not interrupted; old keys stop working once the new ones are deployed.",
                { v1: selected.size },
              )}
            </Typography>
            <TextField
              size="small"
              label={tr("Note (optional)")}
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setQueueing(false)}>{tr("Cancel")}</Button>
          <Button variant="contained" disabled={create.isPending} onClick={() => create.mutate()}>
            {tr("Queue")}
          </Button>
        </DialogActions>
      </Dialog>
    </Stack>
  );
}
