import { useMemo, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  IconButton,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import ChevronRightRoundedIcon from "@mui/icons-material/ChevronRightRounded";
import { useMutation } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Page, PageBody } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import {
  ActionMenu,
  EntityCard,
  Field,
  IconTile,
  Loading,
  SectionCard,
  SidePanel,
  type MenuAction,
} from "@/components/ui";
import * as ipc from "@/ipc/commands";
import {
  useAccount,
  useInvalidateTeam,
  useTeamMembers,
  useTeams,
  useVaultMembers,
  useVaults,
} from "@/ipc/hooks";
import {
  errorMessage,
  type LocalVault,
  type Team,
  type TeamMember,
  type Uuid,
  type VaultAccess,
  type VaultMember,
  type VaultRole,
} from "@/ipc/types";
import { goToSettings, useSettingsIntent } from "@/app/navigation";
import { vaultHint, vaultIcon } from "@/app/vault";
import { PersonAvatar, initialsOf } from "./PersonAvatar";
import { VAULT_ROLES, isTeamAdmin, vaultRoleHint, vaultRoleLabel } from "./roles";

type Panel = { kind: "vault"; id: Uuid } | { kind: "new"; teamId: Uuid | null };

/**
 * Settings → Vaults, after Termius: the list of vaults on this device on the
 * left, "Vault details" with the people who can open it on the right.
 */
export function VaultsPage() {
  const account = useAccount();
  const vaults = useVaults();
  const signedIn = Boolean(account.data?.account);
  const myId = account.data?.account?.userId ?? null;
  const teams = useTeams(signedIn);
  const [panel, setPanel] = useState<Panel | null>(null);

  useSettingsIntent(["vault", "newVault"], (intent) => {
    if (intent.kind === "vault") setPanel({ kind: "vault", id: intent.id });
    else if (intent.kind === "newVault") setPanel({ kind: "new", teamId: intent.teamId ?? null });
  });

  const list = vaults.data ?? [];
  const adminTeams = useMemo(
    () => (teams.data ?? []).filter((t) => isTeamAdmin(t.my_role)),
    [teams.data],
  );
  const open = panel?.kind === "vault" ? list.find((v) => v.id === panel.id) : undefined;

  return (
    <Box sx={{ display: "flex", flex: 1, minHeight: 0 }}>
      <Page>
        <PageBody>
          {vaults.isPending ? (
            <Loading />
          ) : vaults.error ? (
            <Alert severity="error">{errorMessage(vaults.error)}</Alert>
          ) : (
            <Stack spacing={1.5}>
              <SectionCard
                title="Vaults"
                description="Every host, key, snippet and rule lives in exactly one vault. Local stays on this device; Personal syncs to your account; team vaults are shared with the people listed inside."
                action={
                  <Tooltip
                    title={
                      !signedIn
                        ? "Sign in to create shared vaults"
                        : adminTeams.length === 0
                          ? "Only team admins create vaults — create a team first"
                          : ""
                    }
                  >
                    <span>
                      <Button
                        variant="tonal"
                        size="small"
                        startIcon={<AddRoundedIcon />}
                        disabled={adminTeams.length === 0}
                        onClick={() => setPanel({ kind: "new", teamId: adminTeams[0]?.id ?? null })}
                      >
                        New vault
                      </Button>
                    </span>
                  </Tooltip>
                }
              >
                <Stack spacing={0.75}>
                  {list.map((v) => (
                    <VaultRow
                      key={v.id}
                      v={v}
                      team={(teams.data ?? []).find((t) => t.id === v.team_id) ?? null}
                      selected={open?.id === v.id}
                      onClick={() => setPanel({ kind: "vault", id: v.id })}
                    />
                  ))}
                </Stack>
              </SectionCard>
              {!signedIn && (
                <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
                  Sign in under Account & sync to get a Personal vault that follows you across
                  devices and to share vaults with a team.
                </Typography>
              )}
            </Stack>
          )}
        </PageBody>
      </Page>
      {open && (
        <VaultDetails
          key={open.id}
          v={open}
          team={(teams.data ?? []).find((t) => t.id === open.team_id) ?? null}
          myId={myId}
          onClose={() => setPanel(null)}
        />
      )}
      {panel?.kind === "new" && (
        <NewVaultPanel
          teams={adminTeams}
          initialTeamId={panel.teamId}
          myId={myId}
          onClose={() => setPanel(null)}
          onCreated={() => setPanel(null)}
        />
      )}
    </Box>
  );
}

