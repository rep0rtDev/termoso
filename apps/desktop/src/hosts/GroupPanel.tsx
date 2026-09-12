import { useState } from "react";
import {
  Box,
  Button,
  FormControlLabel,
  IconButton,
  MenuItem,
  Radio,
  RadioGroup,
  TextField,
  Typography,
} from "@mui/material";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import { Field, Loading, SectionCard, SidePanel, ToolIconButton } from "@/components/ui";
import {
  useDeleteGroup,
  useDuplicateGroup,
  useGroupForm,
  useGroups,
  useHostChains,
  useInherited,
  useProxies,
  useSaveGroupForm,
} from "@/ipc/hooks";
import {
  emptyGroupForm,
  errorMessage,
  type GroupForm,
  type GroupNode,
  type Uuid,
} from "@/ipc/types";
import { monoFontFamily, sizes } from "@/theme/theme";
import { AgentForwardingRow, CredentialsFields } from "./CredentialsFields";
import { ChainDialog, ProxyDialog } from "./HostAdvancedDialogs";

interface Props {
  vaultId: Uuid;
  /** `null` creates a new group. */
  groupId: Uuid | null;
  initialParentId: Uuid | null;
  onClose: () => void;
  onDeleted: (id: Uuid, parentId: Uuid | null) => void;
  onDuplicated: (g: GroupNode) => void;
}

/** Group Details: name, parent, and the SSH defaults every host inside inherits. */
export function GroupPanel({
  vaultId,
  groupId,
  initialParentId,
  onClose,
  onDeleted,
  onDuplicated,
}: Props) {
  const loaded = useGroupForm(groupId);
  if (groupId !== null && loaded.data === undefined) {
    return (
      <SidePanel title="Group details" onClose={onClose} width={sizes.panel}>
        {loaded.error ? (
          <Typography color="error">{errorMessage(loaded.error)}</Typography>
        ) : (
          <Loading pt={6} />
        )}
      </SidePanel>
    );
  }
  return (
    <GroupEditor
      vaultId={vaultId}
      groupId={groupId}
      initial={loaded.data ?? emptyGroupForm(vaultId, initialParentId)}
      onClose={onClose}
      onDeleted={onDeleted}
      onDuplicated={onDuplicated}
    />
  );
}

const clampPort = (raw: string) =>
  raw === "" ? null : Math.max(1, Math.min(65535, Number(raw) || 1));
const clampSeconds = (raw: string) =>
  raw === "" ? null : Math.max(0, Math.min(86400, Math.floor(Number(raw) || 0)));

/** Groups that may become the parent: everything except the group itself and its subtree. */
export function parentCandidates(groups: GroupNode[], selfId: Uuid | null) {
  if (!selfId) return groups;
  const blocked = new Set<Uuid>([selfId]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const g of groups) {
      if (g.parentId && blocked.has(g.parentId) && !blocked.has(g.id)) {
        blocked.add(g.id);
        grew = true;
      }
    }
  }
  return groups.filter((g) => !blocked.has(g.id));
}

/** `Parent / Child` labels so same-named groups in different branches stay distinguishable. */
export function groupPathLabel(groups: GroupNode[], id: Uuid) {
  const byId = new Map(groups.map((g) => [g.id, g]));
  const parts: string[] = [];
  let cur = byId.get(id);
  let hops = 0;
  while (cur && hops++ < 64) {
    parts.unshift(cur.label);
    cur = cur.parentId ? byId.get(cur.parentId) : undefined;
  }
  return parts.join(" / ");
}

