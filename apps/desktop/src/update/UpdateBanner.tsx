import { Alert, Button } from "@mui/material";
import { dismissBanner, useUpdate } from "./store";

/** One-line notice after an opt-in startup check found a newer release. */
export function UpdateBanner({ onOpenSettings }: { onOpenSettings: () => void }) {
  const banner = useUpdate((s) => s.banner);
  const phase = useUpdate((s) => s.phase);
  if (!banner || phase.kind !== "available") return null;
  return (
    <Alert
      severity="info"
      variant="outlined"
      onClose={dismissBanner}
      sx={{ borderRadius: 0, borderLeft: 0, borderRight: 0, borderTop: 0, py: 0 }}
      action={
        <>
          <Button size="small" onClick={onOpenSettings}>
            View
          </Button>
          <Button size="small" onClick={dismissBanner}>
            Later
          </Button>
        </>
      }
    >
      Termoso {phase.info.version} is available.
    </Alert>
  );
}
