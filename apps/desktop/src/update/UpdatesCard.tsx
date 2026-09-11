import { useState } from "react";
import { Alert, Box, Button, LinearProgress, MenuItem, TextField, Typography } from "@mui/material";
import { SectionCard, SettingRow } from "@/components/ui";
import { monoFontFamily } from "@/theme/theme";
import type { Settings, UpdateCheck } from "@/ipc/types";
import { formatSize } from "@/sftp/format";
import { checkForUpdates, installUpdate, restartToUpdate, useUpdate } from "./store";

export const DEFAULT_FEED =
  "https://github.com/rep0rtDev/termoso/releases/latest/download/latest.json";

export function UpdatesCard({
  settings,
  onChange,
}: {
  settings: Settings;
  onChange: (patch: Partial<Settings>) => void;
}) {
  const phase = useUpdate((s) => s.phase);
  const [feed, setFeed] = useState<string | null>(null);
  const busy = phase.kind === "checking" || phase.kind === "downloading";

  const commitFeed = () => {
    if (feed === null) return;
    const next = feed.trim();
    if (next !== settings.updateUrl) onChange({ updateUrl: next });
    setFeed(null);
  };

  return (
    <SectionCard
      title="Updates"
      action={
        <Box sx={{ display: "flex", gap: 1 }}>
          {phase.kind === "available" && (
            <Button variant="contained" onClick={() => void installUpdate()}>
              Install {phase.info.version}
            </Button>
          )}
          {phase.kind === "installed" && (
            <Button variant="contained" onClick={() => void restartToUpdate()}>
              Restart to finish
            </Button>
          )}
          <Button variant="tonal" disabled={busy} onClick={() => void checkForUpdates()}>
            {phase.kind === "checking" ? "Checking…" : "Check now"}
          </Button>
        </Box>
      }
    >
      <SettingRow
        label="Check for updates"
        hint="Nothing is fetched unless you press the button or enable the startup check."
        control={
          <TextField
            select
            value={settings.updateCheck}
            onChange={(e) => onChange({ updateCheck: e.target.value as UpdateCheck })}
            sx={{ width: 200 }}
          >
            <MenuItem value="manual">Only when I ask</MenuItem>
            <MenuItem value="startup">Once at startup</MenuItem>
          </TextField>
        }
      />
      <SettingRow
        label="Release feed"
        hint="https URL of latest.json. Point it at your own server to keep updates self-hosted; manifests and packages are verified against the signing key built into this app."
        last
        control={
          <TextField
            placeholder={DEFAULT_FEED}
            value={feed ?? settings.updateUrl}
            onChange={(e) => setFeed(e.target.value)}
            onBlur={commitFeed}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitFeed();
            }}
            sx={{ width: 320 }}
            slotProps={{ input: { sx: { fontFamily: monoFontFamily, fontSize: 12 } } }}
          />
        }
      />
      <UpdateStatus />
    </SectionCard>
  );
}

export function UpdateStatus() {
  const phase = useUpdate((s) => s.phase);
  switch (phase.kind) {
    case "idle":
    case "checking":
      return null;
    case "upToDate":
      return <Alert severity="success">You are on the latest version.</Alert>;
    case "available":
      return (
        <Alert severity="info">
          <Typography variant="body2">
            Termoso {phase.info.version} is available (you have {phase.info.currentVersion}).
          </Typography>
          {phase.info.notes && (
            <Typography
              variant="body2"
              sx={{ mt: 1, whiteSpace: "pre-wrap", maxHeight: 200, overflowY: "auto" }}
            >
              {phase.info.notes}
            </Typography>
          )}
        </Alert>
      );
    case "downloading": {
      const pct = phase.total ? Math.round((phase.downloaded / phase.total) * 100) : null;
      return (
        <Box sx={{ display: "flex", flexDirection: "column", gap: 0.75 }}>
          <Typography variant="body2" color="text.secondary">
            Downloading {phase.info.version}… {formatSize(phase.downloaded)}
            {phase.total ? ` / ${formatSize(phase.total)}` : ""}
          </Typography>
          <LinearProgress
            variant={pct === null ? "indeterminate" : "determinate"}
            value={pct ?? undefined}
          />
        </Box>
      );
    }
    case "installed":
      return (
        <Alert severity="success">
          Termoso {phase.version} is installed and will run after a restart.
        </Alert>
      );
    case "error":
      return <Alert severity="error">{phase.message}</Alert>;
  }
}
