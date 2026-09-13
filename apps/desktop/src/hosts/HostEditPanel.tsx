import { useState } from "react";
import {
  Box,
  Button,
  Divider,
  IconButton,
  ListSubheader,
  MenuItem,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import ExpandLessRoundedIcon from "@mui/icons-material/ExpandLessRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import AddCircleOutlineRoundedIcon from "@mui/icons-material/AddCircleOutlineRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import LabelOutlinedIcon from "@mui/icons-material/LabelOutlined";
import FolderOutlinedIcon from "@mui/icons-material/FolderOutlined";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import { distroIcon } from "./distroIcons";
import { DistroGlyph, ProtocolGlyph } from "./HostAvatar";
import { ConnectButton } from "./ConnectSplit";
import { TagsPopover } from "./TagsPopover";
import { AgentForwardingRow, CredentialsFields } from "./CredentialsFields";
import { Field, IconTile, Loading, SectionCard, SidePanel, ToolIconButton } from "@/components/ui";
import {
  useDeleteHost,
  useGroups,
  useHostChains,
  useHostForm,
  useInherited,
  useProxies,
  useSaveHost,
  useSnippets,
  useTags,
} from "@/ipc/hooks";
import {
  emptyHostForm,
  errorMessage,
  type HostForm,
  type HostProtocol,
  type IpVersion,
  type TelnetForm,
  type Uuid,
} from "@/ipc/types";
import { useActiveVault } from "@/app/vault";
import { openTerminal } from "@/terminal/store";
import { terminalThemes } from "@/terminal/themes";
import { monoFontFamily, sizes } from "@/theme/theme";
import { ChainDialog, ProxyDialog } from "./HostAdvancedDialogs";

const IP_VERSIONS: { value: IpVersion; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "4", label: "IPv4" },
  { value: "6", label: "IPv6" },
];

const emptyTelnet = (): TelnetForm => ({
  port: null,
  username: "",
  password: null,
  identityId: null,
  colorScheme: null,
  hasPassword: false,
});

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
      <SidePanel title="Host Details" onClose={onClose} width={sizes.panel}>
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

const protocolsOf = (f: HostForm): HostProtocol[] => [
  ...(f.ssh ? (["ssh"] as const) : []),
  ...(f.telnet ? (["telnet"] as const) : []),
];

