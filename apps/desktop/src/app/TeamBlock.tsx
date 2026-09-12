import { useState } from "react";
import {
  Box,
  ButtonBase,
  Chip,
  CircularProgress,
  Divider,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Popover,
  Tooltip,
  Typography,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import PersonOutlineRoundedIcon from "@mui/icons-material/PersonOutlineRounded";
import PersonAddAltRoundedIcon from "@mui/icons-material/PersonAddAltRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import OpenInNewRoundedIcon from "@mui/icons-material/OpenInNewRounded";
import CloudDoneRoundedIcon from "@mui/icons-material/CloudDoneRounded";
import CloudOffRoundedIcon from "@mui/icons-material/CloudOffRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import ManageAccountsRoundedIcon from "@mui/icons-material/ManageAccountsRounded";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import LoginRoundedIcon from "@mui/icons-material/LoginRounded";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { keys, useAccount, useVaultMembers } from "@/ipc/hooks";
import { useActiveVault } from "./vault";
import * as ipc from "@/ipc/commands";
import { errorMessage, type AccountStatus, type VaultMember, type VaultRole } from "@/ipc/types";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import { goToSettings } from "./navigation";

const BLOCK = 28;

const serverBase = (url: string) => url.replace(/\/+$/, "");

function initialsOf(name: string | null | undefined, email: string): string {
  const n = name?.trim();
  if (n) {
    return n
      .split(/\s+/)
      .slice(0, 2)
      .map((p) => p[0] ?? "")
      .join("")
      .toUpperCase();
  }
  return email.slice(0, 1).toUpperCase();
}

const roleLabel: Record<VaultRole, string> = {
  viewer: "Viewer",
  editor: "Editor",
  manager: "Manager",
};

/**
 * Right end of the top bar, as in Termius: the square avatar of the signed-in
 * user and a glued `+` that is about *people* — who has access to the current
 * vault and inviting more of them. Creating hosts lives on the Hosts toolbar.
 */
export function TeamBlock() {
  return (
    <Box sx={{ display: "flex", alignItems: "center" }}>
      <AccountAvatar />
      <TeamButton />
    </Box>
  );
}

function AvatarTile({
  label,
  size = BLOCK,
  signedIn,
}: {
  label: string;
  size?: number;
  signedIn: boolean;
}) {
  return (
    <Box
      sx={{
        width: size,
        height: size,
        borderRadius: size >= BLOCK ? "7px" : "6px",
        display: "grid",
        placeItems: "center",
        fontSize: Math.round(size * 0.42),
        fontWeight: 600,
        letterSpacing: "0.02em",
        bgcolor: signedIn ? "primary.dark" : "surface.highest",
        color: signedIn ? "primary.contrastText" : "text.secondary",
        flexShrink: 0,
      }}
    >
      {signedIn ? label : <PersonOutlineRoundedIcon sx={{ fontSize: Math.round(size * 0.6) }} />}
    </Box>
  );
}

function syncLine(a: AccountStatus): string {
  const s = a.sync;
  switch (s.state) {
    case "syncing":
      return "Syncing…";
    case "offline":
      return "Server unreachable · working offline";
    case "error":
      return s.lastError ?? "Sync error";
    case "idle":
      return s.lastSyncAt
        ? `Synced ${new Date(s.lastSyncAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}${s.realtime ? " · live" : ""}`
        : "Signed in";
  }
}

function SyncIcon({ a }: { a: AccountStatus }) {
  switch (a.sync.state) {
    case "syncing":
      return <CircularProgress size={16} thickness={5} />;
    case "offline":
      return <CloudOffRoundedIcon fontSize="small" />;
    case "error":
      return <ErrorOutlineRoundedIcon fontSize="small" color="error" />;
    case "idle":
      return <CloudDoneRoundedIcon fontSize="small" color="primary" />;
  }
}

/** The avatar: who is signed in, sync state, quick sign-in / out. */
function AccountAvatar() {
  const { data } = useAccount();
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [confirmOut, setConfirmOut] = useState(false);
  const account = data?.account ?? null;

  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      void qc.invalidateQueries({ queryKey: keys.account });
      setConfirmOut(false);
      if (msg) snackbar.notify(msg, "success");
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const close = () => setAnchor(null);
  const go = (fn: () => void) => () => {
    close();
    fn();
  };

  return (
    <>
      <Tooltip title={account ? account.email : "Not signed in"}>
        <ButtonBase
          onClick={(e) => setAnchor(e.currentTarget)}
          aria-label="Account"
          sx={{
            position: "relative",
            zIndex: 1,
            borderRadius: "7px",
            boxShadow: "0 0 0 2px var(--mui-palette-primary-main)",
            transition: "box-shadow 120ms",
            "&:hover": { boxShadow: "0 0 0 2px var(--mui-palette-primary-light)" },
          }}
        >
          <AvatarTile
            signedIn={Boolean(account)}
            label={account ? initialsOf(account.displayName, account.email) : ""}
          />
        </ButtonBase>
      </Tooltip>
      <Menu
        open={Boolean(anchor)}
        anchorEl={anchor}
        onClose={close}
        slotProps={{ paper: { sx: { minWidth: 260 } } }}
      >
        {account && data
          ? [
              <Box key="who" sx={{ px: 2, pt: 1, pb: 1.25 }}>
                <Typography variant="body2" noWrap sx={{ fontWeight: 600 }}>
                  {account.displayName ?? account.email}
                </Typography>
                {account.displayName && (
                  <Typography
                    variant="caption"
                    color="text.secondary"
                    noWrap
                    sx={{ display: "block" }}
                  >
                    {account.email}
                  </Typography>
                )}
                <Typography
                  variant="caption"
                  color="text.disabled"
                  noWrap
                  sx={{ display: "block" }}
                >
                  {serverBase(account.serverUrl).replace(/^https?:\/\//, "")}
                </Typography>
              </Box>,
              <Divider key="d1" />,
              <MenuItem
                key="sync"
                disabled={data.sync.state === "syncing" || op.isPending}
                onClick={() => {
                  close();
                  op.mutate(async () => {
                    const r = await ipc.accountSyncNow();
                    return r.state === "error" ? null : "Synced";
                  });
                }}
              >
                <ListItemIcon>
                  <SyncIcon a={data} />
                </ListItemIcon>
                <ListItemText primary="Sync now" secondary={syncLine(data)} />
              </MenuItem>,
              <MenuItem key="settings" onClick={go(() => goToSettings("account"))}>
                <ListItemIcon>
                  <ManageAccountsRoundedIcon fontSize="small" />
                </ListItemIcon>
                <ListItemText primary="Account settings" />
              </MenuItem>,
              <Divider key="d2" />,
              <MenuItem key="out" onClick={go(() => setConfirmOut(true))}>
                <ListItemIcon>
                  <LogoutRoundedIcon fontSize="small" />
                </ListItemIcon>
                <ListItemText primary="Sign out" />
              </MenuItem>,
            ]
          : [
              <Box key="who" sx={{ px: 2, pt: 1, pb: 1.25 }}>
                <Typography variant="body2" sx={{ fontWeight: 600 }}>
                  Not signed in
                </Typography>
                <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
                  Everything is stored in the local vault on this device.
                </Typography>
              </Box>,
              <Divider key="d1" />,
              <MenuItem key="in" onClick={go(() => goToSettings("account"))}>
                <ListItemIcon>
                  <LoginRoundedIcon fontSize="small" />
                </ListItemIcon>
                <ListItemText primary="Sign in" secondary="Termoso Cloud or your own server" />
              </MenuItem>,
            ]}
      </Menu>
      <ConfirmDialog
        open={confirmOut}
        title="Sign out?"
        confirmLabel="Sign out"
        danger
        busy={op.isPending}
        onCancel={() => setConfirmOut(false)}
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
    </>
  );
}

/** `+` glued to the avatar: invite people and see who shares the current vault. */
function TeamButton() {
  const { data } = useAccount();
  const vault = useActiveVault();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const account = data?.account ?? null;
  const v = vault.data;
  const teamVaultId = account && v?.kind === "team" ? v.id : null;
  const members = useVaultMembers(anchor ? teamVaultId : null);
  const close = () => setAnchor(null);

  const inviteUrl = account
    ? v?.team_id
      ? `${serverBase(account.serverUrl)}/team/${v.team_id}`
      : `${serverBase(account.serverUrl)}/team`
    : null;

  return (
    <>
      <Tooltip title="Team">
        <ButtonBase
          onClick={(e) => setAnchor(e.currentTarget)}
          aria-label="Team"
          sx={{
            width: BLOCK + 8,
            height: BLOCK,
            ml: -0.75,
            pl: 0.75,
            borderRadius: "0 7px 7px 0",
            bgcolor: "surface.highest",
            color: "text.primary",
            transition: "background-color 120ms",
            "&:hover": { bgcolor: "border.strong" },
          }}
        >
          <AddRoundedIcon sx={{ fontSize: 20 }} />
        </ButtonBase>
      </Tooltip>
      <Popover
        open={Boolean(anchor)}
        anchorEl={anchor}
        onClose={close}
        anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
        transformOrigin={{ vertical: -6, horizontal: "right" }}
        slotProps={{ paper: { sx: { width: 320, p: 0.5 } } }}
      >
        {account ? (
          <>
            <List dense disablePadding>
              <ListItemButton
                onClick={() => {
                  close();
                  if (inviteUrl) void openUrl(inviteUrl);
                }}
                sx={{ borderRadius: 1.5, gap: 1.25 }}
              >
                <ListItemIcon sx={{ minWidth: 0 }}>
                  <PersonAddAltRoundedIcon fontSize="small" />
                </ListItemIcon>
                <ListItemText
                  primary="Invite team members"
                  secondary="Opens your account on the server"
                  slotProps={{ primary: { variant: "body2", sx: { fontWeight: 600 } } }}
                />
                <OpenInNewRoundedIcon sx={{ fontSize: 14, color: "text.disabled" }} />
              </ListItemButton>
            </List>
            <Divider sx={{ my: 0.5 }} />
            <Box sx={{ px: 1.5, pt: 0.75, pb: 0.5 }}>
              <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
                {v ? `${v.name} · ${v.kind === "team" ? "team vault" : "only you"}` : "Vault"}
              </Typography>
            </Box>
            {teamVaultId ? (
              <MemberList
                members={members.data}
                pending={members.isPending}
                error={members.error ? errorMessage(members.error) : null}
                selfId={account.userId}
              />
            ) : (
              <MemberRow
                name={account.displayName ?? null}
                email={account.email}
                role={null}
                self
              />
            )}
          </>
        ) : (
          <Box sx={{ px: 1.5, py: 1.25 }}>
            <Box sx={{ display: "flex", alignItems: "center", gap: 1, mb: 0.75 }}>
              <LockOutlinedIcon fontSize="small" sx={{ color: "text.secondary" }} />
              <Typography variant="body2" sx={{ fontWeight: 600 }}>
                Share with your team
              </Typography>
            </Box>
            <Typography variant="caption" color="text.secondary" sx={{ display: "block", mb: 1 }}>
              Sign in to invite people and share encrypted vaults — end-to-end, the server never
              sees your hosts or keys.
            </Typography>
            <ListItemButton
              onClick={() => {
                close();
                goToSettings("account");
              }}
              sx={{ borderRadius: 1.5, gap: 1.25, mx: -0.5 }}
            >
              <ListItemIcon sx={{ minWidth: 0 }}>
                <LoginRoundedIcon fontSize="small" />
              </ListItemIcon>
              <ListItemText
                primary="Sign in"
                slotProps={{ primary: { variant: "body2", sx: { fontWeight: 600 } } }}
              />
            </ListItemButton>
          </Box>
        )}
      </Popover>
    </>
  );
}

function MemberList({
  members,
  pending,
  error,
  selfId,
}: {
  members: VaultMember[] | undefined;
  pending: boolean;
  error: string | null;
  selfId: string;
}) {
  if (pending && !members) {
    return (
      <Box sx={{ display: "flex", justifyContent: "center", py: 1.5 }}>
        <CircularProgress size={18} thickness={5} />
      </Box>
    );
  }
  if (error) {
    return (
      <Typography variant="caption" color="error" sx={{ display: "block", px: 1.5, pb: 1 }}>
        {error}
      </Typography>
    );
  }
  const list = [...(members ?? [])].sort((a, b) =>
    a.user_id === selfId ? -1 : b.user_id === selfId ? 1 : a.email.localeCompare(b.email),
  );
  return (
    <Box sx={{ maxHeight: 260, overflowY: "auto" }}>
      {list.map((m) => (
        <MemberRow
          key={m.user_id}
          name={m.display_name ?? null}
          email={m.email}
          role={m.role}
          pending={m.pending}
          self={m.user_id === selfId}
        />
      ))}
    </Box>
  );
}

function MemberRow({
  name,
  email,
  role,
  pending,
  self,
}: {
  name: string | null;
  email: string;
  role: VaultRole | null;
  pending?: boolean;
  self?: boolean;
}) {
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1.25, px: 1.5, py: 0.75 }}>
      <AvatarTile size={24} signedIn label={initialsOf(name, email)} />
      <Box sx={{ minWidth: 0, flex: 1 }}>
        <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
          {name ?? email}
          {self ? (
            <Box component="span" sx={{ color: "text.disabled", fontWeight: 400 }}>
              {" "}
              · you
            </Box>
          ) : null}
        </Typography>
        {name && (
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {email}
          </Typography>
        )}
      </Box>
      {pending ? (
        <Chip size="small" label="Pending key" color="warning" variant="outlined" />
      ) : role ? (
        <Chip size="small" label={roleLabel[role]} variant="outlined" />
      ) : null}
    </Box>
  );
}
