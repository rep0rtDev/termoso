import { useState } from "react";
import {
  Alert,
  Button,
  Chip,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { tr } from "@/i18n";
import * as ipc from "@/ipc/commands";

import { errorMessage, type Team } from "@/ipc/types";
import { Loading } from "@/components/ui";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { useSnackbar } from "@/components/Snackbar";
import { Section } from "./Section";
import type { TeamDevice } from "@/ipc/types";

export function DevicesTab({ team }: { team: Team }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const key = ["team", team.id, "devices"];
  const q = useQuery({ queryKey: key, queryFn: () => ipc.teamDevices(team.id) });
  const [rejecting, setRejecting] = useState<TeamDevice | null>(null);
  const refresh = () => qc.invalidateQueries({ queryKey: key });
  const approve = useMutation({
    mutationFn: (d: TeamDevice) => ipc.teamApproveDevice(team.id, d.device_id),
    onSuccess: async () => {
      await refresh();
      snack.notify(tr("Device approved"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const reject = useMutation({
    mutationFn: (d: TeamDevice) => ipc.teamRejectDevice(team.id, d.device_id),
    onSuccess: async () => {
      await refresh();
      setRejecting(null);
      snack.notify(tr("Device rejected and signed out"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  if (q.isPending) return <Loading />;
  if (q.isError) return <Alert severity="error">{errorMessage(q.error)}</Alert>;
  const pending = q.data.devices.filter((d) => d.status === "pending");
  const decided = q.data.devices.filter((d) => d.status !== "pending");
  const chip = (d: TeamDevice) =>
    d.status === "pending" ? (
      <Chip size="small" color="warning" label={tr("Pending")} />
    ) : d.status === "approved" ? (
      <Chip size="small" color="success" variant="outlined" label={tr("Approved")} />
    ) : (
      <Chip size="small" color="error" variant="outlined" label={tr("Rejected")} />
    );
  const rows = (list: TeamDevice[], actions: boolean) => (
    <TableContainer>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{tr("Member")}</TableCell>
            <TableCell>{tr("Device")}</TableCell>
            <TableCell>{tr("Last IP")}</TableCell>
            <TableCell>{tr("Last seen")}</TableCell>
            <TableCell>{tr("Status")}</TableCell>
            {actions && <TableCell align="right" />}
          </TableRow>
        </TableHead>
        <TableBody>
          {list.map((d) => (
            <TableRow key={d.device_id} hover>
              <TableCell>{d.user_email}</TableCell>
              <TableCell>
                <Typography variant="body2">{d.name}</Typography>
                <Typography variant="caption" color="text.secondary">
                  {d.platform}
                  {d.app_version ? ` · ${d.app_version}` : ""}
                </Typography>
              </TableCell>
              <TableCell sx={{ fontFamily: "monospace" }}>{d.last_ip ?? "—"}</TableCell>
              <TableCell>{new Date(d.last_seen_at).toLocaleString()}</TableCell>
              <TableCell>{chip(d)}</TableCell>
              {actions && (
                <TableCell align="right">
                  <Stack direction="row" spacing={1} sx={{ justifyContent: "flex-end" }}>
                    <Button
                      size="small"
                      variant="contained"
                      disabled={approve.isPending}
                      onClick={() => approve.mutate(d)}
                    >
                      {tr("Approve")}
                    </Button>
                    <Button size="small" color="error" onClick={() => setRejecting(d)}>
                      {tr("Reject")}
                    </Button>
                  </Stack>
                </TableCell>
              )}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  );
  return (
    <Stack spacing={2}>
      {!q.data.admin_approval && (
        <Alert severity="info">
          {tr(
            "Admin approval for new devices is off in this team's policies; new devices sign in without a review.",
          )}
        </Alert>
      )}
      <Section
        title={tr("Devices awaiting approval")}
        description={tr(
          "New devices of members sign in only after approval. Rejecting ends the device's sessions; it can sign in again later and will wait here once more.",
        )}
      >
        {pending.length === 0 ? (
          <EmptyState title={tr("No pending devices")} compact />
        ) : (
          rows(pending, true)
        )}
      </Section>
      {decided.length > 0 && (
        <Section title={tr("Reviewed devices")}>{rows(decided, false)}</Section>
      )}
      <ConfirmDialog
        open={rejecting !== null}
        title={tr("Reject device?")}
        confirmLabel={tr("Reject")}
        danger
        busy={reject.isPending}
        onCancel={() => setRejecting(null)}
        onConfirm={() => rejecting && reject.mutate(rejecting)}
      >
        {rejecting &&
          tr("{v1} on {v2} will be signed out and blocked until approved again.", {
            v1: rejecting.user_email,
            v2: rejecting.name,
          })}
      </ConfirmDialog>
    </Stack>
  );
}
