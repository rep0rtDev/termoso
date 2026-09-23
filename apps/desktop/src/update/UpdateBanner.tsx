import { Alert, Button } from "@mui/material";
import { dismissBanner, useUpdate } from "./store";
import { tr } from "@/i18n";

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
            {tr("View")}
          </Button>
          <Button size="small" onClick={dismissBanner}>
            {tr("Later")}
          </Button>
        </>
      }
    >
      {tr("Termoso {version} is available.", { version: phase.info.version })}
    </Alert>
  );
}
