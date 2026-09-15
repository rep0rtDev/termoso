import { useState, type ReactNode } from "react";
import {
  Box,
  ButtonBase,
  CircularProgress,
  Divider,
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
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import CloudDoneRoundedIcon from "@mui/icons-material/CloudDoneRounded";
import CloudOffRoundedIcon from "@mui/icons-material/CloudOffRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import ManageAccountsRoundedIcon from "@mui/icons-material/ManageAccountsRounded";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import LoginRoundedIcon from "@mui/icons-material/LoginRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { keys, useAccount, useTeamInvites, useTeamMembers, useTeams } from "@/ipc/hooks";
import { useActiveVault } from "./vault";
import * as ipc from "@/ipc/commands";
import {
  errorMessage,
  type AccountStatus,
  type TeamInvite,
  type TeamMember,
  type Uuid,
} from "@/ipc/types";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import { AVATAR as BLOCK, PersonAvatar, initialsOf } from "@/team/PersonAvatar";
import { isTeamAdmin, teamRoleLabel } from "@/team/roles";
import { goToSettings, goToSettingsWith } from "./navigation";

const serverBase = (url: string) => url.replace(/\/+$/, "");

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

/** Corner badge on the avatar: spinner while syncing, red on error, grey when the server is unreachable. */
function SyncBadge({ a }: { a: AccountStatus }) {
  const s = a.sync.state;
  if (s === "idle") return null;
  return (
    <Box
      sx={{
        position: "absolute",
        right: -4,
        bottom: -4,
        width: 14,
        height: 14,
        borderRadius: "50%",
        display: "grid",
        placeItems: "center",
        bgcolor: "surface.base",
        color: s === "error" ? "error.main" : "text.secondary",
        boxShadow: "0 0 0 1.5px var(--mui-palette-surface-base)",
        "& svg": { fontSize: 11 },
      }}
    >
      {s === "syncing" ? (
        <CircularProgress size={9} thickness={6} />
      ) : s === "error" ? (
        <ErrorOutlineRoundedIcon />
      ) : (
        <CloudOffRoundedIcon />
      )}
    </Box>
  );
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
      <Tooltip title={account && data ? `${account.email} · ${syncLine(data)}` : "Not signed in"}>
        <ButtonBase
          onClick={(e) => setAnchor(e.currentTarget)}
          aria-label="Account"
          sx={{
            position: "relative",
            zIndex: 10,
            borderRadius: "7px",
            boxShadow: `0 0 0 2px var(--mui-palette-${data?.sync.state === "error" ? "error" : "primary"}-main)`,
            transition: "box-shadow 120ms",
            "&:hover": { boxShadow: "0 0 0 2px var(--mui-palette-primary-light)" },
          }}
        >
          <PersonAvatar
            kind={account ? "account" : "guest"}
            label={account ? initialsOf(account.displayName, account.email) : ""}
            userId={account?.userId}
            avatar={account?.avatar}
          />
          {account && data && <SyncBadge a={data} />}
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

/**
 * Teammates glued to the avatar, as in Termius: up to two overlapping tiles for
 * other members / pending invitees of the current team, then a grey `+`.
 * Clicking any of them opens the team popover — invite, members, pending invites.
 */
function TeamButton() {
  const { data } = useAccount();
  const vault = useActiveVault();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const account = data?.account ?? null;
  const v = vault.data;
  const teams = useTeams(account !== null);
  const team =
    (v?.kind === "team" ? teams.data?.find((t) => t.id === v.team_id) : undefined) ??
    teams.data?.[0] ??
    null;
  const members = useTeamMembers(team?.id ?? null);
  const invites = useTeamInvites(team?.id ?? null, team !== null && isTeamAdmin(team.my_role));
  const close = () => setAnchor(null);

  const others: TeamMember[] = (members.data ?? []).filter((m) => m.user_id !== account?.userId);
  const pending: TeamInvite[] = invites.data ?? [];
  const stack: {
    key: string;
    email: string;
    name: string | null;
    invite: boolean;
    userId?: Uuid;
    avatar?: string | null;
  }[] = [
    ...others.map((m) => ({
      key: m.user_id,
      email: m.email,
      name: m.display_name,
      invite: false,
      userId: m.user_id,
      avatar: m.avatar,
    })),
    ...pending.map((i) => ({ key: i.id, email: i.email, name: null, invite: true })),
  ].slice(0, 2);

  return (
    <>
      <Tooltip title={team ? team.name : "Team"}>
        <ButtonBase
          onClick={(e) => setAnchor(e.currentTarget)}
          aria-label="Team"
          sx={{
            display: "flex",
            alignItems: "center",
            height: BLOCK,
            borderRadius: "0 7px 7px 0",
            "&:hover .team-plus": { bgcolor: "border.strong" },
          }}
        >
          {stack.map((p, i) => (
            <Box
              key={p.key}
              sx={{
                ml: -1,
                position: "relative",
                zIndex: stack.length - i,
                borderRadius: "7px",
                boxShadow: "0 0 0 2px var(--mui-palette-surface-base)",
                opacity: p.invite ? 0.85 : 1,
              }}
            >
              <PersonAvatar
                size={BLOCK}
                seed={p.email}
                kind={p.invite ? "invite" : "account"}
                label={initialsOf(p.name, p.email)}
                userId={p.userId}
                avatar={p.avatar}
              />
            </Box>
          ))}
          <Box
            className="team-plus"
            sx={{
              width: BLOCK + 8,
              height: BLOCK,
              ml: -1,
              pl: 1,
              display: "grid",
              placeItems: "center",
              borderRadius: "0 7px 7px 0",
              bgcolor: "surface.highest",
              color: "text.primary",
              transition: "background-color 120ms",
            }}
          >
            <AddRoundedIcon sx={{ fontSize: 20 }} />
          </Box>
        </ButtonBase>
      </Tooltip>
      <Popover
        open={Boolean(anchor)}
        anchorEl={anchor}
        onClose={close}
        anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
        transformOrigin={{ vertical: -6, horizontal: "right" }}
        slotProps={{ paper: { sx: { width: 300, p: 1 } } }}
      >
        {account ? (
          <>
            <ListItemButton
              onClick={() => {
                close();
                goToSettingsWith({ kind: "invite" });
              }}
              sx={{ borderRadius: 1.5, gap: 1.25, px: 1, py: 0.75 }}
            >
              <ListItemIcon sx={{ minWidth: 0 }}>
                <PersonAvatar size={28} kind="guest" label="" />
              </ListItemIcon>
              <ListItemText
                primary="Invite team members"
                slotProps={{ primary: { variant: "body2", sx: { fontWeight: 500 } } }}
              />
            </ListItemButton>
            <MemberRow
              name={account.displayName ?? null}
              email={account.email}
              userId={account.userId}
              avatar={account.avatar}
              trailing={
                team ? (
                  <Typography variant="caption" color="text.secondary">
                    {teamRoleLabel[team.my_role]}
                  </Typography>
                ) : undefined
              }
            />
            {others.map((m) => (
              <MemberRow
                key={m.user_id}
                name={m.display_name}
                email={m.email}
                userId={m.user_id}
                avatar={m.avatar}
                trailing={
                  <Typography variant="caption" color="text.secondary">
                    {teamRoleLabel[m.role]}
                  </Typography>
                }
              />
            ))}
            {members.isPending && team && (
              <Box sx={{ display: "flex", justifyContent: "center", py: 1 }}>
                <CircularProgress size={16} thickness={5} />
              </Box>
            )}
            {pending.length > 0 && (
              <>
                <Typography
                  variant="caption"
                  color="text.secondary"
                  sx={{ display: "block", px: 1, pt: 1, pb: 0.5 }}
                >
                  Pending invites:
                </Typography>
                {pending.map((i) => (
                  <MemberRow key={i.id} name={null} email={i.email} invite />
                ))}
              </>
            )}
            {!team && !teams.isPending && (
              <ListItemButton
                onClick={() => {
                  close();
                  goToSettings("team");
                }}
                sx={{ borderRadius: 1.5, gap: 1.25, px: 1, py: 0.75 }}
              >
                <ListItemIcon sx={{ minWidth: 0 }}>
                  <GroupsRoundedIcon fontSize="small" />
                </ListItemIcon>
                <ListItemText
                  primary="Create a team"
                  secondary="Share encrypted vaults with colleagues"
                  slotProps={{ primary: { variant: "body2", sx: { fontWeight: 500 } } }}
                />
              </ListItemButton>
            )}
          </>
        ) : (
          <Box sx={{ px: 1, py: 0.75 }}>
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

function MemberRow({
  name,
  email,
  invite,
  userId,
  avatar,
  trailing,
}: {
  name: string | null;
  email: string;
  invite?: boolean;
  userId?: Uuid;
  avatar?: string | null;
  trailing?: ReactNode;
}) {
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1.25, px: 1, py: 0.75 }}>
      <PersonAvatar
        size={28}
        seed={email}
        kind={invite ? "invite" : "account"}
        label={initialsOf(name, email)}
        userId={userId}
        avatar={avatar}
      />
      <Box sx={{ minWidth: 0, flex: 1 }}>
        <Typography variant="body2" noWrap>
          {name ?? email}
        </Typography>
        {name && (
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {email}
          </Typography>
        )}
      </Box>
      {trailing}
    </Box>
  );
}
