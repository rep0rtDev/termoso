import { useState } from "react";
import { tr } from "@/i18n";
import { Alert, Button, Stack, TextField, Typography } from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "@/api/client";
import { teamsApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { Team } from "@/api/types";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Loading } from "@/components/Loading";
import { Section, SettingRow } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { saveBlob } from "@/components/download";
import { formatDate, formatDateTime } from "@/components/format";
import { can } from "./perm";

export function DataTab({ team }: { team: Team }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const q = useQuery({
    queryKey: queryKeys.teamData(team.id),
    queryFn: () => teamsApi.data(team.id),
  });
  const [confirming, setConfirming] = useState(false);
  const [name, setName] = useState("");
  const owner = team.my_role === "owner";
  const exp = useMutation({
    mutationFn: async () => {
      const blob = await teamsApi.exportData(team.id);
      if (!blob) throw new Error(tr("Nothing to export"));
      saveBlob(
        blob,
        `termoso-team-${team.name.replace(/[^\w.-]+/g, "_")}-${new Date().toISOString().slice(0, 10)}.json`,
      );
    },
    onSuccess: () => snack.notify(tr("Export downloaded")),
    onError: (e) => snack.error(errorMessage(e)),
  });
  const schedule = useMutation({
    mutationFn: () => teamsApi.scheduleDeletion(team.id, name.trim()),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: queryKeys.teamData(team.id) });
      setConfirming(false);
      setName("");
      snack.notify(tr("Deletion scheduled; every member has been notified"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const cancel = useMutation({
    mutationFn: () => teamsApi.cancelDeletion(team.id),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: queryKeys.teamData(team.id) });
      snack.notify(tr("Deletion cancelled"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  if (q.isPending) return <Loading />;
  if (q.isError) return <Alert severity="error">{errorMessage(q.error)}</Alert>;
  const d = q.data;
  return (
    <Stack spacing={2}>
      <Section title={tr("Team data")}>
        <SettingRow
          label={tr("Data region")}
          description={tr(
            "Where this server keeps the team's data. Set by the operator (TERMOSO_DATA_REGION).",
          )}
          control={
            <Typography variant="body2">
              {d.region ?? d.server_region ?? tr("Not specified")}
            </Typography>
          }
        />
        <SettingRow
          label={tr("Created")}
          control={<Typography variant="body2">{formatDate(d.created_at)}</Typography>}
        />
        <SettingRow
          label={tr("Export everything")}
          description={tr(
            "One JSON with members, roles, policies, vaults, encrypted entities, recording metadata, audit, webhooks, ACL, access requests, rotation and IdP settings. Entity contents stay encrypted — the server cannot read them.",
          )}
          divider={false}
          control={
            <Button
              variant="outlined"
              disabled={!can(team, "team.export") || exp.isPending}
              onClick={() => exp.mutate()}
            >
              {tr("Download export")}
            </Button>
          }
        />
      </Section>
      <Section title={tr("Delete team and all its data")}>
        {d.deletion_scheduled_at ? (
          <Alert
            severity="warning"
            action={
              owner && (
                <Button
                  color="inherit"
                  size="small"
                  disabled={cancel.isPending}
                  onClick={() => cancel.mutate()}
                >
                  {tr("Cancel deletion")}
                </Button>
              )
            }
          >
            {tr(
              "Deletion is scheduled for {v1}. Until then the team keeps working; the owner can cancel.",
              {
                v1: formatDateTime(d.deletion_scheduled_at),
              },
            )}
          </Alert>
        ) : (
          <SettingRow
            label={tr("Request deletion")}
            description={
              d.deletion_grace_days === 0
                ? tr(
                    "Everything the server stores for this team is removed immediately and cannot be recovered.",
                  )
                : tr(
                    "Everything the server stores for this team is removed after {v1} days; members are notified and the owner can cancel in the meantime.",
                    { v1: d.deletion_grace_days },
                  )
            }
            divider={false}
            control={
              <Button
                color="error"
                variant="outlined"
                disabled={!owner}
                onClick={() => setConfirming(true)}
              >
                {tr("Delete team")}
              </Button>
            }
          />
        )}
      </Section>
      <ConfirmDialog
        open={confirming}
        title={tr("Delete team?")}
        confirmLabel={tr("Schedule deletion")}
        danger
        busy={schedule.isPending}
        onCancel={() => setConfirming(false)}
        onConfirm={() => schedule.mutate()}
      >
        <Stack spacing={2}>
          <Typography variant="body2">
            {tr("Type the team name {v1} to confirm.", { v1: team.name })}
          </Typography>
          <TextField
            size="small"
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </Stack>
      </ConfirmDialog>
    </Stack>
  );
}
