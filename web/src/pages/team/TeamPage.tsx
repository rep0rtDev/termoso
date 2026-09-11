import { useMemo, useState, type SubmitEvent } from "react";
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  FormControl,
  FormControlLabel,
  FormGroup,
  IconButton,
  InputLabel,
  Link,
  List,
  ListItem,
  ListItemText,
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
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import LockOpenRoundedIcon from "@mui/icons-material/LockOpenRounded";
import PersonAddAltRoundedIcon from "@mui/icons-material/PersonAddAltRounded";
import PersonRemoveRoundedIcon from "@mui/icons-material/PersonRemoveRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link as RouterLink, useNavigate, useParams } from "react-router";
import { errorMessage } from "@/api/client";
import { teamsApi, vaultsApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { PendingVaultKey, Team, TeamMember, TeamRole, Vault, VaultRole } from "@/api/types";
import { useAuthState } from "@/auth/store";
import { UnlockCancelled } from "@/auth/unlock";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { CopyField } from "@/components/CopyField";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { RoleChip } from "@/components/RoleChip";
import { Section } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { UserCell } from "@/components/UserCell";
import { formatDate, formatRelative } from "@/components/format";
import { grantPendingKey, newSealedVaultKey } from "@/vaults/keys";

const isAdmin = (r: TeamRole) => r === "owner" || r === "admin";

export function TeamPage() {
  const { id = "" } = useParams();
  const team = useQuery({ queryKey: queryKeys.team(id), queryFn: () => teamsApi.get(id) });
  if (team.isPending) return <Loading />;
  if (team.isError) return <Alert severity="error">{errorMessage(team.error)}</Alert>;
  return <TeamDetail team={team.data} />;
}

function TeamDetail({ team }: { team: Team }) {
  const admin = isAdmin(team.my_role);
  return (
    <>
      <PageHeader
        title={team.name}
        subtitle={
          <Stack direction="row" spacing={1} sx={{ alignItems: "center", flexWrap: "wrap" }}>
            <RoleChip role={team.my_role} />
            <span>
              {team.member_count} {team.member_count === 1 ? "member" : "members"} · created{" "}
              {formatDate(team.created_at)}
            </span>
          </Stack>
        }
        actions={<TeamActions team={team} />}
      />
      {admin && <PendingKeysSection team={team} />}
      <MembersSection team={team} />
      {admin && <InvitesSection team={team} />}
      <TeamVaultsSection team={team} />
    </>
  );
}

function TeamActions({ team }: { team: Team }) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [renameOpen, setRenameOpen] = useState(false);
  const [name, setName] = useState(team.name);
  const [leaveOpen, setLeaveOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);

  const invalidate = async () => {
    await qc.invalidateQueries({ queryKey: queryKeys.teams });
    await qc.invalidateQueries({ queryKey: queryKeys.vaults });
  };
  const rename = useMutation({
    mutationFn: () => teamsApi.update(team.id, name.trim()),
    onSuccess: async () => {
      setRenameOpen(false);
      await invalidate();
      snack.notify("Team renamed");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const leave = useMutation({
    mutationFn: () => teamsApi.leave(team.id),
    onSuccess: async () => {
      await invalidate();
      void navigate("/team", { replace: true });
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const del = useMutation({
    mutationFn: () => teamsApi.delete(team.id),
    onSuccess: async () => {
      await invalidate();
      void navigate("/team", { replace: true });
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  return (
    <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: "wrap" }}>
      {isAdmin(team.my_role) && (
        <Button variant="outlined" onClick={() => setRenameOpen(true)}>
          Rename
        </Button>
      )}
      {team.my_role !== "owner" && (
        <Button variant="outlined" color="error" onClick={() => setLeaveOpen(true)}>
          Leave team
        </Button>
      )}
      {team.my_role === "owner" && (
        <Button variant="outlined" color="error" onClick={() => setDeleteOpen(true)}>
          Delete team
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
          <DialogTitle>Rename team</DialogTitle>
          <DialogContent>
            <TextField
              autoFocus
              fullWidth
              label="Team name"
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
        open={leaveOpen}
        title="Leave team?"
        confirmLabel="Leave"
        danger
        busy={leave.isPending}
        onCancel={() => setLeaveOpen(false)}
        onConfirm={() => leave.mutate()}
      >
        You lose access to all vaults of “{team.name}”. A manager should rotate the vault keys
        afterwards.
      </ConfirmDialog>
      <ConfirmDialog
        open={deleteOpen}
        title="Delete team?"
        confirmLabel="Delete"
        danger
        busy={del.isPending}
        onCancel={() => setDeleteOpen(false)}
        onConfirm={() => del.mutate()}
      >
        Every team vault and all data inside it is deleted for all {team.member_count} members. This
        cannot be undone.
      </ConfirmDialog>
    </Stack>
  );
}

function MembersSection({ team }: { team: Team }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const { session } = useAuthState();
  const me = session?.user.id;
  const members = useQuery({
    queryKey: queryKeys.teamMembers(team.id),
    queryFn: () => teamsApi.members(team.id),
  });
  const [removing, setRemoving] = useState<TeamMember | null>(null);
  const [transfer, setTransfer] = useState<TeamMember | null>(null);

  const invalidate = async () => {
    await qc.invalidateQueries({ queryKey: queryKeys.teamMembers(team.id) });
    await qc.invalidateQueries({ queryKey: queryKeys.teams });
    await qc.invalidateQueries({ queryKey: queryKeys.team(team.id) });
  };
  const setRole = useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: TeamRole }) =>
      teamsApi.updateMember(team.id, userId, role),
    onSuccess: async () => {
      setTransfer(null);
      await invalidate();
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const remove = useMutation({
    mutationFn: (userId: string) => teamsApi.removeMember(team.id, userId),
    onSuccess: async () => {
      setRemoving(null);
      await invalidate();
      snack.notify(
        "Member removed. Rotate affected vault keys to revoke their stale copies.",
        "info",
      );
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  const admin = isAdmin(team.my_role);

  return (
    <Section title="Members" disablePadding>
      {members.isPending ? (
        <Loading minHeight={120} />
      ) : members.isError ? (
        <Box sx={{ p: 3 }}>
          <Alert severity="error">{errorMessage(members.error)}</Alert>
        </Box>
      ) : (
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>Member</TableCell>
              <TableCell>Role</TableCell>
              <TableCell>Joined</TableCell>
              {admin && <TableCell align="right" />}
            </TableRow>
          </TableHead>
          <TableBody>
            {members.data.members.map((m) => {
              const canEdit = admin && m.user_id !== me && m.role !== "owner";
              return (
                <TableRow key={m.user_id} hover>
                  <TableCell>
                    <UserCell email={m.email} displayName={m.display_name} you={m.user_id === me} />
                  </TableCell>
                  <TableCell>
                    {canEdit ? (
                      <Select
                        size="small"
                        value={m.role}
                        onChange={(e) => {
                          const role = e.target.value;
                          if (role === "owner") setTransfer(m);
                          else setRole.mutate({ userId: m.user_id, role });
                        }}
                        sx={{ minWidth: 130 }}
                      >
                        <MenuItem value="member">Member</MenuItem>
                        <MenuItem value="admin">Admin</MenuItem>
                        {team.my_role === "owner" && (
                          <MenuItem value="owner">Owner (transfer)</MenuItem>
                        )}
                      </Select>
                    ) : (
                      <RoleChip role={m.role} />
                    )}
                  </TableCell>
                  <TableCell>{formatDate(m.joined_at)}</TableCell>
                  {admin && (
                    <TableCell align="right">
                      {canEdit && (
                        <Tooltip title="Remove from team">
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
        {removing?.email} loses access to the team and its vaults.
      </ConfirmDialog>
      <ConfirmDialog
        open={transfer !== null}
        title="Transfer ownership?"
        confirmLabel="Transfer"
        busy={setRole.isPending}
        onCancel={() => setTransfer(null)}
        onConfirm={() => {
          if (transfer) setRole.mutate({ userId: transfer.user_id, role: "owner" });
        }}
      >
        {transfer?.email} becomes the owner of “{team.name}” and you become an admin.
      </ConfirmDialog>
    </Section>
  );
}

function InvitesSection({ team }: { team: Team }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const invites = useQuery({
    queryKey: queryKeys.teamInvites(team.id),
    queryFn: () => teamsApi.invites(team.id),
  });
  const vaults = useQuery({ queryKey: queryKeys.vaults, queryFn: vaultsApi.list });
  const teamVaults = useMemo(
    () => (vaults.data?.vaults ?? []).filter((v) => v.team_id === team.id),
    [vaults.data, team.id],
  );
  const [open, setOpen] = useState(false);
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<TeamRole>("member");
  const [vaultIds, setVaultIds] = useState<string[]>([]);
  const [createdUrl, setCreatedUrl] = useState<string | null>(null);
  const [revoking, setRevoking] = useState<string | null>(null);

  const create = useMutation({
    mutationFn: () => teamsApi.createInvite(team.id, email.trim(), role, vaultIds),
    onSuccess: async (r) => {
      setOpen(false);
      setEmail("");
      setVaultIds([]);
      setCreatedUrl(r.url);
      await qc.invalidateQueries({ queryKey: queryKeys.teamInvites(team.id) });
    },
  });
  const revoke = useMutation({
    mutationFn: (inviteId: string) => teamsApi.deleteInvite(team.id, inviteId),
    onSuccess: async () => {
      setRevoking(null);
      await qc.invalidateQueries({ queryKey: queryKeys.teamInvites(team.id) });
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  return (
    <Section
      title="Invitations"
      description="Pending invitations expire automatically. The invitee receives a link by email when SMTP is configured; you can also copy it."
      actions={
        <Button
          variant="contained"
          startIcon={<PersonAddAltRoundedIcon />}
          onClick={() => setOpen(true)}
        >
          Invite
        </Button>
      }
      disablePadding
    >
      {invites.isPending ? (
        <Loading minHeight={100} />
      ) : invites.isError ? (
        <Box sx={{ p: 3 }}>
          <Alert severity="error">{errorMessage(invites.error)}</Alert>
        </Box>
      ) : invites.data.invites.length === 0 ? (
        <EmptyState title="No pending invitations" />
      ) : (
        <List disablePadding>
          {invites.data.invites.map((i) => (
            <ListItem
              key={i.id}
              divider
              secondaryAction={
                <Tooltip title="Revoke invitation">
                  <IconButton
                    edge="end"
                    onClick={() => setRevoking(i.id)}
                    aria-label="Revoke invitation"
                  >
                    <DeleteOutlineRoundedIcon />
                  </IconButton>
                </Tooltip>
              }
            >
              <ListItemText
                primary={
                  <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
                    <span>{i.email}</span>
                    <RoleChip role={i.role} />
                  </Stack>
                }
                secondary={`Sent ${formatRelative(i.created_at)} · expires ${formatDate(i.expires_at)}`}
              />
            </ListItem>
          ))}
        </List>
      )}

      <Dialog
        open={open}
        onClose={create.isPending ? undefined : () => setOpen(false)}
        maxWidth="xs"
        fullWidth
      >
        <form
          onSubmit={(e: SubmitEvent) => {
            e.preventDefault();
            create.mutate();
          }}
        >
          <DialogTitle>Invite to {team.name}</DialogTitle>
          <DialogContent sx={{ display: "grid", gap: 2 }}>
            {create.isError && <Alert severity="error">{errorMessage(create.error)}</Alert>}
            <TextField
              autoFocus
              label="Email"
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              disabled={create.isPending}
            />
            <FormControl>
              <InputLabel id="invite-role">Team role</InputLabel>
              <Select
                labelId="invite-role"
                label="Team role"
                value={role}
                onChange={(e) => setRole(e.target.value)}
              >
                <MenuItem value="member">Member</MenuItem>
                <MenuItem value="admin">Admin</MenuItem>
              </Select>
            </FormControl>
            {teamVaults.length > 0 && (
              <FormControl component="fieldset" variant="standard">
                <Typography variant="subtitle2" sx={{ mb: 0.5 }}>
                  Grant access to vaults (as viewer)
                </Typography>
                <FormGroup>
                  {teamVaults.map((v) => (
                    <FormControlLabel
                      key={v.id}
                      control={
                        <Checkbox
                          checked={vaultIds.includes(v.id)}
                          onChange={(e) =>
                            setVaultIds((ids) =>
                              e.target.checked ? [...ids, v.id] : ids.filter((x) => x !== v.id),
                            )
                          }
                        />
                      }
                      label={v.name}
                    />
                  ))}
                </FormGroup>
                <Typography variant="caption" color="text.secondary">
                  After they join, a vault manager seals the key to them under “Pending vault keys”.
                </Typography>
              </FormControl>
            )}
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button onClick={() => setOpen(false)} color="inherit" disabled={create.isPending}>
              Cancel
            </Button>
            <Button
              type="submit"
              variant="contained"
              disabled={create.isPending || email.trim() === ""}
            >
              Send invitation
            </Button>
          </DialogActions>
        </form>
      </Dialog>

      <Dialog
        open={createdUrl !== null}
        onClose={() => setCreatedUrl(null)}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>Invitation created</DialogTitle>
        <DialogContent sx={{ display: "grid", gap: 2 }}>
          <DialogContentText>
            Share this link with the invitee. It works once and only for their email address.
          </DialogContentText>
          <CopyField label="Invitation link" value={createdUrl ?? ""} />
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button variant="contained" onClick={() => setCreatedUrl(null)}>
            Done
          </Button>
        </DialogActions>
      </Dialog>

      <ConfirmDialog
        open={revoking !== null}
        title="Revoke invitation?"
        confirmLabel="Revoke"
        danger
        busy={revoke.isPending}
        onCancel={() => setRevoking(null)}
        onConfirm={() => {
          if (revoking) revoke.mutate(revoking);
        }}
      >
        The invitation link stops working immediately.
      </ConfirmDialog>
    </Section>
  );
}

function PendingKeysSection({ team }: { team: Team }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const pending = useQuery({
    queryKey: queryKeys.teamPendingKeys(team.id),
    queryFn: () => teamsApi.pendingKeys(team.id),
  });
  const vaults = useQuery({ queryKey: queryKeys.vaults, queryFn: vaultsApi.list });
  const members = useQuery({
    queryKey: queryKeys.teamMembers(team.id),
    queryFn: () => teamsApi.members(team.id),
  });

  const grant = useMutation({
    mutationFn: async (items: PendingVaultKey[]) => {
      const byId = new Map((vaults.data?.vaults ?? []).map((v) => [v.id, v]));
      let granted = 0;
      for (const p of items) {
        const vault = byId.get(p.vault_id);
        if (!vault?.sealed_key) continue;
        await grantPendingKey(vault, p);
        granted += 1;
      }
      return granted;
    },
    onSuccess: async (n) => {
      await qc.invalidateQueries({ queryKey: queryKeys.teamPendingKeys(team.id) });
      await qc.invalidateQueries({ queryKey: ["vaults"] });
      snack.notify(n === 1 ? "Vault key granted" : `${n} vault keys granted`);
    },
    onError: (e) => {
      if (!(e instanceof UnlockCancelled)) snack.error(errorMessage(e));
    },
  });

  const items = pending.data?.items ?? [];
  if (pending.isPending || items.length === 0) return null;

  const vaultName = (id: string) => vaults.data?.vaults.find((v) => v.id === id)?.name ?? "Vault";
  const memberOf = (id: string) => members.data?.members.find((m) => m.user_id === id);
  const grantable = items.filter(
    (p) => vaults.data?.vaults.find((v) => v.id === p.vault_id)?.sealed_key,
  );

  return (
    <Section
      title="Pending vault keys"
      description="These members joined a vault but do not have its key yet. Granting seals the key to their public key in your browser — the server never sees it."
      actions={
        <Button
          variant="contained"
          startIcon={<LockOpenRoundedIcon />}
          disabled={grant.isPending || grantable.length === 0}
          onClick={() => grant.mutate(grantable)}
        >
          Grant all ({grantable.length})
        </Button>
      }
      disablePadding
    >
      <List disablePadding>
        {items.map((p) => {
          const m = memberOf(p.user_id);
          const canGrant = grantable.includes(p);
          return (
            <ListItem
              key={`${p.vault_id}:${p.user_id}`}
              divider
              secondaryAction={
                <Button
                  size="small"
                  variant="outlined"
                  disabled={grant.isPending || !canGrant}
                  onClick={() => grant.mutate([p])}
                >
                  Grant
                </Button>
              }
            >
              <ListItemText
                primary={m ? (m.display_name ?? m.email) : p.user_id}
                secondary={
                  <>
                    <Link component={RouterLink} to={`/vaults/${p.vault_id}`}>
                      {vaultName(p.vault_id)}
                    </Link>{" "}
                    as {p.role}
                    {!canGrant && " · you do not hold this vault's key"}
                  </>
                }
              />
            </ListItem>
          );
        })}
      </List>
    </Section>
  );
}

function TeamVaultsSection({ team }: { team: Team }) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const snack = useSnackbar();
  const { session } = useAuthState();
  const vaults = useQuery({ queryKey: queryKeys.vaults, queryFn: vaultsApi.list });
  const members = useQuery({
    queryKey: queryKeys.teamMembers(team.id),
    queryFn: () => teamsApi.members(team.id),
  });
  const teamVaults = (vaults.data?.vaults ?? []).filter((v) => v.team_id === team.id);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [roles, setRoles] = useState<Record<string, VaultRole | "none">>({});

  const create = useMutation({
    mutationFn: async () => {
      const me = session?.user;
      if (!me) throw new Error("Not signed in");
      const all = members.data?.members ?? [];
      const chosen = all.filter(
        (m) => m.user_id === me.id || (roles[m.user_id] ?? "none") !== "none",
      );
      const sealed = await newSealedVaultKey(chosen);
      const upserts = sealed.map((s) => {
        const role: VaultRole = s.user_id === me.id ? "manager" : (roles[s.user_id] as VaultRole);
        return { user_id: s.user_id, role, sealed_key: s.sealed_key };
      });
      return teamsApi.createVault(team.id, name.trim(), upserts);
    },
    onSuccess: async (v: Vault) => {
      setOpen(false);
      setName("");
      setRoles({});
      await qc.invalidateQueries({ queryKey: queryKeys.vaults });
      void navigate(`/vaults/${v.id}`);
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  const admin = isAdmin(team.my_role);

  return (
    <Section
      title="Team vaults"
      description="Each vault has its own key, sealed to every member. You are always the first manager of a vault you create."
      actions={
        admin && (
          <Button variant="contained" startIcon={<AddRoundedIcon />} onClick={() => setOpen(true)}>
            New vault
          </Button>
        )
      }
      disablePadding
    >
      {vaults.isPending ? (
        <Loading minHeight={100} />
      ) : teamVaults.length === 0 ? (
        <EmptyState
          title="No team vaults yet"
          description={admin ? "Create a vault to start sharing hosts and keys." : undefined}
        />
      ) : (
        <List disablePadding>
          {teamVaults.map((v) => (
            <ListItem
              key={v.id}
              divider
              component={RouterLink}
              to={`/vaults/${v.id}`}
              sx={{ color: "inherit", textDecoration: "none" }}
            >
              <ListItemText
                primary={
                  <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
                    <span>{v.name}</span>
                    <RoleChip role={v.my_role} />
                    {!v.sealed_key && (
                      <Typography variant="caption" color="warning.main">
                        key pending
                      </Typography>
                    )}
                  </Stack>
                }
                secondary={`Key version ${v.key_version} · created ${formatDate(v.created_at)}`}
              />
            </ListItem>
          ))}
        </List>
      )}

      <Dialog
        open={open}
        onClose={create.isPending ? undefined : () => setOpen(false)}
        maxWidth="sm"
        fullWidth
      >
        <form
          onSubmit={(e: SubmitEvent) => {
            e.preventDefault();
            create.mutate();
          }}
        >
          <DialogTitle>New team vault</DialogTitle>
          <DialogContent sx={{ display: "grid", gap: 2 }}>
            <TextField
              autoFocus
              label="Vault name"
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={create.isPending}
            />
            <Typography variant="subtitle2">Members</Typography>
            <Table size="small">
              <TableBody>
                {(members.data?.members ?? []).map((m) => {
                  const isMe = m.user_id === session?.user.id;
                  return (
                    <TableRow key={m.user_id}>
                      <TableCell sx={{ pl: 0 }}>
                        <UserCell email={m.email} displayName={m.display_name} you={isMe} />
                      </TableCell>
                      <TableCell align="right" sx={{ pr: 0 }}>
                        {isMe ? (
                          <RoleChip role="manager" />
                        ) : (
                          <Select
                            size="small"
                            value={roles[m.user_id] ?? "none"}
                            onChange={(e) =>
                              setRoles((r) => ({ ...r, [m.user_id]: e.target.value }))
                            }
                            sx={{ minWidth: 130 }}
                          >
                            <MenuItem value="none">No access</MenuItem>
                            <MenuItem value="viewer">Viewer</MenuItem>
                            <MenuItem value="editor">Editor</MenuItem>
                            <MenuItem value="manager">Manager</MenuItem>
                          </Select>
                        )}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button onClick={() => setOpen(false)} color="inherit" disabled={create.isPending}>
              Cancel
            </Button>
            <Button
              type="submit"
              variant="contained"
              disabled={create.isPending || name.trim() === ""}
            >
              Create
            </Button>
          </DialogActions>
        </form>
      </Dialog>
    </Section>
  );
}
