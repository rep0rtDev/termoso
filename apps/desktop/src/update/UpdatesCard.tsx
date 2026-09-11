import { useState } from "react";
import {
  Alert,
  Box,
  Button,
  LinearProgress,
  MenuItem,
  Paper,
  TextField,
  Typography,
} from "@mui/material";
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
    <Paper variant="outlined" sx={{ p: 2.5, display: "flex", flexDirection: "column", gap: 1.5 }}>
      <Typography variant="h6">Updates</Typography>
      <Box sx={{ display: "flex", gap: 2, flexWrap: "wrap" }}>
        <TextField
          select
          label="Check for updates"
          value={settings.updateCheck}
          onChange={(e) => onChange({ updateCheck: e.target.value as UpdateCheck })}
          sx={{ width: 240 }}
        >
          <MenuItem value="manual">Only when I ask</MenuItem>
          <MenuItem value="startup">Once at startup</MenuItem>
        </TextField>
        <TextField
          label="Release feed (https)"
          placeholder={DEFAULT_FEED}
          value={feed ?? settings.updateUrl}
          onChange={(e) => setFeed(e.target.value)}
          onBlur={commitFeed}
          onKeyDown={(e) => {
            if (e.key === "Enter") commitFeed();
          }}
          sx={{ flex: 1, minWidth: 320 }}
          slotProps={{ input: { sx: { fontFamily: "monospace", fontSize: 13 } } }}
        />
      </Box>
      <Typography variant="body2" color="text.secondary">
        Manifests and packages are verified against the release signing key built into this app.
        Point the feed at your own server to keep updates entirely self-hosted; nothing is fetched
        unless you press the button or enable the startup check.
      </Typography>

      <Box sx={{ display: "flex", gap: 1.5, alignItems: "center", flexWrap: "wrap" }}>
        <Button
          variant="outlined"
          size="small"
          disabled={busy}
          onClick={() => void checkForUpdates()}
        >
          {phase.kind === "checking" ? "Checking…" : "Check now"}
        </Button>
        {phase.kind === "available" && (
          <Button variant="contained" size="small" onClick={() => void installUpdate()}>
            Download and install {phase.info.version}
          </Button>
        )}
        {phase.kind === "installed" && (
          <Button variant="contained" size="small" onClick={() => void restartToUpdate()}>
            Restart to finish
          </Button>
        )}
      </Box>

      <UpdateStatus />
    </Paper>
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
