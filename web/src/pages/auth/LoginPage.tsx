import { useState, type SubmitEvent } from "react";
import { tr, trx } from "@/i18n";
import { Alert, Box, Button, Divider, Link, Stack, TextField, Typography } from "@mui/material";
import { Link as RouterLink, useLocation, useNavigate } from "react-router";
import { ApiError, errorMessage } from "@/api/client";
import { authApi } from "@/api/endpoints";
import type { SsoDiscoverResponse } from "@/api/types";
import { useServerInfo } from "@/api/hooks";
import { login } from "@/auth/flows";
import { startSso } from "@/auth/sso";
import { AuthTitle, PasswordField, useNextPath } from "./common";
import { Turnstile } from "@/components/Turnstile";

export interface SsoLoginState {
  ssoSession: string;
  email: string;
}

export function LoginPage() {
  const navigate = useNavigate();
  const next = useNextPath();
  const location = useLocation();
  const sso = (location.state as { sso?: SsoLoginState } | null)?.sso;
  const [sessionEnded] = useState(() => {
    const ended = sessionStorage.getItem("termoso.session_ended") === "1";
    sessionStorage.removeItem("termoso.session_ended");
    return ended;
  });
  const info = useServerInfo();

  const [email, setEmail] = useState(sso?.email ?? "");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // The server asks for a captcha only after several failed attempts from this address.
  const siteKey = info.data?.turnstile_site_key ?? null;
  const [needCaptcha, setNeedCaptcha] = useState(false);
  const [captcha, setCaptcha] = useState<string | null>(null);
  const [captchaRound, setCaptchaRound] = useState(0);
  // Home-realm discovery: a corporate domain routes the form to the team's IdP.
  const [disc, setDisc] = useState<{ email: string; r: SsoDiscoverResponse } | null>(null);
  const [passwordAnyway, setPasswordAnyway] = useState(false);
  const ssoOnly = info.data?.sso_only === true;
  const discover = (value: string) => {
    const addr = value.trim().toLowerCase();
    if (sso || !addr.includes("@") || disc?.email === addr) return;
    authApi
      .ssoDiscover(addr)
      .then((r) => setDisc({ email: addr, r }))
      .catch(() =>
        setDisc({ email: addr, r: { provider: null, team_name: null, required: false } }),
      );
  };
  const found = disc?.email === email.trim().toLowerCase() ? disc.r : null;
  const idp = found?.provider ?? null;
  const passwordHidden = ssoOnly || (idp !== null && !passwordAnyway);

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const outcome = await login(email, password, sso?.ssoSession, captcha ?? undefined);
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
      if (err instanceof ApiError && err.code === "turnstile_required" && siteKey) {
        setNeedCaptcha(true);
        setError(tr("Please confirm you are human below, then sign in again."));
      } else {
        setError(errorMessage(err));
      }
      setCaptcha(null);
      setCaptchaRound((n) => n + 1);
    } finally {
      setBusy(false);
    }
  };

  const providers = info.data?.sso_providers ?? [];

  return (
    <form onSubmit={submit}>
      <AuthTitle
        title={sso ? tr("Unlock your account") : tr("Sign in")}
        subtitle={
          sso
            ? tr(
                "Signed in with SSO as {email}. Enter your Termoso password to unlock your encrypted data.",
                { email: sso.email },
              )
            : tr("Your password never leaves this device.")
        }
      />
      <Stack spacing={2}>
        {sessionEnded && !error && (
          <Alert severity="info">
            {tr(
              "Your session ended — you signed in on this device again or it was revoked. Sign in to continue.",
            )}
          </Alert>
        )}
        {error && <Alert severity="error">{error}</Alert>}
        <TextField
          label={tr("Email")}
          type="email"
          autoComplete="username"
          autoFocus={!sso}
          required
          value={email}
          onChange={(e) => {
            setEmail(e.target.value);
            setPasswordAnyway(false);
          }}
          onBlur={(e) => discover(e.target.value)}
          disabled={busy || sso !== undefined}
        />
        {idp && (
          <Alert severity="info">
            {found?.team_name
              ? tr("Your organization {v1} uses single sign-on.", { v1: found.team_name })
              : tr("Your organization uses single sign-on.")}
          </Alert>
        )}
        {ssoOnly && !sso && (
          <Alert severity="info">
            {tr(
              "This server accepts single sign-on only. Continue with your organization's provider below.",
            )}
          </Alert>
        )}
        {!passwordHidden && (
          <>
            <PasswordField
              label={tr("Password")}
              autoComplete="current-password"
              autoFocus={sso !== undefined}
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={busy}
            />
            <Box sx={{ display: "flex", justifyContent: "flex-end" }}>
              <Link component={RouterLink} to="/forgot-password" variant="body2">
                {tr("Forgot password?")}
              </Link>
            </Box>
          </>
        )}
        {needCaptcha && siteKey && (
          <Turnstile key={captchaRound} siteKey={siteKey} onToken={setCaptcha} />
        )}
        {idp && passwordHidden ? (
          <>
            <Button
              variant="contained"
              size="large"
              disabled={busy}
              onClick={() => {
                setError(null);
                startSso(idp.id, next).catch((err: unknown) => setError(errorMessage(err)));
              }}
            >
              {tr("Continue with {v1}", { v1: idp.name })}
            </Button>
            {!found?.required && !ssoOnly && (
              <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
                <Link
                  component="button"
                  type="button"
                  variant="body2"
                  onClick={() => setPasswordAnyway(true)}
                >
                  {tr("Sign in with a password instead")}
                </Link>
              </Typography>
            )}
          </>
        ) : passwordHidden ? null : (
          <Button
            type="submit"
            variant="contained"
            size="large"
            disabled={busy || (needCaptcha && captcha === null)}
          >
            {busy ? tr("Signing in…") : tr("Sign in")}
          </Button>
        )}

        {!sso && providers.length > 0 && (
          <>
            {!passwordHidden && (
              <Divider>
                <Typography variant="caption" color="text.secondary">
                  {tr("or continue with")}
                </Typography>
              </Divider>
            )}
            <Stack spacing={1}>
              {providers
                .filter((p) => p.id !== idp?.id)
                .map((p) => (
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

        {ssoOnly && !sso && providers.length === 0 && !idp && (
          <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
            {tr("Enter your work e-mail to find your organization's provider.")}
          </Typography>
        )}
        {info.data?.registration_open !== false && !ssoOnly && !sso && (
          <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
            {trx("New to Termoso?  {v1}", {
              v1: (
                <Link
                  component={RouterLink}
                  to={next === "/account" ? "/signup" : `/signup?next=${encodeURIComponent(next)}`}
                >
                  {tr("Create an account")}
                </Link>
              ),
            })}
          </Typography>
        )}
      </Stack>
    </form>
  );
}
