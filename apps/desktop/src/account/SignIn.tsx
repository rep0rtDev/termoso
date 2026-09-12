import { useState, type KeyboardEvent } from "react";
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Link,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import CloudOutlinedIcon from "@mui/icons-material/CloudOutlined";
import DnsOutlinedIcon from "@mui/icons-material/DnsOutlined";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Field } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, type useAccount } from "@/ipc/hooks";
import { createStore, useStore } from "@/lib/store";
import { CLOUD_HOST, CLOUD_URL, normalizeServerUrl } from "@/lib/cloud";
import { errorMessage, type LoginOutcome, type MfaMethod } from "@/ipc/types";

export const MFA_LABEL: Record<MfaMethod, string> = {
  totp: "Authenticator app",
  webauthn: "Security key",
  email: "Email code",
  backup_code: "Backup code",
};

/** Everything that may differ after signing in / out. */
export function invalidateAll(qc: ReturnType<typeof useQueryClient>) {
  void qc.invalidateQueries({ queryKey: keys.account });
  void qc.invalidateQueries({ queryKey: keys.vaults });
  void qc.invalidateQueries({ queryKey: keys.devices });
  void qc.invalidateQueries({ queryKey: ["hosts"] });
  void qc.invalidateQueries({ queryKey: ["groups"] });
  void qc.invalidateQueries({ queryKey: ["identities"] });
  void qc.invalidateQueries({ queryKey: ["sshKeys"] });
  void qc.invalidateQueries({ queryKey: ["pfRules"] });
  void qc.invalidateQueries({ queryKey: ["snippets"] });
  void qc.invalidateQueries({ queryKey: ["packages"] });
  void qc.invalidateQueries({ queryKey: keys.knownHosts });
}

export function pendingTitle(pending: LoginOutcome): string {
  return pending.step === "deviceApprovalRequired"
    ? "Approve this device"
    : "Two-factor verification";
}

type Where = "cloud" | "custom";
type Mode = "login" | "register";

/**
 * Recovery phrase of a just-created account. Registration signs the user in,
 * which unmounts the sign-in form, so the phrase is kept here and shown by
 * `RecoveryPrompt` at the app root until the user confirms they saved it.
 */
const recoveryStore = createStore<string | null>(null);

export function RecoveryPrompt() {
  const phrase = useStore(recoveryStore, (s) => s);
  return phrase !== null ? (
    <RecoveryDialog phrase={phrase} onDone={() => recoveryStore.set(null)} />
  ) : null;
}

/**
 * Sign in / create account against the free cloud or a self-hosted server.
 * The server is only probed once the user picked it; nothing is contacted
 * before that.
 */
