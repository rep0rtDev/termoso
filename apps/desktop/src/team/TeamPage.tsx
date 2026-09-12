import { useMemo, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import EditRoundedIcon from "@mui/icons-material/EditRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import LinkRoundedIcon from "@mui/icons-material/LinkRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import PersonAddAltRoundedIcon from "@mui/icons-material/PersonAddAltRounded";
import ReplayRoundedIcon from "@mui/icons-material/ReplayRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import ChevronRightRoundedIcon from "@mui/icons-material/ChevronRightRounded";
import { useMutation } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import {
  ActionMenu,
  EntityCard,
  Field,
  IconTile,
  InlineName,
  Loading,
  SectionCard,
  type MenuAction,
} from "@/components/ui";
import * as ipc from "@/ipc/commands";
import {
  useAccount,
  useInvalidateTeam,
  useTeamInvites,
  useTeamMembers,
  useTeamPendingKeys,
  useTeams,
  useVaults,
} from "@/ipc/hooks";
import {
  errorMessage,
  type InviteResult,
  type LocalVault,
  type PendingVaultKey,
  type Team,
  type TeamInvite,
  type TeamMember,
  type TeamRole,
  type Uuid,
} from "@/ipc/types";
import { goToSettings, goToSettingsWith, useSettingsIntent } from "@/app/navigation";
import { vaultIcon } from "@/app/vault";
import { InviteDialog, InviteResultRow } from "./InviteDialog";
import { PersonAvatar, initialsOf } from "./PersonAvatar";
import { isTeamAdmin, teamRoleHint, teamRoleLabel, vaultRoleLabel } from "./roles";

/**
 * Settings → Team, laid out like Termius: the team header with its actions,
 * the members table (active and pending), then the team vaults. Everything
 * that touches keys stays in Rust; this page only shows outcomes.
 */
export function TeamPage() {
  const account = useAccount();
  const signedIn = Boolean(account.data?.account);
  const teams = useTeams(signedIn);
  const [selected, setSelected] = useState<Uuid | null>(null);
  const [creating, setCreating] = useState(false);
  const [joining, setJoining] = useState(false);
  const [inviting, setInviting] = useState(false);

  const list = teams.data ?? [];
  const team = list.find((t) => t.id === selected) ?? list[0] ?? null;
  const myId = account.data?.account?.userId ?? null;

  useSettingsIntent(["invite"], () => {
    if (team && isTeamAdmin(team.my_role)) setInviting(true);
  });

  if (account.isPending || (signedIn && teams.isPending)) {
    return (
      <Page>
        <Loading />
      </Page>
    );
  }

  if (!signedIn) {
    return (
      <Page>
        <EmptyState
          icon={<GroupsRoundedIcon />}
          title="Teams live in your account"
          description="Sign in to Termoso Cloud or your own server to create a team, invite people and share vaults with them."
          action={
            <Button variant="contained" onClick={() => goToSettings("account")}>
              Go to Account & sync
            </Button>
          }
        />
      </Page>
    );
  }

  if (teams.error) {
    return (
      <Page>
        <PageBody>
          <Alert severity="error">{errorMessage(teams.error)}</Alert>
        </PageBody>
      </Page>
    );
  }

  return (
    <Page>
      <PageBody>
        {team ? (
          <TeamView
            team={team}
            teams={list}
            myId={myId}
            onSelect={setSelected}
            onCreate={() => setCreating(true)}
            onJoin={() => setJoining(true)}
            inviting={inviting}
            setInviting={setInviting}
          />
        ) : (
          <NoTeam onCreate={() => setCreating(true)} onJoin={() => setJoining(true)} />
        )}
      </PageBody>
      <CreateTeamDialog
        open={creating}
        onClose={() => setCreating(false)}
        onCreated={(t) => setSelected(t.id)}
      />
      <JoinTeamDialog
        open={joining}
        onClose={() => setJoining(false)}
        onJoined={(t) => setSelected(t.id)}
      />
    </Page>
  );
}

function NoTeam({ onCreate, onJoin }: { onCreate: () => void; onJoin: () => void }) {
  return (
    <Stack spacing={1.5}>
      <SectionCard>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
          <IconTile tone="accent" size={44}>
            <GroupsRoundedIcon />
          </IconTile>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="subtitle1">Work together in a team</Typography>
            <Typography variant="body2" color="text.secondary">
              A team owns shared vaults: hosts, keys, snippets and port-forwarding rules everyone
              you invite can use. Keys are sealed per member — the server never sees them.
            </Typography>
          </Box>
        </Box>
        <Box sx={{ display: "flex", gap: 1, mt: 2 }}>
          <Button variant="contained" startIcon={<AddRoundedIcon />} onClick={onCreate}>
            Create a team
          </Button>
          <Button variant="tonal" startIcon={<LinkRoundedIcon />} onClick={onJoin}>
            Join with an invitation link
          </Button>
        </Box>
      </SectionCard>
    </Stack>
  );
}

function TeamView({
  team,
  teams,
  myId,
  onSelect,
  onCreate,
  onJoin,
  inviting,
  setInviting,
}: {
  team: Team;
  teams: Team[];
  myId: Uuid | null;
  onSelect: (id: Uuid) => void;
  onCreate: () => void;
  onJoin: () => void;
  inviting: boolean;
  setInviting: (v: boolean) => void;
}) {
  const snackbar = useSnackbar();
  const invalidate = useInvalidateTeam();
  const admin = isTeamAdmin(team.my_role);
  const owner = team.my_role === "owner";
  const members = useTeamMembers(team.id);
  const invites = useTeamInvites(team.id, admin);
  const vaults = useVaults();
  const teamVaults = useMemo(
    () => (vaults.data ?? []).filter((v) => v.kind === "team" && v.team_id === team.id),
    [vaults.data, team.id],
  );
  const managed = teamVaults.filter((v) => v.role === "manager" && v.unlocked);
  const pending = useTeamPendingKeys(team.id, admin || managed.length > 0);

  const [menu, setMenu] = useState<HTMLElement | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [confirm, setConfirm] = useState<"leave" | "delete" | null>(null);
  const [resent, setResent] = useState<InviteResult[] | null>(null);

  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      invalidate();
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const menuItems: MenuAction[] = [
    {
      label: "Rename team",
      icon: <EditRoundedIcon fontSize="small" />,
      disabled: !admin,
      onClick: () => setRenaming(true),
    },
    { label: "Create another team", icon: <AddRoundedIcon fontSize="small" />, onClick: onCreate },
    {
      label: "Join with an invitation link",
      icon: <LinkRoundedIcon fontSize="small" />,
      onClick: onJoin,
      divider: true,
    },
    owner
      ? {
          label: "Delete team",
          icon: <DeleteOutlineRoundedIcon fontSize="small" />,
          danger: true,
          onClick: () => setConfirm("delete"),
        }
      : {
          label: "Leave team",
          icon: <LogoutRoundedIcon fontSize="small" />,
          danger: true,
          onClick: () => setConfirm("leave"),
        },
  ];

  const memberList = members.data ?? [];
  const inviteList = invites.data ?? [];
  const pendingList = pending.data ?? [];

  return (
    <Stack spacing={1.5}>
      <SectionCard>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
          <IconTile tone="accent" size={44}>
            <GroupsRoundedIcon />
          </IconTile>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            {renaming ? (
              <InlineName
                value={team.name}
                placeholder="Team name"
                onCancel={() => setRenaming(false)}
                onCommit={(name) => {
                  setRenaming(false);
                  const n = name.trim();
                  if (!n || n === team.name) return;
                  op.mutate(() => ipc.teamRename(team.id, n).then(() => "Team renamed"));
                }}
                sx={{ fontSize: 15, height: 28 }}
              />
            ) : (
              <Box sx={{ display: "flex", alignItems: "center", gap: 0.5, minWidth: 0 }}>
                <Typography variant="subtitle1" noWrap>
                  {team.name}
                </Typography>
                {admin && (
                  <Tooltip title="Rename">
                    <IconButton size="small" onClick={() => setRenaming(true)}>
                      <EditRoundedIcon sx={{ fontSize: 15 }} />
                    </IconButton>
                  </Tooltip>
                )}
              </Box>
            )}
            <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
              {team.member_count} {team.member_count === 1 ? "member" : "members"} · you are{" "}
              {teamRoleLabel[team.my_role].toLowerCase()}
              {teamVaults.length > 0 &&
                ` · ${teamVaults.length} ${teamVaults.length === 1 ? "vault" : "vaults"}`}
            </Typography>
          </Box>
          {teams.length > 1 && (
            <TextField
              select
              size="small"
              value={team.id}
              onChange={(e) => onSelect(e.target.value)}
              sx={{ width: 180 }}
            >
              {teams.map((t) => (
                <MenuItem key={t.id} value={t.id}>
                  {t.name}
                </MenuItem>
              ))}
            </TextField>
          )}
          {admin && (
            <Button
              variant="contained"
              startIcon={<PersonAddAltRoundedIcon />}
              onClick={() => setInviting(true)}
            >
              Invite members
            </Button>
          )}
          <IconButton onClick={(e) => setMenu(e.currentTarget)} aria-label="Team actions">
            <MoreHorizRoundedIcon fontSize="small" />
          </IconButton>
        </Box>
      </SectionCard>

      {pendingList.length > 0 && (
        <PendingKeys
          team={team}
          pending={pendingList}
          members={memberList}
          vaults={teamVaults}
          onGrant={(k) =>
            op.mutate(() =>
              ipc.teamVaultSetAccess(k.vault_id, k.user_id, k.role).then(() => "Key handed over"),
            )
          }
        />
      )}

      <SectionCard
        title="Members"
        description={
          admin
            ? "Change a role from the dropdown; remove someone with the cross. Removing a member rotates the keys of vaults they could open."
            : undefined
        }
      >
        {members.isPending ? (
          <Loading pt={2} />
        ) : (
          <Stack spacing={0.5}>
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: "1fr 150px 130px 36px",
                px: 1.5,
                py: 0.5,
                color: "text.secondary",
                typography: "caption",
              }}
            >
              <span>Member</span>
              <span>Role</span>
              <span>Status</span>
              <span />
            </Box>
            {memberList.map((m) => (
              <MemberRow
                key={m.user_id}
                team={team}
                m={m}
                me={m.user_id === myId}
                onRole={(role) =>
                  op.mutate(() =>
                    ipc
                      .teamMemberSetRole(team.id, m.user_id, role)
                      .then(() =>
                        role === "owner"
                          ? `${m.display_name ?? m.email} is now the owner`
                          : "Role updated",
                      ),
                  )
                }
                onRemove={() =>
                  op.mutate(() =>
                    ipc
                      .teamMemberRemove(team.id, m.user_id)
                      .then(() => `${m.display_name ?? m.email} removed from ${team.name}`),
                  )
                }
              />
            ))}
            {inviteList.map((inv) => (
              <InviteRow
                key={inv.id}
                inv={inv}
                onRevoke={() =>
                  op.mutate(() =>
                    ipc.teamInviteRevoke(team.id, inv.id).then(() => "Invitation revoked"),
                  )
                }
                onResend={() =>
                  op.mutate(async () => {
                    await ipc.teamInviteRevoke(team.id, inv.id);
                    const r = await ipc.teamInvite(team.id, [inv.email], inv.role, []);
                    setResent(r);
                    return null;
                  })
                }
              />
            ))}
          </Stack>
        )}
      </SectionCard>

      <SectionCard
        title="Team vaults"
        description="Each vault has its own key and its own list of people. Members connect with what is inside; managers decide who gets in."
        action={
          admin ? (
            <Button
              variant="tonal"
              size="small"
              startIcon={<AddRoundedIcon />}
              onClick={() => goToSettingsWith({ kind: "newVault", teamId: team.id })}
            >
              New vault
            </Button>
          ) : undefined
        }
      >
        {vaults.isPending ? (
          <Loading pt={2} />
        ) : teamVaults.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            {admin
              ? "No vaults yet — create one to start sharing hosts and keys."
              : "You have not been given access to any vault of this team yet."}
          </Typography>
        ) : (
          <Stack spacing={0.75}>
            {teamVaults.map((v) => (
              <EntityCard
                key={v.id}
                dense
                tile={
                  <IconTile tone={v.unlocked ? "accent" : "neutral"} size={32}>
                    {vaultIcon(v)}
                  </IconTile>
                }
                title={v.name}
                subtitle={
                  v.unlocked
                    ? `You ${vaultRoleLabel[v.role]}`
                    : "Locked on this device — waiting for a manager to hand you the key"
                }
                trailing={
                  <ChevronRightRoundedIcon fontSize="small" sx={{ color: "text.disabled" }} />
                }
                onClick={() => goToSettingsWith({ kind: "vault", id: v.id })}
              />
            ))}
          </Stack>
        )}
      </SectionCard>

      <ActionMenu anchor={menu} onClose={() => setMenu(null)} items={menuItems} />
      <InviteDialog
        team={team}
        vaults={managed}
        open={inviting}
        onClose={() => setInviting(false)}
      />
      <ResentDialog results={resent} onClose={() => setResent(null)} />
      <ConfirmDialog
        open={confirm === "leave"}
        title={`Leave ${team.name}?`}
        confirmLabel="Leave team"
        danger
        busy={op.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={() =>
          op.mutate(() => ipc.teamLeave(team.id).then(() => `You left ${team.name}`), {
            onSuccess: () => setConfirm(null),
          })
        }
      >
        Its vaults disappear from this device and you lose access to everything inside. Managers of
        those vaults can rotate their keys afterwards.
      </ConfirmDialog>
      <ConfirmDialog
        open={confirm === "delete"}
        title={`Delete ${team.name}?`}
        confirmLabel="Delete team"
        danger
        busy={op.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={() =>
          op.mutate(() => ipc.teamDelete(team.id).then(() => `${team.name} deleted`), {
            onSuccess: () => setConfirm(null),
          })
        }
      >
        Every team vault and everything inside is deleted for all {team.member_count} members. This
        cannot be undone.
      </ConfirmDialog>
    </Stack>
  );
}

