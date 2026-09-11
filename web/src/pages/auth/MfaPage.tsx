import { useState, type SubmitEvent } from "react";
import { Alert, Button, Stack, Tab, Tabs, TextField, Typography } from "@mui/material";
import FingerprintRoundedIcon from "@mui/icons-material/FingerprintRounded";
import { Navigate, useLocation, useNavigate } from "react-router";
import {
  startAuthentication,
  type PublicKeyCredentialRequestOptionsJSON,
} from "@simplewebauthn/browser";
import { errorMessage } from "@/api/client";
import { authApi } from "@/api/endpoints";
import type { MfaMethod } from "@/api/types";
import { verifyMfa, type LoginOutcome } from "@/auth/flows";
import { AuthTitle } from "./common";

interface MfaState {
  mfaToken: string;
  methods: MfaMethod[];
  next: string;
}

const labels: Record<MfaMethod, string> = {
  totp: "Authenticator app",
  webauthn: "Security key",
  email: "Email code",
  backup_code: "Backup code",
};

function isWebauthnChallenge(
  v: unknown,
): v is { publicKey: PublicKeyCredentialRequestOptionsJSON } {
  return typeof v === "object" && v !== null && "publicKey" in v;
}

export function MfaPage() {
  const location = useLocation();
  const navigate = useNavigate();
  const state = location.state as MfaState | null;
  const [method, setMethod] = useState<MfaMethod>(state?.methods[0] ?? "totp");
  const [code, setCode] = useState("");
  const [emailSent, setEmailSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (!state) return <Navigate to="/login" replace />;

  const finish = (outcome: LoginOutcome) => {
    if (outcome.kind === "done") void navigate(state.next, { replace: true });
    else if (outcome.kind === "approval")
      void navigate("/login/approve", {
        replace: true,
        state: {
          approvalToken: outcome.approvalToken,
          emailHint: outcome.emailHint,
          next: state.next,
        },
      });
    else setError("Unexpected response from server");
  };

  const run = async (fn: () => Promise<LoginOutcome>) => {
    setBusy(true);
    setError(null);
    try {
      finish(await fn());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const submitCode = (e: SubmitEvent) => {
    e.preventDefault();
    const trimmed = code.trim();
    if (method === "totp")
      void run(() => verifyMfa(state.mfaToken, { method: "totp", code: trimmed }));
    else if (method === "email")
      void run(() => verifyMfa(state.mfaToken, { method: "email", code: trimmed }));
    else if (method === "backup_code")
      void run(() => verifyMfa(state.mfaToken, { method: "backup_code", code: trimmed }));
  };

  const sendEmail = async () => {
    setBusy(true);
    setError(null);
    try {
      await authApi.mfaEmailSend(state.mfaToken);
      setEmailSent(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const authenticateWithKey = () =>
    run(async () => {
      const challenge = await authApi.mfaWebauthnChallenge(state.mfaToken);
      if (!isWebauthnChallenge(challenge)) throw new Error("Malformed WebAuthn challenge");
      const credential = await startAuthentication({ optionsJSON: challenge.publicKey });
      return verifyMfa(state.mfaToken, { method: "webauthn", credential });
    });

  return (
    <>
      <AuthTitle
        title="Two-factor verification"
        subtitle="Confirm it's you to finish signing in."
      />
      {state.methods.length > 1 && (
        <Tabs
          value={method}
          onChange={(_, v: MfaMethod) => {
            setMethod(v);
            setCode("");
            setError(null);
          }}
          variant="scrollable"
          allowScrollButtonsMobile
          sx={{ mb: 2 }}
        >
          {state.methods.map((m) => (
            <Tab key={m} value={m} label={labels[m]} />
          ))}
        </Tabs>
      )}
      <Stack spacing={2}>
        {error && <Alert severity="error">{error}</Alert>}
        {method === "webauthn" ? (
          <>
            <Typography variant="body2" color="text.secondary">
              Use the passkey or hardware security key registered on your account.
            </Typography>
            <Button
              variant="contained"
              size="large"
              startIcon={<FingerprintRoundedIcon />}
              onClick={() => void authenticateWithKey()}
              disabled={busy}
            >
              Use security key
            </Button>
          </>
        ) : (
          <form onSubmit={submitCode}>
            <Stack spacing={2}>
              {method === "email" && (
                <Button variant="outlined" onClick={() => void sendEmail()} disabled={busy}>
                  {emailSent ? "Resend code" : "Send code to my email"}
                </Button>
              )}
              <TextField
                label={method === "backup_code" ? "Backup code" : "6-digit code"}
                autoFocus
                required
                autoComplete="one-time-code"
                inputMode={method === "backup_code" ? "text" : "numeric"}
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
                Verify
              </Button>
            </Stack>
          </form>
        )}
        <Button
          color="inherit"
          onClick={() => navigate("/login", { replace: true })}
          disabled={busy}
        >
          Back to sign in
        </Button>
      </Stack>
    </>
  );
}
