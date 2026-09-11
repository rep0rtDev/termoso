import { useState, type SubmitEvent } from "react";
import { Alert, Button, Link, Stack, TextField, Typography } from "@mui/material";
import { Link as RouterLink, useLocation, useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { useServerInfo } from "@/api/hooks";
import { register } from "@/auth/flows";
import { AuthTitle, PasswordField, passwordProblem, useNextPath } from "./common";

export interface SignupState {
  inviteToken?: string;
  sso?: { ssoSession: string; email: string; displayName?: string };
  lockedEmail?: string;
}

export function SignupPage() {
  const navigate = useNavigate();
  const next = useNextPath();
  const location = useLocation();
  const state = (location.state as SignupState | null) ?? {};
  const info = useServerInfo();

  const [email, setEmail] = useState(state.sso?.email ?? state.lockedEmail ?? "");
  const [displayName, setDisplayName] = useState(state.sso?.displayName ?? "");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const pwProblem = password.length > 0 ? passwordProblem(password) : null;
  const mismatch = confirm.length > 0 && confirm !== password;
  const lockEmail = state.sso !== undefined || state.lockedEmail !== undefined;

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    if (pwProblem || mismatch) return;
    setBusy(true);
    setError(null);
    try {
      const out = await register({
        email,
        password,
        displayName,
        inviteToken: state.inviteToken,
        ssoSession: state.sso?.ssoSession,
      });
      void navigate("/signup/recovery-key", {
        replace: true,
        state: { phrase: out.recoveryPhrase, email: out.session.user.email, next },
      });
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  if (info.data && !info.data.registration_open && !state.inviteToken && !state.sso) {
    return (
      <>
        <AuthTitle
          title="Registration is closed"
          subtitle="This server accepts new accounts by invitation only."
        />
        <Button component={RouterLink} to="/login" variant="contained">
          Back to sign in
        </Button>
      </>
    );
  }

  return (
    <form onSubmit={submit}>
      <AuthTitle
        title="Create your account"
        subtitle="Everything you store is end-to-end encrypted. Your password unlocks your keys locally; the server only ever sees ciphertext."
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
          disabled={busy || lockEmail}
        />
        <TextField
          label="Name (optional)"
          autoComplete="name"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
          disabled={busy}
        />
        <PasswordField
          label="Password"
          autoComplete="new-password"
          required
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          error={pwProblem !== null}
          helperText={pwProblem ?? "At least 10 characters. Use a passphrase you can remember."}
          disabled={busy}
        />
        <PasswordField
          label="Confirm password"
          autoComplete="new-password"
          required
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          error={mismatch}
          helperText={mismatch ? "Passwords do not match" : undefined}
          disabled={busy}
        />
        <Alert severity="info" variant="outlined">
          After sign-up you will get a 24-word recovery key. It is the only way to reset a forgotten
          password without losing your data — nobody, including the server operator, can recover it
          for you.
        </Alert>
        <Button type="submit" variant="contained" size="large" disabled={busy}>
          {busy ? "Creating account…" : "Create account"}
        </Button>
        <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
          Already have an account?{" "}
          <Link component={RouterLink} to="/login">
            Sign in
          </Link>
        </Typography>
      </Stack>
    </form>
  );
}
