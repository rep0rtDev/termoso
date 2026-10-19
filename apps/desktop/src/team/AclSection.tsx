import { useState } from "react";
import {
  Alert,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  MenuItem,
  Select,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { tr } from "@/i18n";
import * as ipc from "@/ipc/commands";
import { useGroups, useHosts, useVaultMembers } from "@/ipc/hooks";
import { errorMessage, type Team, type LocalVault } from "@/ipc/types";
import type { AclEntityKind, AclMode, AclPrincipalKind, EntityAclRule } from "@/ipc/types";
import { Loading } from "@/components/ui";
import { EmptyState } from "@/components/EmptyState";
import { useSnackbar } from "@/components/Snackbar";
import { Section } from "./Section";
import { can } from "./roles";

const MODES: { value: AclMode; label: () => string; hint: () => string }[] = [
  { value: "none", label: () => tr("Hidden"), hint: () => tr("Not shown at all") },
  {
    value: "connect",
    label: () => tr("Connect only"),
    hint: () => tr("Can connect, secrets stay hidden"),
  },
  { value: "read", label: () => tr("Read"), hint: () => tr("Sees everything, cannot change") },
  { value: "edit", label: () => tr("Edit"), hint: () => tr("Full access to the host or group") },
];

/** Group/host access rules of a team vault, picked by name (the desktop holds the vault key). */
export function AclSection({ vault, team }: { vault: LocalVault; team: Team }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const manage = can(team, "acl.manage");
  const key = ["team", team.id, "acl", vault.id];
  const q = useQuery({ queryKey: key, queryFn: () => ipc.teamAcl(team.id, vault.id) });
  const hosts = useHosts(vault.id);
  const groups = useGroups(vault.id);
  const [adding, setAdding] = useState(false);
  const refresh = () => qc.invalidateQueries({ queryKey: key });
  const entityName = (r: EntityAclRule) =>
    r.entity_kind === "host"
      ? (hosts.data?.find((h) => h.id === r.entity_id)?.label ?? r.entity_id.slice(0, 8))
      : (groups.data?.find((g) => g.id === r.entity_id)?.label ?? r.entity_id.slice(0, 8));
  const setMode = useMutation({
    mutationFn: ({ r, mode }: { r: EntityAclRule; mode: AclMode }) =>
      ipc.teamUpsertAcl(team.id, vault.id, {
        entity_id: r.entity_id,
        entity_kind: r.entity_kind,
        principal_kind: r.principal_kind,
        role_id: r.role_id ?? undefined,
        user_id: r.user_id ?? undefined,
        group_name: r.group_name ?? undefined,
        mode,
      }),
    onSuccess: refresh,
    onError: (e) => snack.error(errorMessage(e)),
  });
  const del = useMutation({
    mutationFn: (r: EntityAclRule) => ipc.teamDeleteAcl(team.id, vault.id, r.id),
    onSuccess: async () => {
      await refresh();
      snack.notify(tr("Rule removed"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  if (q.isPending) return <Loading />;
  if (q.isError) return <Alert severity="error">{errorMessage(q.error)}</Alert>;
  const rules = q.data;
  if (!manage && rules.length === 0) return null;
  const principalChip = (r: EntityAclRule) =>
    r.principal_kind === "role"
      ? tr("Role")
      : r.principal_kind === "member"
        ? tr("Member")
        : tr("IdP group");
  return (
    <Section
      title={tr("Group and host access")}
      description={tr(
        "Rules narrow what a role, a member or an IdP group sees inside this vault: a group or host can be hidden, connect-only (no secrets), read-only or editable. Most specific rule wins (member › IdP group › role); a group rule covers the hosts inside it. Owners and admins are not limited.",
      )}
      actions={
        manage && (
          <Button variant="outlined" startIcon={<AddRoundedIcon />} onClick={() => setAdding(true)}>
            {tr("Add rule")}
          </Button>
        )
      }
    >
      <Alert severity="info" sx={{ mb: 2 }}>
        <strong>{tr("ACL does not replace a separate vault.")}</strong>{" "}
        {tr(
          "Every member of this vault holds its key and could decrypt everything in it with a modified client. Rules are for order and discipline inside the vault; hosts that some members must never see belong in a vault they are not a member of.",
        )}
      </Alert>
      {rules.length === 0 ? (
        <EmptyState
          title={tr("No rules")}
          description={tr(
            "Everyone in the vault sees every group and host according to their team permissions.",
          )}
          compact
        />
      ) : (
        <TableContainer>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>{tr("Who")}</TableCell>
                <TableCell>{tr("What")}</TableCell>
                <TableCell>{tr("Access")}</TableCell>
                {manage && <TableCell align="right" />}
              </TableRow>
            </TableHead>
            <TableBody>
              {rules.map((r) => (
                <TableRow key={r.id} hover>
                  <TableCell>
                    <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
                      <Chip size="small" variant="outlined" label={principalChip(r)} />
                      <Typography variant="body2">{r.principal_name}</Typography>
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2">{entityName(r)}</Typography>
                    <Typography variant="caption" color="text.secondary">
                      {r.entity_kind === "group" ? tr("Group") : tr("Host")}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    {manage ? (
                      <Select
                        size="small"
                        value={r.mode}
                        disabled={setMode.isPending}
                        onChange={(e) => setMode.mutate({ r, mode: e.target.value })}
                        sx={{ minWidth: 150 }}
                      >
                        {MODES.map((m) => (
                          <MenuItem key={m.value} value={m.value}>
                            {m.label()}
                          </MenuItem>
                        ))}
                      </Select>
                    ) : (
                      (MODES.find((m) => m.value === r.mode)?.label() ?? r.mode)
                    )}
                  </TableCell>
                  {manage && (
                    <TableCell align="right">
                      <Tooltip title={tr("Remove rule")}>
                        <IconButton
                          size="small"
                          aria-label={tr("Remove rule")}
                          disabled={del.isPending}
                          onClick={() => del.mutate(r)}
                        >
                          <DeleteOutlineRoundedIcon fontSize="small" />
                        </IconButton>
                      </Tooltip>
                    </TableCell>
                  )}
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      )}
      {manage && (
        <AddRuleDialog
          open={adding}
          vault={vault}
          team={team}
          onClose={() => setAdding(false)}
          onDone={() => {
            setAdding(false);
            void refresh();
          }}
        />
      )}
    </Section>
  );
}

function AddRuleDialog({
  open,
  vault,
  team,
  onClose,
  onDone,
}: {
  open: boolean;
  vault: LocalVault;
  team: Team;
  onClose: () => void;
  onDone: () => void;
}) {
  const snack = useSnackbar();
  const roles = useQuery({
    queryKey: ["team", team.id, "roles"],
    queryFn: () => ipc.teamRoles(team.id),
    enabled: open,
  });
  const members = useVaultMembers(open ? vault.id : null);
  const hosts = useHosts(open ? vault.id : null);
  const groups = useGroups(open ? vault.id : null);
  const [principalKind, setPrincipalKind] = useState<AclPrincipalKind>("role");
  const [roleId, setRoleId] = useState("");
  const [userId, setUserId] = useState("");
  const [groupName, setGroupName] = useState("");
  const [entityKind, setEntityKind] = useState<AclEntityKind>("group");
  const [entityId, setEntityId] = useState("");
  const [mode, setMode] = useState<AclMode>("connect");
  const principalOk =
    principalKind === "role"
      ? roleId !== ""
      : principalKind === "member"
        ? userId !== ""
        : groupName.trim() !== "";
  const valid = principalOk && entityId !== "";
  const m = useMutation({
    mutationFn: () =>
      ipc.teamUpsertAcl(team.id, vault.id, {
        entity_id: entityId,
        entity_kind: entityKind,
        principal_kind: principalKind,
        role_id: principalKind === "role" ? roleId : undefined,
        user_id: principalKind === "member" ? userId : undefined,
        group_name: principalKind === "group" ? groupName.trim() : undefined,
        mode,
      }),
    onSuccess: () => {
      snack.notify(tr("Rule added"));
      setEntityId("");
      onDone();
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const roleList = (roles.data?.roles ?? []).filter((r) => r.slug !== "owner");
  const entities =
    entityKind === "host"
      ? (hosts.data ?? []).map((h) => ({ id: h.id, label: h.label }))
      : (groups.data ?? []).map((g) => ({ id: g.id, label: g.label }));
  return (
    <Dialog open={open} onClose={onClose} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("Add access rule")}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 1 }}>
          <TextField
            select
            size="small"
            label={tr("Applies to")}
            value={principalKind}
            onChange={(e) => setPrincipalKind(e.target.value as AclPrincipalKind)}
          >
            <MenuItem value="role">{tr("Team role")}</MenuItem>
            <MenuItem value="member">{tr("Vault member")}</MenuItem>
            <MenuItem value="group">{tr("IdP / SCIM group")}</MenuItem>
          </TextField>
          {principalKind === "role" && (
            <TextField
              select
              size="small"
              label={tr("Role")}
              value={roleId}
              onChange={(e) => setRoleId(e.target.value)}
            >
              {roleList.map((r) => (
                <MenuItem key={r.id} value={r.id}>
                  {tr(r.name)}
                </MenuItem>
              ))}
            </TextField>
          )}
          {principalKind === "member" && (
            <TextField
              select
              size="small"
              label={tr("Member")}
              value={userId}
              onChange={(e) => setUserId(e.target.value)}
            >
              {(members.data ?? []).map((u) => (
                <MenuItem key={u.user_id} value={u.user_id}>
                  {u.display_name ?? u.email}
                </MenuItem>
              ))}
            </TextField>
          )}
          {principalKind === "group" && (
            <TextField
              size="small"
              label={tr("Group name")}
              value={groupName}
              onChange={(e) => setGroupName(e.target.value)}
              helperText={tr("Exactly as the IdP or SCIM sends it.")}
            />
          )}
          <TextField
            select
            size="small"
            label={tr("Object")}
            value={entityKind}
            onChange={(e) => {
              setEntityKind(e.target.value as AclEntityKind);
              setEntityId("");
            }}
          >
            <MenuItem value="group">{tr("Group (and the hosts inside)")}</MenuItem>
            <MenuItem value="host">{tr("Host")}</MenuItem>
          </TextField>
          <TextField
            select
            size="small"
            label={entityKind === "host" ? tr("Host") : tr("Group")}
            value={entityId}
            onChange={(e) => setEntityId(e.target.value)}
          >
            {entities.map((x) => (
              <MenuItem key={x.id} value={x.id}>
                {x.label}
              </MenuItem>
            ))}
          </TextField>
          <TextField
            select
            size="small"
            label={tr("Access")}
            value={mode}
            onChange={(e) => setMode(e.target.value as AclMode)}
            helperText={MODES.find((x) => x.value === mode)?.hint()}
          >
            {MODES.map((x) => (
              <MenuItem key={x.value} value={x.value}>
                {x.label()}
              </MenuItem>
            ))}
          </TextField>
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{tr("Cancel")}</Button>
        <Button variant="contained" disabled={!valid || m.isPending} onClick={() => m.mutate()}>
          {tr("Add")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