function MemberRow({
  team,
  m,
  me,
  onRole,
  onRemove,
}: {
  team: Team;
  m: TeamMember;
  me: boolean;
  onRole: (role: TeamRole) => void;
  onRemove: () => void;
}) {
  const admin = isTeamAdmin(team.my_role);
  const owner = team.my_role === "owner";
  const [confirmOwner, setConfirmOwner] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const canChange = admin && !me && m.role !== "owner";
  const canRemove = admin && !me && m.role !== "owner";
  const name = m.display_name ?? m.email;
  return (
    <Box
      sx={{
        display: "grid",
        gridTemplateColumns: "1fr 150px 130px 36px",
        alignItems: "center",
        px: 1.5,
        minHeight: 48,
        borderRadius: 2,
        bgcolor: "surface.high",
        "& .row-actions": { opacity: 0, transition: "opacity 100ms" },
        "&:hover .row-actions, &:focus-within .row-actions": { opacity: 1 },
      }}
    >
      <Box sx={{ display: "flex", alignItems: "center", gap: 1.25, minWidth: 0 }}>
        <PersonAvatar label={initialsOf(m.display_name, m.email)} />
        <Box sx={{ minWidth: 0 }}>
          <Box sx={{ display: "flex", alignItems: "center", gap: 0.75, minWidth: 0 }}>
            <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
              {name}
            </Typography>
            {me && <Chip size="small" label="you" />}
          </Box>
          {m.display_name && (
            <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
              {m.email}
            </Typography>
          )}
        </Box>
      </Box>
      <Box>
        {canChange ? (
          <TextField
            select
            size="small"
            value={m.role}
            onChange={(e) => {
              const r = e.target.value as TeamRole;
              if (r === "owner") setConfirmOwner(true);
              else onRole(r);
            }}
            sx={{ width: 136 }}
          >
            <MenuItem value="member">{teamRoleLabel.member}</MenuItem>
            <MenuItem value="admin">{teamRoleLabel.admin}</MenuItem>
            {owner && <MenuItem value="owner">Make owner…</MenuItem>}
          </TextField>
        ) : (
          <Tooltip title={teamRoleHint[m.role]}>
            <Typography variant="body2" color="text.secondary">
              {teamRoleLabel[m.role]}
            </Typography>
          </Tooltip>
        )}
      </Box>
      <Box>
        <Chip size="small" color="success" label="Active" />
      </Box>
      <Box className="row-actions" sx={{ display: "flex", justifyContent: "flex-end" }}>
        {canRemove && (
          <Tooltip title="Remove from team">
            <IconButton size="small" onClick={() => setConfirmRemove(true)}>
              <CloseRoundedIcon fontSize="small" />
            </IconButton>
          </Tooltip>
        )}
      </Box>
      <ConfirmDialog
        open={confirmOwner}
        title={`Make ${name} the owner?`}
        confirmLabel="Transfer ownership"
        onCancel={() => setConfirmOwner(false)}
        onConfirm={() => {
          setConfirmOwner(false);
          onRole("owner");
        }}
      >
        You become an admin. Only the owner can delete the team or hand ownership on again.
      </ConfirmDialog>
      <ConfirmDialog
        open={confirmRemove}
        title={`Remove ${name}?`}
        confirmLabel="Remove"
        danger
        onCancel={() => setConfirmRemove(false)}
        onConfirm={() => {
          setConfirmRemove(false);
          onRemove();
        }}
      >
        They lose access to every vault of {team.name}. Keys of the vaults they could open are
        rotated so old copies stop working.
      </ConfirmDialog>
    </Box>
  );
}

