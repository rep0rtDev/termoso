import { useState } from "react";
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import { useMutation } from "@tanstack/react-query";
import { Field } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { useAccount } from "@/ipc/hooks";
import { useStore } from "@/lib/store";
import { errorMessage, type MfaMethod, type ReauthOutcome } from "@/ipc/types";
import { MFA_LABEL } from "./SignIn";
import { reauthStore } from "./reauth";
import { tr, trx } from "@/i18n";

/**
 * Asks for the account password (and second factor) before a sensitive
 * change. Mounted once at the app root; opened through `requestReauth`.
 */
export function ReauthDialog() {
  const request = useStore(reauthStore, (s) => s.request);
  if (!request) return null;
  return (
    <Body key="open" onDone={() => request.resolve(true)} onCancel={() => request.resolve(false)} />
  );
}

function Body({ onDone, onCancel }: { onDone: () => void; onCancel: () => void }) {
  const snackbar = useSnackbar();
  const account = useAccount().data?.account ?? null;
  const [password, setPassword] = useState("");
  const [pending, setPending] = useState<Exclude<ReauthOutcome, { step: "done" }> | null>(null);

  const finish = (o: ReauthOutcome) => {
    if (o.step === "done") {
      onDone();
    } else {
      setPending(o);
    }
  };
  const start = useMutation({
    mutationFn: () => ipc.accountReauthStart(password),
    onSuccess: (o) => {
      setPassword("");
      finish(o);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const cancel = useMutation({
    mutationFn: ipc.accountReauthCancel,
    onSettled: onCancel,
  });
  const busy = start.isPending || cancel.isPending;

  return (
    <Dialog open onClose={busy ? undefined : () => cancel.mutate()} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Confirm it’s you")}</DialogTitle>
      <DialogContent>
        {pending ? (
          <SecondStep pending={pending} onOutcome={finish} onCancel={() => cancel.mutate()} />
        ) : (
          <Stack spacing={1.5}>
            <Typography variant="body2" color="text.secondary">
              {trx(
                "This change affects the security of your account. Enter the password for {account} to continue.",
                { account: <b>{account?.email ?? tr("your account")}</b> },
              )}
            </Typography>
            <Field
              label={tr("Password")}
              hint={tr(
                "Signed in through SSO without a password? Leave it empty to get a code by email.",
              )}
            >
              <TextField
                autoFocus
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="current-password"
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !busy) start.mutate();
                }}
              />
            </Field>
          </Stack>
        )}
      </DialogContent>
      {!pending && (
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button color="inherit" disabled={busy} onClick={() => cancel.mutate()}>
            {tr("Cancel")}
          </Button>
          <Button variant="contained" disabled={busy} onClick={() => start.mutate()}>
            {tr("Continue")}
          </Button>
        </DialogActions>
      )}
    </Dialog>
  );
}

/** Second factor or emailed code, mirroring the sign-in step. */
function SecondStep({
  pending,
  onOutcome,
  onCancel,
}: {
  pending: Exclude<ReauthOutcome, { step: "done" }>;
  onOutcome: (o: ReauthOutcome) => void;
  onCancel: () => void;
}) {
  const snackbar = useSnackbar();
  const methods = pending.step === "mfaRequired" ? pending.methods : [];
  const usable = methods.filter((m) => m !== "webauthn");
  const [method, setMethod] = useState<MfaMethod>(usable[0] ?? "totp");
  const [code, setCode] = useState("");
  const [emailSent, setEmailSent] = useState(false);

  const submit = useMutation({
    mutationFn: async () => {
      const c = code.trim();
      if (pending.step === "emailCodeRequired") return ipc.accountReauthEmailCode(c);
      switch (method) {
        case "totp":
          return ipc.accountReauthMfa({ method: "totp", code: c });
        case "backup_code":
          return ipc.accountReauthMfa({ method: "backup_code", code: c });
        case "email":
          return ipc.accountReauthMfa({ method: "email", code: c });
        case "webauthn":
          throw new Error(tr("Security keys are not available in the desktop app yet"));
      }
    },
    onSuccess: (o) => {
      setCode("");
      onOutcome(o);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const side = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => msg && snackbar.notify(msg),
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const busy = submit.isPending;

  return (
    <Stack spacing={1.5}>
      {pending.step === "emailCodeRequired" ? (
        <Typography variant="body2" color="text.secondary">
          {trx("A confirmation code was sent to {email}.", { email: <b>{pending.emailHint}</b> })}
        </Typography>
      ) : (
        <>
          <Typography variant="body2" color="text.secondary">
            {tr("Enter your second factor to continue.")}
          </Typography>
          {methods.length > 1 && (
            <ToggleButtonGroup
              exclusive
              value={method}
              onChange={(_, v: MfaMethod | null) => v && setMethod(v)}
              sx={{ flexWrap: "wrap" }}
            >
              {methods.map((m) => (
                <ToggleButton key={m} value={m} disabled={m === "webauthn"}>
                  {tr(MFA_LABEL[m])}
                </ToggleButton>
              ))}
            </ToggleButtonGroup>
          )}
          {methods.includes("webauthn") && (
            <Typography variant="caption" color="text.secondary">
              {tr(
                "Security keys need a browser origin and are not available inside the desktop app yet — use another method.",
              )}
            </Typography>
          )}
          {method === "email" && (
            <Button
              variant="tonal"
              disabled={side.isPending}
              onClick={() =>
                side.mutate(async () => {
                  await ipc.accountReauthMfaEmailSend();
                  setEmailSent(true);
                  return tr("Code sent");
                })
              }
            >
              {emailSent ? tr("Send again") : tr("Send code to my email")}
            </Button>
          )}
        </>
      )}
      <Field
        label={
          method === "backup_code" && pending.step === "mfaRequired"
            ? tr("Backup code")
            : tr("Code")
        }
      >
        <TextField
          autoFocus
          value={code}
          onChange={(e) => setCode(e.target.value)}
          autoComplete="one-time-code"
          slotProps={{ htmlInput: { style: { fontFamily: "monospace", letterSpacing: 2 } } }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && code.trim() && !busy) submit.mutate();
          }}
        />
      </Field>
      <Stack direction="row" spacing={1} sx={{ justifyContent: "space-between", pb: 0.5 }}>
        <Button color="inherit" disabled={busy} onClick={onCancel}>
          {tr("Cancel")}
        </Button>
        <Button variant="contained" disabled={!code.trim() || busy} onClick={() => submit.mutate()}>
          {tr("Verify")}
        </Button>
      </Stack>
    </Stack>
  );
}
