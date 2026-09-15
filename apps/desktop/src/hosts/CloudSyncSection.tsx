import { useState } from "react";
import { Box, Button, CircularProgress, Typography } from "@mui/material";
import SyncRoundedIcon from "@mui/icons-material/SyncRounded";
import { useSnackbar } from "@/components/Snackbar";
import { SectionCard } from "@/components/ui";
import { useCloudSyncGroups, useRunCloudSync } from "@/ipc/hooks";
import { errorMessage, type GroupNode } from "@/ipc/types";
import { CloudSyncDialog } from "./CloudSyncDialog";
import { reportLine, syncSummary } from "./cloudSync";

const TONE_COLOR = {
  ok: "text.secondary",
  idle: "text.secondary",
  running: "text.secondary",
  paused: "text.secondary",
  error: "error.main",
} as const;

/** Group details → Cloud sync card: status line, “Sync now” and the settings dialog. */
export function CloudSyncSection({ group, readOnly }: { group: GroupNode; readOnly: boolean }) {
  const snackbar = useSnackbar();
  const synced = useCloudSyncGroups(group.vaultId);
  const run = useRunCloudSync();
  const [open, setOpen] = useState(false);
  const existing = (synced.data ?? []).find((g) => g.groupId === group.id) ?? null;
  const summary = existing ? syncSummary(existing) : null;
  const report = existing ? reportLine(existing) : null;
  const running = !!existing?.running || run.isPending;

  const syncNow = () =>
    run.mutate(group.id, {
      onSuccess: (g) => {
        if (g.status.error) snackbar.error(g.status.error);
        else snackbar.notify(`“${group.label}” synced: ${reportLine(g) ?? "done"}`);
      },
      onError: (e) => snackbar.error(errorMessage(e)),
    });

  return (
    <SectionCard
      title="Cloud sync"
      tone={summary?.tone === "error" ? "warning" : undefined}
      action={
        existing ? (
          <Box sx={{ display: "flex", gap: 0.5 }}>
            <Button
              size="small"
              color="inherit"
              disabled={running || readOnly || !existing.hasSecret}
              onClick={syncNow}
              startIcon={
                running ? (
                  <CircularProgress size={12} color="inherit" />
                ) : (
                  <SyncRoundedIcon fontSize="small" />
                )
              }
            >
              Sync now
            </Button>
            <Button size="small" color="inherit" onClick={() => setOpen(true)}>
              Settings
            </Button>
          </Box>
        ) : (
          <Button size="small" color="inherit" disabled={readOnly} onClick={() => setOpen(true)}>
            Set up…
          </Button>
        )
      }
    >
      {existing && summary ? (
        <Box data-testid="cloud-sync-summary">
          <Typography variant="body2" sx={{ color: TONE_COLOR[summary.tone] }}>
            {summary.text}
          </Typography>
          {report && (
            <Typography variant="caption" color="text.secondary">
              {report}
            </Typography>
          )}
        </Box>
      ) : (
        <Typography variant="body2" color="text.secondary">
          Mirror the machines of an AWS, DigitalOcean or Azure account into this group, on a
          schedule. Credentials stay encrypted on this device.
        </Typography>
      )}
      <CloudSyncDialog
        open={open}
        group={group}
        existing={existing}
        readOnly={readOnly}
        onClose={() => setOpen(false)}
      />
    </SectionCard>
  );
}
