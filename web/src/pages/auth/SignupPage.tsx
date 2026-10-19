import { useState, type SubmitEvent } from "react";
import { tr, trx } from "@/i18n";
import { Alert, Button, Link, Stack, TextField, Typography } from "@mui/material";
import { Link as RouterLink, useLocation, useNavigate } from "react-router";
import { ApiError, errorMessage } from "@/api/client";
import { useServerInfo } from "@/api/hooks";
import { register } from "@/auth/flows";
import { startSso } from "@/auth/sso";
import { AuthTitle, PasswordField, passwordProblem, useNextPath } from "./common";
import { Turnstile } from "@/components/Turnstile";

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
  // The domain is claimed by a team for single sign-on: offer that instead.
  const [ssoRequired, setSsoRequired] = useState<{ provider: string; team: string | null } | null>(
    null,
  );
  const [busy, setBusy] = useState(false);
  const siteKey = info.data?.turnstile_site_key ?? null;
  const [captcha, setCaptcha] = useState<string | null>(null);
  const [captchaRound, setCaptchaRound] = useState(0);

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
        turnstileToken: captcha ?? undefined,
      });
      void navigate("/signup/recovery-key", {
        replace: true,
        state: { phrase: out.recoveryPhrase, email: out.session.user.email, next },
      });
    } catch (err) {
      if (err instanceof ApiError && err.code === "sso_required") {
        const d = err.details as { provider?: string; team_name?: string } | null;
        if (d?.provider) {
          setSsoRequired({ provider: d.provider, team: d.team_name ?? null });
          return;
        }
      }
      setError(errorMessage(err));
      // Tokens are single-use: get a fresh one for the next attempt.
      setCaptcha(null);
      setCaptchaRound((n) => n + 1);
    } finally {
      setBusy(false);
    }
  };

  if (info.data && !info.data.registration_open && !state.inviteToken && !state.sso) {
    return (
      <>
        <AuthTitle
          title={tr("Registration is closed")}
          subtitle={tr("This server accepts new accounts by invitation only.")}
        />
        <Button component={RouterLink} to="/login" variant="contained">
          {tr("Back to sign in")}
        </Button>
      </>
    );
  }

  return (
    <form onSubmit={submit}>
      <AuthTitle
        title={tr("Create your account")}
        subtitle={tr(
          "Everything you store is end-to-end encrypted. Your password unlocks your keys locally; the server only ever sees ciphertext.",
        )}
      />
      <Stack spacing={2}>
        {error && <Alert severity="error">{error}</Alert>}
        {ssoRequired && (
          <Alert
            severity="info"
            action={
              <Button
                size="small"
                variant="contained"
                disabled={busy}
                onClick={() => {
                  setError(null);
                  startSso(ssoRequired.provider, next).catch((e: unknown) =>
                    setError(errorMessage(e)),
                  );
                }}
              >
                {tr("Continue with SSO")}
              </Button>
            }
          >
            {ssoRequired.team
              ? tr(
                  "{v1} manages this e-mail domain through single sign-on. Password accounts cannot be created for it — sign in with your organisation's provider and your account is created automatically.",
                  { v1: ssoRequired.team },
                )
              : tr(
                  "This e-mail domain is managed through single sign-on. Sign in with your organisation's provider and your account is created automatically.",
                )}
          </Alert>
        )}
        <TextField
          label={tr("Email")}
          type="email"
          autoComplete="username"
          required
          value={email}
          onChange={(e) => {
            setEmail(e.target.value);
            setSsoRequired(null);
          }}
          disabled={busy || lockEmail}
        />
        <TextField
          label={tr("Name (optional)")}
          autoComplete="name"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
          disabled={busy}
        />
        <PasswordField
          label={tr("Password")}
          autoComplete="new-password"
          required
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          error={pwProblem !== null}
          helperText={pwProblem ?? tr("At least 10 characters. Use a passphrase you can remember.")}
          disabled={busy}
        />
        <PasswordField
          label={tr("Confirm password")}
          autoComplete="new-password"
          required
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          error={mismatch}
          helperText={mismatch ? tr("Passwords do not match") : undefined}
          disabled={busy}
        />
        <Alert severity="info">
          {tr(
            "After sign-up you will get a 24-word recovery key. It is the only way to reset a forgotten password without losing your data — nobody, including the server operator, can recover it for you.",
          )}
        </Alert>
        {siteKey && <Turnstile key={captchaRound} siteKey={siteKey} onToken={setCaptcha} />}
        <Button
          type="submit"
          variant="contained"
          size="large"
          disabled={busy || (siteKey !== null && captcha === null)}
        >
          {busy ? tr("Creating account…") : tr("Create account")}
        </Button>
        <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
          {trx("Already have an account?  {v1}", {
            v1: (
              <Link component={RouterLink} to="/login">
                {tr("Sign in")}
              </Link>
            ),
          })}
        </Typography>
      </Stack>
    </form>
  );
}