export function SignInForm({
  onOutcome,
  autoFocus,
}: {
  onOutcome: (o: LoginOutcome) => void;
  autoFocus?: boolean;
}) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const [where, setWhere] = useState<Where>("cloud");
  const [mode, setMode] = useState<Mode>("login");
  const [serverUrl, setServerUrl] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [invite, setInvite] = useState("");
  const [touched, setTouched] = useState(false);

  const url = where === "cloud" ? CLOUD_URL : normalizeServerUrl(serverUrl);
  const info = useQuery({
    queryKey: ["serverInfo", url],
    queryFn: () => ipc.accountServerInfo(url ?? ""),
    enabled: touched && url !== null,
    retry: false,
    staleTime: 60_000,
  });

  const login = useMutation({
    mutationFn: () => ipc.accountLogin({ serverUrl: url ?? "", email: email.trim(), password }),
    onSuccess: (o) => {
      setPassword("");
      onOutcome(o);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const register = useMutation({
    mutationFn: () =>
      ipc.accountRegister({
        serverUrl: url ?? "",
        email: email.trim(),
        password,
        displayName: displayName.trim() || null,
        inviteToken: invite.trim() || null,
      }),
    onSuccess: (r) => {
      setPassword("");
      recoveryStore.set(r.recoveryPhrase);
      invalidateAll(qc);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const busy = login.isPending || register.isPending;
  const registrationClosed = info.data !== undefined && !info.data.registration_open;
  const canSubmit =
    info.data !== undefined &&
    email.trim().length > 0 &&
    (mode === "login"
      ? password.length > 0
      : password.length >= 12 && (!registrationClosed || invite.trim().length > 0));
  const submit = () => (mode === "login" ? login.mutate() : register.mutate());
  const onEnter = (e: KeyboardEvent) => {
    if (e.key === "Enter" && canSubmit && !busy) submit();
  };

  const serverLine = info.isError
    ? errorMessage(info.error)
    : info.data
      ? `${info.data.name} · v${info.data.version}${registrationClosed ? " · invite only" : ""}`
      : info.isFetching
        ? "Checking server…"
        : where === "cloud"
          ? " "
          : "Enter the address of your Termoso server";

  return (
    <Stack spacing={2}>
      <ToggleButtonGroup
        exclusive
        fullWidth
        value={where}
        onChange={(_, v: Where | null) => {
          if (!v) return;
          setWhere(v);
          setTouched(true);
        }}
      >
        <ToggleButton value="cloud">
          <CloudOutlinedIcon sx={{ fontSize: 16, mr: 0.75 }} />
          Termoso Cloud
        </ToggleButton>
        <ToggleButton value="custom">
          <DnsOutlinedIcon sx={{ fontSize: 16, mr: 0.75 }} />
          Own server
        </ToggleButton>
      </ToggleButtonGroup>

      {where === "custom" ? (
        <Field label="Server URL">
          <TextField
            autoFocus={autoFocus}
            placeholder="https://termoso.example.com"
            value={serverUrl}
            onChange={(e) => {
              setServerUrl(e.target.value);
              setTouched(true);
            }}
            error={info.isError}
            helperText={serverLine}
            slotProps={{
              input: {
                endAdornment: info.isFetching ? <CircularProgress size={14} /> : undefined,
              },
            }}
          />
        </Field>
      ) : (
        <Box
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1.25,
            px: 1.5,
            py: 1.25,
            borderRadius: 2,
            bgcolor: "surface.highest",
          }}
        >
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="body2" sx={{ fontWeight: 500 }}>
              {CLOUD_HOST}
            </Typography>
            <Typography
              variant="caption"
              color={info.isError ? "error" : "text.secondary"}
              noWrap
              sx={{ display: "block" }}
            >
              {info.data || info.isError || info.isFetching
                ? serverLine
                : "Free · end-to-end encrypted · no telemetry"}
            </Typography>
          </Box>
          {info.isFetching && <CircularProgress size={14} />}
        </Box>
      )}

      <Field label="Email">
        <TextField
          autoFocus={autoFocus && where === "cloud"}
          type="email"
          autoComplete="username"
          value={email}
          onChange={(e) => {
            setEmail(e.target.value);
            setTouched(true);
          }}
          onKeyDown={onEnter}
        />
      </Field>
      {mode === "register" && (
        <Field label="Display name" hint="Optional">
          <TextField
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            onKeyDown={onEnter}
          />
        </Field>
      )}
      <Field
        label="Password"
        hint={mode === "register" ? "At least 12 characters. It never leaves this device." : null}
      >
        <TextField
          type="password"
          autoComplete={mode === "login" ? "current-password" : "new-password"}
          value={password}
          onChange={(e) => {
            setPassword(e.target.value);
            setTouched(true);
          }}
          onKeyDown={onEnter}
        />
      </Field>
      {mode === "register" && registrationClosed && (
        <Field label="Invite token" hint="This server only accepts invited users.">
          <TextField value={invite} onChange={(e) => setInvite(e.target.value)} />
        </Field>
      )}

      <Button variant="contained" size="large" disabled={!canSubmit || busy} onClick={submit}>
        {busy ? (
          <CircularProgress size={18} color="inherit" />
        ) : mode === "login" ? (
          "Sign in"
        ) : (
          "Create account"
        )}
      </Button>

      <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
        {mode === "login" ? "New here? " : "Already have an account? "}
        <Link
          component="button"
          type="button"
          underline="hover"
          onClick={() => setMode(mode === "login" ? "register" : "login")}
        >
          {mode === "login" ? "Create a free account" : "Sign in"}
        </Link>
      </Typography>
    </Stack>
  );
}

export function RecoveryDialog({ phrase, onDone }: { phrase: string; onDone: () => void }) {
  const snackbar = useSnackbar();
  const [ack, setAck] = useState(false);
  return (
    <Dialog open maxWidth="sm" fullWidth>
      <DialogTitle>Save your recovery phrase</DialogTitle>
      <DialogContent>
        <Alert severity="warning" sx={{ mb: 2 }}>
          This is the only way to regain access if you forget your password. Termoso does not keep a
          copy anywhere — not on this device, not on the server.
        </Alert>
        <Box
          sx={{
            p: 2,
            borderRadius: 2,
            bgcolor: "surface.high",
            fontFamily: "monospace",
            fontSize: 15,
            lineHeight: 1.8,
            userSelect: "all",
            wordSpacing: 6,
          }}
        >
          {phrase}
        </Box>
        <Button
          startIcon={<ContentCopyRoundedIcon />}
          sx={{ mt: 1 }}
          onClick={() => {
            void navigator.clipboard.writeText(phrase).then(() => snackbar.notify("Copied"));
          }}
        >
          Copy to clipboard
        </Button>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2, justifyContent: "space-between" }}>
        <Button color={ack ? "primary" : "inherit"} onClick={() => setAck(!ack)}>
          {ack ? "✓ I have stored it safely" : "I have stored it safely"}
        </Button>
        <Button variant="contained" disabled={!ack} onClick={onDone}>
          Continue
        </Button>
      </DialogActions>
    </Dialog>
  );
}

/** Second step of a login: MFA code or new-device approval. */
export function PendingForm({
  pending,
  onOutcome,
}: {
  pending: LoginOutcome;
  onOutcome: (o: LoginOutcome | null) => void;
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
      if (pending.step === "deviceApprovalRequired") return ipc.accountDeviceApprove(c);
      switch (method) {
        case "totp":
          return ipc.accountMfa({ method: "totp", code: c });
        case "backup_code":
          return ipc.accountMfa({ method: "backup_code", code: c });
        case "email":
          return ipc.accountMfa({ method: "email", code: c });
        case "webauthn":
          throw new Error("Security keys are not available in the desktop app yet");
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
  const cancel = useMutation({
    mutationFn: ipc.accountCancelLogin,
    onSuccess: () => onOutcome(null),
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const busy = submit.isPending || cancel.isPending;

  return (
    <Stack spacing={1.5}>
      {pending.step === "deviceApprovalRequired" ? (
        <Typography variant="body2" color="text.secondary">
          A confirmation code was sent to <b>{pending.emailHint}</b>. Enter it to trust this device.
        </Typography>
      ) : (
        <>
          {methods.length > 1 && (
            <ToggleButtonGroup
              exclusive
              value={method}
              onChange={(_, v: MfaMethod | null) => v && setMethod(v)}
              sx={{ flexWrap: "wrap" }}
            >
              {methods.map((m) => (
                <ToggleButton key={m} value={m} disabled={m === "webauthn"}>
                  {MFA_LABEL[m]}
                </ToggleButton>
              ))}
            </ToggleButtonGroup>
          )}
          {methods.includes("webauthn") && (
            <Typography variant="caption" color="text.secondary">
              Security keys need a browser origin and are not available inside the desktop app yet —
              use another method.
            </Typography>
          )}
          {method === "email" && (
            <Button
              variant="tonal"
              disabled={side.isPending}
              onClick={() =>
                side.mutate(async () => {
                  await ipc.accountMfaEmailSend();
                  setEmailSent(true);
                  return "Code sent";
                })
              }
            >
              {emailSent ? "Send again" : "Send code to my email"}
            </Button>
          )}
        </>
      )}
      <Field
        label={method === "backup_code" && pending.step === "mfaRequired" ? "Backup code" : "Code"}
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
      <Stack direction="row" spacing={1} sx={{ justifyContent: "space-between" }}>
        <Stack direction="row" spacing={1}>
          <Button color="inherit" disabled={busy} onClick={() => cancel.mutate()}>
            Cancel
          </Button>
          {pending.step === "deviceApprovalRequired" && (
            <Button
              disabled={side.isPending}
              onClick={() =>
                side.mutate(async () => {
                  await ipc.accountDeviceResend();
                  return "Code sent again";
                })
              }
            >
              Resend
            </Button>
          )}
        </Stack>
        <Button variant="contained" disabled={!code.trim() || busy} onClick={() => submit.mutate()}>
          Verify
        </Button>
      </Stack>
    </Stack>
  );
}

/** The MFA / approval step of a login, kept in sync with the account query. */
export function usePendingLogin(status: ReturnType<typeof useAccount>) {
  const qc = useQueryClient();
  const [local, setLocal] = useState<{ at: number; value: LoginOutcome | null } | null>(null);
  const pending = local?.at === status.dataUpdatedAt ? local.value : (status.data?.pending ?? null);
  const onOutcome = (o: LoginOutcome | null) => {
    setLocal({ at: status.dataUpdatedAt, value: o?.step === "done" ? null : o });
    if (o?.step === "done" || o === null) invalidateAll(qc);
    else void qc.invalidateQueries({ queryKey: keys.account });
  };
  return { pending: pending && pending.step !== "done" ? pending : null, onOutcome };
}
