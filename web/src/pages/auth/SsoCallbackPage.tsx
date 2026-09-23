import { useEffect, useState } from "react";
import { Alert, Button, Stack } from "@mui/material";
import { useNavigate, useSearchParams } from "react-router";
import { errorMessage } from "@/api/client";
import { authApi } from "@/api/endpoints";
import { Loading } from "@/components/Loading";
import { AuthTitle } from "./common";
import { takeSsoNext } from "@/auth/sso";
import type { SsoResult } from "@/api/types";

// Terminal results are one-shot on the server; share a single in-flight poll
// per flow so overlapping effects (StrictMode, fast remounts) do not race.
const inflight = new Map<string, Promise<SsoResult>>();
function pollOnce(flow: string): Promise<SsoResult> {
  const pending = inflight.get(flow);
  if (pending) return pending;
  const p = authApi.ssoPoll(flow).finally(() => inflight.delete(flow));
  inflight.set(flow, p);
  return p;
}

export function SsoCallbackPage() {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const [pollError, setError] = useState<string | null>(null);
  const flow = params.get("flow");
  const error = flow ? pollError : "Missing SSO flow id";

  useEffect(() => {
    if (!flow) return;
    let cancelled = false;
    let attempts = 0;
    const tick = async () => {
      try {
        const r = await pollOnce(flow);
        if (cancelled) return;
        const next = takeSsoNext();
        switch (r.status) {
          case "pending":
            if (attempts++ < 30) window.setTimeout(() => void tick(), 1000);
            else setError("The identity provider did not finish in time. Please try again.");
            return;
          case "failed":
            setError(r.message);
            return;
          case "login_required":
            void navigate(`/login?next=${encodeURIComponent(next)}`, {
              replace: true,
              state: { sso: { ssoSession: r.sso_session, email: r.email } },
            });
            return;
          case "registration_required":
            void navigate(`/signup?next=${encodeURIComponent(next)}`, {
              replace: true,
              state: {
                sso: { ssoSession: r.sso_session, email: r.email, displayName: r.display_name },
              },
            });
            return;
        }
      } catch (e) {
        if (!cancelled) setError(errorMessage(e));
      }
    };
    void tick();
    return () => {
      cancelled = true;
    };
  }, [flow, navigate]);

  if (error) {
    return (
      <>
        <AuthTitle title="Single sign-on failed" />
        <Stack spacing={2}>
          <Alert severity="error">{error}</Alert>
          <Button variant="contained" onClick={() => navigate("/login", { replace: true })}>
            Back to sign in
          </Button>
        </Stack>
      </>
    );
  }
  return (
    <>
      <AuthTitle title="Completing sign-in…" subtitle="Talking to your identity provider." />
      <Loading minHeight={120} />
    </>
  );
}
