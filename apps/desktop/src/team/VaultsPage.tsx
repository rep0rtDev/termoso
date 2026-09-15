import { useMemo, useState, type ReactNode } from "react";
import {
  Alert,
  Autocomplete,
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
import CheckRoundedIcon from "@mui/icons-material/CheckRounded";
import AddBoxRoundedIcon from "@mui/icons-material/AddBoxRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import PersonOutlineRoundedIcon from "@mui/icons-material/PersonOutlineRounded";
import GroupsOutlinedIcon from "@mui/icons-material/GroupsOutlined";
import ComputerOutlinedIcon from "@mui/icons-material/ComputerOutlined";
import WorkspacePremiumRoundedIcon from "@mui/icons-material/WorkspacePremiumRounded";
import { useMutation } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Page, PageBody } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import {
  ActionMenu,
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
import { goToSettings, goToSettingsWith, useSettingsIntent } from "@/app/navigation";
import { vaultHint, vaultIcon } from "@/app/vault";
import { PersonAvatar, initialsOf } from "./PersonAvatar";
import { VAULT_ROLES, isTeamAdmin, vaultRoleHint, vaultRoleLabel } from "./roles";

type Panel = { kind: "vault"; id: Uuid } | { kind: "new"; teamId: Uuid | null };

/**
 * Settings → Vaults, after Termius: one compact card listing the vaults on
 * this device (`Personal · Only you`, `Team · Everyone`, …, `Add vault`) and
 * "Vault details" with the people who can open it on the right.
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
            <Stack spacing={1.5} sx={{ maxWidth: 640, mx: "auto", width: "100%" }}>
              <SectionCard sx={{ p: 1, gap: 0 }}>
                {list.map((v) => (
                  <VaultRow
                    key={v.id}
                    v={v}
                    team={(teams.data ?? []).find((t) => t.id === v.team_id) ?? null}
                    selected={open?.id === v.id}
                    onClick={() => setPanel({ kind: "vault", id: v.id })}
                  />
                ))}
                {panel?.kind === "new" && (
                  <ListRow
                    selected
                    icon={<GroupsOutlinedIcon fontSize="small" />}
                    title={
                      <Typography variant="body2" color="text.disabled">
                        Enter vault name, e.g. Production…
                      </Typography>
                    }
                  />
                )}
                <Tooltip
                  title={
                    !signedIn
                      ? "Sign in to create shared vaults"
                      : adminTeams.length === 0
                        ? "Only team admins create vaults — create a team first"
                        : ""
                  }
                  placement="bottom-start"
                >
                  <Box sx={{ mt: 0.5 }}>
                    <ListRow
                      icon={<AddBoxRoundedIcon fontSize="small" />}
                      title="Add vault"
                      disabled={adminTeams.length === 0}
                      onClick={() => setPanel({ kind: "new", teamId: adminTeams[0]?.id ?? null })}
                    />
                  </Box>
                </Tooltip>
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

/** One line of the vaults card: `icon  Name  · who`, optional right-hand note. */
function ListRow({
  icon,
  title,
  hint,
  trailing,
  selected,
  disabled,
  onClick,
}: {
  icon: ReactNode;
  title: ReactNode;
  hint?: ReactNode;
  trailing?: ReactNode;
  selected?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}) {
  return (
    <Box
      role={onClick ? "button" : undefined}
      tabIndex={onClick && !disabled ? 0 : undefined}
      onClick={disabled ? undefined : onClick}
      onKeyDown={(e) => {
        if (!disabled && onClick && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          onClick();
        }
      }}
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 1.25,
        px: 1.5,
        minHeight: 44,
        borderRadius: 1.5,
        cursor: onClick && !disabled ? "pointer" : "default",
        opacity: disabled ? 0.5 : 1,
        bgcolor: selected ? "surface.highest" : "transparent",
        "&:hover": onClick && !disabled && !selected ? { bgcolor: "action.hover" } : undefined,
        "&:focus-visible": {
          outline: "2px solid",
          outlineColor: "primary.main",
          outlineOffset: -2,
        },
      }}
    >
      <Box sx={{ display: "grid", placeItems: "center", color: "text.primary", flexShrink: 0 }}>
        {icon}
      </Box>
      <Box sx={{ display: "flex", alignItems: "center", gap: 0.75, minWidth: 0, flex: 1 }}>
        {typeof title === "string" ? (
          <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
            {title}
          </Typography>
        ) : (
          title
        )}
        {hint}
      </Box>
      {trailing}
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
  const everyone = team !== null && count !== undefined && count >= team.member_count;
  const who =
    v.kind === "team"
      ? everyone
        ? "Everyone"
        : count !== undefined
          ? `${count} ${count === 1 ? "person" : "people"}`
          : (team?.name ?? "Team")
      : v.kind === "personal"
        ? "Only you"
        : "This device";
  const whoIcon =
    v.kind === "team" ? (
      <GroupsOutlinedIcon sx={{ fontSize: 15 }} />
    ) : v.kind === "personal" ? (
      <PersonOutlineRoundedIcon sx={{ fontSize: 15 }} />
    ) : (
      <ComputerOutlinedIcon sx={{ fontSize: 15 }} />
    );
  const note = !v.unlocked ? "Locked" : v.role === "viewer" ? "Read-only" : null;
  return (
    <ListRow
      icon={vaultIcon(v)}
      title={v.name}
      selected={selected}
      onClick={onClick}
      hint={
        <Box
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 0.5,
            color: "text.secondary",
            typography: "caption",
            minWidth: 0,
          }}
        >
          {whoIcon}
          <Typography variant="caption" noWrap>
            {who}
          </Typography>
        </Box>
      }
      trailing={
        note ? (
          <Typography variant="caption" color="text.secondary">
            {note}
          </Typography>
        ) : undefined
      }
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
      <SectionCard sx={{ flexDirection: "row", alignItems: "center", gap: 1.5 }}>
        <IconTile tone={!v.unlocked ? "neutral" : isTeam ? "accent" : "info"} size={32}>
          {vaultIcon(v)}
        </IconTile>
        {manager ? (
          <TextField
            fullWidth
            size="small"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onBlur={commitName}
            onKeyDown={(e) => {
              if (e.key === "Enter") (e.target as HTMLInputElement).blur();
              if (e.key === "Escape") setName(v.name);
            }}
          />
        ) : (
          <Typography variant="subtitle2" noWrap sx={{ flex: 1 }}>
            {v.name}
          </Typography>
        )}
      </SectionCard>
      {!isTeam && (
        <SectionCard>
          <Typography variant="caption" color="text.secondary">
            {v.kind === "personal"
              ? "Your Personal vault is encrypted with a key only your devices hold. Nobody else can be given access — copy items to a team vault to share them."
              : "Local is stored on this computer only. Sign in to sync a Personal vault or share with a team."}
          </Typography>
        </SectionCard>
      )}

      {isTeam && (
        <SectionCard title="People with access to this vault">
          {!v.unlocked && (
            <Alert severity="info" icon={<KeyRoundedIcon fontSize="small" />}>
              Locked on this device until a manager hands you the key.
            </Alert>
          )}
          {manager && candidates.length === 0 && (
            <Box
              sx={{
                bgcolor: "surface.highest",
                borderRadius: 1.5,
                p: 1.25,
                typography: "caption",
                color: "text.secondary",
              }}
            >
              Everyone in the team has access to this vault.{" "}
              <Button
                size="small"
                onClick={() => goToSettingsWith({ kind: "invite" })}
                sx={{ p: 0, minWidth: 0, fontSize: "inherit", verticalAlign: "baseline" }}
              >
                Invite members
              </Button>
            </Box>
          )}
          {manager && candidates.length > 0 && (
            <MemberPicker
              candidates={candidates}
              disabled={op.isPending}
              onPick={(m) =>
                op.mutate(() =>
                  ipc
                    .teamVaultSetAccess(v.id, m.user_id, "editor")
                    .then(
                      () => `${m.display_name ?? m.email} can edit — key sealed to their account`,
                    ),
                )
              }
            />
          )}
          {members.isPending ? (
            <Loading pt={1} />
          ) : (
            <Stack spacing={0.25}>
              {[false, true].map((pendingGroup) => {
                const rows = memberList.filter((m) => m.pending === pendingGroup);
                if (rows.length === 0) return null;
                return (
                  <Box key={pendingGroup ? "pending" : "active"}>
                    {pendingGroup && (
                      <Typography
                        variant="subtitle2"
                        sx={{ mt: 1, mb: 0.5, pt: 1.5, borderTop: 1, borderColor: "border.light" }}
                      >
                        Pending
                      </Typography>
                    )}
                    {rows.map((m) => (
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
                  </Box>
                );
              })}
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

/** `Add members by name or email` — picks a teammate who is not in the vault yet. */
function MemberPicker({
  candidates,
  disabled,
  onPick,
}: {
  candidates: TeamMember[];
  disabled?: boolean;
  onPick: (m: TeamMember) => void;
}) {
  return (
    <Autocomplete
      size="small"
      options={candidates}
      disabled={disabled}
      getOptionLabel={(m) => m.display_name ?? m.email}
      value={null}
      blurOnSelect
      clearOnBlur
      noOptionsText="Everyone is already listed"
      onChange={(_, m) => {
        if (m) onPick(m);
      }}
      renderOption={(props, m) => {
        const { key, ...rest } = props;
        return (
          <li key={key} {...rest}>
            <Box sx={{ display: "flex", alignItems: "center", gap: 1, minWidth: 0 }}>
              <PersonAvatar
                size={24}
                seed={m.email}
                label={initialsOf(null, m.email)}
                userId={m.user_id}
                avatar={m.avatar}
              />
              <Box sx={{ minWidth: 0 }}>
                <Typography variant="body2" noWrap>
                  {m.display_name ?? m.email}
                </Typography>
                {m.display_name && (
                  <Typography variant="caption" color="text.secondary" noWrap>
                    {m.email}
                  </Typography>
                )}
              </Box>
            </Box>
          </li>
        );
      }}
      renderInput={(params) => <TextField {...params} placeholder="Add members by name or email" />}
    />
  );
}

/**
 * Termius-style inline role control: `can edit ▾` opening can edit / can view /
 * can manage / remove access. `removeLabel` is omitted when the row cannot be removed.
 */
function RoleMenuButton({
  value,
  onChange,
  onRemove,
  removeLabel,
  disabled,
}: {
  value: VaultRole | null;
  onChange: (r: VaultRole) => void;
  onRemove?: () => void;
  removeLabel?: string;
  disabled?: boolean;
}) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const items: MenuAction[] = [...VAULT_ROLES]
    .sort((a, b) => (a === "editor" ? -1 : b === "editor" ? 1 : 0))
    .map((r) => ({
      label: vaultRoleLabel[r],
      icon: value === r ? <CheckRoundedIcon fontSize="small" /> : <Box sx={{ width: 20 }} />,
      onClick: () => onChange(r),
    }));
  if (onRemove) {
    items.push({
      label: removeLabel ?? "remove access",
      danger: true,
      divider: true,
      onClick: onRemove,
    });
  }
  return (
    <>
      <Button
        size="small"
        color="inherit"
        disabled={disabled}
        endIcon={<ExpandMoreRoundedIcon sx={{ fontSize: 18 }} />}
        onClick={(e) => setAnchor(e.currentTarget)}
        sx={{ color: "text.secondary", minWidth: 0, px: 0.75, fontWeight: 400, flexShrink: 0 }}
      >
        {value ? vaultRoleLabel[value] : "no access"}
      </Button>
      <ActionMenu anchor={anchor} onClose={() => setAnchor(null)} items={items} />
    </>
  );
}

/** Small `YOU` badge and owner/manager crown, as Termius draws them next to a name. */
function YouChip() {
  return (
    <Chip
      size="small"
      label="YOU"
      color="info"
      sx={{ height: 16, fontSize: 9, fontWeight: 700, "& .MuiChip-label": { px: 0.75 } }}
    />
  );
}

function ManagerCrown({ title }: { title: string }) {
  return (
    <Tooltip title={title}>
      <WorkspacePremiumRoundedIcon sx={{ fontSize: 16, color: "info.main" }} />
    </Tooltip>
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
        px: 0.5,
        minHeight: 44,
        borderRadius: 1.5,
      }}
    >
      <PersonAvatar
        size={28}
        seed={m.email}
        label={initialsOf(null, m.email)}
        userId={m.user_id}
        avatar={m.avatar}
      />
      <Box sx={{ flex: 1, minWidth: 0, display: "flex", alignItems: "center", gap: 0.75 }}>
        <Typography
          variant="body2"
          noWrap
          sx={{ minWidth: 0, color: m.pending ? "text.secondary" : "text.primary" }}
        >
          {name}
        </Typography>
        {me && <YouChip />}
        {m.role === "manager" && <ManagerCrown title="Manages this vault" />}
      </Box>
      {editable ? (
        m.pending ? (
          <Tooltip title="Has access on paper but no key yet — seal the vault key to their account">
            <Button
              size="small"
              startIcon={<KeyRoundedIcon />}
              onClick={() => onRole(m.role)}
              sx={{ flexShrink: 0 }}
            >
              Hand over key
            </Button>
          </Tooltip>
        ) : (
          <RoleMenuButton
            value={m.role}
            onChange={onRole}
            onRemove={() => setConfirmRemove(true)}
            removeLabel={me ? "leave vault" : "remove access"}
          />
        )
      ) : (
        <Tooltip
          title={
            onlyManager && manager
              ? "The only manager — add another before changing this"
              : m.pending
                ? "Waiting for a manager to hand over the key"
                : vaultRoleHint[m.role]
          }
        >
          <Typography variant="body2" color="text.secondary" sx={{ flexShrink: 0, px: 0.75 }}>
            {vaultRoleLabel[m.role]}
          </Typography>
        </Tooltip>
      )}
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
      <SectionCard sx={{ flexDirection: "row", alignItems: "center", gap: 1.5 }}>
        <IconTile tone="accent" size={32}>
          <GroupsOutlinedIcon fontSize="small" />
        </IconTile>
        <TextField
          autoFocus
          fullWidth
          size="small"
          value={name}
          placeholder="Vault name"
          onChange={(e) => setName(e.target.value)}
        />
      </SectionCard>
      {teams.length > 1 && (
        <SectionCard>
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
        </SectionCard>
      )}
      <SectionCard title="People with access to this vault">
        {members.isPending ? (
          <Loading pt={1} />
        ) : (
          <Stack spacing={0.5}>
            {others.length === 0 ? (
              <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
                Nobody else in {team?.name ?? "the team"} yet — invite people from the Team page.
              </Typography>
            ) : (
              <MemberPicker
                candidates={others.filter((m) => !access.has(m.user_id))}
                onPick={(m) => {
                  const next = new Map(access);
                  next.set(m.user_id, "editor");
                  setAccess(next);
                }}
              />
            )}
            <Box sx={{ display: "flex", alignItems: "center", gap: 1, px: 0.5, minHeight: 40 }}>
              <PersonAvatar
                size={28}
                seed={me?.email}
                label={me ? initialsOf(null, me.email) : ""}
                kind={me ? "account" : "guest"}
                userId={me?.userId}
                avatar={me?.avatar}
              />
              <Box sx={{ flex: 1, minWidth: 0, display: "flex", alignItems: "center", gap: 0.75 }}>
                <Typography variant="body2" noWrap sx={{ minWidth: 0 }}>
                  {me?.displayName ?? me?.email ?? "You"}
                </Typography>
                <YouChip />
                <ManagerCrown title="You manage this vault" />
              </Box>
              <Typography variant="body2" color="text.secondary" sx={{ px: 0.75 }}>
                can manage
              </Typography>
            </Box>
            {others
              .filter((m) => access.has(m.user_id))
              .map((m) => (
                <Box
                  key={m.user_id}
                  sx={{ display: "flex", alignItems: "center", gap: 1, px: 0.5, minHeight: 40 }}
                >
                  <PersonAvatar
                    size={28}
                    seed={m.email}
                    label={initialsOf(null, m.email)}
                    userId={m.user_id}
                    avatar={m.avatar}
                  />
                  <Typography variant="body2" noWrap sx={{ flex: 1, minWidth: 0 }}>
                    {m.display_name ?? m.email}
                  </Typography>
                  <RoleMenuButton
                    value={access.get(m.user_id) ?? null}
                    onChange={(r) => {
                      const next = new Map(access);
                      next.set(m.user_id, r);
                      setAccess(next);
                    }}
                    onRemove={() => {
                      const next = new Map(access);
                      next.delete(m.user_id);
                      setAccess(next);
                    }}
                  />
                </Box>
              ))}
          </Stack>
        )}
        <Typography variant="caption" color="text.secondary">
          The vault key is sealed to each listed account on this device; you can change access
          later.
        </Typography>
      </SectionCard>
    </SidePanel>
  );
}