/**
 * Host Details laid out the Termius way: Address → General → "SSH on … port"
 * (credentials, Show more) → "Telnet on … port" (or "+ Add Telnet"), with
 * Connect pinned at the bottom.
 */
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
  const snippets = useSnippets(vaultId);
  const proxies = useProxies(vaultId);
  const chains = useHostChains(vaultId);
  const save = useSaveHost();
  const del = useDeleteHost();

  const [form, setForm] = useState<HostForm>(initial);
  const inherited = useInherited(form.groupId);
  const inh = inherited.data ?? null;
  const inheritedFrom = inh && inh.groupPath.length > 0 ? inh.groupPath.join(" / ") : null;
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [touched, setTouched] = useState(false);
  const [more, setMore] = useState(
    initial.hostChainId !== null ||
      initial.proxyId !== null ||
      initial.startupSnippetId !== null ||
      initial.envVariables.length > 0 ||
      initial.agentForwarding ||
      initial.keepAliveInterval !== null ||
      initial.timeout !== null ||
      initial.colorScheme !== null,
  );
  const [dialog, setDialog] = useState<"proxy" | "chain" | null>(null);
  const [tagAnchor, setTagAnchor] = useState<HTMLElement | null>(null);

  const set = <K extends keyof HostForm>(k: K, v: HostForm[K]) => {
    setTouched(true);
    setForm((f) => ({ ...f, [k]: v }));
  };
  const patch = (p: Partial<HostForm>) => {
    setTouched(true);
    setForm((f) => ({ ...f, ...p }));
  };
  const patchTelnet = (p: Partial<TelnetForm>) => {
    setTouched(true);
    setForm((f) => ({ ...f, telnet: { ...(f.telnet ?? emptyTelnet()), ...p } }));
  };

  const readOnly = useActiveVault().readOnly;
  const protocols = protocolsOf(form);
  const canSave =
    form.address.trim().length > 0 && protocols.length > 0 && !save.isPending && !readOnly;
  const sshPortPlaceholder = String(inh?.port ?? 22);
  const chainName = (id: Uuid | null) =>
    (chains.data ?? []).find((c) => c.id === id)?.data.label ?? null;
  const proxyName = (id: Uuid | null) => {
    const p = (proxies.data ?? []).find((c) => c.id === id);
    return p ? `${p.data.kind.toUpperCase()} ${p.data.host}:${p.data.port}` : null;
  };
  const icon = distroIcon(form.icon) ?? distroIcon(form.osName);
  const glyphProtocol: HostProtocol = form.ssh ? "ssh" : "telnet";
  const selectedTags = (tags.data ?? []).filter((t) => form.tagIds.includes(t.id));
  const selectedTagIds = new Set(form.tagIds);

  const onSave = (thenConnect: HostProtocol | null) => {
    save.mutate(form, {
      onSuccess: (card) => {
        snackbar.notify(hostId ? "Host saved" : `Host “${card.label}” added`);
        if (thenConnect) openTerminal({ kind: "host", host_id: card.id, protocol: thenConnect });
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
      <ConnectButton hostId={hostId} protocol={protocols[0] ?? null} />
    ) : (
      <>
        <Button
          variant="tonal"
          size="large"
          disabled={!canSave}
          onClick={() => onSave(null)}
          sx={{ height: 40, borderRadius: 2.5, flex: "0 0 auto !important", px: 2.5 }}
        >
          {save.isPending ? "Saving…" : "Save"}
        </Button>
        <ConnectButton
          hostId={hostId}
          disabled={!canSave}
          onClick={() => onSave(protocols[0] ?? null)}
        />
      </>
    );

  return (
    <SidePanel
      title="Host Details"
      subtitle={hostId ? form.label || form.address : "New host"}
      onClose={onClose}
      width={sizes.panel}
      actions={
        hostId && (
          <ToolIconButton title="Delete host" color="error" onClick={() => setConfirmDelete(true)}>
            <DeleteOutlineRoundedIcon fontSize="small" />
          </ToolIconButton>
        )
      }
      footer={footer}
    >
      <Box
        component="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (canSave && (touched || !hostId)) onSave(null);
        }}
        sx={{ display: "contents" }}
      >
        <SectionCard title="Address">
          <Box sx={{ display: "flex", gap: 1.5, alignItems: "center" }}>
            <IconTile size={sizes.tile} color={icon?.color}>
              {icon ? <DistroGlyph icon={icon} /> : <ProtocolGlyph protocol={glyphProtocol} />}
            </IconTile>
            <TextField
              required
              autoFocus={!hostId}
              value={form.address}
              onChange={(e) => set("address", e.target.value)}
              placeholder="IP or Hostname"
              slotProps={{ input: { sx: { fontFamily: monoFontFamily } } }}
              error={touched && form.address.trim().length === 0}
              sx={{ flex: 1 }}
            />
          </Box>
          <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
            <Typography variant="body2" color="text.secondary" sx={{ flex: 1 }}>
              IP version
            </Typography>
            <ToggleButtonGroup
              exclusive
              size="small"
              value={form.ipVersion}
              onChange={(_e, v: IpVersion | null) => {
                if (v) set("ipVersion", v);
              }}
            >
              {IP_VERSIONS.map((o) => (
                <ToggleButton key={o.value} value={o.value} sx={{ px: 1.5 }}>
                  {o.label}
                </ToggleButton>
              ))}
            </ToggleButtonGroup>
          </Box>
        </SectionCard>

        <SectionCard title="General">
          <TextField
            value={form.label}
            onChange={(e) => set("label", e.target.value)}
            placeholder="Label"
          />
          <TextField
            select
            value={form.groupId ?? ""}
            onChange={(e) => set("groupId", e.target.value === "" ? null : e.target.value)}
            slotProps={{
              select: {
                displayEmpty: true,
                renderValue: (v) => {
                  const g = (groups.data ?? []).find((x) => x.id === v);
                  return (
                    <Box sx={{ display: "flex", alignItems: "center", gap: 1.25 }}>
                      <FolderOutlinedIcon fontSize="small" sx={{ color: "text.secondary" }} />
                      <Box component="span" sx={{ color: g ? "text.primary" : "text.disabled" }}>
                        {g?.label ?? "Parent Group"}
                      </Box>
                    </Box>
                  );
                },
              },
            }}
          >
            <MenuItem value="">
              <em>No group</em>
            </MenuItem>
            {(groups.data ?? []).map((g) => (
              <MenuItem key={g.id} value={g.id}>
                {g.label}
              </MenuItem>
            ))}
          </TextField>
          {inheritedFrom &&
            inh &&
            (inh.username !== null ||
              inh.hasPassword ||
              inh.sshKeyId !== null ||
              inh.identityId !== null ||
              inh.port !== null) && (
              <Typography variant="caption" color="text.secondary" sx={{ mt: -1 }}>
                Inherits credentials and connection defaults from {inheritedFrom}.
              </Typography>
            )}
          <Box
            component="button"
            type="button"
            onClick={(e) => setTagAnchor(e.currentTarget)}
            aria-label="Tags"
            sx={{
              all: "unset",
              boxSizing: "border-box",
              display: "flex",
              alignItems: "center",
              gap: 1.25,
              minHeight: sizes.control,
              px: 1.5,
              py: 0.5,
              borderRadius: 1.5,
              border: 1,
              borderColor: "border.basic",
              cursor: "pointer",
              "&:hover": { borderColor: "border.strong" },
              "&:focus-visible": { borderColor: "primary.main" },
            }}
          >
            <LabelOutlinedIcon fontSize="small" sx={{ color: "text.secondary" }} />
            {selectedTags.length === 0 ? (
              <Typography variant="body1" sx={{ color: "text.disabled" }}>
                Tags
              </Typography>
            ) : (
              <Box sx={{ display: "flex", flexWrap: "wrap", gap: 0.5, flex: 1 }}>
                {selectedTags.map((t) => (
                  <Box
                    key={t.id}
                    component="span"
                    sx={{
                      px: 1,
                      height: 22,
                      lineHeight: "22px",
                      borderRadius: 1,
                      bgcolor: "surface.highest",
                      fontSize: 12,
                      fontWeight: 500,
                    }}
                  >
                    {t.label}
                  </Box>
                ))}
              </Box>
            )}
          </Box>
          <TextField
            multiline
            minRows={1}
            maxRows={6}
            value={form.notes}
            onChange={(e) => set("notes", e.target.value)}
            placeholder="Notes"
          />
        </SectionCard>

        {form.ssh ? (
          <SectionCard
            title={
              <PortTitle
                name="SSH"
                value={form.port}
                placeholder={sshPortPlaceholder}
                onChange={(v) => set("port", v)}
              />
            }
            action={
              form.telnet && (
                <Tooltip title="Remove SSH">
                  <IconButton
                    size="small"
                    aria-label="Remove SSH"
                    onClick={() => set("ssh", false)}
                  >
                    <CloseRoundedIcon fontSize="small" />
                  </IconButton>
                </Tooltip>
              )
            }
          >
            <Divider />
            <CredentialsFields
              vaultId={vaultId}
              ssh
              agentForwarding={false}
              inherited={inh}
              inlineLabel="Set on this host"
              value={form}
              onChange={patch}
            />

            <Button
              variant="text"
              color="inherit"
              onClick={() => setMore((v) => !v)}
              endIcon={more ? <ExpandLessRoundedIcon /> : <ExpandMoreRoundedIcon />}
              sx={{ alignSelf: "flex-start", color: "text.secondary", ml: -1 }}
            >
              {more ? "Show less" : "Show more"}
            </Button>

            {more && (
              <>
                <AgentForwardingRow value={form} onChange={patch} inherited={inh} />
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
                <Field label="Host Chaining">
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
                      <em>
                        {inh?.hostChainId && chainName(inh.hostChainId)
                          ? `Inherited — ${chainName(inh.hostChainId)}`
                          : "Direct connection"}
                      </em>
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
                      <em>
                        {inh?.proxyId && proxyName(inh.proxyId)
                          ? `Inherited — ${proxyName(inh.proxyId)}`
                          : "None"}
                      </em>
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
                <Field
                  label="Environment Variables"
                  hint={
                    inh && inh.envVariables.length > 0 && inheritedFrom
                      ? `From ${inheritedFrom}: ${inh.envVariables.map(([k, v]) => `${k}=${v}`).join(", ")}`
                      : "Sent with the session; the server must accept them (AcceptEnv)."
                  }
                >
                  <Box sx={{ display: "flex", flexDirection: "column", gap: 1 }}>
                    {form.envVariables.map(([k, v], i) => (
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
                    ))}
                    <Button
                      size="small"
                      color="inherit"
                      startIcon={<AddRoundedIcon />}
                      onClick={() => set("envVariables", [...form.envVariables, ["", ""]])}
                      sx={{ alignSelf: "flex-start", color: "text.secondary" }}
                    >
                      Add variable
                    </Button>
                  </Box>
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
                <ThemeField value={form.colorScheme} onChange={(v) => set("colorScheme", v)} />
              </>
            )}
          </SectionCard>
        ) : (
          <AddSectionButton label="Add SSH" onClick={() => set("ssh", true)} />
        )}

        {form.telnet ? (
          <SectionCard
            title={
              <PortTitle
                name="Telnet"
                value={form.telnet.port}
                placeholder="23"
                onChange={(v) => patchTelnet({ port: v })}
              />
            }
            action={
              <Tooltip title="Remove Telnet">
                <IconButton
                  size="small"
                  aria-label="Remove Telnet"
                  onClick={() => set("telnet", null)}
                >
                  <CloseRoundedIcon fontSize="small" />
                </IconButton>
              </Tooltip>
            }
          >
            <Divider />
            <CredentialsFields
              vaultId={vaultId}
              ssh={false}
              inlineLabel="Set on this host"
              value={{
                identityId: form.telnet.identityId,
                username: form.telnet.username,
                password: form.telnet.password,
                hasPassword: form.telnet.hasPassword,
                sshKeyId: null,
                agentForwarding: false,
              }}
              onChange={({ identityId, username, password }) => {
                const p: Partial<TelnetForm> = {};
                if (identityId !== undefined) p.identityId = identityId;
                if (username !== undefined) p.username = username;
                if (password !== undefined) p.password = password;
                patchTelnet(p);
              }}
            />
            <ThemeField
              value={form.telnet.colorScheme}
              onChange={(v) => patchTelnet({ colorScheme: v })}
            />
          </SectionCard>
        ) : (
          <AddSectionButton label="Add Telnet" onClick={() => set("telnet", emptyTelnet())} />
        )}
      </Box>

      <TagsPopover
        anchor={tagAnchor}
        vaultId={vaultId}
        selected={selectedTagIds}
        allowCreate
        onCreated={(id) => set("tagIds", [...form.tagIds, id])}
        onToggle={(t, on) =>
          set("tagIds", on ? [...form.tagIds, t.id] : form.tagIds.filter((x) => x !== t.id))
        }
        onDeleted={(t) => setForm((f) => ({ ...f, tagIds: f.tagIds.filter((x) => x !== t.id) }))}
        onClose={() => setTagAnchor(null)}
      />

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

/** "SSH on [22] port" — the section title doubles as the port field. */
function PortTitle({
  name,
  value,
  placeholder,
  onChange,
}: {
  name: string;
  value: number | null;
  placeholder: string;
  onChange: (v: number | null) => void;
}) {
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
      <Typography variant="subtitle2">{name} on</Typography>
      <TextField
        type="number"
        size="small"
        value={value ?? ""}
        onChange={(e) => onChange(clampPort(e.target.value))}
        placeholder={placeholder}
        aria-label={`${name} port`}
        slotProps={{
          htmlInput: { min: 1, max: 65535, sx: { textAlign: "center", px: 0.5 } },
          input: { sx: { fontFamily: monoFontFamily, height: 28 } },
        }}
        sx={{ width: 72 }}
      />
      <Typography variant="subtitle2">port</Typography>
    </Box>
  );
}

/** Full-width tonal "+ Add Telnet" row between the section cards. */
function AddSectionButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <Button
      variant="tonal"
      size="large"
      startIcon={<AddCircleOutlineRoundedIcon />}
      onClick={onClick}
      sx={{ borderRadius: 2, height: 44 }}
    >
      {label}
    </Button>
  );
}

