import { useState, type ReactNode } from "react";
import { Alert, Box, Button, Chip, Stack, Tooltip, Typography } from "@mui/material";
import SyncRoundedIcon from "@mui/icons-material/SyncRounded";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import CloudDoneRoundedIcon from "@mui/icons-material/CloudDoneRounded";
import CloudOffRoundedIcon from "@mui/icons-material/CloudOffRounded";
import BoltRoundedIcon from "@mui/icons-material/BoltRounded";
import DevicesRoundedIcon from "@mui/icons-material/DevicesRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import LockOpenRoundedIcon from "@mui/icons-material/LockOpenRounded";
import WorkspacePremiumRoundedIcon from "@mui/icons-material/WorkspacePremiumRounded";
import BackupRoundedIcon from "@mui/icons-material/BackupRounded";
import SettingsBackupRestoreRoundedIcon from "@mui/icons-material/SettingsBackupRestoreRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { PageBody } from "@/components/PageHeader";
import {
  EntityCard,
  IconTile,
  Loading,
  Mono,
  SectionCard,
  SettingRow,
  ToolIconButton,
} from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { useAccount, useDevices } from "@/ipc/hooks";
import { sizes } from "@/theme/theme";
import {
  errorMessage,
  type AccountStatus,
  type Device,
  type LoginOutcome,
  type SyncStatus,
} from "@/ipc/types";
import { PendingForm, SignInForm, invalidateAll, pendingTitle, usePendingLogin } from "./SignIn";
import { BackupDialog, pickBackupFile, type BackupMode } from "./BackupDialog";

// ───────────────────────────── plan ─────────────────────────────

function PlanCard({ account }: { account: AccountStatus["account"] | undefined }) {
  const server = account?.serverUrl.replace(/\/+$/, "");
  return (
    <SectionCard>
      <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
        <IconTile tone="accent">
          <WorkspacePremiumRoundedIcon />
        </IconTile>
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography variant="body1" sx={{ fontWeight: 500 }}>
            Free · {server ? "Self-hosted" : "Offline"}
          </Typography>
          <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
            {server ? (
              <>
                Every feature, unlimited devices, your own server at <Mono>{server}</Mono>.
              </>
            ) : (
              "Every feature on this device. Sign in to a self-hosted server to sync between devices."
            )}{" "}
            No telemetry, ever.
          </Typography>
        </Box>
        <Chip size="small" color="success" label="Open source" />
      </Box>
    </SectionCard>
  );
}

// ───────────────────────────── backup ─────────────────────────────

function BackupCard() {
  const [mode, setMode] = useState<BackupMode | null>(null);
  return (
    <SectionCard title="Backup">
      <SettingRow
        label="Export encrypted backup"
        hint="All unlocked vaults in one password-protected .termoso file. Works offline, no account needed."
        control={
          <Button
            variant="tonal"
            startIcon={<BackupRoundedIcon />}
            onClick={() => setMode("export")}
          >
            Export…
          </Button>
        }
      />
      <SettingRow
        label="Restore from backup"
        hint="Merges a .termoso file into a vault of this device; nothing is deleted."
        last
        control={
          <Button
            variant="tonal"
            startIcon={<SettingsBackupRestoreRoundedIcon />}
            onClick={() =>
              void pickBackupFile().then((path) => {
                if (path) setMode({ kind: "restore", path });
              })
            }
          >
            Restore…
          </Button>
        }
      />
      <BackupDialog
        key={mode === null ? "closed" : typeof mode === "string" ? mode : mode.path}
        mode={mode}
        onClose={() => setMode(null)}
      />
    </SectionCard>
  );
}

// ───────────────────────────── signed out ─────────────────────────────

function SignInCard({ onOutcome }: { onOutcome: (o: LoginOutcome) => void }) {
  return (
    <SectionCard title="Sign in to sync" sx={{ maxWidth: 480 }}>
      <Typography variant="body2" color="text.secondary">
        Sync is optional. When you sign in, your vaults are encrypted on this device before they
        leave it — the server only ever sees ciphertext.
      </Typography>
      <SignInForm onOutcome={onOutcome} />
    </SectionCard>
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
  return (
    <SectionCard title={pendingTitle(pending)} sx={{ maxWidth: 480 }}>
      <PendingForm pending={pending} onOutcome={onOutcome} />
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
    <>
      <SectionCard>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
          <IconTile tone="accent">{(a.displayName ?? a.email).slice(0, 1).toUpperCase()}</IconTile>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="body1" noWrap sx={{ fontWeight: 500 }}>
              {a.displayName ?? a.email}
              {a.isAdmin && <Chip size="small" label="admin" sx={{ ml: 1 }} />}
            </Typography>
            <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
              {a.email} · <Mono>{a.serverUrl.replace(/\/+$/, "")}</Mono> · signed in{" "}
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
    </>
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
  const status = useAccount();
  const { pending, onOutcome } = usePendingLogin(status);

  return (
    <PageBody>
      <Stack spacing={1.5} sx={{ maxWidth: 760 }}>
        {status.isPending ? (
          <Loading />
        ) : (
          <>
            <PlanCard account={status.data?.account} />
            {status.error ? (
              <Alert severity="error">{errorMessage(status.error)}</Alert>
            ) : status.data.account ? (
              <SignedIn status={{ ...status.data, account: status.data.account }} />
            ) : pending ? (
              <PendingCard key={pending.step} pending={pending} onOutcome={onOutcome} />
            ) : (
              <SignInCard onOutcome={onOutcome} />
            )}
            <BackupCard />
          </>
        )}
      </Stack>
    </PageBody>
  );
}
