import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Checkbox,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  IconButton,
  InputAdornment,
  MenuItem,
  Select,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import ArrowBackRoundedIcon from "@mui/icons-material/ArrowBackRounded";
import CheckCircleOutlineRoundedIcon from "@mui/icons-material/CheckCircleOutlineRounded";
import CloudOutlinedIcon from "@mui/icons-material/CloudOutlined";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import VisibilityOffOutlinedIcon from "@mui/icons-material/VisibilityOffOutlined";
import VisibilityOutlinedIcon from "@mui/icons-material/VisibilityOutlined";
import FolderOpenRoundedIcon from "@mui/icons-material/FolderOpenRounded";
import { useQueryClient } from "@tanstack/react-query";
import { useSnackbar } from "@/components/Snackbar";
import { Field, IconTile, Mono } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { useGroups, useTags, useVaults } from "@/ipc/hooks";
import {
  errorMessage,
  type CloudImportReport,
  type CloudInstance,
  type CloudPreview,
  type CloudProvider,
  type GroupNode,
  type Uuid,
} from "@/ipc/types";
import { HostGlyph } from "./HostAvatar";
import { distroIcon } from "./distroIcons";
import { TagChip } from "./TagChip";
import { Row, VaultSelect, WarningList } from "./ImportDialog";
import {
  AWS_REGIONS,
  CLOUD_PROVIDERS,
  PRIVACY_NOTE,
  cloudErrorMessage,
  emptyDraft,
  importable,
  toConfig,
  type Draft,
} from "./cloud";

export { CLOUD_PROVIDERS } from "./cloud";

type Step =
  | { kind: "credentials"; busy: boolean; error: string | null }
  | { kind: "preview"; preview: CloudPreview }
  | { kind: "done"; report: CloudImportReport; providerName: string };

/**
 * Hosts → New host → AWS / DigitalOcean / Azure Integration. Credentials go to
 * Rust for one discovery call; the preview that comes back holds only machine
 * facts, and hosts are written when the user confirms a selection.
 */
export function CloudImportDialog({
  open,
  vaultId,
  provider,
  onClose,
  onImported,
}: {
  open: boolean;
  vaultId: Uuid;
  provider: CloudProvider;
  onClose: () => void;
  onImported: () => void;
}) {
  const [tall, setTall] = useState(false);
  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth="md"
      fullWidth
      slotProps={{
        paper: { sx: { height: tall ? "min(720px, calc(100vh - 64px))" : undefined } },
      }}
    >
      {open && (
        <Body
          vaultId={vaultId}
          initialProvider={provider}
          onClose={onClose}
          onImported={onImported}
          onStep={(k) => setTall(k === "preview")}
        />
      )}
    </Dialog>
  );
}