function VaultRow({
  v,
  team,
  selected,
  onClick,
}: {
  v: LocalVault;
  team: Team | null;
  selected: boolean;
  onClick: () => void;
}) {
  const members = useVaultMembers(v.kind === "team" ? v.id : null);
  const count = members.data?.length;
  const subtitle =
    v.kind === "team"
      ? [
          team?.name,
          count !== undefined ? `${count} ${count === 1 ? "member" : "members"}` : null,
          !v.unlocked ? "locked on this device" : v.role === "viewer" ? "read-only" : null,
        ]
          .filter(Boolean)
          .join(" · ")
      : v.kind === "personal"
        ? "Only you · synced with your account"
        : "This device only · never leaves it";
  return (
    <EntityCard
      dense
      selected={selected}
      tile={
        <IconTile tone={!v.unlocked ? "neutral" : v.kind === "team" ? "accent" : "info"} size={32}>
          {vaultIcon(v)}
        </IconTile>
      }
      title={v.name}
      subtitle={subtitle}
      trailing={<ChevronRightRoundedIcon fontSize="small" sx={{ color: "text.disabled" }} />}
      onClick={onClick}
    />
  );
}

/* --------------------------------------------------------------- details */

function VaultDetails({
  v,
  team,
  myId,
  onClose,
}: {
  v: LocalVault;
  team: Team | null;
  myId: Uuid | null;
  onClose: () => void;
}) {
  const snackbar = useSnackbar();
  const invalidate = useInvalidateTeam();
  const isTeam = v.kind === "team";
  const manager = isTeam && v.unlocked && v.role === "manager";
  const members = useVaultMembers(isTeam ? v.id : null);
  const teamMembers = useTeamMembers(isTeam ? v.team_id : null);
  const [name, setName] = useState(v.name);
  const [menu, setMenu] = useState<HTMLElement | null>(null);
  const [confirm, setConfirm] = useState<"rotate" | "delete" | null>(null);
  const [addUser, setAddUser] = useState<Uuid | null>(null);
  const [addRole, setAddRole] = useState<VaultRole>("editor");

  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      invalidate();
      setConfirm(null);
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const memberList = members.data ?? [];
  const candidates = (teamMembers.data ?? []).filter(
    (m) => !memberList.some((x) => x.user_id === m.user_id),
  );

  const menuItems: MenuAction[] = [
    {
      label: "Rotate vault key",
      icon: <RefreshRoundedIcon fontSize="small" />,
      disabled: !manager,
      onClick: () => setConfirm("rotate"),
      divider: true,
    },
    {
      label: "Delete vault",
      icon: <DeleteOutlineRoundedIcon fontSize="small" />,
      danger: true,
      disabled: !manager,
      onClick: () => setConfirm("delete"),
    },
  ];

  const commitName = () => {
    const n = name.trim();
    if (!n || n === v.name) {
      setName(v.name);
      return;
    }
    op.mutate(() => ipc.teamVaultRename(v.id, n).then(() => "Vault renamed"));
  };

  return (
    <SidePanel
      title="Vault details"
      subtitle={team ? team.name : vaultHint(v)}
      onClose={onClose}
      actions={
        isTeam ? (
          <IconButton onClick={(e) => setMenu(e.currentTarget)} aria-label="Vault actions">
            <MoreHorizRoundedIcon fontSize="small" />
          </IconButton>
        ) : undefined
      }
    >
      <SectionCard>
        <Field label="Title">
          <TextField
            fullWidth
            value={name}
            disabled={!manager}
            onChange={(e) => setName(e.target.value)}
            onBlur={commitName}
            onKeyDown={(e) => {
              if (e.key === "Enter") (e.target as HTMLInputElement).blur();
              if (e.key === "Escape") setName(v.name);
            }}
          />
        </Field>
        {!isTeam && (
          <Typography variant="caption" color="text.secondary">
            {v.kind === "personal"
              ? "Your Personal vault is encrypted with a key only your devices hold. Nobody else can be given access — copy items to a team vault to share them."
              : "Local is stored on this computer only. Sign in to sync a Personal vault or share with a team."}
          </Typography>
        )}
      </SectionCard>

      {isTeam && (
        <SectionCard
          title="People with access to this vault"
          description={
            manager
              ? "Change what someone can do from the dropdown, or remove them with the cross. Removing rotates the key for everyone else."
              : v.unlocked
                ? "Only a manager of this vault can change who has access."
                : "This vault is locked on your device until a manager hands you its key."
          }
        >
          {manager && candidates.length === 0 && (
            <Typography variant="caption" color="text.secondary">
              Everyone in the team already has access. Invite more people from the Team page.
            </Typography>
          )}
          {manager && candidates.length > 0 && (
            <Stack spacing={1}>
              <TextField
                select
                size="small"
                fullWidth
                value={addUser ?? ""}
                onChange={(e) => setAddUser(e.target.value || null)}
                slotProps={{
                  select: {
                    displayEmpty: true,
                    renderValue: (val) => {
                      const m = candidates.find((c) => c.user_id === val);
                      return m ? (
                        (m.display_name ?? m.email)
                      ) : (
                        <Typography component="span" color="text.disabled">
                          Add a team member…
                        </Typography>
                      );
                    },
                  },
                }}
              >
                {candidates.map((m) => (
                  <MenuItem key={m.user_id} value={m.user_id}>
                    <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
                      <PersonAvatar size={22} label={initialsOf(m.display_name, m.email)} />
                      <Box>
                        <Typography variant="body2">{m.display_name ?? m.email}</Typography>
                        {m.display_name && (
                          <Typography variant="caption" color="text.secondary">
                            {m.email}
                          </Typography>
                        )}
                      </Box>
                    </Box>
                  </MenuItem>
                ))}
              </TextField>
              <Box sx={{ display: "flex", gap: 1 }}>
                <RoleSelect value={addRole} onChange={setAddRole} width="100%" />
                <Button
                  variant="contained"
                  disabled={!addUser || op.isPending}
                  onClick={() => {
                    const uid = addUser;
                    if (!uid) return;
                    op.mutate(() =>
                      ipc
                        .teamVaultSetAccess(v.id, uid, addRole)
                        .then(() => "Access granted — the key is sealed to their account"),
                    );
                    setAddUser(null);
                  }}
                >
                  Add
                </Button>
              </Box>
            </Stack>
          )}
          {members.isPending ? (
            <Loading pt={1} />
          ) : (
            <Stack spacing={0.5}>
              {memberList.map((m) => (
                <VaultMemberRow
                  key={m.user_id}
                  m={m}
                  me={m.user_id === myId}
                  manager={manager}
                  onlyManager={
                    m.role === "manager" &&
                    memberList.filter((x) => x.role === "manager" && !x.pending).length <= 1
                  }
                  onRole={(role) =>
                    op.mutate(() =>
                      ipc
                        .teamVaultSetAccess(v.id, m.user_id, role)
                        .then(() => (m.pending ? "Key handed over" : "Access updated")),
                    )
                  }
                  onRemove={() =>
                    op.mutate(() =>
                      ipc
                        .teamVaultRemoveAccess(v.id, m.user_id)
                        .then(() =>
                          m.user_id === myId
                            ? `You left ${v.name}`
                            : `${m.display_name ?? m.email} removed · key rotated`,
                        ),
                    )
                  }
                />
              ))}
            </Stack>
          )}
        </SectionCard>
      )}

      {isTeam && (
        <SectionCard title="Encryption">
          <Box sx={{ display: "flex", alignItems: "center", gap: 1.25 }}>
            <IconTile tone="neutral" size={32}>
              <KeyRoundedIcon />
            </IconTile>
            <Box sx={{ flex: 1, minWidth: 0 }}>
              <Typography variant="body2">Key version {v.key_version}</Typography>
              <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
                Sealed separately to each member's account. Rotating re-encrypts everything under a
                fresh key and re-seals it to the current members.
              </Typography>
            </Box>
          </Box>
        </SectionCard>
      )}

      <ActionMenu anchor={menu} onClose={() => setMenu(null)} items={menuItems} />
      <ConfirmDialog
        open={confirm === "rotate"}
        title={`Rotate the key of ${v.name}?`}
        confirmLabel="Rotate key"
        busy={op.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={() =>
          op.mutate(() => ipc.teamVaultRotateKey(v.id).then(() => "Vault key rotated"))
        }
      >
        Everything in the vault is re-encrypted with a new key on this device and re-sealed to the{" "}
        {memberList.filter((m) => !m.pending).length} current members. Old copies of the key stop
        working. Do this after removing someone by other means or if you suspect a device was
        compromised.
      </ConfirmDialog>
      <ConfirmDialog
        open={confirm === "delete"}
        title={`Delete ${v.name}?`}
        confirmLabel="Delete vault"
        danger
        busy={op.isPending}
        onCancel={() => setConfirm(null)}
        onConfirm={() =>
          op.mutate(() =>
            ipc.teamVaultDelete(v.id).then(() => {
              onClose();
              return `${v.name} deleted`;
            }),
          )
        }
      >
        All hosts, keys, snippets and rules inside disappear for every member. This cannot be undone
        — export a backup first if you need one.
      </ConfirmDialog>
    </SidePanel>
  );
}

function RoleSelect({
  value,
  onChange,
  disabled,
  width = 132,
}: {
  value: VaultRole;
  onChange: (r: VaultRole) => void;
  disabled?: boolean;
  width?: number | string;
}) {
  return (
    <TextField
      select
      size="small"
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value as VaultRole)}
      sx={{ width, flexShrink: width === "100%" ? 1 : 0, minWidth: 0 }}
    >
      {VAULT_ROLES.map((r) => (
        <MenuItem key={r} value={r}>
          <Tooltip title={vaultRoleHint[r]} placement="left">
            <span>{vaultRoleLabel[r]}</span>
          </Tooltip>
        </MenuItem>
      ))}
    </TextField>
  );
}

