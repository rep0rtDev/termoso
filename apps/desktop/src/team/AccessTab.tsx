import { useState } from "react";
import {
  Alert,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
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
import type { AccessRequest, AccessRole } from "@/ipc/types";

const STATUS: Record<
  AccessRequest["status"],
  { label: () => string; color: "default" | "warning" | "success" | "error" }
> = {
  pending: { label: () => tr("Pending"), color: "warning" },
  approved: { label: () => tr("Active"), color: "success" },
  denied: { label: () => tr("Denied"), color: "error" },
  revoked: { label: () => tr("Revoked"), color: "default" },
  expired: { label: () => tr("Expired"), color: "default" },
  cancelled: { label: () => tr("Cancelled"), color: "default" },
};

export function AccessTab({ team, myId }: { team: Team; myId: string | null }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const key = ["team", team.id, "access"];
  const q = useQuery({ queryKey: key, queryFn: () => ipc.teamAccessRequests(team.id) });
  const vaults = useVaults();
  const [creating, setCreating] = useState(false);
  const [deciding, setDeciding] = useState<AccessRequest | null>(null);
  const refresh = async () => {
    await qc.invalidateQueries({ queryKey: key });
    await qc.invalidateQueries({ queryKey: ["vaults"] });
  };
  const act = useMutation({
    mutationFn: async ({ r, op }: { r: AccessRequest; op: "cancel" | "deny" | "revoke" }) => {
      if (op === "cancel") await ipc.teamCancelAccessRequest(team.id, r.id);
      else await ipc.teamDecideAccessRequest(team.id, r.id, op, {});
    },
    onSuccess: refresh,
    onError: (e) => snack.error(errorMessage(e)),
  });
  if (q.isPending) return <Loading />;
  if (q.isError) return <Alert severity="error">{errorMessage(q.error)}</Alert>;
  const teamVaults = (vaults.data ?? []).filter((v) => v.kind === "team" && v.team_id === team.id);
  const open = q.data.requests.filter((r) => r.status === "pending" || r.status === "approved");
  const closed = q.data.requests.filter(
    (r) => !(r.status === "pending" || r.status === "approved"),
  );
  const table = (rows: AccessRequest[]) => (
    <TableContainer>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{tr("Member")}</TableCell>
            <TableCell>{tr("Vault")}</TableCell>
            <TableCell>{tr("Access")}</TableCell>
            <TableCell>{tr("Reason")}</TableCell>
            <TableCell>{tr("Status")}</TableCell>
            <TableCell align="right" />
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((r) => {
            const st = STATUS[r.status];
            const mine = r.user_id === myId;
            return (
              <TableRow key={r.id} hover>
                <TableCell>{mine ? tr("You") : r.user_email}</TableCell>
                <TableCell>{r.vault_name}</TableCell>
                <TableCell>
                  <Typography variant="body2">
                    {r.role === "editor" ? tr("Editor") : tr("Viewer")} ·{" "}
                    {tr("{v1} h", { v1: r.hours })}
                  </Typography>
                  {r.expires_at && r.status === "approved" && (
                    <Typography variant="caption" color="text.secondary">
                      {tr("until {v1}", { v1: new Date(r.expires_at).toLocaleString() })}
                    </Typography>
                  )}
                </TableCell>
                <TableCell sx={{ maxWidth: 260 }}>
                  <Typography variant="body2" noWrap title={r.reason}>
                    {r.reason}
                  </Typography>
                </TableCell>
                <TableCell>
                  <Chip
                    size="small"
                    color={st.color}
                    variant={st.color === "default" ? "outlined" : "filled"}
                    label={st.label()}
                  />
                </TableCell>
                <TableCell align="right">
                  <Stack direction="row" spacing={1} sx={{ justifyContent: "flex-end" }}>
                    {r.status === "pending" && q.data.can_approve && (
                      <>
                        <Button size="small" variant="contained" onClick={() => setDeciding(r)}>
                          {tr("Approve")}
                        </Button>
                        <Button
                          size="small"
                          color="error"
                          onClick={() => act.mutate({ r, op: "deny" })}
                        >
                          {tr("Deny")}
                        </Button>
                      </>
                    )}
                    {r.status === "pending" && mine && (
                      <Button size="small" onClick={() => act.mutate({ r, op: "cancel" })}>
                        {tr("Withdraw")}
                      </Button>
                    )}
                    {r.status === "approved" && q.data.can_approve && (
                      <Button
                        size="small"
                        color="error"
                        onClick={() => act.mutate({ r, op: "revoke" })}
                      >
                        {tr("Revoke")}
                      </Button>
                    )}
                  </Stack>
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </TableContainer>
  );
  return (
    <Stack spacing={2}>
      <Section
        title={tr("Access requests")}
        description={tr(
          "Temporary (just-in-time) access to a team vault: a member asks for viewer or editor rights for a number of hours, an approver grants them, and the server removes the membership when the time is up. Approval needs a manager's client to seal the vault key for the member.",
        )}
        actions={
          teamVaults.length > 0 && (
            <Button variant="contained" onClick={() => setCreating(true)}>
              {tr("Request access")}
            </Button>
          )
        }
      >
        {open.length === 0 ? <EmptyState title={tr("No open requests")} compact /> : table(open)}
      </Section>
      {closed.length > 0 && <Section title={tr("History")}>{table(closed)}</Section>}
      <RequestDialog
        open={creating}
        team={team}
        vaults={teamVaults.map((v) => ({ id: v.id, name: v.name }))}
        onClose={() => setCreating(false)}
        onDone={() => {
          setCreating(false);
          void refresh();
        }}
      />
      <DecideDialog
        request={deciding}
        team={team}
        onClose={() => setDeciding(null)}
        onDone={() => {
          setDeciding(null);
          void refresh();
        }}
      />
    </Stack>
  );
}

function RequestDialog({
  open,
  team,
  vaults,
  onClose,
  onDone,
}: {
  open: boolean;
  team: Team;
  vaults: { id: string; name: string }[];
  onClose: () => void;
  onDone: () => void;
}) {
  const snack = useSnackbar();
  const [vault, setVault] = useState("");
  const [role, setRole] = useState<AccessRole>("viewer");
  const [hours, setHours] = useState(8);
  const [reason, setReason] = useState("");
  const vaultId = vault !== "" ? vault : (vaults[0]?.id ?? "");
  const m = useMutation({
    mutationFn: () =>
      ipc.teamCreateAccessRequest(team.id, { vault_id: vaultId, role, hours, reason }),
    onSuccess: () => {
      snack.notify(tr("Request sent to the approvers"));
      setReason("");
      onDone();
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  return (
    <Dialog open={open} onClose={onClose} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Request access")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 1 }}>
          <TextField
            select
            size="small"
            label={tr("Vault")}
            value={vaultId}
            onChange={(e) => setVault(e.target.value)}
          >
            {vaults.map((v) => (
              <MenuItem key={v.id} value={v.id}>
                {v.name}
              </MenuItem>
            ))}
          </TextField>
          <TextField
            select
            size="small"
            label={tr("Access")}
            value={role}
            onChange={(e) => setRole(e.target.value as AccessRole)}
          >
            <MenuItem value="viewer">{tr("Viewer")}</MenuItem>
            <MenuItem value="editor">{tr("Editor")}</MenuItem>
          </TextField>
          <TextField
            size="small"
            type="number"
            label={tr("Hours")}
            value={hours}
            onChange={(e) => setHours(Number(e.target.value))}
            slotProps={{ htmlInput: { min: 1, max: 720 } }}
          />
          <TextField
            size="small"
            multiline
            minRows={2}
            label={tr("Reason")}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            helperText={tr("Shown to the approvers and kept in the audit log.")}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{tr("Cancel")}</Button>
        <Button
          variant="contained"
          disabled={!reason.trim() || hours < 1 || m.isPending}
          onClick={() => m.mutate()}
        >
          {tr("Send")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function DecideDialog({
  request,
  team,
  onClose,
  onDone,
}: {
  request: AccessRequest | null;
  team: Team;
  onClose: () => void;
  onDone: () => void;
}) {
  const snack = useSnackbar();
  const [hours, setHours] = useState<number | null>(null);
  const [role, setRole] = useState<AccessRole | null>(null);
  const [note, setNote] = useState("");
  const m = useMutation({
    mutationFn: () => {
      if (!request) return Promise.reject(new Error("no request"));
      return ipc.teamDecideAccessRequest(team.id, request.id, "approve", {
        hours: hours ?? undefined,
        role: role ?? undefined,
        note: note.trim() || undefined,
      });
    },
    onSuccess: () => {
      snack.notify(
        tr("Access granted — open the vault to seal its key for the member (Pending vault keys)."),
      );
      setHours(null);
      setRole(null);
      setNote("");
      onDone();
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  if (!request) return null;
  return (
    <Dialog open onClose={onClose} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Approve access")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 1 }}>
          <Typography variant="body2" color="text.secondary">
            {tr("{v1} asks for {v2} access to {v3} for {v4} h: {v5}", {
              v1: request.user_email,
              v2: request.role === "editor" ? tr("editor") : tr("viewer"),
              v3: request.vault_name,
              v4: request.hours,
              v5: request.reason,
            })}
          </Typography>
          <TextField
            select
            size="small"
            label={tr("Access")}
            value={role ?? request.role}
            onChange={(e) => setRole(e.target.value as AccessRole)}
          >
            <MenuItem value="viewer">{tr("Viewer")}</MenuItem>
            <MenuItem value="editor">{tr("Editor")}</MenuItem>
          </TextField>
          <TextField
            size="small"
            type="number"
            label={tr("Hours")}
            value={hours ?? request.hours}
            onChange={(e) => setHours(Number(e.target.value))}
            slotProps={{ htmlInput: { min: 1, max: request.hours } }}
            helperText={tr("You can shorten the request, not extend it.")}
          />
          <TextField
            size="small"
            label={tr("Note (optional)")}
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{tr("Cancel")}</Button>
        <Button variant="contained" disabled={m.isPending} onClick={() => m.mutate()}>
          {tr("Approve")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
