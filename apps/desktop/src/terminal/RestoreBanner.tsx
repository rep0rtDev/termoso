import { Alert, Button } from "@mui/material";
import RestoreRoundedIcon from "@mui/icons-material/RestoreRounded";
import { useSnackbar } from "@/components/Snackbar";
import { dismissPrevious, restorePrevious, snapshotConnections, useWorkspaces } from "./workspaces";
import { tr, trn } from "@/i18n";

/** One-line offer on the home screen to reopen the previous session's tabs. */
export function RestoreBanner() {
  const previous = useWorkspaces((s) => s.previous);
  const snackbar = useSnackbar();
  if (!previous) return null;
  const n = snapshotConnections(previous);
  const tabs = previous.tabs.length;
  return (
    <Alert
      severity="info"
      variant="outlined"
      icon={<RestoreRoundedIcon fontSize="inherit" />}
      onClose={dismissPrevious}
      sx={{ borderRadius: 0, borderLeft: 0, borderRight: 0, borderTop: 0, py: 0 }}
      action={
        <>
          <Button
            size="small"
            onClick={() => {
              restorePrevious();
              snackbar.notify(`Restoring ${n} connection${n === 1 ? "" : "s"}`);
            }}
          >
            {tr("Restore")}
          </Button>
          <Button size="small" onClick={dismissPrevious}>
            {tr("Dismiss")}
          </Button>
        </>
      }
    >
      {tr("Your previous session had {connections} in {tabs}.", {
        connections: trn(n, "{count} connection", "{count} connections"),
        tabs: trn(tabs, "{count} tab", "{count} tabs"),
      })}
    </Alert>
  );
}
