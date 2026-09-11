import { useState } from "react";
import {
  Box,
  Button,
  Chip,
  IconButton,
  InputAdornment,
  MenuItem,
  Switch,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import VisibilityRoundedIcon from "@mui/icons-material/VisibilityRounded";
import VisibilityOffRoundedIcon from "@mui/icons-material/VisibilityOffRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import ExpandLessRoundedIcon from "@mui/icons-material/ExpandLessRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import LabelOutlinedIcon from "@mui/icons-material/LabelOutlined";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import {
  Field,
  Loading,
  SectionCard,
  SettingRow,
  SidePanel,
  ToolIconButton,
} from "@/components/ui";
import {
  useCreateTag,
  useDeleteHost,
  useGroups,
  useHostChains,
  useHostForm,
  useIdentities,
  useProxies,
  useSaveHost,
  useSnippets,
  useSshKeys,
  useTags,
} from "@/ipc/hooks";
import { emptyHostForm, errorMessage, type HostForm, type Uuid } from "@/ipc/types";
import { goToSftp } from "@/app/navigation";
import { openTerminal } from "@/terminal/store";
import { openSftpForHost } from "@/sftp/store";
import { monoFontFamily, sizes } from "@/theme/theme";
import { ChainDialog, ProxyDialog } from "./HostAdvancedDialogs";

interface Props {
  vaultId: Uuid;
  hostId: Uuid | null;
  initialGroupId: Uuid | null;
  onClose: () => void;
}

/** Loads the form for an existing host (or starts blank) and hands it to the editor. */
export function HostEditPanel({ vaultId, hostId, initialGroupId, onClose }: Props) {
  const loaded = useHostForm(hostId);
  if (hostId !== null && loaded.data === undefined) {
    return (
      <SidePanel title="Edit host" onClose={onClose} width={sizes.panel}>
        {loaded.error ? (
          <Typography color="error">{errorMessage(loaded.error)}</Typography>
        ) : (
          <Loading pt={6} />
        )}
      </SidePanel>
    );
  }
  return (
    <HostEditor
      vaultId={vaultId}
      hostId={hostId}
      initial={loaded.data ?? emptyHostForm(vaultId, initialGroupId)}
      onClose={onClose}
    />
  );
}

const clampPort = (raw: string) =>
  raw === "" ? null : Math.max(1, Math.min(65535, Number(raw) || 1));

const clampSeconds = (raw: string) =>
  raw === "" ? null : Math.max(0, Math.min(86400, Math.floor(Number(raw) || 0)));

function HostEditor({
  vaultId,
  hostId,
  initial,
  onClose,
}: {
  vaultId: Uuid;
  hostId: Uuid | null;
  initial: HostForm;
  onClose: () => void;
}) {
  const snackbar = useSnackbar();
  const groups = useGroups(vaultId);
  const tags = useTags(vaultId);
  const identities = useIdentities(vaultId);
  const sshKeys = useSshKeys(vaultId);
  const snippets = useSnippets(vaultId);
  const proxies = useProxies(vaultId);
  const chains = useHostChains(vaultId);
  const save = useSaveHost();
  const del = useDeleteHost();

  const [form, setForm] = useState<HostForm>(initial);
  const [showPassword, setShowPassword] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [touched, setTouched] = useState(false);
  const [more, setMore] = useState(
    initial.hostChainId !== null ||
      initial.proxyId !== null ||
      initial.startupSnippetId !== null ||
      initial.envVariables.length > 0 ||
      initial.keepAliveInterval !== null ||
      initial.timeout !== null,
  );
  const [dialog, setDialog] = useState<"proxy" | "chain" | null>(null);
  const [newTag, setNewTag] = useState("");
  const createTag = useCreateTag();

  const addTag = () => {
    const label = newTag.trim();
    if (!label) return;
    const existing = (tags.data ?? []).find((t) => t.label.toLowerCase() === label.toLowerCase());
    if (existing) {
      if (!form.tagIds.includes(existing.id)) set("tagIds", [...form.tagIds, existing.id]);
      setNewTag("");
      return;
    }
    createTag.mutate(
      { vaultId, label },
      {
        onSuccess: (e) => {
          set("tagIds", [...form.tagIds, e.id]);
          setNewTag("");
        },
        onError: (err) => snackbar.error(errorMessage(err)),
      },
    );
  };

  const set = <K extends keyof HostForm>(k: K, v: HostForm[K]) => {
    setTouched(true);
    setForm((f) => ({ ...f, [k]: v }));
  };

  const ssh = form.protocol === "ssh";
  const usingIdentity = form.identityId !== null;
  const canSave = form.address.trim().length > 0 && !save.isPending;
  const defaultPort = ssh ? 22 : 23;

  const onSave = (thenConnect: boolean) => {
    save.mutate(form, {
      onSuccess: (card) => {
        snackbar.notify(hostId ? "Host saved" : `Host “${card.label}” added`);
        if (thenConnect) openTerminal({ kind: "host", host_id: card.id });
        onClose();
      },
      onError: (e) => snackbar.error(errorMessage(e)),
    });
  };

  const onDelete = () => {
    if (!hostId) return;
    del.mutate(
      { id: hostId, vaultId },
      {
        onSuccess: () => {
          snackbar.notify("Host deleted");
          setConfirmDelete(false);
          onClose();
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );
  };

  const footer =
    hostId && !touched ? (
      <>
        <Button color="inherit" onClick={onClose}>
          Close
        </Button>
        <Button
          variant="contained"
          startIcon={<PlayArrowRoundedIcon />}
          onClick={() => openTerminal({ kind: "host", host_id: hostId })}
        >
          Connect
        </Button>
      </>
    ) : (
      <>
        <Button color="inherit" onClick={onClose} disabled={save.isPending}>
          Cancel
        </Button>
        {!hostId && (
          <Button variant="tonal" disabled={!canSave} onClick={() => onSave(true)}>
            Save & connect
          </Button>
        )}
        <Button variant="contained" disabled={!canSave} onClick={() => onSave(false)}>
          {save.isPending ? "Saving…" : "Save"}
        </Button>
      </>
    );

  return (
    <SidePanel
      title={hostId ? form.label || form.address || "Edit host" : "New host"}
      subtitle={hostId ? (ssh ? "SSH host" : "Telnet host") : undefined}
      onClose={onClose}
      width={sizes.panel}
      actions={
        hostId && (
          <>
            {ssh && (
              <ToolIconButton
                title={touched ? "Save before opening SFTP" : "Open SFTP"}
                disabled={touched}
                onClick={() => {
                  openSftpForHost(hostId, form.label || form.address);
                  goToSftp();
                }}
              >
                <FolderCopyRoundedIcon fontSize="small" />
              </ToolIconButton>
            )}
            <ToolIconButton
              title="Delete host"
              color="error"
              onClick={() => setConfirmDelete(true)}
            >
              <DeleteOutlineRoundedIcon fontSize="small" />
            </ToolIconButton>
          </>
        )
      }
      footer={footer}
    >
      <Box
        component="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (canSave && (touched || !hostId)) onSave(false);
        }}
        sx={{ display: "contents" }}
      >
        <SectionCard title="Address">
          <Field label="Address">
            <TextField
              required
              autoFocus={!hostId}
              value={form.address}
              onChange={(e) => set("address", e.target.value)}
              placeholder="hostname or IP"
              slotProps={{ input: { sx: { fontFamily: monoFontFamily } } }}
              error={touched && form.address.trim().length === 0}
            />
          </Field>
          <Field label="Label">
            <TextField
              value={form.label}
              onChange={(e) => set("label", e.target.value)}
              placeholder={form.address || "Defaults to the address"}
            />
          </Field>
          <Field label="Group">
            <TextField
              select
              value={form.groupId ?? ""}
              onChange={(e) => set("groupId", e.target.value === "" ? null : e.target.value)}
            >
              <MenuItem value="">
                <em>None</em>
              </MenuItem>
              {(groups.data ?? []).map((g) => (
                <MenuItem key={g.id} value={g.id}>
                  {g.label}
                </MenuItem>
              ))}
            </TextField>
          </Field>
          <Box sx={{ display: "flex", gap: 1.5, alignItems: "flex-end" }}>
            <Field label="Protocol" sx={{ flex: 1 }}>
              <ToggleButtonGroup
                exclusive
                fullWidth
                value={form.protocol}
                onChange={(_e, v: HostForm["protocol"] | null) => {
                  if (v) set("protocol", v);
                }}
              >
                <ToggleButton value="ssh">SSH</ToggleButton>
                <ToggleButton value="telnet">Telnet</ToggleButton>
              </ToggleButtonGroup>
            </Field>
            <Field label="Port" sx={{ width: 104, flexShrink: 0 }}>
              <TextField
                type="number"
                value={form.port ?? ""}
                onChange={(e) => set("port", clampPort(e.target.value))}
                placeholder={String(defaultPort)}
                slotProps={{ htmlInput: { min: 1, max: 65535 } }}
              />
            </Field>
          </Box>
        </SectionCard>

        <SectionCard title="Credentials">
          <Field label="Use">
            <TextField
              select
              value={form.identityId ?? "inline"}
              onChange={(e) => {
                const v = e.target.value;
                set("identityId", v === "inline" ? null : v);
              }}
            >
              <MenuItem value="inline">Credentials set on this host</MenuItem>
              {(identities.data ?? []).map((i) => (
                <MenuItem key={i.id} value={i.id}>
                  {i.label}
                  <Typography
                    component="span"
                    variant="caption"
                    color="text.secondary"
                    sx={{ ml: 1 }}
                  >
                    {i.username}
                  </Typography>
                </MenuItem>
              ))}
            </TextField>
          </Field>
          {!usingIdentity && (
            <>
              <Field label="Username">
                <TextField
                  value={form.username}
                  onChange={(e) => set("username", e.target.value)}
                  autoComplete="off"
                  placeholder="root"
                />
              </Field>
              <Field
                label="Password"
                hint={
                  form.hasPassword && form.password === null
                    ? "A password is stored. Type to replace it or clear it to remove."
                    : undefined
                }
              >
                <TextField
                  type={showPassword ? "text" : "password"}
                  value={form.password ?? ""}
                  onChange={(e) => set("password", e.target.value)}
                  autoComplete="new-password"
                  placeholder={form.hasPassword && form.password === null ? "••••••••" : ""}
                  slotProps={{
                    input: {
                      endAdornment: (
                        <InputAdornment position="end">
                          {form.hasPassword && form.password === null && (
                            <Button
                              size="small"
                              color="inherit"
                              onClick={() => set("password", "")}
                            >
                              Clear
                            </Button>
                          )}
                          <IconButton
                            size="small"
                            onClick={() => setShowPassword((v) => !v)}
                            aria-label="Toggle password visibility"
                          >
                            {showPassword ? (
                              <VisibilityOffRoundedIcon fontSize="small" />
                            ) : (
                              <VisibilityRoundedIcon fontSize="small" />
                            )}
                          </IconButton>
                        </InputAdornment>
                      ),
                    },
                  }}
                />
              </Field>
              {ssh && (
                <Field label="SSH key">
                  <TextField
                    select
                    value={form.sshKeyId ?? ""}
                    onChange={(e) => set("sshKeyId", e.target.value === "" ? null : e.target.value)}
                  >
                    <MenuItem value="">
                      <em>None — password or agent</em>
                    </MenuItem>
                    {(sshKeys.data ?? []).map((k) => (
                      <MenuItem key={k.id} value={k.id}>
                        {k.label}
                        <Typography
                          component="span"
                          variant="caption"
                          color="text.secondary"
                          sx={{ ml: 1 }}
                        >
                          {k.keyType}
                        </Typography>
                      </MenuItem>
                    ))}
                  </TextField>
                </Field>
              )}
            </>
          )}
          {ssh && (
            <SettingRow
              label="Agent forwarding"
              hint="Expose the local SSH agent on the remote side."
              last
              control={
                <Switch
                  checked={form.agentForwarding}
                  onChange={(e) => set("agentForwarding", e.target.checked)}
                />
              }
            />
          )}
        </SectionCard>

        <SectionCard title="Tags">
          {(tags.data ?? []).length > 0 && (
            <Box sx={{ display: "flex", flexWrap: "wrap", gap: 0.75 }}>
              {(tags.data ?? []).map((t) => {
                const on = form.tagIds.includes(t.id);
                return (
                  <Chip
                    key={t.id}
                    label={t.label}
                    size="small"
                    color={on ? "primary" : "default"}
                    variant={on ? "filled" : "outlined"}
                    onClick={() =>
                      set(
                        "tagIds",
                        on ? form.tagIds.filter((x) => x !== t.id) : [...form.tagIds, t.id],
                      )
                    }
                  />
                );
              })}
            </Box>
          )}
          <TextField
            value={newTag}
            onChange={(e) => setNewTag(e.target.value)}
            placeholder="Add a tag and press Enter"
            disabled={createTag.isPending}
            onKeyDown={(e) => {
              if (e.key !== "Enter") return;
              e.preventDefault();
              addTag();
            }}
            slotProps={{
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <LabelOutlinedIcon fontSize="small" />
                  </InputAdornment>
                ),
              },
            }}
          />
        </SectionCard>

        {ssh && (
          <Button
            variant="text"
            color="inherit"
            onClick={() => setMore((v) => !v)}
            endIcon={more ? <ExpandLessRoundedIcon /> : <ExpandMoreRoundedIcon />}
            sx={{ alignSelf: "flex-start", color: "text.secondary" }}
          >
            {more ? "Show less" : "Show more"}
          </Button>
        )}

        {ssh && more && (
          <>
            <SectionCard title="Connection">
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
              <Field label="Startup snippet" hint="Runs right after the shell opens.">
                <TextField
                  select
                  value={form.startupSnippetId ?? ""}
                  onChange={(e) =>
                    set("startupSnippetId", e.target.value === "" ? null : e.target.value)
                  }
                >
                  <MenuItem value="">
                    <em>None</em>
                  </MenuItem>
                  {(snippets.data ?? []).map((s) => (
                    <MenuItem key={s.id} value={s.id}>
                      {s.label}
                    </MenuItem>
                  ))}
                </TextField>
              </Field>
              <Box sx={{ display: "flex", gap: 1.5 }}>
                <Field label="Keep-alive, s" sx={{ flex: 1 }}>
                  <TextField
                    type="number"
                    value={form.keepAliveInterval ?? ""}
                    onChange={(e) => set("keepAliveInterval", clampSeconds(e.target.value))}
                    placeholder="default"
                    slotProps={{ htmlInput: { min: 0, max: 86400 } }}
                  />
                </Field>
                <Field label="Timeout, s" sx={{ flex: 1 }}>
                  <TextField
                    type="number"
                    value={form.timeout ?? ""}
                    onChange={(e) => set("timeout", clampSeconds(e.target.value))}
                    placeholder="default"
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
                  Sent to the server with the session. The server must accept them (
                  <code>AcceptEnv</code>).
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
                          form.envVariables.map((row, j) =>
                            j === i ? [e.target.value, row[1]] : row,
                          ),
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
                          form.envVariables.map((row, j) =>
                            j === i ? [row[0], e.target.value] : row,
                          ),
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
          </>
        )}

        <SectionCard title="Notes">
          <TextField
            multiline
            minRows={3}
            value={form.notes}
            onChange={(e) => set("notes", e.target.value)}
            placeholder="Anything worth remembering about this host"
          />
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
        excludeHostId={hostId}
        onClose={() => setDialog(null)}
        onCreated={(id) => set("hostChainId", id)}
      />

      <ConfirmDialog
        open={confirmDelete}
        title="Delete host?"
        danger
        confirmLabel="Delete"
        busy={del.isPending}
        onCancel={() => setConfirmDelete(false)}
        onConfirm={onDelete}
      >
        “{form.label || form.address}” and its inline credentials will be removed from this device.
      </ConfirmDialog>
    </SidePanel>
  );
}