function InviteRow({
  inv,
  onRevoke,
  onResend,
}: {
  inv: TeamInvite;
  onRevoke: () => void;
  onResend: () => void;
}) {
  const [now] = useState(() => Date.now());
  const expired = new Date(inv.expires_at).getTime() < now;
  return (
    <Box
      sx={{
        display: "grid",
        gridTemplateColumns: "1fr 150px 130px 72px",
        alignItems: "center",
        px: 1.5,
        minHeight: 48,
        borderRadius: 2,
        bgcolor: "surface.high",
        "& .row-actions": { opacity: 0, transition: "opacity 100ms" },
        "&:hover .row-actions, &:focus-within .row-actions": { opacity: 1 },
      }}
    >
      <Box sx={{ display: "flex", alignItems: "center", gap: 1.25, minWidth: 0 }}>
        <PersonAvatar kind="invite" label="" />
        <Box sx={{ minWidth: 0 }}>
          <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
            {inv.email}
          </Typography>
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {expired
              ? "Invitation expired"
              : `Invited ${new Date(inv.created_at).toLocaleDateString()} · expires ${new Date(inv.expires_at).toLocaleDateString()}`}
          </Typography>
        </Box>
      </Box>
      <Typography variant="body2" color="text.secondary">
        {teamRoleLabel[inv.role]}
      </Typography>
      <Box>
        <Chip
          size="small"
          color={expired ? "default" : "warning"}
          label={expired ? "Expired" : "Pending"}
        />
      </Box>
      <Box className="row-actions" sx={{ display: "flex", justifyContent: "flex-end" }}>
        <Tooltip title="Issue a new link">
          <IconButton size="small" onClick={onResend}>
            <ReplayRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <Tooltip title="Revoke invitation">
          <IconButton size="small" onClick={onRevoke}>
            <CloseRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </Box>
    </Box>
  );
}