function GroupEditor({
  vaultId,
  groupId,
  initial,
  onClose,
  onDeleted,
  onDuplicated,
}: Omit<Props, "initialParentId"> & { initial: GroupForm }) {
  const snackbar = useSnackbar();
  const groups = useGroups(vaultId);
  const proxies = useProxies(vaultId);
  const chains = useHostChains(vaultId);
  const save = useSaveGroupForm();
  const duplicate = useDuplicateGroup();

  const [form, setForm] = useState<GroupForm>(initial);
  const [touched, setTouched] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [dialog, setDialog] = useState<"proxy" | "chain" | null>(null);

  const inherited = useInherited(form.parentId);
  const inh = inherited.data ?? null;
  const inheritedFrom = inh && inh.groupPath.length > 0 ? inh.groupPath.join(" / ") : null;

  const node = (groups.data ?? []).find((g) => g.id === groupId) ?? null;
  const set = <K extends keyof GroupForm>(k: K, v: GroupForm[K]) => {
    setTouched(true);
    setForm((f) => ({ ...f, [k]: v }));
  };
  const patch = (p: Partial<GroupForm>) => {
    setTouched(true);
    setForm((f) => ({ ...f, ...p }));
  };

  const canSave = form.label.trim().length > 0 && !save.isPending;
  const onSave = () =>
    save.mutate(
      { ...form, label: form.label.trim() },
      {
        onSuccess: (g) => {
          snackbar.notify(groupId ? "Group saved" : `Group “${g.label}” created`);
          onClose();
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );

  const onDuplicate = () => {
    if (!groupId) return;
    duplicate.mutate(groupId, {
      onSuccess: (g) => {
        snackbar.notify(`Duplicated as “${g.label}”`);
        onDuplicated(g);
      },
      onError: (e) => snackbar.error(errorMessage(e)),
    });
  };

  const contents = node ? groupContents(node) : null;

  return (
    <SidePanel
      title={groupId ? form.label || "Group details" : "New group"}
      subtitle={groupId ? ["Group", contents].filter(Boolean).join(" · ") : undefined}
      onClose={onClose}
      width={sizes.panel}
      actions={
        groupId && (
          <>
            <ToolIconButton
              title="Duplicate group"
              disabled={duplicate.isPending}
              onClick={onDuplicate}
            >
              <ContentCopyRoundedIcon fontSize="small" />
            </ToolIconButton>
            <ToolIconButton
              title="Delete group"
              color="error"
              onClick={() => setConfirmDelete(true)}
            >
              <DeleteOutlineRoundedIcon fontSize="small" />
            </ToolIconButton>
          </>
        )
      }
      footer={
        <>
          <Button color="inherit" onClick={onClose} disabled={save.isPending}>
            {groupId && !touched ? "Close" : "Cancel"}
          </Button>
          <Button
            variant="contained"
            disabled={!canSave || (!!groupId && !touched)}
            onClick={onSave}
          >
            {save.isPending ? "Saving…" : "Save"}
          </Button>
        </>
      }
    >
      <Box
        component="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (canSave && (touched || !groupId)) onSave();
        }}
        sx={{ display: "contents" }}
      >
        <SectionCard title="Group">
          <Field label="Name">
            <TextField
              required
              autoFocus={!groupId}
              value={form.label}
              onChange={(e) => set("label", e.target.value)}
              placeholder="Production, Staging, Home lab…"
              error={touched && form.label.trim().length === 0}
            />
          </Field>
          <Field
            label="Parent group"
            hint={
              inheritedFrom
                ? `Defaults not set here are inherited from ${inheritedFrom}.`
                : undefined
            }
          >
            <TextField
              select
              value={form.parentId ?? ""}
              onChange={(e) => set("parentId", e.target.value === "" ? null : e.target.value)}
            >
              <MenuItem value="">
                <em>Top level</em>
              </MenuItem>
              {parentCandidates(groups.data ?? [], groupId).map((g) => (
                <MenuItem key={g.id} value={g.id}>
                  {groupPathLabel(groups.data ?? [], g.id)}
                </MenuItem>
              ))}
            </TextField>
          </Field>
        </SectionCard>

        <SectionCard>
          <CredentialsFields
            vaultId={vaultId}
            ssh
            agentForwarding={false}
            inherited={inh}
            inlineLabel="Set on this group"
            value={form}
            onChange={patch}
          />
          <Typography variant="caption" color="text.secondary">
            Hosts in this group use these when their own credentials are left empty.
          </Typography>
        </SectionCard>

        <SectionCard title="Connection">
          <AgentForwardingRow value={form} onChange={patch} inherited={inh} />
          <Field label="Port" sx={{ width: 140 }}>
            <TextField
              type="number"
              value={form.port ?? ""}
              onChange={(e) => set("port", clampPort(e.target.value))}
              placeholder={String(inh?.port ?? 22)}
              slotProps={{ htmlInput: { min: 1, max: 65535 } }}
            />
          </Field>
          <Field label="Jump host">
            <TextField
              select
              value={form.hostChainId ?? ""}
              onChange={(e) => {
                const v = e.target.value;
                if (v === "__new") setDialog("chain");
                else set("hostChainId", v === "" ? null : v);
              }}
            >
              <MenuItem value="">
                <em>Direct connection</em>
              </MenuItem>
              {(chains.data ?? []).map((c) => (
                <MenuItem key={c.id} value={c.id}>
                  {c.data.label}
                  <Typography
                    component="span"
                    variant="caption"
                    color="text.secondary"
                    sx={{ ml: 1 }}
                  >
                    {c.data.host_ids.length} hop{c.data.host_ids.length === 1 ? "" : "s"}
                  </Typography>
                </MenuItem>
              ))}
              <MenuItem value="__new" sx={{ color: "primary.main" }}>
                <AddRoundedIcon fontSize="small" sx={{ mr: 1 }} />
                New host chain…
              </MenuItem>
            </TextField>
          </Field>
          <Field label="Proxy">
            <TextField
              select
              value={form.proxyId ?? ""}
              onChange={(e) => {
                const v = e.target.value;
                if (v === "__new") setDialog("proxy");
                else set("proxyId", v === "" ? null : v);
              }}
            >
              <MenuItem value="">
                <em>None</em>
              </MenuItem>
              {(proxies.data ?? []).map((p) => (
                <MenuItem key={p.id} value={p.id}>
                  {p.data.kind.toUpperCase()}
                  <Typography
                    component="span"
                    variant="caption"
                    color="text.secondary"
                    sx={{ ml: 1, fontFamily: monoFontFamily }}
                  >
                    {p.data.host}:{p.data.port}
                  </Typography>
                </MenuItem>
              ))}
              <MenuItem value="__new" sx={{ color: "primary.main" }}>
                <AddRoundedIcon fontSize="small" sx={{ mr: 1 }} />
                New proxy…
              </MenuItem>
            </TextField>
          </Field>
          <Box sx={{ display: "flex", gap: 1.5 }}>
            <Field label="Keep-alive, s" sx={{ flex: 1 }}>
              <TextField
                type="number"
                value={form.keepAliveInterval ?? ""}
                onChange={(e) => set("keepAliveInterval", clampSeconds(e.target.value))}
                placeholder={inh?.keepAliveInterval?.toString() ?? "default"}
                slotProps={{ htmlInput: { min: 0, max: 86400 } }}
              />
            </Field>
            <Field label="Timeout, s" sx={{ flex: 1 }}>
              <TextField
                type="number"
                value={form.timeout ?? ""}
                onChange={(e) => set("timeout", clampSeconds(e.target.value))}
                placeholder={inh?.timeout?.toString() ?? "default"}
                slotProps={{ htmlInput: { min: 0, max: 86400 } }}
              />
            </Field>
          </Box>
        </SectionCard>

        <SectionCard
          title="Environment variables"
          action={
            <Button
              size="small"
              color="inherit"
              startIcon={<AddRoundedIcon />}
              onClick={() => set("envVariables", [...form.envVariables, ["", ""]])}
            >
              Add
            </Button>
          }
        >
          {form.envVariables.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              Sent with every session to hosts in this group.
            </Typography>
          ) : (
            form.envVariables.map(([k, v], i) => (
              <Box key={i} sx={{ display: "flex", gap: 1, alignItems: "center" }}>
                <TextField
                  value={k}
                  placeholder="NAME"
                  onChange={(e) =>
                    set(
                      "envVariables",
                      form.envVariables.map((row, j) => (j === i ? [e.target.value, row[1]] : row)),
                    )
                  }
                  slotProps={{ input: { sx: { fontFamily: monoFontFamily } } }}
                  sx={{ flex: 1 }}
                />
                <TextField
                  value={v}
                  placeholder="value"
                  onChange={(e) =>
                    set(
                      "envVariables",
                      form.envVariables.map((row, j) => (j === i ? [row[0], e.target.value] : row)),
                    )
                  }
                  slotProps={{ input: { sx: { fontFamily: monoFontFamily } } }}
                  sx={{ flex: 1.4 }}
                />
                <IconButton
                  size="small"
                  aria-label="Remove variable"
                  onClick={() =>
                    set(
                      "envVariables",
                      form.envVariables.filter((_row, j) => j !== i),
                    )
                  }
                >
                  <CloseRoundedIcon fontSize="small" />
                </IconButton>
              </Box>
            ))
          )}
        </SectionCard>
      </Box>

      <ProxyDialog
        open={dialog === "proxy"}
        vaultId={vaultId}
        onClose={() => setDialog(null)}
        onCreated={(id) => set("proxyId", id)}
      />
      <ChainDialog
        open={dialog === "chain"}
        vaultId={vaultId}
        excludeHostId={null}
        onClose={() => setDialog(null)}
        onCreated={(id) => set("hostChainId", id)}
      />

      {node && (
        <DeleteGroupDialog
          group={confirmDelete ? node : null}
          onClose={() => setConfirmDelete(false)}
          onDeleted={(id, parentId) => {
            onDeleted(id, parentId);
            onClose();
          }}
        />
      )}
    </SidePanel>
  );
}