function VaultMemberRow({
  m,
  me,
  manager,
  onlyManager,
  onRole,
  onRemove,
}: {
  m: VaultMember;
  me: boolean;
  manager: boolean;
  /** The last manager cannot be demoted or removed — the vault would have nobody to run it. */
  onlyManager: boolean;
  onRole: (r: VaultRole) => void;
  onRemove: () => void;
}) {
  const [confirmRemove, setConfirmRemove] = useState(false);
  const name = m.display_name ?? m.email;
  const editable = manager && !onlyManager;
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 1,
        px: 1,
        minHeight: 44,
        borderRadius: 1.5,
        "&:hover": { bgcolor: "surface.highest" },
        "& .row-actions": { opacity: 0, transition: "opacity 100ms" },
        "&:hover .row-actions, &:focus-within .row-actions": { opacity: 1 },
      }}
    >
      <PersonAvatar size={24} label={initialsOf(m.display_name, m.email)} />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 0.75, minWidth: 0 }}>
          <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
            {name}
          </Typography>
          {me && <Chip size="small" label="you" />}
          {m.pending && (
            <Tooltip title="Has access on paper but no key yet — hand it over from the dropdown or the Team page">
              <Chip size="small" color="warning" label="Pending key" />
            </Tooltip>
          )}
        </Box>
      </Box>
      {editable ? (
        m.pending ? (
          <Button
            size="small"
            variant="tonal"
            startIcon={<KeyRoundedIcon />}
            onClick={() => onRole(m.role)}
          >
            Hand over key
          </Button>
        ) : (
          <RoleSelect value={m.role} onChange={onRole} width={124} />
        )
      ) : (
        <Tooltip
          title={
            onlyManager && manager
              ? "The only manager — add another before changing this"
              : vaultRoleHint[m.role]
          }
        >
          <Typography variant="body2" color="text.secondary" sx={{ flexShrink: 0 }}>
            {vaultRoleLabel[m.role]}
          </Typography>
        </Tooltip>
      )}
      <Box className="row-actions" sx={{ width: 28, display: "flex", justifyContent: "flex-end" }}>
        {editable && (
          <Tooltip title={me ? "Leave this vault" : "Remove access"}>
            <IconButton size="small" onClick={() => setConfirmRemove(true)}>
              <CloseRoundedIcon fontSize="small" />
            </IconButton>
          </Tooltip>
        )}
      </Box>
      <ConfirmDialog
        open={confirmRemove}
        title={me ? "Leave this vault?" : `Remove ${name}?`}
        confirmLabel={me ? "Leave" : "Remove"}
        danger
        onCancel={() => setConfirmRemove(false)}
        onConfirm={() => {
          setConfirmRemove(false);
          onRemove();
        }}
      >
        {me
          ? "The vault disappears from your devices. Another manager can let you back in."
          : "They can no longer open the vault. Its key is rotated so a copy they may still hold is useless."}
      </ConfirmDialog>
    </Box>
  );
}

