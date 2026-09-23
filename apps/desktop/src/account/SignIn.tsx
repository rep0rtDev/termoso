import { useEffect, useState, type KeyboardEvent } from "react";
import { copyToClipboard } from "@/lib/clipboard";
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
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
import VerifiedUserOutlinedIcon from "@mui/icons-material/VerifiedUserOutlined";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Field } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, type useAccount } from "@/ipc/hooks";
import { createStore, useStore } from "@/lib/store";
import { CLOUD_HOST, CLOUD_URL, normalizeServerUrl } from "@/lib/cloud";
import { errorMessage, type LoginOutcome, type MfaMethod, type SsoProvider } from "@/ipc/types";
import { cancelSso, clearSso, ssoStore, startSso } from "./sso";
import { tr, trx, msg } from "@/i18n";

export const MFA_LABEL: Record<MfaMethod, string> = {
  totp: msg("Authenticator app"),
  webauthn: msg("Security key"),
  email: msg("Email code"),
  backup_code: msg("Backup code"),
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
    ? tr("Approve this device")
    : tr("Two-factor verification");
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
 * Sign in / create account against Termoso Cloud or a self-hosted server.
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
  // A browser sign-in that is still running (or just finished) survives
  // remounts of this form; pick the server it was started against back up.
  const ssoState = useStore(ssoStore, (s) => s);
  const resumed = ssoState.phase !== "idle" ? ssoState.serverUrl : null;
  const [where, setWhere] = useState<Where>(resumed && resumed !== CLOUD_URL ? "custom" : "cloud");
  const [serverUrl, setServerUrl] = useState(resumed && resumed !== CLOUD_URL ? resumed : "");
  const [touched, setTouched] = useState(resumed !== null);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [invite, setInvite] = useState("");
  const [chosenMode, setMode] = useState<Mode>("login");

  const url = where === "cloud" ? CLOUD_URL : normalizeServerUrl(serverUrl);
  const info = useQuery({
    queryKey: ["serverInfo", url],
    queryFn: () => ipc.accountServerInfo(url ?? ""),
    enabled: touched && url !== null,
    retry: false,
    staleTime: 60_000,
  });

  const sso = ssoState.phase !== "idle" && ssoState.serverUrl === url ? ssoState : null;
  const waiting = sso?.phase === "waiting";
  const verifiedSso = sso?.phase === "verified" ? sso : null;
  const verified = verifiedSso?.outcome ?? null;
  const mode: Mode =
    verified?.step === "registrationRequired"
      ? "register"
      : verified?.step === "loginRequired"
        ? "login"
        : chosenMode;
  // Switching servers abandons a browser sign-in started against another one.
  useEffect(() => {
    if (ssoState.phase !== "idle" && url !== null && ssoState.serverUrl !== url) void cancelSso();
  }, [ssoState, url]);

  const login = useMutation({
    mutationFn: () =>
      ipc.accountLogin({
        serverUrl: url ?? "",
        email: (verified?.email ?? email).trim(),
        password,
        sso: verified !== null,
      }),
    onSuccess: (o) => {
      setPassword("");
      if (verified) clearSso();
      onOutcome(o);
    },
    onError: (e) => {
      if (verified) clearSso();
      snackbar.error(errorMessage(e));
    },
  });
  const register = useMutation({
    mutationFn: () =>
      ipc.accountRegister({
        serverUrl: url ?? "",
        email: (verified?.email ?? email).trim(),
        password,
        displayName: displayName.trim() || null,
        inviteToken: invite.trim() || null,
        sso: verified !== null,
      }),
    onSuccess: (r) => {
      setPassword("");
      if (verified) clearSso();
      recoveryStore.set(r.recoveryPhrase);
      invalidateAll(qc);
    },
    onError: (e) => {
      if (verified) clearSso();
      snackbar.error(errorMessage(e));
    },
  });
  const ssoStart = useMutation({
    mutationFn: (provider: SsoProvider) =>
      startSso(url ?? "", provider, (message) => snackbar.error(message)),
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const busy = login.isPending || register.isPending;
  const registrationClosed = info.data !== undefined && !info.data.registration_open;
  // SSO-verified users may register on an invite-only server when the admin allows it.
  const needsInvite =
    info.data !== undefined &&
    !info.data.registration_open &&
    !(verified?.step === "registrationRequired" && info.data.sso_registration);
  const providers = info.data?.sso_providers ?? [];
  const canSubmit =
    info.data !== undefined &&
    (verified !== null || email.trim().length > 0) &&
    (mode === "login"
      ? password.length > 0
      : password.length >= 12 && (!needsInvite || invite.trim().length > 0));
  const submit = () => (mode === "login" ? login.mutate() : register.mutate());
  const onEnter = (e: KeyboardEvent) => {
    if (e.key === "Enter" && canSubmit && !busy) submit();
  };

  const serverLine = info.isError
    ? errorMessage(info.error)
    : info.data
      ? `${info.data.name} · v${info.data.version}${registrationClosed ? " · invite only" : ""}`
      : info.isFetching
        ? tr("Checking server…")
        : where === "cloud"
          ? " "
          : tr("Enter the address of your Termoso server");

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
          {tr("Termoso Cloud")}
        </ToggleButton>
        <ToggleButton value="custom">
          <DnsOutlinedIcon sx={{ fontSize: 16, mr: 0.75 }} />
          {tr("Own server")}
        </ToggleButton>
      </ToggleButtonGroup>

      {where === "custom" ? (
        <Field label={tr("Server URL")}>
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
            <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
              {tr(
                "Completely free for everyone — no limits, no plans, no strings attached. End-to-end encrypted, no telemetry.",
              )}
            </Typography>
            {(info.data !== undefined || info.isError || info.isFetching) && (
              <Typography
                variant="caption"
                color={info.isError ? "error" : "text.disabled"}
                noWrap
                sx={{ mt: 0.25 }}
              >
                {serverLine}
              </Typography>
            )}
          </Box>
          {info.isFetching && <CircularProgress size={14} />}
        </Box>
      )}

      {waiting ? (
        <SsoWaiting provider={sso.provider} onCancel={() => void cancelSso()} />
      ) : (
        <>
          {verifiedSso ? (
            <SsoVerifiedBanner
              provider={verifiedSso.provider}
              email={verifiedSso.outcome.email}
              onReset={() => {
                clearSso();
                setPassword("");
              }}
            />
          ) : (
            <Field label={tr("Email")}>
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
          )}
          {mode === "register" && (
            <Field label={tr("Display name")} hint={tr("Optional")}>
              <TextField
                value={displayName}
                placeholder={
                  verified?.step === "registrationRequired"
                    ? (verified.displayName ?? undefined)
                    : undefined
                }
                onChange={(e) => setDisplayName(e.target.value)}
                onKeyDown={onEnter}
              />
            </Field>
          )}
          {
            <Field
              label={verified ? tr("Termoso password") : tr("Password")}
              hint={
                mode === "register"
                  ? verified
                    ? tr(
                        "Choose a password for Termoso, at least 12 characters. It encrypts your vaults and never leaves this device — your identity provider never sees it.",
                      )
                    : tr("At least 12 characters. It never leaves this device.")
                  : verified
                    ? tr(
                        "Your Termoso password unlocks the encrypted vaults; it is separate from the identity provider.",
                      )
                    : null
              }
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
          }
          {mode === "register" && needsInvite && (
            <Field label={tr("Invite token")} hint={tr("This server only accepts invited users.")}>
              <TextField value={invite} onChange={(e) => setInvite(e.target.value)} />
            </Field>
          )}

          {
            <Button variant="contained" size="large" disabled={!canSubmit || busy} onClick={submit}>
              {busy ? (
                <CircularProgress size={18} color="inherit" />
              ) : mode === "login" ? (
                tr("Sign in")
              ) : (
                tr("Create account")
              )}
            </Button>
          }
        </>
      )}

      {!sso && providers.length > 0 && (
        <>
          <Divider>
            <Typography variant="caption" color="text.secondary">
              {tr("or")}
            </Typography>
          </Divider>
          <Stack spacing={1}>
            {providers.map((p) => (
              <Button
                key={p.id}
                variant="outlined"
                size="large"
                disabled={ssoStart.isPending || busy}
                onClick={() => ssoStart.mutate(p)}
              >
                {tr("Continue with")} {p.name}
              </Button>
            ))}
          </Stack>
        </>
      )}

      {!waiting && !verified && (
        <Typography variant="body2" color="text.secondary" sx={{ textAlign: "center" }}>
          {mode === "login" ? tr("New here?") : tr("Already have an account?")}{" "}
          <Link
            component="button"
            type="button"
            underline="hover"
            onClick={() => setMode(mode === "login" ? "register" : "login")}
          >
            {mode === "login" ? tr("Create a free account") : tr("Sign in")}
          </Link>
        </Typography>
      )}
    </Stack>
  );
}

function SsoWaiting({ provider, onCancel }: { provider: SsoProvider; onCancel: () => void }) {
  return (
    <Stack
      spacing={1.5}
      sx={{
        alignItems: "center",
        textAlign: "center",
        p: 2,
        borderRadius: 2,
        bgcolor: "surface.highest",
      }}
    >
      <CircularProgress size={22} />
      <Typography variant="body2">
        {tr("Finish signing in with {provider} in your browser.", { provider: provider.name })}
      </Typography>
      <Typography variant="caption" color="text.secondary">
        {tr(
          "Termoso picks up automatically when the browser comes back. Nothing from the provider is stored on this device.",
        )}
      </Typography>
      <Button color="inherit" size="small" onClick={onCancel}>
        {tr("Cancel")}
      </Button>
    </Stack>
  );
}

function SsoVerifiedBanner({
  provider,
  email,
  onReset,
}: {
  provider: SsoProvider;
  email: string;
  onReset: () => void;
}) {
  return (
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
      <VerifiedUserOutlinedIcon color="primary" sx={{ fontSize: 20 }} />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body2" sx={{ fontWeight: 500 }} noWrap>
          {email}
        </Typography>
        <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
          {tr("Verified with {provider}", { provider: provider.name })}
        </Typography>
      </Box>
      <Button color="inherit" size="small" onClick={onReset}>
        {tr("Change")}
      </Button>
    </Box>
  );
}

export function RecoveryDialog({ phrase, onDone }: { phrase: string; onDone: () => void }) {
  const snackbar = useSnackbar();
  const [ack, setAck] = useState(false);
  return (
    <Dialog open maxWidth="sm" fullWidth>
      <DialogTitle>{tr("Save your recovery phrase")}</DialogTitle>
      <DialogContent>
        <Alert severity="warning" sx={{ mb: 2 }}>
          {tr(
            "This is the only way to regain access if you forget your password. Termoso does not keep a copy anywhere — not on this device, not on the server.",
          )}
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
            void copyToClipboard(phrase)
              .then(() => snackbar.notify(tr("Copied")))
              .catch(() => snackbar.error(tr("Clipboard is not available")));
          }}
        >
          {tr("Copy to clipboard")}
        </Button>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2, justifyContent: "space-between" }}>
        <Button color={ack ? "primary" : "inherit"} onClick={() => setAck(!ack)}>
          {ack ? tr("✓ I have stored it safely") : tr("I have stored it safely")}
        </Button>
        <Button variant="contained" disabled={!ack} onClick={onDone}>
          {tr("Continue")}
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
          {trx("A confirmation code was sent to {email}. Enter it to trust this device.", {
            email: <b>{pending.emailHint}</b>,
          })}
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
                  await ipc.accountMfaEmailSend();
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
      <Stack direction="row" spacing={1} sx={{ justifyContent: "space-between" }}>
        <Stack direction="row" spacing={1}>
          <Button color="inherit" disabled={busy} onClick={() => cancel.mutate()}>
            {tr("Cancel")}
          </Button>
          {pending.step === "deviceApprovalRequired" && (
            <Button
              disabled={side.isPending}
              onClick={() =>
                side.mutate(async () => {
                  await ipc.accountDeviceResend();
                  return tr("Code sent again");
                })
              }
            >
              {tr("Resend")}
            </Button>
          )}
        </Stack>
        <Button variant="contained" disabled={!code.trim() || busy} onClick={() => submit.mutate()}>
          {tr("Verify")}
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
