import { useState, type SubmitEvent } from "react";
import { Alert, Box, Button, Divider, Link, Stack, TextField, Typography } from "@mui/material";
import { Link as RouterLink, useLocation, useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { useServerInfo } from "@/api/hooks";
import { login } from "@/auth/flows";
import { startSso } from "@/auth/sso";
import { AuthTitle, PasswordField, useNextPath } from "./common";

export interface SsoLoginState {
  ssoSession: string;
  email: string;
}

export function LoginPage() {
  const navigate = useNavigate();
  const next = useNextPath();
  const location = useLocation();
  const sso = (location.state as { sso?: SsoLoginState } | null)?.sso;
  const info = useServerInfo();

  const [email, setEmail] = useState(sso?.email ?? "");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const outcome = await login(email, password, sso?.ssoSession);
      switch (outcome.kind) {
        case "done":
          void navigate(next, { replace: true });
          break;
        case "mfa":
          void navigate("/login/mfa", {
            state: { mfaToken: outcome.mfaToken, methods: outcome.methods, next },
          });
          break;
        case "approval":
          void navigate("/login/approve", {
            state: { approvalToken: outcome.approvalToken, emailHint: outcome.emailHint, next },
          });
          break;
      }
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const providers = info.data?.sso_providers ?? [];

  return (
    <form onSubmit={submit}>
      <AuthTitle
        title={sso ? "Unlock your account" : "Sign in"}
        subtitle={
          sso
            ? `Signed in with SSO as ${sso.email}. Enter your Termoso password to unlock your encrypted data.`
            : "Your password never leaves this device."
        }
      />
      <Stack spacing={2}>
        {error && <Alert severity="error">{error}</Alert>}
        <TextField
          label="Email"
          type="email"
          autoComplete="username"
          autoFocus={!sso}
          required
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          disabled={busy || sso !== undefined}
        />
        <PasswordField
          label="Password"
          autoComplete="current-password"
          autoFocus={sso !== undefined}
          required
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          disabled={busy}
        />
        <Box sx={{ display: "flex", justifyContent: "flex-end" }}>
          <Link component={RouterLink} to="/forgot-password" variant="body2">
            Forgot password?
          </Link>
        </Box>
        <Button type="submit" variant="contained" size="large" disabled={busy}>
          {busy ? "Signing in…" : "Sign in"}
        </Button>

        {!sso && providers.length > 0 && (
          <>
            <Divider>
              <Typography variant="caption" color="text.secondary">
                or continue with
              </Typography>
            </Divider>
            <Stack spacing={1}>
              {providers.map((p) => (
                <Button
                  key={p.id}
                  variant="outlined"
                  color="inherit"
                  disabled={busy}
                  onClick={() => {
                    setError(null);
                    startSso(p.id, next).catch((err: unknown) => setError(errorMessage(err)));
                  }}
                >
                  {p.name}
                </Button>
              ))}
            </Stack>
          </>
        )}

        {info.data?.registration_open !== false && !sso && (
          <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
            New to Termoso?{" "}
            <Link
              component={RouterLink}
              to={next === "/account" ? "/signup" : `/signup?next=${encodeURIComponent(next)}`}
            >
              Create an account
            </Link>
          </Typography>
        )}
      </Stack>
    </form>
  );
}