/* ------------------------------------------------------------- new vault */

function NewVaultPanel({
  teams,
  initialTeamId,
  myId,
  onClose,
  onCreated,
}: {
  teams: Team[];
  initialTeamId: Uuid | null;
  myId: Uuid | null;
  onClose: () => void;
  onCreated: () => void;
}) {
  const snackbar = useSnackbar();
  const invalidate = useInvalidateTeam();
  const me = useAccount().data?.account ?? null;
  const [teamId, setTeamId] = useState<Uuid | null>(
    teams.find((t) => t.id === initialTeamId)?.id ?? teams[0]?.id ?? null,
  );
  const [name, setName] = useState("");
  const [access, setAccess] = useState<Map<Uuid, VaultRole>>(new Map());
  const members = useTeamMembers(teamId);
  const team = teams.find((t) => t.id === teamId) ?? null;

  const create = useMutation({
    mutationFn: () => {
      if (!teamId) throw new Error("Pick a team");
      const list: VaultAccess[] = [...access].map(([userId, role]) => ({ userId, role }));
      return ipc.teamVaultCreate(teamId, name.trim(), list);
    },
    onSuccess: () => {
      invalidate();
      snackbar.notify(`${name.trim()} created`);
      onCreated();
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  if (teams.length === 0) {
    return (
      <SidePanel title="New vault" onClose={onClose}>
        <Alert severity="info">
          Shared vaults belong to a team. Create one under Settings → Team first.
        </Alert>
        <Button variant="tonal" onClick={() => goToSettings("team")}>
          Go to Team
        </Button>
      </SidePanel>
    );
  }

  const others: TeamMember[] = (members.data ?? []).filter((m) => m.user_id !== myId);

  return (
    <SidePanel
      title="New vault"
      subtitle={team ? `In ${team.name}` : undefined}
      onClose={onClose}
      footer={
        <>
          <Button color="inherit" onClick={onClose} disabled={create.isPending}>
            Cancel
          </Button>
          <Button
            variant="contained"
            disabled={!name.trim() || !teamId || create.isPending}
            onClick={() => create.mutate()}
          >
            {create.isPending ? "Creating…" : "Create vault"}
          </Button>
        </>
      }
    >
      <SectionCard>
        <Field label="Title">
          <TextField
            autoFocus
            fullWidth
            value={name}
            placeholder="e.g. Production, Staging, Customer X"
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        {teams.length > 1 && (
          <Field label="Team">
            <TextField
              select
              fullWidth
              value={teamId ?? ""}
              onChange={(e) => {
                setTeamId(e.target.value);
                setAccess(new Map());
              }}
            >
              {teams.map((t) => (
                <MenuItem key={t.id} value={t.id}>
                  {t.name}
                </MenuItem>
              ))}
            </TextField>
          </Field>
        )}
      </SectionCard>
      <SectionCard
        title="People with access to this vault"
        description="You are the manager. Choose what each teammate can do to let them in right away — the key is sealed to their accounts on this device; you can change this later."
      >
        {members.isPending ? (
          <Loading pt={1} />
        ) : (
          <Stack spacing={0.5}>
            <Box sx={{ display: "flex", alignItems: "center", gap: 1, px: 1, minHeight: 40 }}>
              <PersonAvatar
                size={24}
                label={me ? initialsOf(me.displayName, me.email) : ""}
                kind={me ? "account" : "guest"}
              />
              <Typography variant="body2" noWrap sx={{ flex: 1, minWidth: 0, fontWeight: 500 }}>
                {me?.displayName ?? me?.email ?? "You"}
                <Chip size="small" label="you" sx={{ ml: 0.75 }} />
              </Typography>
              <Typography variant="body2" color="text.secondary">
                can manage
              </Typography>
            </Box>
            {others.map((m) => {
              const role = access.get(m.user_id);
              return (
                <Box
                  key={m.user_id}
                  sx={{ display: "flex", alignItems: "center", gap: 1, px: 1, minHeight: 40 }}
                >
                  <PersonAvatar size={24} label={initialsOf(m.display_name, m.email)} />
                  <Typography variant="body2" noWrap sx={{ flex: 1, minWidth: 0 }}>
                    {m.display_name ?? m.email}
                  </Typography>
                  <TextField
                    select
                    size="small"
                    value={role ?? "none"}
                    onChange={(e) => {
                      const next = new Map(access);
                      if (e.target.value === "none") next.delete(m.user_id);
                      else next.set(m.user_id, e.target.value as VaultRole);
                      setAccess(next);
                    }}
                    sx={{ width: 132, flexShrink: 0 }}
                  >
                    <MenuItem value="none">no access</MenuItem>
                    {VAULT_ROLES.map((r) => (
                      <MenuItem key={r} value={r}>
                        {vaultRoleLabel[r]}
                      </MenuItem>
                    ))}
                  </TextField>
                </Box>
              );
            })}
            {others.length === 0 && (
              <Typography variant="caption" color="text.secondary" sx={{ px: 1 }}>
                Nobody else in {team?.name ?? "the team"} yet — invite people from the Team page.
              </Typography>
            )}
          </Stack>
        )}
      </SectionCard>
    </SidePanel>
  );
}