function Body({
  vaultId,
  initialProvider,
  onClose,
  onImported,
  onStep,
}: {
  vaultId: Uuid;
  initialProvider: CloudProvider;
  onClose: () => void;
  onImported: () => void;
  onStep: (kind: Step["kind"]) => void;
}) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vaults = useVaults();
  const [provider, setProvider] = useState<CloudProvider>(initialProvider);
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [reveal, setReveal] = useState(false);
  const [step, setStep] = useState<Step>({ kind: "credentials", busy: false, error: null });
  const [target, setTarget] = useState<Uuid>(vaultId);
  const [selected, setSelected] = useState<number[]>([]);
  const [groupId, setGroupId] = useState<Uuid | null>(null);
  const [tagIds, setTagIds] = useState<Uuid[]>([]);
  const [username, setUsername] = useState("");
  const [port, setPort] = useState("");
  const [removeMissing, setRemoveMissing] = useState(false);
  const [applying, setApplying] = useState(false);
  const groups = useGroups(target);
  const tags = useTags(target);

  const meta = CLOUD_PROVIDERS.find((p) => p.id === provider) ?? {
    id: provider,
    name: provider,
    short: provider,
  };

  const stepKind = step.kind;
  useEffect(() => onStep(stepKind), [stepKind, onStep]);

  const previewId = step.kind === "preview" ? step.preview.id : null;
  useEffect(() => {
    if (!previewId) return;
    return () => {
      void ipc.cloudDiscard(previewId).catch(() => undefined);
    };
  }, [previewId]);

  // Group / tags belong to a vault; changing the destination resets them.
  const changeTarget = (id: Uuid) => {
    setTarget(id);
    setGroupId(null);
    setTagIds([]);
  };

  const config = toConfig(provider, draft);

  const discover = async () => {
    if (!config) return;
    setStep({ kind: "credentials", busy: true, error: null });
    try {
      const preview = await ipc.cloudDiscover(target, config);
      // The secret has done its job; don't keep it in webview state.
      setDraft((d) => ({
        aws: { ...d.aws, secretAccessKey: "" },
        digitalOcean: { token: "" },
        azure: { ...d.azure, clientSecret: "" },
      }));
      setReveal(false);
      setSelected(importable(preview));
      setStep({ kind: "preview", preview });
    } catch (e) {
      setStep({ kind: "credentials", busy: false, error: cloudErrorMessage(e, meta.name) });
    }
  };

  const apply = async () => {
    if (step.kind !== "preview") return;
    const portNum = port.trim() === "" ? null : Number(port);
    if (portNum !== null && (!Number.isInteger(portNum) || portNum < 1 || portNum > 65535)) {
      snackbar.error("Port must be between 1 and 65535");
      return;
    }
    setApplying(true);
    try {
      const report = await ipc.cloudImport(target, step.preview.id, {
        instances: selected,
        groupId,
        tagIds,
        username: username.trim(),
        port: portNum,
        removeMissing,
      });
      await Promise.all(
        (["hosts", "hostForm", "groups", "tags"] as const).map((k) =>
          qc.invalidateQueries({ queryKey: [k] }),
        ),
      );
      onImported();
      setStep({ kind: "done", report, providerName: step.preview.providerName });
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setApplying(false);
    }
  };

  const toggle = (i: number) =>
    setSelected((sel) =>
      sel.includes(i) ? sel.filter((x) => x !== i) : [...sel, i].sort((a, b) => a - b),
    );

  const targetVault = (vaults.data ?? []).find((v) => v.id === target);
  const vaultOk =
    targetVault !== undefined && targetVault.unlocked && targetVault.role !== "viewer";

  if (step.kind === "credentials") {
    return (
      <>
        <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
          <IconTile tone="accent" size={36}>
            <CloudOutlinedIcon />
          </IconTile>
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography component="div" variant="h6" noWrap>
              {meta.name} integration
            </Typography>
            <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
              Add the machines from your account as hosts
            </Typography>
          </Box>
        </DialogTitle>
        <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 2, pt: 0 }}>
          <ToggleButtonGroup
            exclusive
            size="small"
            value={provider}
            onChange={(_, v: CloudProvider | null) => {
              if (v) {
                setProvider(v);
                setStep({ kind: "credentials", busy: false, error: null });
              }
            }}
            disabled={step.busy}
            sx={{ alignSelf: "flex-start" }}
          >
            {CLOUD_PROVIDERS.map((p) => (
              <ToggleButton key={p.id} value={p.id} sx={{ px: 2 }}>
                {p.short}
              </ToggleButton>
            ))}
          </ToggleButtonGroup>

          {step.error && (
            <Alert severity="error" variant="outlined">
              {step.error}
            </Alert>
          )}

          <Box
            component="form"
            onSubmit={(e) => {
              e.preventDefault();
              void discover();
            }}
            sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}
          >
            {provider === "aws" && (
              <>
                <Field label="Region">
                  <Autocomplete
                    freeSolo
                    size="small"
                    options={AWS_REGIONS}
                    value={draft.aws.region}
                    onInputChange={(_, v) =>
                      setDraft((d) => ({ ...d, aws: { ...d.aws, region: v } }))
                    }
                    renderInput={(params) => (
                      <TextField {...params} placeholder="us-east-1" autoComplete="off" />
                    )}
                  />
                </Field>
                <Field label="Access Key ID">
                  <TextField
                    fullWidth
                    size="small"
                    value={draft.aws.accessKeyId}
                    onChange={(e) =>
                      setDraft((d) => ({ ...d, aws: { ...d.aws, accessKeyId: e.target.value } }))
                    }
                    placeholder="AKIA…"
                    autoComplete="off"
                    slotProps={{ input: { spellCheck: false } }}
                  />
                </Field>
                <Field label="Secret Access Key">
                  <SecretField
                    value={draft.aws.secretAccessKey}
                    reveal={reveal}
                    onReveal={() => setReveal((v) => !v)}
                    onChange={(v) =>
                      setDraft((d) => ({ ...d, aws: { ...d.aws, secretAccessKey: v } }))
                    }
                  />
                </Field>
                <Box sx={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 1.5 }}>
                  <Field label="Service">
                    <Select
                      fullWidth
                      size="small"
                      value={draft.aws.service}
                      onChange={(e) =>
                        setDraft((d) => ({
                          ...d,
                          aws: { ...d.aws, service: e.target.value },
                        }))
                      }
                    >
                      <MenuItem value="ec2">EC2</MenuItem>
                      <MenuItem value="lightsail">Lightsail</MenuItem>
                    </Select>
                  </Field>
                  <Field label="IP address type">
                    <Select
                      fullWidth
                      size="small"
                      value={draft.aws.addressType}
                      onChange={(e) =>
                        setDraft((d) => ({
                          ...d,
                          aws: { ...d.aws, addressType: e.target.value },
                        }))
                      }
                    >
                      <MenuItem value="public">Public</MenuItem>
                      <MenuItem value="private">Private</MenuItem>
                    </Select>
                  </Field>
                </Box>
              </>
            )}
            {provider === "digital_ocean" && (
              <Field
                label="Token"
                hint="A personal access token with read scope is enough (API → Tokens in the DigitalOcean control panel)."
              >
                <SecretField
                  value={draft.digitalOcean.token}
                  reveal={reveal}
                  onReveal={() => setReveal((v) => !v)}
                  onChange={(v) => setDraft((d) => ({ ...d, digitalOcean: { token: v } }))}
                  placeholder="dop_v1_…"
                />
              </Field>
            )}
            {provider === "azure" && (
              <>
                <Field label="Tenant ID">
                  <TextField
                    fullWidth
                    size="small"
                    value={draft.azure.tenantId}
                    onChange={(e) =>
                      setDraft((d) => ({ ...d, azure: { ...d.azure, tenantId: e.target.value } }))
                    }
                    autoComplete="off"
                    slotProps={{ input: { spellCheck: false } }}
                  />
                </Field>
                <Field label="Client ID">
                  <TextField
                    fullWidth
                    size="small"
                    value={draft.azure.clientId}
                    onChange={(e) =>
                      setDraft((d) => ({ ...d, azure: { ...d.azure, clientId: e.target.value } }))
                    }
                    autoComplete="off"
                    slotProps={{ input: { spellCheck: false } }}
                  />
                </Field>
                <Field
                  label="Client Secret"
                  hint="An app registration with the Reader role on the subscriptions you want to list."
                >
                  <SecretField
                    value={draft.azure.clientSecret}
                    reveal={reveal}
                    onReveal={() => setReveal((v) => !v)}
                    onChange={(v) =>
                      setDraft((d) => ({ ...d, azure: { ...d.azure, clientSecret: v } }))
                    }
                  />
                </Field>
              </>
            )}
            {/* Enter submits the form. */}
            <button type="submit" hidden disabled={!config || step.busy} />
          </Box>

          <Box
            sx={{
              display: "flex",
              gap: 1,
              alignItems: "flex-start",
              bgcolor: "surface.high",
              borderRadius: 2,
              px: 1.5,
              py: 1,
            }}
          >
            <LockOutlinedIcon fontSize="small" color="success" sx={{ mt: "1px" }} />
            <Typography variant="caption" color="text.secondary">
              {PRIVACY_NOTE}
            </Typography>
          </Box>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2, gap: 1.5 }}>
          <Box sx={{ display: "flex", alignItems: "center", gap: 1, mr: "auto", minWidth: 0 }}>
            <Typography variant="body2" color="text.secondary" sx={{ whiteSpace: "nowrap" }}>
              Import into
            </Typography>
            <VaultSelect vaults={vaults.data ?? []} value={target} onChange={changeTarget} />
          </Box>
          <Button color="inherit" onClick={onClose} disabled={step.busy}>
            Cancel
          </Button>
          <Button
            variant="contained"
            onClick={() => void discover()}
            disabled={!config || step.busy || !vaultOk}
          >
            {step.busy ? "Connecting…" : "Load machines"}
          </Button>
        </DialogActions>
      </>
    );
  }

  if (step.kind === "done") {
    const r = step.report;
    const rows: [string, number][] = [
      ["Added", r.created],
      ["Updated", r.updated],
      ["Unchanged", r.unchanged],
      ["Removed", r.removed],
      ["Skipped", r.skipped],
    ];
    const n = (count: number) => (count === 1 ? "1 host" : `${count} hosts`);
    const headline =
      r.created > 0
        ? `${n(r.created)} added from ${step.providerName}`
        : r.updated > 0
          ? `${n(r.updated)} updated from ${step.providerName}`
          : r.removed > 0
            ? `${n(r.removed)} removed — no longer on ${step.providerName}`
            : "Hosts are already up to date";
    return (
      <>
        <DialogTitle>Import complete</DialogTitle>
        <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
          <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
            <IconTile tone="accent" size={48}>
              <CheckCircleOutlineRoundedIcon />
            </IconTile>
            <Box>
              <Typography variant="subtitle1">{headline}</Typography>
              <Typography variant="body2" color="text.secondary">
                Run the integration again any time to pick up new machines and address changes; your
                SSH settings on the hosts are kept.
              </Typography>
            </Box>
          </Box>
          <Box sx={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: 1 }}>
            {rows.map(([label, n]) => (
              <Box
                key={label}
                sx={{
                  bgcolor: "surface.high",
                  borderRadius: 2,
                  px: 1.5,
                  py: 1.25,
                  opacity: n === 0 ? 0.55 : 1,
                }}
              >
                <Typography variant="h6" sx={{ lineHeight: 1.2 }}>
                  {n}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  {label}
                </Typography>
              </Box>
            ))}
          </Box>
          {r.warnings.length > 0 && <WarningList warnings={r.warnings} title="Notes" open />}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button variant="contained" onClick={onClose}>
            Done
          </Button>
        </DialogActions>
      </>
    );
  }

  const { preview } = step;
  const usable = importable(preview);
  const noAddress = preview.instances.length - usable.length;
  const linked = preview.instances.filter((i) => i.action === "update").length;
  const newCount = selected.filter((i) => preview.instances[i]?.action === "new").length;
  const updCount = selected.length - newCount;

  return (
    <>
      <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1, pr: 2 }}>
        <IconButton
          size="small"
          onClick={() => setStep({ kind: "credentials", busy: false, error: null })}
          aria-label="Back to credentials"
        >
          <ArrowBackRoundedIcon fontSize="small" />
        </IconButton>
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography component="div" variant="h6" noWrap>
            {preview.providerName}
            {preview.service ? ` · ${preview.service === "ec2" ? "EC2" : "Lightsail"}` : ""}
          </Typography>
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {preview.instances.length === 1
              ? "1 machine found"
              : `${preview.instances.length} machines found`}
            {linked > 0 && ` · ${linked} already imported`}
            {preview.addressType === "private" && " · private addresses"}
          </Typography>
        </Box>
        <Button
          size="small"
          color="inherit"
          onClick={() => setSelected(selected.length < usable.length ? usable : [])}
        >
          {selected.length < usable.length ? "Select all" : "Select none"}
        </Button>
      </DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5, pt: 0 }}>
        {preview.instances.length === 0 ? (
          <Alert severity="info" variant="outlined">
            No machines were found in this account
            {preview.provider === "aws" ? " and region" : ""}.
          </Alert>
        ) : (
          <>
            {noAddress > 0 && (
              <Alert severity="info" variant="outlined" sx={{ py: 0.25 }}>
                {noAddress === 1 ? "1 machine has" : `${noAddress} machines have`} no{" "}
                {preview.addressType ?? "public"} address right now (stopped or not exposed) and
                can't be imported.
              </Alert>
            )}
            <Box
              sx={{
                flex: 1,
                minHeight: 0,
                overflowY: "auto",
                display: "flex",
                flexDirection: "column",
                gap: 0.75,
                pr: 0.5,
              }}
            >
              {preview.instances.map((inst, i) => (
                <InstanceRow
                  key={inst.instanceId}
                  inst={inst}
                  checked={selected.includes(i)}
                  onToggle={() => {
                    if (inst.action !== "no_address") toggle(i);
                  }}
                />
              ))}
            </Box>
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: "1fr 1fr",
                gap: 1.5,
                pt: 0.5,
                borderTop: 1,
                borderColor: "divider",
              }}
            >
              <Field label="Group">
                <GroupSelect
                  groups={groups.data ?? []}
                  value={groupId}
                  onChange={setGroupId}
                  disabled={!vaultOk}
                />
              </Field>
              <Field label="Tags">
                <Autocomplete
                  multiple
                  size="small"
                  options={tags.data ?? []}
                  getOptionLabel={(t) => t.label}
                  isOptionEqualToValue={(a, b) => a.id === b.id}
                  value={(tags.data ?? []).filter((t) => tagIds.includes(t.id))}
                  onChange={(_, v) => setTagIds(v.map((t) => t.id))}
                  renderValue={(value, getItemProps) =>
                    value.map((t, idx) => {
                      const { key, ...props } = getItemProps({ index: idx });
                      return <TagChip key={key} label={t.label} color={t.color} {...props} />;
                    })
                  }
                  renderInput={(params) => (
                    <TextField {...params} placeholder={tagIds.length ? "" : "No tags"} />
                  )}
                  disabled={!vaultOk}
                />
              </Field>
              <Field label="Username" hint="Set on new hosts only; imported hosts keep theirs.">
                <TextField
                  fullWidth
                  size="small"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder={
                    preview.provider === "aws"
                      ? "ec2-user"
                      : preview.provider === "azure"
                        ? "azureuser"
                        : "root"
                  }
                  autoComplete="off"
                />
              </Field>
              <Field label="Port">
                <TextField
                  fullWidth
                  size="small"
                  value={port}
                  onChange={(e) => setPort(e.target.value.replace(/[^\d]/g, ""))}
                  placeholder="22"
                  slotProps={{ input: { inputMode: "numeric" } }}
                />
              </Field>
            </Box>
            {linked > 0 && (
              <FormControlLabel
                sx={{ ml: -0.75 }}
                control={
                  <Checkbox
                    size="small"
                    checked={removeMissing}
                    onChange={(e) => setRemoveMissing(e.target.checked)}
                  />
                }
                label={
                  <Typography variant="body2">
                    Remove hosts imported from {preview.providerName} earlier that no longer exist
                  </Typography>
                }
              />
            )}
          </>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2, gap: 1.5 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1, mr: "auto", minWidth: 0 }}>
          <Typography variant="body2" color="text.secondary" sx={{ whiteSpace: "nowrap" }}>
            Import into
          </Typography>
          <VaultSelect vaults={vaults.data ?? []} value={target} onChange={changeTarget} />
        </Box>
        <Button color="inherit" onClick={onClose} disabled={applying}>
          Cancel
        </Button>
        <Button
          variant="contained"
          onClick={() => void apply()}
          disabled={applying || !vaultOk || (selected.length === 0 && !removeMissing)}
        >
          {applying
            ? "Importing…"
            : selected.length === 0
              ? "Import"
              : updCount === 0
                ? `Add ${newCount} host${newCount === 1 ? "" : "s"}`
                : newCount === 0
                  ? `Update ${updCount} host${updCount === 1 ? "" : "s"}`
                  : `Add ${newCount}, update ${updCount}`}
        </Button>
      </DialogActions>
    </>
  );
}