function PendingKeys({
  team,
  pending,
  members,
  vaults,
  onGrant,
}: {
  team: Team;
  pending: PendingVaultKey[];
  members: TeamMember[];
  vaults: LocalVault[];
  onGrant: (k: PendingVaultKey) => void;
}) {
  const grantable = pending.filter((k) => {
    const v = vaults.find((x) => x.id === k.vault_id);
    return v?.unlocked && v.role === "manager";
  });
  return (
    <SectionCard
      title="Waiting for a vault key"
      description={`Members who were given access to a vault of ${team.name} but have not received its key yet. Handing it over seals the key to their account on this device.`}
      tone="warning"
    >
      <Stack spacing={0.75}>
        {pending.map((k) => {
          const v = vaults.find((x) => x.id === k.vault_id);
          const m = members.find((x) => x.user_id === k.user_id);
          const can = grantable.includes(k);
          return (
            <EntityCard
              key={`${k.vault_id}:${k.user_id}`}
              dense
              tile={<PersonAvatar label={initialsOf(m?.display_name, m?.email ?? "?")} />}
              title={m?.display_name ?? m?.email ?? "Unknown member"}
              subtitle={`${v?.name ?? "Vault"} · ${vaultRoleLabel[k.role]}`}
              trailing={
                can ? (
                  <Button
                    size="small"
                    variant="tonal"
                    startIcon={<KeyRoundedIcon />}
                    onClick={() => onGrant(k)}
                  >
                    Hand over key
                  </Button>
                ) : (
                  <Tooltip
                    title={
                      v && !v.unlocked
                        ? "This vault is locked on your device"
                        : "Only a manager of this vault can hand over its key"
                    }
                  >
                    <LockRoundedIcon fontSize="small" sx={{ color: "text.disabled" }} />
                  </Tooltip>
                )
              }
            />
          );
        })}
      </Stack>
    </SectionCard>
  );
}