function groupContents(g: GroupNode) {
  return [
    g.groupCount > 0 ? `${g.groupCount} group${g.groupCount === 1 ? "" : "s"}` : null,
    `${g.hostCount} host${g.hostCount === 1 ? "" : "s"}`,
  ]
    .filter(Boolean)
    .join(", ");
}

/** Remove a group, either lifting its contents to the parent or deleting the whole subtree. */
export function DeleteGroupDialog({
  group,
  onClose,
  onDeleted,
}: {
  group: GroupNode | null;
  onClose: () => void;
  onDeleted: (id: Uuid, parentId: Uuid | null) => void;
}) {
  const snackbar = useSnackbar();
  const del = useDeleteGroup();
  const [mode, setMode] = useState<"lift" | "all">("lift");
  const empty = group ? group.hostCount === 0 && group.groupCount === 0 : true;
  const onConfirm = () => {
    if (!group) return;
    const recursive = !empty && mode === "all";
    del.mutate(
      { id: group.id, vaultId: group.vaultId, recursive },
      {
        onSuccess: () => {
          snackbar.notify(
            empty
              ? `Removed “${group.label}”`
              : recursive
                ? "Group and everything inside removed"
                : "Group removed; its contents moved up one level",
            "info",
          );
          onClose();
          onDeleted(group.id, group.parentId);
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );
  };
  return (
    <ConfirmDialog
      open={group !== null}
      title={empty ? "Remove group?" : "Delete group?"}
      danger
      confirmLabel={empty ? "Remove" : "Delete"}
      busy={del.isPending}
      onCancel={onClose}
      onConfirm={onConfirm}
    >
      {group && (
        <>
          <Typography variant="body2" sx={{ mb: empty ? 0 : 1.5 }}>
            {empty
              ? `“${group.label}” is empty.`
              : `“${group.label}” contains ${groupContents(group)}.`}
          </Typography>
          {!empty && (
            <RadioGroup value={mode} onChange={(e) => setMode(e.target.value as "lift" | "all")}>
              <FormControlLabel
                value="lift"
                control={<Radio size="small" />}
                label="Keep the contents — move them up one level"
              />
              <FormControlLabel
                value="all"
                control={<Radio size="small" />}
                label="Delete the group with all its hosts and sub-groups"
              />
            </RadioGroup>
          )}
        </>
      )}
    </ConfirmDialog>
  );
}