function SecretField({
  value,
  reveal,
  onReveal,
  onChange,
  placeholder,
}: {
  value: string;
  reveal: boolean;
  onReveal: () => void;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <TextField
      fullWidth
      size="small"
      type={reveal ? "text" : "password"}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      autoComplete="off"
      slotProps={{
        input: {
          spellCheck: false,
          endAdornment: (
            <InputAdornment position="end">
              <IconButton
                size="small"
                edge="end"
                onClick={onReveal}
                aria-label={reveal ? "Hide" : "Show"}
              >
                {reveal ? (
                  <VisibilityOffOutlinedIcon fontSize="small" />
                ) : (
                  <VisibilityOutlinedIcon fontSize="small" />
                )}
              </IconButton>
            </InputAdornment>
          ),
        },
      }}
    />
  );
}

function InstanceRow({
  inst,
  checked,
  onToggle,
}: {
  inst: CloudInstance;
  checked: boolean;
  onToggle: () => void;
}) {
  const icon = distroIcon(inst.osName);
  const disabled = inst.action === "no_address";
  const details = [inst.state, inst.region, inst.size].filter(
    (s): s is string => typeof s === "string" && s.length > 0,
  );
  const row = (
    <Row
      tile={
        <IconTile color={icon?.color}>
          <HostGlyph osName={inst.osName} protocol="ssh" />
        </IconTile>
      }
      title={inst.label}
      subtitle={
        <>
          <Mono secondary>{inst.address ?? "no address"}</Mono>
          {details.length > 0 && ` · ${details.join(" · ")}`}
        </>
      }
      meta={
        <>
          <Chip size="small" label={inst.instanceId} sx={{ maxWidth: 360 }} />
          {inst.os && (
            <Chip size="small" variant="outlined" label={inst.os} sx={{ maxWidth: 260 }} />
          )}
          {inst.action === "update" && <Chip size="small" color="info" label="Already imported" />}
        </>
      }
      checked={checked}
      onToggle={onToggle}
    />
  );
  if (!disabled) return row;
  return (
    <Tooltip title="No address of the selected type; start the machine or switch the address type.">
      <Box sx={{ opacity: 0.45, pointerEvents: "none" }}>{row}</Box>
    </Tooltip>
  );
}

