import { useState } from "react";
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
  Divider,
  IconButton,
  Paper,
  Stack,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tabs,
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
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useAccount, useDevices } from "@/ipc/hooks";
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
    <Paper variant="outlined" sx={{ p: 3, maxWidth: 480 }}>
      <Typography variant="h6" gutterBottom>
        Connect to a Termoso server
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        Sync is optional. When you sign in, your vaults are encrypted on this device before they
        leave it — the server only ever sees ciphertext.
      </Typography>
      <Stack spacing={2}>
        <TextField
          label="Server URL"
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
        <Tabs
          value={tab}
          onChange={(_, v: "login" | "register") => setTab(v)}
          sx={{ minHeight: 36, "& .MuiTab-root": { minHeight: 36 } }}
        >
          <Tab value="login" label="Sign in" />
          <Tab
            value="register"
            label="Create account"
            disabled={info.data !== undefined && !info.data.registration_open && !invite}
          />
        </Tabs>
        <TextField
          label="Email"
          type="email"
          autoComplete="username"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />
        {tab === "register" && (
          <TextField
            label="Display name (optional)"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
        )}
        <TextField
          label="Password"
          type="password"
          autoComplete={tab === "login" ? "current-password" : "new-password"}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          helperText={
            tab === "register" ? "At least 12 characters. It never leaves this device." : undefined
          }
          onKeyDown={(e) => {
            if (e.key === "Enter" && canSubmit && !busy) {
              if (tab === "login") login.mutate();
              else register.mutate();
            }
          }}
        />
        {tab === "register" && info.data && !info.data.registration_open && (
          <TextField
            label="Invite token"
            value={invite}
            onChange={(e) => setInvite(e.target.value)}
            helperText="This server only accepts invited users."
          />
        )}
        <Button
          variant="contained"
          size="large"
          disabled={!canSubmit || busy}
          onClick={() => (tab === "login" ? login.mutate() : register.mutate())}
        >
          {busy ? (
            <CircularProgress size={22} color="inherit" />
          ) : tab === "login" ? (
            "Sign in"
          ) : (
            "Create account"
          )}
        </Button>
      </Stack>

      {phrase !== null && <RecoveryDialog phrase={phrase} onDone={() => setPhrase(null)} />}
    </Paper>
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
        <Paper
          variant="outlined"
          sx={{
            p: 2,
            fontFamily: "monospace",
            fontSize: 15,
            lineHeight: 1.8,
            userSelect: "all",
            wordSpacing: 6,
          }}
        >
          {phrase}
        </Paper>
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
    <Paper variant="outlined" sx={{ p: 3, maxWidth: 480 }}>
      <Typography variant="h6" gutterBottom>
        {pending.step === "deviceApprovalRequired"
          ? "Approve this device"
          : "Two-factor verification"}
      </Typography>
      <Stack spacing={2}>
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
                size="small"
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
                variant="outlined"
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
        <TextField
          autoFocus
          label={
            method === "backup_code" && pending.step === "mfaRequired" ? "Backup code" : "Code"
          }
          value={code}
          onChange={(e) => setCode(e.target.value)}
          autoComplete="one-time-code"
          slotProps={{ htmlInput: { style: { fontFamily: "monospace", letterSpacing: 2 } } }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && code.trim() && !busy) submit.mutate();
          }}
        />
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
    </Paper>
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
    <Stack spacing={2.5} sx={{ maxWidth: 760, mt: 2 }}>
      <Paper variant="outlined" sx={{ p: 2.5 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 2 }}>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="h6" noWrap>
              {a.displayName ?? a.email}
              {a.isAdmin && <Chip size="small" label="admin" sx={{ ml: 1 }} />}
            </Typography>
            <Typography variant="body2" color="text.secondary" noWrap>
              {a.email} · {a.serverUrl}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              Signed in {new Date(a.signedInAt).toLocaleString()}
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
      </Paper>

      <Paper variant="outlined" sx={{ p: 2.5 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1.5, mb: 1.5 }}>
          <Typography variant="h6" sx={{ flex: 1 }}>
            Sync
          </Typography>
          <SyncChip s={s} />
          {s.realtime && (
            <Tooltip title="Realtime channel connected: changes from other devices arrive instantly">
              <BoltRoundedIcon fontSize="small" color="primary" />
            </Tooltip>
          )}
          <Button
            size="small"
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
        </Box>
        {s.lastError && (
          <Alert severity="error" sx={{ mb: 1.5 }}>
            {s.lastError}
          </Alert>
        )}
        <Stack direction="row" spacing={4}>
          <Stat
            k="Last sync"
            v={s.lastSyncAt ? new Date(s.lastSyncAt).toLocaleString() : "never"}
          />
          <Stat k="Pushed" v={String(s.pushed)} />
          <Stat k="Pulled" v={String(s.pulled)} />
          <Stat k="Conflicts" v={String(s.conflicts)} />
        </Stack>
        <Divider sx={{ my: 2 }} />
        <Typography variant="overline" color="text.secondary">
          Vaults on this device
        </Typography>
        <Stack direction="row" spacing={1} sx={{ flexWrap: "wrap", gap: 1 }}>
          {status.vaults.map((v) => (
            <Chip
              key={v.id}
              variant="outlined"
              label={`${v.name} · ${v.kind}${v.unlocked ? "" : " · locked"} · ${v.role}`}
              color={v.unlocked ? "default" : "warning"}
            />
          ))}
        </Stack>
      </Paper>

      <Paper variant="outlined" sx={{ p: 2.5 }}>
        <Typography variant="h6" sx={{ mb: 1 }}>
          Devices
        </Typography>
        {devices.isPending ? (
          <CircularProgress size={22} />
        ) : devices.error ? (
          <Typography variant="body2" color="error">
            {errorMessage(devices.error)}
          </Typography>
        ) : (
          <Table size="small">
            <TableHead>
              <TableRow sx={{ "& th": { color: "text.secondary", fontWeight: 600 } }}>
                <TableCell>Name</TableCell>
                <TableCell>Platform</TableCell>
                <TableCell>Version</TableCell>
                <TableCell>Last seen</TableCell>
                <TableCell padding="checkbox" />
              </TableRow>
            </TableHead>
            <TableBody>
              {devices.data.map((d) => (
                <TableRow key={d.id} hover>
                  <TableCell>
                    {d.name}
                    {d.current && <Chip size="small" label="this device" sx={{ ml: 1 }} />}
                  </TableCell>
                  <TableCell>{d.platform}</TableCell>
                  <TableCell>{d.app_version}</TableCell>
                  <TableCell>
                    {new Date(d.last_seen_at).toLocaleString()}
                    {d.last_ip && (
                      <Typography
                        component="span"
                        variant="caption"
                        color="text.secondary"
                        sx={{ ml: 1 }}
                      >
                        {d.last_ip}
                      </Typography>
                    )}
                  </TableCell>
                  <TableCell padding="checkbox">
                    {!d.current && (
                      <Tooltip title="Revoke this device">
                        <IconButton
                          size="small"
                          onClick={() => setConfirm({ kind: "revoke", device: d })}
                        >
                          <DeleteOutlineRoundedIcon fontSize="small" />
                        </IconButton>
                      </Tooltip>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </Paper>

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

function Stat({ k, v }: { k: string; v: string }) {
  return (
    <Box>
      <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
        {k}
      </Typography>
      <Typography variant="body2">{v}</Typography>
    </Box>
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
    <Page>
      <PageHeader
        title="Account"
        description="Optional end-to-end encrypted sync between your devices, on a server you choose."
      />
      <PageBody>
        {status.isPending ? (
          <Box sx={{ display: "flex", justifyContent: "center", pt: 8 }}>
            <CircularProgress size={28} />
          </Box>
        ) : status.error ? (
          <Alert severity="error" sx={{ mt: 2 }}>
            {errorMessage(status.error)}
          </Alert>
        ) : status.data.account ? (
          <SignedIn status={{ ...status.data, account: status.data.account }} />
        ) : (
          <Box sx={{ mt: 2 }}>
            {pending && pending.step !== "done" ? (
              <PendingCard key={pending.step} pending={pending} onOutcome={onOutcome} />
            ) : (
              <SignInCard onOutcome={onOutcome} />
            )}
          </Box>
        )}
      </PageBody>
    </Page>
  );
}
