import { useState, type SubmitEvent } from "react";
import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Stack,
  Tab,
  Tabs,
  TextField,
  Typography,
} from "@mui/material";
import FingerprintRoundedIcon from "@mui/icons-material/FingerprintRounded";
import {
  startAuthentication,
  type PublicKeyCredentialRequestOptionsJSON,
} from "@simplewebauthn/browser";
import { errorMessage } from "@/api/client";
import { authApi } from "@/api/endpoints";
import type { MfaMethod } from "@/api/types";
import {
  clearPendingReauth,
  reauthenticate,
  reauthenticateMfa,
  reauthenticateWithEmailCode,
  type ReauthOutcome,
} from "./flows";
import { useAuthState } from "./store";
import { unlockPrompt, useUnlockPromptOpen } from "./unlock";

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

type Stage =
  | { kind: "password" }
  | { kind: "email"; emailHint: string }
  | { kind: "mfa"; mfaToken: string; methods: MfaMethod[] };

/**
 * "Confirm it's you": re-authenticates the current session (step-up) before a
 * sensitive change, and — when the password is used — also loads the account
 * private key into this tab. The password never leaves the browser (OPAQUE).
 */
export function UnlockDialog() {
  const open = useUnlockPromptOpen();
  return (
    <Dialog open={open} maxWidth="xs" fullWidth>
      {open && <Body />}
    </Dialog>
  );
}

function Body() {
  const { session } = useAuthState();
  const needsKey = unlockPrompt.needsKey();
  const [stage, setStage] = useState<Stage>({ kind: "password" });
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [method, setMethod] = useState<MfaMethod>("totp");
  const [emailSent, setEmailSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const apply = (outcome: ReauthOutcome) => {
    switch (outcome.kind) {
      case "done":
        unlockPrompt.resolve();
        break;
      case "email":
        setCode("");
        setStage(outcome);
        break;
      case "mfa":
        setCode("");
        setMethod(outcome.methods[0] ?? "totp");
        setStage(outcome);
        break;
    }
  };

  const run = async (fn: () => Promise<ReauthOutcome>) => {
    setBusy(true);
    setError(null);
    try {
      apply(await fn());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const submit = (e: SubmitEvent) => {
    e.preventDefault();
    const trimmed = code.trim();
    switch (stage.kind) {
      case "password":
        void run(() => reauthenticate(password));
        break;
      case "email":
        void run(() => reauthenticateWithEmailCode(trimmed));
        break;
      case "mfa":
        if (method === "webauthn") break;
        void run(() => reauthenticateMfa(stage.mfaToken, { method, code: trimmed }));
        break;
    }
  };

  const sendEmail = async (mfaToken: string) => {
    setBusy(true);
    setError(null);
    try {
      await authApi.mfaEmailSend(mfaToken);
      setEmailSent(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const authenticateWithKey = (mfaToken: string) =>
    run(async () => {
      const challenge = await authApi.mfaWebauthnChallenge(mfaToken);
      if (!isWebauthnChallenge(challenge)) throw new Error("Malformed WebAuthn challenge");
      const credential = await startAuthentication({ optionsJSON: challenge.publicKey });
      return reauthenticateMfa(mfaToken, { method: "webauthn", credential });
    });

  const cancel = () => {
    clearPendingReauth();
    unlockPrompt.cancel();
  };

  const canSubmit =
    stage.kind === "password"
      ? password.length > 0 || !needsKey
      : stage.kind === "mfa" && method === "webauthn"
        ? false
        : code.trim().length > 0;

  return (
    <form onSubmit={submit}>
      <DialogTitle>{needsKey ? "Unlock encryption keys" : "Confirm it's you"}</DialogTitle>
      <DialogContent sx={{ display: "grid", gap: 2 }}>
        {stage.kind === "password" && (
          <DialogContentText>
            {needsKey
              ? "This action needs your account key, which only lives in this browser tab. Enter your password to unlock it — it is never sent to the server."
              : `This is a sensitive change to ${session?.user.email ?? "your account"}. Enter your password to continue — it is never sent to the server.`}
            {!needsKey &&
              " Accounts without a password (single sign-on) get a code by email instead: leave the field empty."}
          </DialogContentText>
        )}
        {stage.kind === "email" && (
          <DialogContentText>
            Enter the code we sent to {stage.emailHint || "your email"}.
          </DialogContentText>
        )}
        {stage.kind === "mfa" && stage.methods.length > 1 && (
          <Tabs
            value={method}
            onChange={(_, v: MfaMethod) => {
              setMethod(v);
              setCode("");
              setError(null);
            }}
            variant="scrollable"
            allowScrollButtonsMobile
          >
            {stage.methods.map((m) => (
              <Tab key={m} value={m} label={labels[m]} />
            ))}
          </Tabs>
        )}
        {error && <Alert severity="error">{error}</Alert>}
        {stage.kind === "password" && (
          <TextField
            autoFocus
            label="Password"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            disabled={busy}
          />
        )}
        {stage.kind === "email" && (
          <TextField
            autoFocus
            label="6-digit code"
            required
            autoComplete="one-time-code"
            inputMode="numeric"
            value={code}
            onChange={(e) => setCode(e.target.value)}
            disabled={busy}
          />
        )}
        {stage.kind === "mfa" &&
          (method === "webauthn" ? (
            <Stack spacing={1.5}>
              <Typography variant="body2" color="text.secondary">
                Use the passkey or hardware security key registered on your account.
              </Typography>
              <Button
                variant="contained"
                startIcon={<FingerprintRoundedIcon />}
                onClick={() => void authenticateWithKey(stage.mfaToken)}
                disabled={busy}
              >
                Use security key
              </Button>
            </Stack>
          ) : (
            <Stack spacing={2}>
              {method === "email" && (
                <Button
                  variant="outlined"
                  onClick={() => void sendEmail(stage.mfaToken)}
                  disabled={busy}
                >
                  {emailSent ? "Resend code" : "Send code to my email"}
                </Button>
              )}
              <TextField
                autoFocus
                label={method === "backup_code" ? "Backup code" : "6-digit code"}
                required
                autoComplete="one-time-code"
                inputMode={method === "backup_code" ? "text" : "numeric"}
                value={code}
                onChange={(e) => setCode(e.target.value)}
                disabled={busy}
              />
            </Stack>
          ))}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={cancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        {!(stage.kind === "mfa" && method === "webauthn") && (
          <Button type="submit" variant="contained" disabled={busy || !canSubmit}>
            {stage.kind === "password" ? (needsKey ? "Unlock" : "Continue") : "Verify"}
          </Button>
        )}
      </DialogActions>
    </form>
  );
}
