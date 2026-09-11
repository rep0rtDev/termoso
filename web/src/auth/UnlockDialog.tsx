import { useState, type SubmitEvent } from "react";
import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  TextField,
} from "@mui/material";
import { errorMessage } from "@/api/client";
import { login } from "./flows";
import { authStore } from "./store";
import { unlockPrompt, useUnlockPromptOpen } from "./unlock";

/**
 * Re-authenticates with the password to load the account private key into this tab.
 * The server never sees the password: OPAQUE yields the export key that unwraps it.
 */
export function UnlockDialog() {
  const open = useUnlockPromptOpen();
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    const session = authStore.get().session;
    if (!session) return;
    setBusy(true);
    setError(null);
    try {
      const outcome = await login(session.user.email, password);
      if (outcome.kind !== "done") {
        setError("This account needs additional verification; please sign out and sign in again.");
        return;
      }
      const pk = authStore.get().privateKey;
      if (!pk) throw new Error("Unlock failed");
      setPassword("");
      unlockPrompt.resolve(pk);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const cancel = () => {
    setPassword("");
    setError(null);
    unlockPrompt.cancel();
  };

  return (
    <Dialog open={open} onClose={busy ? undefined : cancel} maxWidth="xs" fullWidth>
      <form onSubmit={submit}>
        <DialogTitle>Unlock encryption keys</DialogTitle>
        <DialogContent sx={{ display: "grid", gap: 2 }}>
          <DialogContentText>
            This action needs your account key, which only lives in this browser tab. Enter your
            password to unlock it — it is never sent to the server.
          </DialogContentText>
          {error && <Alert severity="error">{error}</Alert>}
          <TextField
            autoFocus
            label="Password"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            disabled={busy}
          />
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={cancel} disabled={busy} color="inherit">
            Cancel
          </Button>
          <Button type="submit" variant="contained" disabled={busy || password.length === 0}>
            Unlock
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  );
}
