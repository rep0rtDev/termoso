import { Box, CircularProgress, Stack, Tooltip, Typography } from "@mui/material";
import CheckCircleRoundedIcon from "@mui/icons-material/CheckCircleRounded";
import ErrorRoundedIcon from "@mui/icons-material/ErrorRounded";
import ScheduleRoundedIcon from "@mui/icons-material/ScheduleRounded";
import type { RunTarget, SnippetRun, TargetState } from "./run";

export const STATE_LABEL: Record<TargetState, string> = {
  pending: "Waiting",
  connecting: "Connecting",
  running: "Running",
  done: "Done",
  failed: "Failed",
};

/** Small status glyph for one execution target. */
export function TargetStateIcon({ state }: { state: TargetState }) {
  switch (state) {
    case "done":
      return <CheckCircleRoundedIcon fontSize="small" color="success" />;
    case "failed":
      return <ErrorRoundedIcon fontSize="small" color="error" />;
    case "pending":
      return <ScheduleRoundedIcon fontSize="small" sx={{ color: "text.disabled" }} />;
    default:
      return <CircularProgress size={14} thickness={5} sx={{ m: "3px" }} />;
  }
}

export function targetDetail(t: RunTarget): string {
  if (t.message) return t.message;
  if (t.state === "done" && t.exit !== null) return `Exit code ${t.exit}`;
  return STATE_LABEL[t.state];
}

/** Per-target rows of a run (label, address, state). */
export function RunTargets({ run, dense }: { run: SnippetRun; dense?: boolean }) {
  return (
    <Stack spacing={dense ? 0.25 : 0.5}>
      {run.targets.map((t) => (
        <Box
          key={t.key}
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1,
            minHeight: dense ? 28 : 36,
            px: dense ? 0 : 0.5,
          }}
        >
          <Tooltip title={STATE_LABEL[t.state]}>
            <Box sx={{ display: "flex", flexShrink: 0 }}>
              <TargetStateIcon state={t.state} />
            </Box>
          </Tooltip>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="body2" noWrap>
              {t.label}
            </Typography>
            {!dense && t.subtitle && (
              <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
                {t.subtitle}
              </Typography>
            )}
          </Box>
          <Typography
            variant="caption"
            noWrap
            sx={{
              flexShrink: 1,
              minWidth: 0,
              maxWidth: "45%",
              color: t.state === "failed" ? "error.main" : "text.secondary",
            }}
          >
            {targetDetail(t)}
          </Typography>
        </Box>
      ))}
    </Stack>
  );
}
