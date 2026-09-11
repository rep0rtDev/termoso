import { useState, type SubmitEvent } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  FormControl,
  IconButton,
  InputLabel,
  Link,
  MenuItem,
  Select,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import AutorenewRoundedIcon from "@mui/icons-material/AutorenewRounded";
import LockOpenRoundedIcon from "@mui/icons-material/LockOpenRounded";
import PersonAddAltRoundedIcon from "@mui/icons-material/PersonAddAltRounded";
import PersonRemoveRoundedIcon from "@mui/icons-material/PersonRemoveRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link as RouterLink, useNavigate, useParams } from "react-router";
import { errorMessage } from "@/api/client";
import { teamsApi, vaultsApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { Vault, VaultMember, VaultRole } from "@/api/types";
import { useAuthState } from "@/auth/store";
import { UnlockCancelled } from "@/auth/unlock";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { RoleChip } from "@/components/RoleChip";
import { Section } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { UserCell } from "@/components/UserCell";
import { formatDate } from "@/components/format";
import { rotateVaultKey, upsertMemberWithKey } from "@/vaults/keys";

export function VaultPage() {
  const { id = "" } = useParams();
  const vault = useQuery({ queryKey: queryKeys.vault(id), queryFn: () => vaultsApi.get(id) });
  if (vault.isPending) return <Loading />;
  if (vault.isError) return <Alert severity="error">{errorMessage(vault.error)}</Alert>;
  return <VaultDetail vault={vault.data} />;
}

function VaultDetail({ vault }: { vault: Vault }) {
  const team = useQuery({
    queryKey: queryKeys.team(vault.team_id ?? ""),
    queryFn: () => teamsApi.get(vault.team_id ?? ""),
    enabled: vault.team_id !== undefined,
  });
  const manager = vault.my_role === "manager";
  return (
    <>
      <PageHeader
        title={vault.name}
        subtitle={
          <Stack direction="row" spacing={1} sx={{ alignItems: "center", flexWrap: "wrap" }}>
            <RoleChip role={vault.my_role} />
            <span>
              {vault.kind === "personal" ? (
                "Personal vault"
              ) : team.data ? (
                <>
                  Team{" "}
                  <Link component={RouterLink} to={`/team/${team.data.id}`}>
                    {team.data.name}
                  </Link>
                </>
              ) : (
                "Team vault"
              )}{" "}
              · key version {vault.key_version} · created {formatDate(vault.created_at)}
            </span>
            {!vault.sealed_key && (
              <Chip size="small" color="warning" variant="outlined" label="Your key is pending" />
            )}
          </Stack>
        }
        actions={<VaultActions vault={vault} manager={manager} />}
      />
      {!vault.sealed_key && (
        <Alert severity="warning" sx={{ mb: 2.5 }}>
          You are a member of this vault but nobody has sealed the vault key to you yet. Ask a vault
          manager or team admin to grant it under “Pending vault keys” on the team page.
        </Alert>
      )}
      {vault.kind === "personal" ? (
        <Section
          title="Personal vault"
          description="Only you hold this key. It is wrapped with your account key and unwrapped locally when you sign in."
        >
          <Typography variant="body2" color="text.secondary">
            Hosts, keys and snippets synced from the desktop and mobile apps are stored here. Team
            vaults can be shared; the personal vault cannot.
          </Typography>
        </Section>
      ) : (
        <MembersSection
          vault={vault}
          manager={manager}
          teamAdmin={team.data ? team.data.my_role !== "member" : false}
        />
      )}
    </>
  );
}

function VaultActions({ vault, manager }: { vault: Vault; manager: boolean }) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [renameOpen, setRenameOpen] = useState(false);
  const [name, setName] = useState(vault.name);
  const [rotateOpen, setRotateOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const members = useQuery({
    queryKey: queryKeys.vaultMembers(vault.id),
    queryFn: () => vaultsApi.members(vault.id),
    enabled: vault.kind === "team",
  });

  const invalidate = async () => {
    await qc.invalidateQueries({ queryKey: queryKeys.vaults });
    await qc.invalidateQueries({ queryKey: queryKeys.vault(vault.id) });
    await qc.invalidateQueries({ queryKey: queryKeys.vaultMembers(vault.id) });
  };
  const rename = useMutation({
    mutationFn: () => vaultsApi.update(vault.id, name.trim()),
    onSuccess: async () => {
      setRenameOpen(false);
      await invalidate();
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const rotate = useMutation({
    mutationFn: () => rotateVaultKey(vault, members.data?.members ?? []),
    onSuccess: async (v) => {
      setRotateOpen(false);
      await invalidate();
      snack.notify(`Vault key rotated to version ${v}`);
    },
    onError: (e) => {
      setRotateOpen(false);
      if (!(e instanceof UnlockCancelled)) snack.error(errorMessage(e));
    },
  });
  const del = useMutation({
    mutationFn: () => vaultsApi.delete(vault.id),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: queryKeys.vaults });
      void navigate("/vaults", { replace: true });
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  const holdKey = Boolean(vault.sealed_key);
  return (
    <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: "wrap" }}>
      {(manager || vault.kind === "personal") && (
        <Button variant="outlined" onClick={() => setRenameOpen(true)}>
          Rename
        </Button>
      )}
      {manager && vault.kind === "team" && (
        <Tooltip
          title={
            holdKey
              ? "Generate a new vault key and re-seal it to current members"
              : "You need the vault key to rotate it"
          }
        >
          <span>
            <Button
              variant="outlined"
              startIcon={<AutorenewRoundedIcon />}
              disabled={!holdKey}
              onClick={() => setRotateOpen(true)}
            >
              Rotate key
            </Button>
          </span>
        </Tooltip>
      )}
      {manager && vault.kind === "team" && (
        <Button variant="outlined" color="error" onClick={() => setDeleteOpen(true)}>
          Delete vault
        </Button>
      )}

      <Dialog
        open={renameOpen}
        onClose={rename.isPending ? undefined : () => setRenameOpen(false)}
        maxWidth="xs"
        fullWidth
      >
        <form
          onSubmit={(e: SubmitEvent) => {
            e.preventDefault();
            rename.mutate();
          }}
        >
          <DialogTitle>Rename vault</DialogTitle>
          <DialogContent>
            <TextField
              autoFocus
              fullWidth
              label="Vault name"
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
              sx={{ mt: 1 }}
            />
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button onClick={() => setRenameOpen(false)} color="inherit">
              Cancel
            </Button>
            <Button
              type="submit"
              variant="contained"
              disabled={rename.isPending || name.trim() === ""}
            >
              Save
            </Button>
          </DialogActions>
        </form>
      </Dialog>
      <ConfirmDialog
        open={rotateOpen}
        title="Rotate vault key?"
        confirmLabel="Rotate"
        busy={rotate.isPending}
        onCancel={() => setRotateOpen(false)}
        onConfirm={() => rotate.mutate()}
      >
        A new key is generated in your browser and sealed to every member who currently holds the
        key. Removed members' stale copies become useless. Apps re-encrypt existing records on their
        next sync.
      </ConfirmDialog>
      <ConfirmDialog
        open={deleteOpen}
        title="Delete vault?"
        confirmLabel="Delete"
        danger
        busy={del.isPending}
        onCancel={() => setDeleteOpen(false)}
        onConfirm={() => del.mutate()}
      >
        All records and session logs in “{vault.name}” are deleted for every member. This cannot be
        undone.
      </ConfirmDialog>
    </Stack>
  );
}

function MembersSection({
  vault,
  manager,
  teamAdmin,
}: {
  vault: Vault;
  manager: boolean;
  teamAdmin: boolean;
}) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const { session } = useAuthState();
  const me = session?.user.id;
  const members = useQuery({
    queryKey: queryKeys.vaultMembers(vault.id),
    queryFn: () => vaultsApi.members(vault.id),
  });
  const teamMembers = useQuery({
    queryKey: queryKeys.teamMembers(vault.team_id ?? ""),
    queryFn: () => teamsApi.members(vault.team_id ?? ""),
    enabled: manager && vault.team_id !== undefined,
  });
  const [addOpen, setAddOpen] = useState(false);
  const [addUser, setAddUser] = useState("");
  const [addRole, setAddRole] = useState<VaultRole>("viewer");
  const [removing, setRemoving] = useState<VaultMember | null>(null);
  const [leaveOpen, setLeaveOpen] = useState(false);

  const invalidate = async () => {
    await qc.invalidateQueries({ queryKey: queryKeys.vaultMembers(vault.id) });
    await qc.invalidateQueries({ queryKey: queryKeys.vaults });
    await qc.invalidateQueries({ queryKey: queryKeys.vault(vault.id) });
    if (vault.team_id)
      await qc.invalidateQueries({ queryKey: queryKeys.teamPendingKeys(vault.team_id) });
  };
  const upsert = useMutation({
    mutationFn: ({
      user_id,
      public_key,
      role,
    }: {
      user_id: string;
      public_key: string;
      role: VaultRole;
    }) => upsertMemberWithKey(vault, { user_id, public_key }, role),
    onSuccess: async () => {
      setAddOpen(false);
      setAddUser("");
      await invalidate();
    },
    onError: (e) => {
      if (!(e instanceof UnlockCancelled)) snack.error(errorMessage(e));
    },
  });
  const remove = useMutation({
    mutationFn: (userId: string) => vaultsApi.removeMember(vault.id, userId),
    onSuccess: async (_r, userId) => {
      setRemoving(null);
      setLeaveOpen(false);
      await invalidate();
      if (userId !== me)
        snack.notify("Member removed. Rotate the key so their stale copy stops working.", "info");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  const holdKey = Boolean(vault.sealed_key);
  const current = new Set((members.data?.members ?? []).map((m) => m.user_id));
  const candidates = (teamMembers.data?.members ?? []).filter((m) => !current.has(m.user_id));
  const selected = candidates.find((c) => c.user_id === addUser);

  return (
    <Section
      title="Members"
      description={
        manager
          ? holdKey
            ? "Adding a member seals the vault key to their public key locally."
            : "You cannot add members or grant keys until you hold the vault key yourself."
          : undefined
      }
      actions={
        <Stack direction="row" spacing={1}>
          {vault.my_role !== "manager" ||
          (members.data?.members.filter((m) => m.role === "manager" && !m.pending).length ?? 0) >
            1 ||
          teamAdmin ? (
            <Button variant="outlined" color="error" onClick={() => setLeaveOpen(true)}>
              Leave vault
            </Button>
          ) : null}
          {manager && (
            <Button
              variant="contained"
              startIcon={<PersonAddAltRoundedIcon />}
              disabled={!holdKey}
              onClick={() => setAddOpen(true)}
            >
              Add member
            </Button>
          )}
        </Stack>
      }
      disablePadding
    >
      {members.isPending ? (
        <Loading minHeight={120} />
      ) : members.isError ? (
        <Box sx={{ p: 3 }}>
          <Alert severity="error">{errorMessage(members.error)}</Alert>
        </Box>
      ) : members.data.members.length === 0 ? (
        <EmptyState title="No members" />
      ) : (
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>Member</TableCell>
              <TableCell>Role</TableCell>
              <TableCell>Key</TableCell>
              <TableCell>Added</TableCell>
              {manager && <TableCell align="right" />}
            </TableRow>
          </TableHead>
          <TableBody>
            {members.data.members.map((m) => {
              const isMe = m.user_id === me;
              const canEdit = manager && holdKey;
              return (
                <TableRow key={m.user_id} hover>
                  <TableCell>
                    <UserCell email={m.email} displayName={m.display_name} you={isMe} />
                  </TableCell>
                  <TableCell>
                    {canEdit ? (
                      <Select
                        size="small"
                        value={m.role}
                        disabled={upsert.isPending}
                        onChange={(e) =>
                          upsert.mutate({
                            user_id: m.user_id,
                            public_key: m.public_key,
                            role: e.target.value,
                          })
                        }
                        sx={{ minWidth: 120 }}
                      >
                        <MenuItem value="viewer">Viewer</MenuItem>
                        <MenuItem value="editor">Editor</MenuItem>
                        <MenuItem value="manager">Manager</MenuItem>
                      </Select>
                    ) : (
                      <RoleChip role={m.role} />
                    )}
                  </TableCell>
                  <TableCell>
                    {m.pending ? (
                      canEdit ? (
                        <Button
                          size="small"
                          startIcon={<LockOpenRoundedIcon />}
                          disabled={upsert.isPending}
                          onClick={() =>
                            upsert.mutate({
                              user_id: m.user_id,
                              public_key: m.public_key,
                              role: m.role,
                            })
                          }
                        >
                          Grant key
                        </Button>
                      ) : (
                        <Chip size="small" color="warning" variant="outlined" label="Pending" />
                      )
                    ) : (
                      <Chip size="small" variant="outlined" label={`v${m.key_version}`} />
                    )}
                  </TableCell>
                  <TableCell>{formatDate(m.added_at)}</TableCell>
                  {manager && (
                    <TableCell align="right">
                      {!isMe && (
                        <Tooltip title="Remove from vault">
                          <IconButton
                            size="small"
                            onClick={() => setRemoving(m)}
                            aria-label="Remove member"
                          >
                            <PersonRemoveRoundedIcon fontSize="small" />
                          </IconButton>
                        </Tooltip>
                      )}
                    </TableCell>
                  )}
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      )}

      <Dialog
        open={addOpen}
        onClose={upsert.isPending ? undefined : () => setAddOpen(false)}
        maxWidth="xs"
        fullWidth
      >
        <form
          onSubmit={(e: SubmitEvent) => {
            e.preventDefault();
            if (selected)
              upsert.mutate({
                user_id: selected.user_id,
                public_key: selected.public_key,
                role: addRole,
              });
          }}
        >
          <DialogTitle>Add member</DialogTitle>
          <DialogContent sx={{ display: "grid", gap: 2 }}>
            {candidates.length === 0 ? (
              <DialogContentText>
                Every team member already belongs to this vault. Invite more people on the team page
                first.
              </DialogContentText>
            ) : (
              <>
                <FormControl sx={{ mt: 1 }}>
                  <InputLabel id="add-user">Team member</InputLabel>
                  <Select
                    labelId="add-user"
                    label="Team member"
                    value={addUser}
                    onChange={(e) => setAddUser(e.target.value)}
                  >
                    {candidates.map((c) => (
                      <MenuItem key={c.user_id} value={c.user_id}>
                        {c.display_name ? `${c.display_name} (${c.email})` : c.email}
                      </MenuItem>
                    ))}
                  </Select>
                </FormControl>
                <FormControl>
                  <InputLabel id="add-role">Vault role</InputLabel>
                  <Select
                    labelId="add-role"
                    label="Vault role"
                    value={addRole}
                    onChange={(e) => setAddRole(e.target.value)}
                  >
                    <MenuItem value="viewer">Viewer — read only</MenuItem>
                    <MenuItem value="editor">Editor — create and edit records</MenuItem>
                    <MenuItem value="manager">Manager — manage members and keys</MenuItem>
                  </Select>
                </FormControl>
              </>
            )}
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button onClick={() => setAddOpen(false)} color="inherit" disabled={upsert.isPending}>
              Cancel
            </Button>
            <Button type="submit" variant="contained" disabled={upsert.isPending || !selected}>
              Add
            </Button>
          </DialogActions>
        </form>
      </Dialog>

      <ConfirmDialog
        open={removing !== null}
        title="Remove member?"
        confirmLabel="Remove"
        danger
        busy={remove.isPending}
        onCancel={() => setRemoving(null)}
        onConfirm={() => {
          if (removing) remove.mutate(removing.user_id);
        }}
      >
        {removing?.email} loses access to “{vault.name}”. Rotate the vault key afterwards to
        invalidate their copy.
      </ConfirmDialog>
      <ConfirmDialog
        open={leaveOpen}
        title="Leave vault?"
        confirmLabel="Leave"
        danger
        busy={remove.isPending}
        onCancel={() => setLeaveOpen(false)}
        onConfirm={() => {
          if (me) remove.mutate(me);
        }}
      >
        You lose access to “{vault.name}”. A manager can add you back later.
      </ConfirmDialog>
    </Section>
  );
}
