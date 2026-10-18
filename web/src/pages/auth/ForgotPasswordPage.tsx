import { useState, type SubmitEvent } from "react";
import { tr, trx } from "@/i18n";
import { Alert, Button, Link, Stack, TextField, Typography } from "@mui/material";
import { Link as RouterLink, useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { recoverAccount } from "@/auth/flows";
import { monoFontFamily } from "@/theme/theme";
import { AuthTitle, PasswordField, passwordProblem } from "./common";
import { useServerInfo } from "@/api/hooks";
import { Turnstile } from "@/components/Turnstile";

export function ForgotPasswordPage() {
  const navigate = useNavigate();
  const [email, setEmail] = useState("");
  const [phrase, setPhrase] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const info = useServerInfo();
  const siteKey = info.data?.turnstile_site_key ?? null;
  const [captcha, setCaptcha] = useState<string | null>(null);
  const [captchaRound, setCaptchaRound] = useState(0);

  const words = phrase.trim().split(/\s+/).filter(Boolean);
  const phraseOk = words.length === 24;
  const pwProblem = password.length > 0 ? passwordProblem(password) : null;
  const mismatch = confirm.length > 0 && confirm !== password;

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    if (!phraseOk || pwProblem || mismatch) return;
    setBusy(true);
    setError(null);
    try {
      const out = await recoverAccount(
        email,
        words.join(" ").toLowerCase(),
        password,
        captcha ?? undefined,
      );
      void navigate("/signup/recovery-key", {
        replace: true,
        state: {
          phrase: out.recoveryPhrase,
          email: out.session.user.email,
          title: tr("Your new recovery key"),
          next: "/account",
        },
      });
    } catch (err) {
      setError(errorMessage(err));
      setCaptcha(null);
      setCaptchaRound((n) => n + 1);
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit}>
      <AuthTitle
        title={tr("Reset your password")}
        subtitle={tr(
          "Termoso cannot email you a reset link: your data is encrypted with keys only you hold. Use your 24-word recovery key instead.",
        )}
      />
      <Stack spacing={2}>
        {error && <Alert severity="error">{error}</Alert>}
        <TextField
          label={tr("Email")}
          type="email"
          autoComplete="username"
          required
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          disabled={busy}
        />
        <TextField
          label={tr("Recovery key (24 words)")}
          multiline
          minRows={3}
          required
          value={phrase}
          onChange={(e) => setPhrase(e.target.value)}
          helperText={tr("{count}/24 words", { count: words.length })}
          error={phrase.length > 0 && words.length > 24}
          slotProps={{ input: { sx: { fontFamily: monoFontFamily, fontSize: "0.875rem" } } }}
          disabled={busy}
        />
        <PasswordField
          label={tr("New password")}
          autoComplete="new-password"
          required
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          error={pwProblem !== null}
          helperText={pwProblem ?? undefined}
          disabled={busy}
        />
        <PasswordField
          label={tr("Confirm new password")}
          autoComplete="new-password"
          required
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          error={mismatch}
          helperText={mismatch ? tr("Passwords do not match") : undefined}
          disabled={busy}
        />
        <Alert severity="info">
          {tr("All other sessions will be signed out and a new recovery key will be generated.")}
        </Alert>
        {siteKey && <Turnstile key={captchaRound} siteKey={siteKey} onToken={setCaptcha} />}
        <Button
          type="submit"
          variant="contained"
          size="large"
          disabled={busy || !phraseOk || (siteKey !== null && captcha === null)}
        >
          {busy ? tr("Resetting…") : tr("Reset password")}
        </Button>
        <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
          <Link component={RouterLink} to="/login">
            {tr("Back to sign in")}
          </Link>
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
          {trx("Lost the recovery key too?  {v1}  — the old encrypted data cannot be recovered.", {
            v1: (
              <Link component={RouterLink} to="/start-over">
                {tr("Start over with an empty vault")}
              </Link>
            ),
          })}
        </Typography>
      </Stack>
    </form>
  );
}
