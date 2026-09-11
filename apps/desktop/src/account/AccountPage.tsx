import { useState, type ReactNode } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import SyncRoundedIcon from "@mui/icons-material/SyncRounded";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import CloudDoneRoundedIcon from "@mui/icons-material/CloudDoneRounded";
import CloudOffRoundedIcon from "@mui/icons-material/CloudOffRounded";
import BoltRoundedIcon from "@mui/icons-material/BoltRounded";
import DevicesRoundedIcon from "@mui/icons-material/DevicesRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import LockOpenRoundedIcon from "@mui/icons-material/LockOpenRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { PageBody } from "@/components/PageHeader";
import {
  EntityCard,
  Field,
  IconTile,
  Loading,
  Mono,
  SectionCard,
  SettingRow,
  ToolIconButton,
} from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useAccount, useDevices } from "@/ipc/hooks";
import { sizes } from "@/theme/theme";
import {
  errorMessage,
  type AccountStatus,
  type Device,
  type LoginOutcome,
  type MfaMethod,
  type SyncStatus,
} from "@/ipc/types";

const MFA_LABEL: Record<MfaMethod, string> = {
  totp: "Authenticator app",
  webauthn: "Security key",
  email: "Email code",
  backup_code: "Backup code",
};

function invalidateAll(qc: ReturnType<typeof useQueryClient>) {
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

// ───────────────────────────── signed out ─────────────────────────────

function SignInCard({ onOutcome }: { onOutcome: (o: LoginOutcome) => void }) {
  const snackbar = useSnackbar();
  const [tab, setTab] = useState<"login" | "register">("login");
  const [serverUrl, setServerUrl] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [invite, setInvite] = useState("");
  const [phrase, setPhrase] = useState<string | null>(null);

  const url = serverUrl.trim().replace(/\/+$/, "");
  const info = useQuery({
    queryKey: ["serverInfo", url],
    queryFn: () => ipc.accountServerInfo(url),
    enabled: /^https?:\/\/\S+$/.test(url),
    retry: false,
    staleTime: 60_000,
  });

  const login = useMutation({
    mutationFn: () => ipc.accountLogin({ serverUrl: url, email: email.trim(), password }),
    onSuccess: (o) => {
      setPassword("");
      onOutcome(o);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const register = useMutation({
    mutationFn: () =>
      ipc.accountRegister({
        serverUrl: url,
        email: email.trim(),
        password,
        displayName: displayName.trim() || null,
        inviteToken: invite.trim() || null,
      }),
    onSuccess: (r) => {
      setPassword("");
      setPhrase(r.recoveryPhrase);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const busy = login.isPending || register.isPending;
  const canSubmit =
    info.data !== undefined &&
    email.trim().length > 0 &&
    (tab === "login" ? password.length > 0 : password.length >= 12);

  return (
    <SectionCard title="Connect to a Termoso server" sx={{ maxWidth: 480 }}>
      <Typography variant="body2" color="text.secondary">
        Sync is optional. When you sign in, your vaults are encrypted on this device before they
        leave it — the server only ever sees ciphertext.
      </Typography>
      <Stack spacing={1.5}>
        <Field label="Server URL">
          <TextField
            placeholder="https://termoso.example.com"
            value={serverUrl}
            onChange={(e) => setServerUrl(e.target.value)}
            error={info.isError}
            helperText={
              info.isError
                ? errorMessage(info.error)
                : info.data
                  ? `${info.data.name} · v${info.data.version}${
                      info.data.registration_open ? "" : " · registration closed"
                    }`
                  : " "
            }
            slotProps={{
              input: {
                endAdornment: info.isFetching ? <CircularProgress size={16} /> : undefined,
              },
            }}
          />
        </Field>
        <ToggleButtonGroup
          exclusive
          fullWidth
          value={tab}
          onChange={(_, v: "login" | "register" | null) => v && setTab(v)}
        >
          <ToggleButton value="login">Sign in</ToggleButton>
          <ToggleButton
            value="register"
            disabled={info.data !== undefined && !info.data.registration_open && !invite}
          >
            Create account
          </ToggleButton>
        </ToggleButtonGroup>
        <Field label="Email">
          <TextField
            type="email"
            autoComplete="username"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </Field>
        {tab === "register" && (
          <Field label="Display name" hint="Optional">
            <TextField value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
          </Field>
        )}
        <Field label="Password">
          <TextField
            type="password"
            autoComplete={tab === "login" ? "current-password" : "new-password"}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            helperText={
              tab === "register"
                ? "At least 12 characters. It never leaves this device."
                : undefined
            }
            onKeyDown={(e) => {
              if (e.key === "Enter" && canSubmit && !busy) {
                if (tab === "login") login.mutate();
                else register.mutate();
              }
            }}
          />
        </Field>
        {tab === "register" && info.data && !info.data.registration_open && (
          <Field label="Invite token" hint="This server only accepts invited users.">
            <TextField value={invite} onChange={(e) => setInvite(e.target.value)} />
          </Field>
        )}
        <Button
          variant="contained"
          disabled={!canSubmit || busy}
          onClick={() => (tab === "login" ? login.mutate() : register.mutate())}
        >
          {busy ? (
            <CircularProgress size={18} color="inherit" />
          ) : tab === "login" ? (
            "Sign in"
          ) : (
            "Create account"
          )}
        </Button>
      </Stack>

      {phrase !== null && <RecoveryDialog phrase={phrase} onDone={() => setPhrase(null)} />}
    </SectionCard>
  );
}

function RecoveryDialog({ phrase, onDone }: { phrase: string; onDone: () => void }) {
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

// ───────────────────────────── pending login ─────────────────────────────

function PendingCard({
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
    <SectionCard
      title={
        pending.step === "deviceApprovalRequired"
          ? "Approve this device"
          : "Two-factor verification"
      }
      sx={{ maxWidth: 480 }}
    >
      <Stack spacing={1.5}>
        {pending.step === "deviceApprovalRequired" ? (
          <Typography variant="body2" color="text.secondary">
            A confirmation code was sent to <b>{pending.emailHint}</b>. Enter it to trust this
            device.
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
                Security keys need a browser origin and are not available inside the desktop app yet
                — use another method.
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
          label={
            method === "backup_code" && pending.step === "mfaRequired" ? "Backup code" : "Code"
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
          <Button
            variant="contained"
            disabled={!code.trim() || busy}
            onClick={() => submit.mutate()}
          >
            Verify
          </Button>
        </Stack>
      </Stack>
    </SectionCard>
  );
}

// ───────────────────────────── signed in ─────────────────────────────

function SyncChip({ s }: { s: SyncStatus }) {
  switch (s.state) {
    case "syncing":
      return <Chip size="small" color="primary" icon={<SyncRoundedIcon />} label="Syncing" />;
    case "offline":
      return <Chip size="small" icon={<CloudOffRoundedIcon />} label="Offline" />;
    case "error":
      return <Chip size="small" color="error" label="Error" />;
    case "idle":
      return (
        <Chip size="small" color="success" icon={<CloudDoneRoundedIcon />} label="Up to date" />
      );
  }
}

function SignedIn({
  status,
}: {
  status: AccountStatus & { account: NonNullable<AccountStatus["account"]> };
}) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const devices = useDevices(true);
  const [confirm, setConfirm] = useState<
    { kind: "none" } | { kind: "signOut" } | { kind: "revoke"; device: Device }
  >({
    kind: "none",
  });
  const a = status.account;
  const s = status.sync;

  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      invalidateAll(qc);
      setConfirm({ kind: "none" });
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  return (
    <Stack spacing={1.5} sx={{ maxWidth: 760 }}>
      <SectionCard>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
          <IconTile tone="accent">{(a.displayName ?? a.email).slice(0, 1).toUpperCase()}</IconTile>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="body1" noWrap sx={{ fontWeight: 500 }}>
              {a.displayName ?? a.email}
              {a.isAdmin && <Chip size="small" label="admin" sx={{ ml: 1 }} />}
            </Typography>
            <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
              {a.email} · <Mono>{a.serverUrl}</Mono> · signed in{" "}
              {new Date(a.signedInAt).toLocaleDateString()}
            </Typography>
          </Box>
          <Button
            color="inherit"
            startIcon={<LogoutRoundedIcon />}
            onClick={() => setConfirm({ kind: "signOut" })}
          >
            Sign out
          </Button>
        </Box>
      </SectionCard>

      <SectionCard
        title="Sync"
        action={
          <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
            <SyncChip s={s} />
            {s.realtime && (
              <Tooltip title="Realtime channel connected: changes from other devices arrive instantly">
                <BoltRoundedIcon fontSize="small" color="primary" />
              </Tooltip>
            )}
            <Button
              variant="tonal"
              startIcon={<SyncRoundedIcon />}
              disabled={op.isPending || s.state === "syncing"}
              onClick={() =>
                op.mutate(async () => {
                  const r = await ipc.accountSyncNow();
                  return r.lastError ?? `Synced: ${r.pushed} pushed, ${r.pulled} pulled`;
                })
              }
            >
              Sync now
            </Button>
          </Stack>
        }
      >
        {s.lastError && <Alert severity="error">{s.lastError}</Alert>}
        <SettingRow
          label="Last sync"
          control={
            <Value>{s.lastSyncAt ? new Date(s.lastSyncAt).toLocaleString() : "never"}</Value>
          }
        />
        <SettingRow label="Pushed" control={<Value>{String(s.pushed)}</Value>} />
        <SettingRow label="Pulled" control={<Value>{String(s.pulled)}</Value>} />
        <SettingRow label="Conflicts" last control={<Value>{String(s.conflicts)}</Value>} />
      </SectionCard>

      <SectionCard title="Vaults on this device">
        <Stack spacing={1}>
          {status.vaults.map((v) => (
            <EntityCard
              key={v.id}
              dense
              sx={{ bgcolor: "surface.highest", "&:hover": { bgcolor: "surface.highest" } }}
              tile={
                <IconTile size={sizes.tileSmall} tone={v.unlocked ? "neutral" : "warning"}>
                  {v.unlocked ? <LockOpenRoundedIcon /> : <LockOutlinedIcon />}
                </IconTile>
              }
              title={v.name}
              subtitle={`${v.kind} · ${v.role}`}
              trailing={!v.unlocked && <Chip size="small" color="warning" label="Locked" />}
            />
          ))}
        </Stack>
      </SectionCard>

      <SectionCard title="Devices">
        {devices.isPending ? (
          <Loading pt={2} />
        ) : devices.error ? (
          <Typography variant="body2" color="error">
            {errorMessage(devices.error)}
          </Typography>
        ) : (
          <Stack spacing={1}>
            {devices.data.map((d) => (
              <EntityCard
                key={d.id}
                dense
                sx={{ bgcolor: "surface.highest", "&:hover": { bgcolor: "surface.highest" } }}
                tile={
                  <IconTile size={sizes.tileSmall}>
                    <DevicesRoundedIcon />
                  </IconTile>
                }
                title={
                  <>
                    {d.name}
                    {d.current && <Chip size="small" label="this device" sx={{ ml: 1 }} />}
                  </>
                }
                subtitle={
                  <>
                    {d.platform} · v{d.app_version} · seen{" "}
                    {new Date(d.last_seen_at).toLocaleString()}
                    {d.last_ip && (
                      <>
                        {" · "}
                        <Mono>{d.last_ip}</Mono>
                      </>
                    )}
                  </>
                }
                actions={
                  !d.current && (
                    <ToolIconButton
                      title="Revoke this device"
                      onClick={() => setConfirm({ kind: "revoke", device: d })}
                    >
                      <DeleteOutlineRoundedIcon fontSize="small" />
                    </ToolIconButton>
                  )
                }
              />
            ))}
          </Stack>
        )}
      </SectionCard>

      {confirm.kind === "signOut" && (
        <ConfirmDialog
          open
          title="Sign out?"
          confirmLabel="Sign out"
          danger
          busy={op.isPending}
          onCancel={() => setConfirm({ kind: "none" })}
          onConfirm={() =>
            op.mutate(async () => {
              await ipc.accountSignOut();
              return "Signed out";
            })
          }
        >
          Synced vaults and their keys are removed from this device; your local vault stays. Data on
          the server is untouched and comes back when you sign in again.
        </ConfirmDialog>
      )}
      {confirm.kind === "revoke" && (
        <ConfirmDialog
          open
          title="Revoke device?"
          confirmLabel="Revoke"
          danger
          busy={op.isPending}
          onCancel={() => setConfirm({ kind: "none" })}
          onConfirm={() => {
            const id = confirm.device.id;
            op.mutate(async () => {
              await ipc.accountDeviceRevoke(id);
              return "Device revoked";
            });
          }}
        >
          <b>{confirm.device.name}</b> will be signed out and must log in again to sync.
        </ConfirmDialog>
      )}
    </Stack>
  );
}

function Value({ children }: { children: ReactNode }) {
  return (
    <Typography variant="body2" color="text.secondary">
      {children}
    </Typography>
  );
}

// ───────────────────────────── page ─────────────────────────────

export function AccountPage() {
  const qc = useQueryClient();
  const status = useAccount();
  const [local, setLocal] = useState<{ at: number; value: LoginOutcome | null } | null>(null);
  const pending = local?.at === status.dataUpdatedAt ? local.value : (status.data?.pending ?? null);

  const onOutcome = (o: LoginOutcome | null) => {
    setLocal({ at: status.dataUpdatedAt, value: o?.step === "done" ? null : o });
    if (o?.step === "done" || o === null) invalidateAll(qc);
    else void qc.invalidateQueries({ queryKey: keys.account });
  };

  return (
    <PageBody>
      {status.isPending ? (
        <Loading />
      ) : status.error ? (
        <Alert severity="error">{errorMessage(status.error)}</Alert>
      ) : status.data.account ? (
        <SignedIn status={{ ...status.data, account: status.data.account }} />
      ) : pending && pending.step !== "done" ? (
        <PendingCard key={pending.step} pending={pending} onOutcome={onOutcome} />
      ) : (
        <SignInCard onOutcome={onOutcome} />
      )}
    </PageBody>
  );
}
