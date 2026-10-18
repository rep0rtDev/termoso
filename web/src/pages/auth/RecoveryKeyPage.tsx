import { useState } from "react";
import { tr } from "@/i18n";
import { Alert, Button, Checkbox, FormControlLabel, Stack } from "@mui/material";
import { Navigate, useLocation, useNavigate } from "react-router";
import { RecoveryPhraseGrid } from "@/components/RecoveryPhrase";
import { AuthTitle } from "./common";

interface RecoveryKeyState {
  phrase: string;
  email?: string;
  next?: string;
  title?: string;
}

export function RecoveryKeyPage() {
  const location = useLocation();
  const navigate = useNavigate();
  const state = location.state as RecoveryKeyState | null;
  const [saved, setSaved] = useState(false);

  if (!state) return <Navigate to="/account" replace />;

  return (
    <>
      <AuthTitle
        title={state.title ?? tr("Save your recovery key")}
        subtitle={tr(
          "Write these 24 words down or store them in a password manager. They are shown only once.",
        )}
      />
      <Stack spacing={2.5}>
        <RecoveryPhraseGrid phrase={state.phrase} email={state.email} />
        <Alert severity="warning">
          {tr(
            "If you forget your password and lose this key, your encrypted data cannot be recovered.",
          )}
        </Alert>
        <FormControlLabel
          control={<Checkbox checked={saved} onChange={(e) => setSaved(e.target.checked)} />}
          label={tr("I have stored my recovery key in a safe place")}
        />
        <Button
          variant="contained"
          size="large"
          disabled={!saved}
          onClick={() => navigate(state.next ?? "/account", { replace: true })}
        >
          {tr("Continue")}
        </Button>
      </Stack>
    </>
  );
}