function ResentDialog({
  results,
  onClose,
}: {
  results: InviteResult[] | null;
  onClose: () => void;
}) {
  return (
    <Dialog open={results !== null} onClose={onClose} maxWidth="sm" fullWidth>
      <DialogTitle>New invitation link</DialogTitle>
      <DialogContent>
        <Stack spacing={1}>
          {(results ?? []).map((r) => (
            <InviteResultRow key={r.email} result={r} />
          ))}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button variant="contained" onClick={onClose}>
          Done
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function CreateTeamDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (t: Team) => void;
}) {
  return (
    <Dialog open={open} onClose={onClose} maxWidth="xs" fullWidth>
      {open && <CreateTeamBody onClose={onClose} onCreated={onCreated} />}
    </Dialog>
  );
}

function CreateTeamBody({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: (t: Team) => void;
}) {
  const snackbar = useSnackbar();
  const invalidate = useInvalidateTeam();
  const [name, setName] = useState("");
  const create = useMutation({
    mutationFn: () => ipc.teamCreate(name.trim()),
    onSuccess: (t) => {
      invalidate();
      onCreated(t);
      onClose();
      snackbar.notify(`${t.name} created`);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  return (
    <>
      <DialogTitle>New team</DialogTitle>
      <DialogContent>
        <Stack spacing={1.5}>
          <Field label="Team name">
            <TextField
              autoFocus
              fullWidth
              size="small"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && name.trim()) create.mutate();
              }}
            />
          </Field>
          <Typography variant="caption" color="text.secondary">
            You become the owner. A first vault named after the team is created and sealed to your
            account; invite people afterwards.
          </Typography>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={onClose} disabled={create.isPending}>
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={!name.trim() || create.isPending}
          onClick={() => create.mutate()}
        >
          {create.isPending ? "Creating…" : "Create team"}
        </Button>
      </DialogActions>
    </>
  );
}

