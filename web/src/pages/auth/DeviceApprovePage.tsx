import { useState, type SubmitEvent } from "react";
import { tr } from "@/i18n";
import { Alert, Button, Stack, TextField } from "@mui/material";
import { Navigate, useLocation, useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { authApi } from "@/api/endpoints";
import { approveDevice } from "@/auth/flows";
import { AuthTitle } from "./common";

interface ApproveState {
  approvalToken: string;
  emailHint: string;
  next: string;
}

export function DeviceApprovePage() {
  const location = useLocation();
  const navigate = useNavigate();
  const state = location.state as ApproveState | null;
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [resent, setResent] = useState(false);
  const [busy, setBusy] = useState(false);

  if (!state) return <Navigate to="/login" replace />;

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const outcome = await approveDevice(state.approvalToken, code.trim());
      if (outcome.kind === "done") void navigate(state.next, { replace: true });
      else if (outcome.kind === "mfa")
        void navigate("/login/mfa", {
          replace: true,
          state: { mfaToken: outcome.mfaToken, methods: outcome.methods, next: state.next },
        });
      else setError(tr("Unexpected response from server"));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const resend = async () => {
    setBusy(true);
    setError(null);
    try {
      await authApi.deviceApproveResend(state.approvalToken);
      setResent(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit}>
      <AuthTitle
        title={tr("Approve this browser")}
        subtitle={`This is a new device. We sent a confirmation code to ${state.emailHint}.`}
      />
      <Stack spacing={2}>
        {error && <Alert severity="error">{error}</Alert>}
        {resent && <Alert severity="success">{tr("A new code was sent.")}</Alert>}
        <TextField
          label={tr("Confirmation code")}
          autoFocus
          required
          autoComplete="one-time-code"
          inputMode="numeric"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          disabled={busy}
        />
        <Button
          type="submit"
          variant="contained"
          size="large"
          disabled={busy || code.trim().length === 0}
        >
          {tr("Approve")}
        </Button>
        <Button color="inherit" onClick={() => void resend()} disabled={busy}>
          {tr("Resend code")}
        </Button>
      </Stack>
    </form>
  );
}
