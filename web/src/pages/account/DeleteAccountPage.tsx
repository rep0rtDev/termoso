import { useState, type SubmitEvent } from "react";
import { Alert, Button, Link, Stack, TextField, Typography } from "@mui/material";
import { useQuery } from "@tanstack/react-query";
import { Link as RouterLink, useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { accountApi, teamsApi } from "@/api/endpoints";
import { queryKeys, useServerInfo } from "@/api/hooks";
import { authStore, useAuthState } from "@/auth/store";
import { PageHeader } from "@/components/PageHeader";
import { Section } from "@/components/Section";

export function DeleteAccountPage() {
  const navigate = useNavigate();
  const { session } = useAuthState();
  const info = useServerInfo();
  const teams = useQuery({ queryKey: queryKeys.teams, queryFn: teamsApi.list });
  const [typed, setTyped] = useState("");
  const [code, setCode] = useState("");
  const [codeSent, setCodeSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const email = session?.user.email ?? "";
  const mayNeedCode =
    (info.data?.features.email ?? false) && (session?.user.email_verified ?? false);
  const owned = teams.data?.teams.filter((t) => t.my_role === "owner") ?? [];
  const confirmed = typed.trim().toLowerCase() === email.toLowerCase();

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    if (!confirmed) return;
    setBusy(true);
    setError(null);
    try {
      const result = await accountApi.delete(codeSent ? code.trim() : undefined);
      if (result === "code_sent") {
        setCodeSent(true);
        return;
      }
      authStore.signOut();
      void navigate("/login", { replace: true });
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <PageHeader
        title="Delete account"
        subtitle="Permanently remove your account and every encrypted record it owns."
      />
      <Section title="This cannot be undone" danger>
        <form onSubmit={submit}>
          <Stack spacing={2}>
            <Typography variant="body2">
              Hosts, keys, snippets, session logs, devices and your personal vault are deleted from
              this server. Team vaults you belong to stay with the team, but your access is removed.
            </Typography>
            {owned.length > 0 && (
              <Alert severity="warning">
                You own {owned.length} {owned.length === 1 ? "team" : "teams"} (
                {owned.map((t) => t.name).join(", ")}). Transfer ownership or delete{" "}
                {owned.length === 1 ? "it" : "them"} first on the{" "}
                <Link component={RouterLink} to="/team">
                  Team
                </Link>{" "}
                page.
              </Alert>
            )}
            {error && <Alert severity="error">{error}</Alert>}
            <TextField
              label={`Type ${email} to confirm`}
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              autoComplete="off"
              disabled={busy || codeSent}
            />
            {codeSent && (
              <>
                <Alert severity="info">
                  We emailed you a confirmation code. Enter it to finish.
                </Alert>
                <TextField
                  autoFocus
                  label="Confirmation code"
                  required
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                  disabled={busy}
                />
              </>
            )}
            <Stack direction="row" spacing={1}>
              <Button
                type="submit"
                variant="contained"
                color="error"
                disabled={
                  busy || !confirmed || owned.length > 0 || (codeSent && code.trim() === "")
                }
              >
                {codeSent || !mayNeedCode ? "Delete account" : "Send confirmation code"}
              </Button>
              <Button color="inherit" onClick={() => void navigate(-1)} disabled={busy}>
                Cancel
              </Button>
            </Stack>
          </Stack>
        </form>
      </Section>
    </>
  );
}