function JoinTeamDialog({
  open,
  onClose,
  onJoined,
}: {
  open: boolean;
  onClose: () => void;
  onJoined: (t: Team) => void;
}) {
  return (
    <Dialog open={open} onClose={onClose} maxWidth="xs" fullWidth>
      {open && <JoinTeamBody onClose={onClose} onJoined={onJoined} />}
    </Dialog>
  );
}

function JoinTeamBody({ onClose, onJoined }: { onClose: () => void; onJoined: (t: Team) => void }) {
  const snackbar = useSnackbar();
  const invalidate = useInvalidateTeam();
  const [link, setLink] = useState("");
  const join = useMutation({
    mutationFn: () => ipc.teamAcceptInvite(link.trim()),
    onSuccess: (t) => {
      invalidate();
      onJoined(t);
      onClose();
      snackbar.notify(`You joined ${t.name}`);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  return (
    <>
      <DialogTitle>Join a team</DialogTitle>
      <DialogContent>
        <Stack spacing={1.5}>
          <Field label="Invitation link or code">
            <TextField
              autoFocus
              fullWidth
              size="small"
              value={link}
              onChange={(e) => setLink(e.target.value)}
              placeholder="https://…/invite/…"
              onKeyDown={(e) => {
                if (e.key === "Enter" && link.trim()) join.mutate();
              }}
            />
          </Field>
          <Typography variant="caption" color="text.secondary">
            The link must have been issued for the e-mail address of this account.
          </Typography>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button color="inherit" onClick={onClose} disabled={join.isPending}>
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={!link.trim() || join.isPending}
          onClick={() => join.mutate()}
        >
          {join.isPending ? "Joining…" : "Join"}
        </Button>
      </DialogActions>
    </>
  );
}
