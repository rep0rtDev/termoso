import { useState, type SubmitEvent } from "react";
import { Alert, Button, Link, Stack, TextField, Typography } from "@mui/material";
import { Link as RouterLink, useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { recoverAccount } from "@/auth/flows";
import { monoFontFamily } from "@/theme/theme";
import { AuthTitle, PasswordField, passwordProblem } from "./common";

export function ForgotPasswordPage() {
  const navigate = useNavigate();
  const [email, setEmail] = useState("");
  const [phrase, setPhrase] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

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
      const out = await recoverAccount(email, words.join(" ").toLowerCase(), password);
      void navigate("/signup/recovery-key", {
        replace: true,
        state: {
          phrase: out.recoveryPhrase,
          email: out.session.user.email,
          title: "Your new recovery key",
          next: "/account",
        },
      });
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit}>
      <AuthTitle
        title="Reset your password"
        subtitle="Termoso cannot email you a reset link: your data is encrypted with keys only you hold. Use your 24-word recovery key instead."
      />
      <Stack spacing={2}>
        {error && <Alert severity="error">{error}</Alert>}
        <TextField
          label="Email"
          type="email"
          autoComplete="username"
          required
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          disabled={busy}
        />
        <TextField
          label="Recovery key (24 words)"
          multiline
          minRows={3}
          required
          value={phrase}
          onChange={(e) => setPhrase(e.target.value)}
          helperText={`${words.length}/24 words`}
          error={phrase.length > 0 && words.length > 24}
          slotProps={{ input: { sx: { fontFamily: monoFontFamily, fontSize: "0.875rem" } } }}
          disabled={busy}
        />
        <PasswordField
          label="New password"
          autoComplete="new-password"
          required
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          error={pwProblem !== null}
          helperText={pwProblem ?? undefined}
          disabled={busy}
        />
        <PasswordField
          label="Confirm new password"
          autoComplete="new-password"
          required
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          error={mismatch}
          helperText={mismatch ? "Passwords do not match" : undefined}
          disabled={busy}
        />
        <Alert severity="info">
          All other sessions will be signed out and a new recovery key will be generated.
        </Alert>
        <Button type="submit" variant="contained" size="large" disabled={busy || !phraseOk}>
          {busy ? "Resetting…" : "Reset password"}
        </Button>
        <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
          <Link component={RouterLink} to="/login">
            Back to sign in
          </Link>
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
          Lost the recovery key too?{" "}
          <Link component={RouterLink} to="/start-over">
            Start over with an empty vault
          </Link>{" "}
          — the old encrypted data cannot be recovered.
        </Typography>
      </Stack>
    </form>
  );
}