function GroupSelect({
  groups,
  value,
  onChange,
  disabled,
}: {
  groups: GroupNode[];
  value: Uuid | null;
  onChange: (id: Uuid | null) => void;
  disabled?: boolean;
}) {
  const paths = useMemo(() => {
    const byId = new Map(groups.map((g) => [g.id, g]));
    const path = (g: GroupNode): string[] => {
      const parent = g.parentId ? byId.get(g.parentId) : undefined;
      return parent ? [...path(parent), g.label] : [g.label];
    };
    return groups
      .map((g) => ({ id: g.id, path: path(g).join(" / ") }))
      .sort((a, b) => a.path.localeCompare(b.path));
  }, [groups]);
  return (
    <Select
      fullWidth
      size="small"
      displayEmpty
      value={value ?? ""}
      onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
      disabled={disabled}
      renderValue={(id) => {
        const p = paths.find((x) => x.id === id);
        return (
          <Box sx={{ display: "flex", alignItems: "center", gap: 1, minWidth: 0 }}>
            <FolderOpenRoundedIcon fontSize="small" color={p ? "inherit" : "disabled"} />
            <Typography variant="body2" noWrap>
              {p?.path ?? "No group"}
            </Typography>
          </Box>
        );
      }}
    >
      <MenuItem value="">No group</MenuItem>
      {paths.map((p) => (
        <MenuItem key={p.id} value={p.id}>
          {p.path}
        </MenuItem>
      ))}
    </Select>
  );
}
