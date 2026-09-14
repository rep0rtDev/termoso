import { useState, type SubmitEvent } from "react";
import {
  Alert,
  AlertTitle,
  Button,
  Checkbox,
  FormControlLabel,
  Stack,
  Typography,
} from "@mui/material";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import { errorMessage } from "@/api/client";
import { authApi } from "@/api/endpoints";
import { startOverFinish } from "@/auth/flows";
import { Loading } from "@/components/Loading";
import { formatDateTime, formatRelative } from "@/components/format";
import { AuthTitle, PasswordField, passwordProblem } from "./common";
import { IrreversibleWarning } from "./StartOverPage";

/** Opened from the "finish" link in the start-over email. */
export function StartOverFinishPage() {
  const { token = "" } = useParams();
  const navigate = useNavigate();
  const status = useQuery({
    queryKey: ["start-over", token],
    queryFn: () => authApi.startOverStatus(token),
    retry: false,
  });
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [acknowledged, setAcknowledged] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const pwProblem = password.length > 0 ? passwordProblem(password) : null;
  const mismatch = confirm.length > 0 && confirm !== password;

  if (status.isPending) return <Loading />;
  if (status.isError) {
    return (
      <Stack spacing={2}>
        <AuthTitle title="This link is no longer valid" />
        <Alert severity="warning">{errorMessage(status.error)}</Alert>
        <Typography variant="body2" color="text.secondary">
          The reset was cancelled, already completed, or the link expired. You can request a new one
          from the sign-in page.
        </Typography>
        <Button variant="outlined" onClick={() => void navigate("/login")}>
          Back to sign in
        </Button>
      </Stack>
    );
  }
  const st = status.data;

  if (!st.ready) {
    return (
      <Stack spacing={2}>
        <AuthTitle
          title="Not yet"
          subtitle={`The reset for ${st.email_hint} can be completed ${formatRelative(st.scheduled_for)} (${formatDateTime(st.scheduled_for)}).`}
        />
        <IrreversibleWarning />
        <Alert severity="info">
          Come back to this link after that time. If you did not ask for this reset, use the cancel
          link from the same email — or sign in on a device that is still signed in and cancel it
          from there.
        </Alert>
        <Button variant="outlined" onClick={() => void status.refetch()}>
          Check again
        </Button>
      </Stack>
    );
  }

  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    if (!acknowledged || pwProblem || mismatch) return;
    setBusy(true);
    setError(null);
    try {
      const out = await startOverFinish(token, st.email, password);
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
      <AuthTitle title="Start over" subtitle={`Set a new password for ${st.email_hint}.`} />
      <Stack spacing={2}>
        <Alert severity="error" variant="outlined">
          <AlertTitle>Last warning — this cannot be undone</AlertTitle>
          Confirming deletes your encrypted vault on the server: hosts, passwords, keys, snippets
          and settings. Termoso cannot recover them for you — there is no decryption key on the
          server. All devices and sessions are signed out and your SSH ID keys are removed. Team
          vaults need to be shared with you again by a team admin. You start with an empty vault and
          a new recovery key.
        </Alert>
        {error && <Alert severity="error">{error}</Alert>}
        <PasswordField
          label="New password"
          autoComplete="new-password"
          required
          autoFocus
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
        <FormControlLabel
          control={
            <Checkbox
              checked={acknowledged}
              onChange={(e) => setAcknowledged(e.target.checked)}
              disabled={busy}
            />
          }
          label="I understand that my old encrypted data is lost forever."
        />
        <Button
          type="submit"
          variant="contained"
          size="large"
          color="error"
          disabled={
            busy || !acknowledged || password.length === 0 || pwProblem !== null || mismatch
          }
        >
          {busy ? "Resetting…" : "Delete my data and start over"}
        </Button>
      </Stack>
    </form>
  );
}

/** Opened from the "cancel" link in the start-over email. */
export function StartOverCancelPage() {
  const { token = "" } = useParams();
  const navigate = useNavigate();
  const [state, setState] = useState<"idle" | "busy" | "done">("idle");
  const [error, setError] = useState<string | null>(null);

  const cancel = async () => {
    setState("busy");
    setError(null);
    try {
      await authApi.startOverCancel(token);
      setState("done");
    } catch (err) {
      setError(errorMessage(err));
      setState("idle");
    }
  };

  if (state === "done") {
    return (
      <Stack spacing={2}>
        <AuthTitle title="Reset cancelled" subtitle="Nothing was changed on your account." />
        <Alert severity="warning">
          If you did not request this reset, someone else has access to your mailbox. Change your
          email password, then sign in to Termoso and review Devices and Security.
        </Alert>
        <Button variant="contained" onClick={() => void navigate("/login")}>
          Go to sign in
        </Button>
      </Stack>
    );
  }

  return (
    <Stack spacing={2}>
      <AuthTitle
        title="Cancel the account reset?"
        subtitle="This stops the scheduled start-over. Your encrypted data stays as it is."
      />
      {error && <Alert severity="error">{error}</Alert>}
      <Button
        variant="contained"
        size="large"
        onClick={() => void cancel()}
        disabled={state === "busy"}
      >
        {state === "busy" ? "Cancelling…" : "Cancel the reset"}
      </Button>
    </Stack>
  );
}
