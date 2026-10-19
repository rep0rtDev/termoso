import { useState } from "react";
import { Alert, Button, Stack, TextField, Typography } from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { tr } from "@/i18n";
import * as ipc from "@/ipc/commands";

import { errorMessage, type Team } from "@/ipc/types";
import { Loading, SettingRow } from "@/components/ui";
import { ConfirmDialog } from "@/components/ConfirmDialog";

import { useSnackbar } from "@/components/Snackbar";
import { Section } from "./Section";
import { can } from "./roles";
import { save } from "@tauri-apps/plugin-dialog";

export function DataTab({ team }: { team: Team }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const key = ["team", team.id, "data"];
  const q = useQuery({ queryKey: key, queryFn: () => ipc.teamData(team.id) });
  const [confirming, setConfirming] = useState(false);
  const [name, setName] = useState("");
  const owner = team.my_role === "owner";
  const exp = useMutation({
    mutationFn: async () => {
      const json = await ipc.teamExport(team.id);
      const path = await save({
        defaultPath: `termoso-team-${team.name.replace(/[^\w.-]+/g, "_")}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return false;
      await ipc.saveTextFile(path, json);
      return true;
    },
    onSuccess: (saved) => saved && snack.notify(tr("Export downloaded")),
    onError: (e) => snack.error(errorMessage(e)),
  });
  const schedule = useMutation({
    mutationFn: () => ipc.teamScheduleDeletion(team.id, name.trim()),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: key });
      setConfirming(false);
      setName("");
      snack.notify(tr("Deletion scheduled; every member has been notified"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const cancel = useMutation({
    mutationFn: () => ipc.teamCancelDeletion(team.id),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: key });
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
          hint={tr(
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
          control={
            <Typography variant="body2">{new Date(d.created_at).toLocaleDateString()}</Typography>
          }
        />
        <SettingRow
          label={tr("Export everything")}
          hint={tr(
            "One JSON with members, roles, policies, vaults, encrypted entities, recording metadata, audit, webhooks, ACL, access requests, rotation and IdP settings. Entity contents stay encrypted — the server cannot read them.",
          )}
          last
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
      <Section title={tr("Delete team and all its data")} danger>
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
              { v1: new Date(d.deletion_scheduled_at).toLocaleString() },
            )}
          </Alert>
        ) : (
          <SettingRow
            label={tr("Request deletion")}
            hint={
              d.deletion_grace_days === 0
                ? tr(
                    "Everything the server stores for this team is removed immediately and cannot be recovered.",
                  )
                : tr(
                    "Everything the server stores for this team is removed after {v1} days; members are notified and the owner can cancel in the meantime.",
                    { v1: d.deletion_grace_days },
                  )
            }
            last
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