/** Colour scheme the terminal opens with for this section; empty follows Settings. */
function ThemeField({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (v: string | null) => void;
}) {
  const dark = terminalThemes.filter((t) => t.dark);
  const light = terminalThemes.filter((t) => !t.dark);
  const current = terminalThemes.find((t) => t.id === value) ?? null;
  return (
    <TextField
      select
      value={value ?? ""}
      onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
      slotProps={{
        select: {
          displayEmpty: true,
          renderValue: () => (
            <Box sx={{ display: "flex", alignItems: "center" }}>
              {current ? (
                <>
                  <ThemeSwatch background={current.background} ansi={current.ansi} />
                  {current.name}
                </>
              ) : (
                <Box component="span" sx={{ color: "text.disabled" }}>
                  Terminal theme · app default
                </Box>
              )}
            </Box>
          ),
        },
      }}
    >
      <MenuItem value="">
        <em>App default</em>
      </MenuItem>
      <ListSubheader disableSticky>Dark</ListSubheader>
      {dark.map((t) => (
        <MenuItem key={t.id} value={t.id}>
          <ThemeSwatch background={t.background} ansi={t.ansi} />
          {t.name}
        </MenuItem>
      ))}
      <ListSubheader disableSticky>Light</ListSubheader>
      {light.map((t) => (
        <MenuItem key={t.id} value={t.id}>
          <ThemeSwatch background={t.background} ansi={t.ansi} />
          {t.name}
        </MenuItem>
      ))}
    </TextField>
  );
}

function ThemeSwatch({ background, ansi }: { background: string; ansi: readonly string[] }) {
  return (
    <Box
      sx={{
        display: "inline-flex",
        gap: "2px",
        p: "3px",
        mr: 1.25,
        borderRadius: "4px",
        bgcolor: background,
        border: 1,
        borderColor: "border.light",
      }}
    >
      {[1, 2, 3, 4, 5, 6].map((i) => (
        <Box key={i} sx={{ width: 6, height: 10, borderRadius: "1px", bgcolor: ansi[i] }} />
      ))}
    </Box>
  );
}
