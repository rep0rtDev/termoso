import { useState, type SubmitEvent } from "react";
import { Alert, AlertTitle, Button, Link, Stack, TextField, Typography } from "@mui/material";
import { Link as RouterLink } from "react-router";
import { ApiError, errorMessage } from "@/api/client";
import { authApi } from "@/api/endpoints";
import { formatDateTime, formatRelative } from "@/components/format";
import { AuthTitle } from "./common";

/** The one thing every Start-over screen must say, in red. */
export function IrreversibleWarning() {
  return (
    <Alert severity="error" variant="outlined">
      <AlertTitle>This permanently destroys your encrypted data</AlertTitle>
      Your hosts, passwords, keys, snippets, settings and other vault contents are encrypted with
      keys only you hold. Without the password or the recovery key nobody — including Termoso — can
      decrypt them: the server never has a decryption key. Starting over keeps your email address
      and team memberships but gives you a <strong>new, empty vault</strong>. The old data is
      deleted and cannot be brought back.
    </Alert>
  );
}

type Stage =
  | { kind: "email" }
  | { kind: "code"; requestToken: string; emailHint: string; needMfa: boolean }
  | { kind: "scheduled"; scheduledFor: string; emailHint: string };

/**
 * Lost both the password and the recovery key. The account can be reclaimed
 * with a fresh, empty vault — the old encrypted data is unrecoverable by design.
 */
export function StartOverPage() {
  const [stage, setStage] = useState<Stage>({ kind: "email" });
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [mfaCode, setMfaCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      if (stage.kind === "email") {
        const r = await authApi.startOverRequest(email.trim());
        setStage({
          kind: "code",
          requestToken: r.request_token,
          emailHint: r.email_hint,
          needMfa: false,
        });
      } else if (stage.kind === "code") {
        const mfa = mfaCode.trim();
        try {
          const r = await authApi.startOverConfirm(
            stage.requestToken,
            code.trim(),
            mfa === "" ? undefined : mfa,
          );
          setStage({ kind: "scheduled", scheduledFor: r.scheduled_for, emailHint: r.email_hint });
        } catch (err) {
          if (err instanceof ApiError && err.code === "mfa_required" && !stage.needMfa) {
            setStage({ ...stage, needMfa: true });
            return;
          }
          throw err;
        }
      }
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  if (stage.kind === "scheduled") {
    return (
      <Stack spacing={2}>
        <AuthTitle
          title="Reset scheduled"
          subtitle={`The reset for ${stage.emailHint} can be completed ${formatRelative(stage.scheduledFor)} (${formatDateTime(stage.scheduledFor)}).`}
        />
        <IrreversibleWarning />
        <Alert severity="info">
          We emailed you two links: one to <strong>finish</strong> the reset once the waiting period
          is over, and one to <strong>cancel</strong> it. Every signed-in device of the account was
          notified too. If you still have a signed-in Termoso app, you can cancel from there as
          well.
        </Alert>
        <Typography variant="body2" color="text.secondary">
          The 24-hour delay exists so that the real owner can stop a reset they did not start.
          Nothing is deleted until you open the finish link and confirm.
        </Typography>
        <Button component={RouterLink} to="/login" variant="outlined">
          Back to sign in
        </Button>
      </Stack>
    );
  }

  return (
    <form onSubmit={submit}>
      <AuthTitle
        title="Start over"
        subtitle="For when both your password and your 24-word recovery key are gone."
      />
      <Stack spacing={2}>
        <IrreversibleWarning />
        {error && <Alert severity="error">{error}</Alert>}
        {stage.kind === "email" && (
          <>
            <TextField
              label="Account email"
              type="email"
              autoComplete="username"
              required
              autoFocus
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              disabled={busy}
            />
            <Typography variant="body2" color="text.secondary">
              We will send a confirmation code. After you confirm, the reset is{" "}
              <strong>scheduled 24 hours later</strong> and can be cancelled from the email or from
              any signed-in device.
            </Typography>
            <Button type="submit" variant="contained" size="large" color="error" disabled={busy}>
              {busy ? "Sending…" : "Send confirmation code"}
            </Button>
          </>
        )}
        {stage.kind === "code" && (
          <>
            <Typography variant="body2">
              Enter the code we sent to <strong>{stage.emailHint}</strong>.
            </Typography>
            <TextField
              label="6-digit code"
              required
              autoFocus
              autoComplete="one-time-code"
              inputMode="numeric"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              disabled={busy}
            />
            {stage.needMfa && (
              <TextField
                label="Two-factor code (authenticator or backup code)"
                required
                autoComplete="one-time-code"
                value={mfaCode}
                onChange={(e) => setMfaCode(e.target.value)}
                helperText="Two-factor authentication is enabled on this account."
                disabled={busy}
              />
            )}
            <Button
              type="submit"
              variant="contained"
              size="large"
              color="error"
              disabled={busy || code.trim().length === 0}
            >
              {busy ? "Confirming…" : "I understand, schedule the reset"}
            </Button>
          </>
        )}
        <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
          Still have your recovery key?{" "}
          <Link component={RouterLink} to="/forgot-password">
            Reset the password instead
          </Link>
        </Typography>
      </Stack>
    </form>
  );
}
