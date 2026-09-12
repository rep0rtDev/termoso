import { useMemo, useState } from "react";
import {
  Box,
  Button,
  Checkbox,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  FormControlLabel,
  IconButton,
  List,
  ListItemButton,
  ListItemText,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import CodeRoundedIcon from "@mui/icons-material/CodeRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import EditRoundedIcon from "@mui/icons-material/EditRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import CreateNewFolderRoundedIcon from "@mui/icons-material/CreateNewFolderRounded";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import {
  EntityCard,
  Field,
  IconTile,
  Loading,
  SectionCard,
  SidePanel,
  SplitButton,
  ToolIconButton,
} from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import { HostAvatar } from "@/hosts/HostAvatar";
import * as ipc from "@/ipc/commands";
import { useGroups, useHosts, usePackages, useSnippets } from "@/ipc/hooks";
import { useActiveVault } from "@/app/vault";
import {
  errorMessage,
  type HostCard,
  type PackageNode,
  type SnippetCard,
  type SnippetForm,
  type Uuid,
} from "@/ipc/types";
import { useCreateRequests } from "@/app/navigation";
import { NameDialog } from "@/sftp/dialogs";
import { monoFontFamily, sizes } from "@/theme/theme";
import { terminalStore, type Pane } from "@/terminal/store";
import { startRun, summarize, useLastRun, watchRun, type SnippetRun } from "./run";
import { RunTargets, TargetStateIcon } from "./RunStatus";
import { TargetsDialog } from "./TargetsDialog";

const VAR_HINT = "Use {{name}} placeholders; you will be asked for values on run.";

function SnippetDialog({
  vaultId,
  packages,
  initial,
  defaultPackage,
  busy,
  onCancel,
  onConfirm,
}: {
  vaultId: Uuid;
  packages: PackageNode[];
  initial: SnippetCard | null;
  defaultPackage: Uuid | null;
  busy: boolean;
  onCancel: () => void;
  onConfirm: (form: SnippetForm) => void;
}) {
  const [f, setF] = useState<SnippetForm>({
    id: initial?.id ?? null,
    vaultId,
    label: initial?.label ?? "",
    script: initial?.script ?? "",
    packageId: initial ? initial.packageId : defaultPackage,
    closeAfterRun: initial?.closeAfterRun ?? false,
    sortOrder: initial?.sortOrder ?? 0,
  });
  const set = <K extends keyof SnippetForm>(k: K, v: SnippetForm[K]) => setF({ ...f, [k]: v });
  const valid = f.label.trim().length > 0 && f.script.trim().length > 0;
  return (
    <Dialog open onClose={busy ? undefined : onCancel} maxWidth="sm" fullWidth>
      <DialogTitle>{initial ? "Edit snippet" : "New snippet"}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          <Stack direction="row" spacing={2}>
            <Field label="Label" sx={{ flex: 1 }}>
              <TextField autoFocus value={f.label} onChange={(e) => set("label", e.target.value)} />
            </Field>
            <Field label="Package" sx={{ width: 200 }}>
              <TextField
                select
                value={f.packageId ?? ""}
                onChange={(e) => set("packageId", e.target.value === "" ? null : e.target.value)}
              >
                <MenuItem value="">
                  <em>None</em>
                </MenuItem>
                {packages.map((p) => (
                  <MenuItem key={p.id} value={p.id}>
                    {p.label}
                  </MenuItem>
                ))}
              </TextField>
            </Field>
          </Stack>
          <Field label="Script">
            <TextField
              value={f.script}
              onChange={(e) => set("script", e.target.value)}
              multiline
              minRows={6}
              maxRows={16}
              helperText={VAR_HINT}
              slotProps={{ htmlInput: { spellCheck: false, style: { fontFamily: "monospace" } } }}
            />
          </Field>
          <FormControlLabel
            control={
              <Checkbox
                checked={f.closeAfterRun}
                onChange={(e) => set("closeAfterRun", e.target.checked)}
              />
            }
            label="Close the terminal after running"
          />
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button variant="contained" disabled={!valid || busy} onClick={() => onConfirm(f)}>
          Save
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function VariableFields({
  variables,
  vars,
  onChange,
}: {
  variables: string[];
  vars: Record<string, string>;
  onChange: (vars: Record<string, string>) => void;
}) {
  return (
    <Stack spacing={1.5}>
      <Typography variant="overline" color="text.secondary">
        Variables
      </Typography>
      {variables.map((v, i) => (
        <Field key={v} label={v}>
          <TextField
            size="small"
            autoFocus={i === 0}
            value={vars[v] ?? ""}
            onChange={(e) => onChange({ ...vars, [v]: e.target.value })}
          />
        </Field>
      ))}
    </Stack>
  );
}

const emptyVars = (snippet: SnippetCard) =>
  Object.fromEntries(snippet.variables.map((v) => [v, ""]));
const varsComplete = (snippet: SnippetCard, vars: Record<string, string>) =>
  snippet.variables.every((v) => (vars[v] ?? "").length > 0);

/** Ask for `{{variable}}` values once before a run fans out to its targets. */
export function VariablesDialog({
  snippet,
  description,
  onCancel,
  onConfirm,
}: {
  snippet: SnippetCard;
  description: string;
  onCancel: () => void;
  onConfirm: (vars: Record<string, string>) => void;
}) {
  const [vars, setVars] = useState<Record<string, string>>(() => emptyVars(snippet));
  const valid = varsComplete(snippet, vars);
  return (
    <Dialog open onClose={onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Run “{snippet.label}”</DialogTitle>
      <DialogContent>
        <Stack
          component="form"
          id="snippet-vars"
          spacing={2}
          sx={{ mt: 0.5 }}
          onSubmit={(e) => {
            e.preventDefault();
            if (valid) onConfirm(vars);
          }}
        >
          <VariableFields variables={snippet.variables} vars={vars} onChange={setVars} />
          <Typography variant="body2" color="text.secondary">
            {description}
          </Typography>
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} color="inherit">
          Cancel
        </Button>
        <Button
          type="submit"
          form="snippet-vars"
          variant="contained"
          disabled={!valid}
          startIcon={<PlayArrowRoundedIcon />}
        >
          Run
        </Button>
      </DialogActions>
    </Dialog>
  );
}

/** Pick open terminals to type the snippet into (plus variables if any). */
export function RunDialog({
  snippet,
  busy,
  onCancel,
  onConfirm,
}: {
  snippet: SnippetCard;
  busy: boolean;
  onCancel: () => void;
  onConfirm: (sessionIds: Uuid[], vars: Record<string, string>) => void;
}) {
  const panes = useMemo(
    () =>
      Object.values(terminalStore.get().panes).filter(
        (p): p is Pane => p.status === "connected" && p.protocol !== null,
      ),
    [],
  );
  const active = terminalStore.get();
  const activePane = active.tabs.find((t) => t.id === active.activeTabId)?.activePaneId ?? null;
  const [selected, setSelected] = useState<Set<Uuid>>(
    () => new Set(activePane && panes.some((p) => p.id === activePane) ? [activePane] : []),
  );
  const [vars, setVars] = useState<Record<string, string>>(() => emptyVars(snippet));
  const toggle = (id: Uuid) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const valid = selected.size > 0 && varsComplete(snippet, vars);

  return (
    <Dialog open onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Run “{snippet.label}” in open terminals</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          {snippet.variables.length > 0 && (
            <VariableFields variables={snippet.variables} vars={vars} onChange={setVars} />
          )}
          <Box sx={{ display: "flex", alignItems: "center" }}>
            <Typography variant="overline" color="text.secondary" sx={{ flex: 1 }}>
              Terminals
            </Typography>
            {panes.length > 1 && (
              <Button
                size="small"
                color="inherit"
                onClick={() =>
                  setSelected(
                    selected.size === panes.length ? new Set() : new Set(panes.map((p) => p.id)),
                  )
                }
              >
                {selected.size === panes.length ? "Clear" : "Select all"}
              </Button>
            )}
          </Box>
          {panes.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              No connected terminals. Open a host first.
            </Typography>
          ) : (
            <List dense disablePadding sx={{ mx: -1 }}>
              {panes.map((p) => (
                <ListItemButton key={p.id} onClick={() => toggle(p.id)} sx={{ borderRadius: 1.5 }}>
                  <Checkbox edge="start" size="small" checked={selected.has(p.id)} tabIndex={-1} />
                  <ListItemText
                    primary={p.title}
                    secondary={p.subtitle}
                    slotProps={{ primary: { noWrap: true }, secondary: { noWrap: true } }}
                  />
                </ListItemButton>
              ))}
            </List>
          )}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={!valid || busy}
          startIcon={<PlayArrowRoundedIcon />}
          onClick={() => onConfirm([...selected], vars)}
        >
          Run
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function Script({ script }: { script: string }) {
  return (
    <Box
      component="pre"
      sx={{
        m: 0,
        fontFamily: monoFontFamily,
        fontSize: 12,
        lineHeight: 1.5,
        whiteSpace: "pre-wrap",
        wordBreak: "break-word",
        maxHeight: 72,
        overflow: "hidden",
      }}
    >
      {script}
    </Box>
  );
}

function fmtTime(ts: number) {
  return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/** Termius-style detail panel: script, targets for execution, last run. */
function SnippetPanel({
  snippet,
  hosts,
  packages,
  lastRun,
  busy,
  onClose,
  onEdit,
  onDelete,
  onAddTargets,
  onRemoveTarget,
  onRun,
  onRunInTerminals,
}: {
  snippet: SnippetCard;
  hosts: readonly HostCard[];
  packages: readonly PackageNode[];
  lastRun: SnippetRun | undefined;
  busy: boolean;
  onClose: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onAddTargets: () => void;
  onRemoveTarget: (hostId: Uuid) => void;
  onRun: () => void;
  onRunInTerminals: () => void;
}) {
  const pkg = packages.find((p) => p.id === snippet.packageId);
  const targets = snippet.targetHostIds
    .map((id) => hosts.find((h) => h.id === id))
    .filter((h): h is HostCard => h !== undefined);
  const openCount = Object.values(terminalStore.get().panes).filter(
    (p) => p.status === "connected" && p.protocol !== null,
  ).length;
  const stateFor = (hostId: Uuid) => lastRun?.targets.find((t) => t.hostId === hostId)?.state;

  return (
    <SidePanel
      title={snippet.label}
      subtitle={pkg ? pkg.label : "No package"}
      onClose={onClose}
      actions={
        <>
          <ToolIconButton title="Edit" onClick={onEdit}>
            <EditRoundedIcon fontSize="small" />
          </ToolIconButton>
          <ToolIconButton title="Delete" onClick={onDelete}>
            <DeleteOutlineRoundedIcon fontSize="small" />
          </ToolIconButton>
        </>
      }
      footer={
        <>
          <Button
            variant="tonal"
            startIcon={<TerminalRoundedIcon />}
            disabled={busy || openCount === 0}
            onClick={onRunInTerminals}
          >
            Open terminals
          </Button>
          <Button
            variant="contained"
            startIcon={<PlayArrowRoundedIcon />}
            disabled={busy || targets.length === 0}
            onClick={onRun}
          >
            Run
          </Button>
        </>
      }
    >
      <SectionCard title="Script">
        <Box
          component="pre"
          sx={{
            m: 0,
            p: 1.5,
            borderRadius: 1.5,
            bgcolor: "surface.base",
            fontFamily: monoFontFamily,
            fontSize: 12,
            lineHeight: 1.6,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
            maxHeight: 200,
            overflow: "auto",
          }}
        >
          {snippet.script}
        </Box>
        {(snippet.variables.length > 0 || snippet.closeAfterRun) && (
          <Box sx={{ display: "flex", flexWrap: "wrap", gap: 0.5 }}>
            {snippet.variables.map((v) => (
              <Chip key={v} label={`{{${v}}}`} size="small" sx={{ fontFamily: monoFontFamily }} />
            ))}
            {snippet.closeAfterRun && (
              <Chip label="closes terminal" size="small" variant="outlined" />
            )}
          </Box>
        )}
      </SectionCard>

      <SectionCard
        title="Targets for execution"
        action={
          <Button
            size="small"
            startIcon={<AddRoundedIcon />}
            onClick={onAddTargets}
            disabled={busy}
          >
            Add targets
          </Button>
        }
      >
        {targets.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            No targets yet. Add hosts or whole groups — Run connects to each of them and types the
            script.
          </Typography>
        ) : (
          <Stack spacing={0.5} sx={{ mx: -1 }}>
            {targets.map((h) => {
              const state = stateFor(h.id);
              return (
                <Box
                  key={h.id}
                  sx={{
                    display: "flex",
                    alignItems: "center",
                    gap: 1.25,
                    px: 1,
                    py: 0.5,
                    borderRadius: 1.5,
                    "&:hover": { bgcolor: "surface.highest" },
                    "&:hover .target-remove": { opacity: 1 },
                  }}
                >
                  <HostAvatar host={h} size={sizes.tileSmall} />
                  <Box sx={{ flex: 1, minWidth: 0 }}>
                    <Typography variant="body2" noWrap>
                      {h.label}
                    </Typography>
                    <Typography
                      variant="caption"
                      color="text.secondary"
                      noWrap
                      sx={{ display: "block" }}
                    >
                      {h.username ? `${h.username}@${h.address}` : h.address} ·{" "}
                      {h.protocol.toUpperCase()}
                    </Typography>
                  </Box>
                  {state && <TargetStateIcon state={state} />}
                  <Tooltip title="Remove target">
                    <IconButton
                      className="target-remove"
                      size="small"
                      aria-label={`Remove ${h.label}`}
                      onClick={() => onRemoveTarget(h.id)}
                      disabled={busy}
                      sx={{ opacity: 0, transition: "opacity 120ms" }}
                    >
                      <CloseRoundedIcon sx={{ fontSize: 16 }} />
                    </IconButton>
                  </Tooltip>
                </Box>
              );
            })}
          </Stack>
        )}
      </SectionCard>

      {lastRun && (
        <SectionCard
          title="Last run"
          action={
            <Typography variant="caption" color="text.secondary">
              {fmtTime(lastRun.startedAt)} · {summarize(lastRun)}
            </Typography>
          }
        >
          <RunTargets run={lastRun} />
        </SectionCard>
      )}
    </SidePanel>
  );
}

type DialogState =
  | { kind: "none" }
  | { kind: "edit"; snippet: SnippetCard | null }
  | { kind: "run"; snippet: SnippetCard }
  | { kind: "vars"; snippet: SnippetCard }
  | { kind: "targets"; snippet: SnippetCard }
  | { kind: "delete"; snippet: SnippetCard }
  | { kind: "package"; pkg: PackageNode | null }
  | { kind: "deletePackage"; pkg: PackageNode };

type PkgFilter = { kind: "all" } | { kind: "none" } | { kind: "pkg"; id: Uuid };
const ALL: PkgFilter = { kind: "all" };
const NONE: PkgFilter = { kind: "none" };

export function SnippetsPage() {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vault = useActiveVault();
  const vaultId = vault.data?.id ?? null;
  const snippets = useSnippets(vaultId);
  const packages = usePackages(vaultId);
  const hosts = useHosts(vaultId);
  const groups = useGroups(vaultId);
  const [pkgFilter, setPkgFilter] = useState<PkgFilter>(ALL);
  const [dialog, setDialog] = useState<DialogState>({ kind: "none" });
  const [selectedId, setSelectedId] = useState<Uuid | null>(null);
  const lastRun = useLastRun(selectedId);

  useCreateRequests(["snippet"], () => setDialog({ kind: "edit", snippet: null }));

  // A deleted snippet simply stops matching; the panel closes on its own.
  const selected = (snippets.data ?? []).find((s) => s.id === selectedId) ?? null;

  const invalidate = () => {
    void qc.invalidateQueries({ queryKey: ["snippets"] });
    void qc.invalidateQueries({ queryKey: ["packages"] });
    void qc.invalidateQueries({ queryKey: ["hostForm"] });
  };
  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      invalidate();
      setDialog({ kind: "none" });
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const hostList = hosts.data ?? [];
  const launch = (
    snippet: SnippetCard,
    vars: Record<string, string>,
    sessionIds: Uuid[],
    hostIds: Uuid[],
  ) => {
    setDialog({ kind: "none" });
    setSelectedId(snippet.id);
    const run = startRun({ snippet, vars, sessionIds, hostIds, hosts: hostList });
    if (run.targets.length === 0) {
      snackbar.error("Nothing to run on");
      return;
    }
    watchRun(run.id, (done) => {
      if (done.targets.some((t) => t.state === "failed")) snackbar.error(summarize(done));
      else snackbar.notify(summarize(done));
    });
  };
  const runOnTargets = (snippet: SnippetCard) => {
    if (snippet.variables.length > 0) setDialog({ kind: "vars", snippet });
    else launch(snippet, {}, [], snippet.targetHostIds);
  };

  const visible = (snippets.data ?? []).filter((s) =>
    pkgFilter.kind === "all"
      ? true
      : pkgFilter.kind === "none"
        ? s.packageId === null
        : s.packageId === pkgFilter.id,
  );
  const loading = vault.isPending || snippets.isPending || packages.isPending;
  const loadError = vault.error ?? snippets.error ?? packages.error;
  const currentPkg = pkgFilter.kind === "pkg" ? pkgFilter.id : null;

  const pkgList = packages.data ?? [];

  return (
    <Page>
      <PageHeader
        actions={
          <SplitButton
            label="New snippet"
            icon={<AddRoundedIcon />}
            disabled={!vaultId}
            onClick={() => setDialog({ kind: "edit", snippet: null })}
            items={[
              {
                label: "New snippet",
                icon: <AddRoundedIcon fontSize="small" />,
                onClick: () => setDialog({ kind: "edit", snippet: null }),
              },
              {
                label: "New package",
                icon: <CreateNewFolderRoundedIcon fontSize="small" />,
                onClick: () => setDialog({ kind: "package", pkg: null }),
              },
            ]}
          />
        }
        trailing={
          <Typography variant="body2" color="text.secondary" sx={{ px: 1 }}>
            {pkgFilter.kind === "all"
              ? `${snippets.data?.length ?? 0} ${snippets.data?.length === 1 ? "snippet" : "snippets"}`
              : `${visible.length} of ${snippets.data?.length ?? 0}`}
          </Typography>
        }
      />
      <Box sx={{ flex: 1, minHeight: 0, display: "flex" }}>
        <Box
          sx={{
            width: 200,
            flexShrink: 0,
            borderRight: 1,
            borderColor: "border.light",
            overflowY: "auto",
            py: 1,
            px: 1,
          }}
        >
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ px: 1, display: "block", mb: 0.5 }}
          >
            Packages
          </Typography>
          <List dense disablePadding>
            <ListItemButton selected={pkgFilter.kind === "all"} onClick={() => setPkgFilter(ALL)}>
              <ListItemText primary="All snippets" />
              <Typography variant="caption" color="text.secondary">
                {snippets.data?.length ?? 0}
              </Typography>
            </ListItemButton>
            <ListItemButton selected={pkgFilter.kind === "none"} onClick={() => setPkgFilter(NONE)}>
              <ListItemText primary="Unpackaged" />
              <Typography variant="caption" color="text.secondary">
                {(snippets.data ?? []).filter((x) => x.packageId === null).length}
              </Typography>
            </ListItemButton>
            {pkgList.length > 0 && <Divider sx={{ my: 1 }} />}
            {pkgList.map((p) => (
              <ListItemButton
                key={p.id}
                selected={pkgFilter.kind === "pkg" && pkgFilter.id === p.id}
                onClick={() => setPkgFilter({ kind: "pkg", id: p.id })}
                sx={{ pr: 0.5, "&:hover .pkg-actions": { opacity: 1 } }}
              >
                <FolderRoundedIcon fontSize="small" sx={{ mr: 1, color: "text.secondary" }} />
                <ListItemText primary={p.label} slotProps={{ primary: { noWrap: true } }} />
                <Box className="pkg-actions" sx={{ display: "flex", opacity: 0 }}>
                  <IconButton
                    size="small"
                    aria-label="Rename package"
                    onClick={(e) => {
                      e.stopPropagation();
                      setDialog({ kind: "package", pkg: p });
                    }}
                  >
                    <EditRoundedIcon sx={{ fontSize: 16 }} />
                  </IconButton>
                  <IconButton
                    size="small"
                    aria-label="Delete package"
                    onClick={(e) => {
                      e.stopPropagation();
                      setDialog({ kind: "deletePackage", pkg: p });
                    }}
                  >
                    <DeleteOutlineRoundedIcon sx={{ fontSize: 16 }} />
                  </IconButton>
                </Box>
              </ListItemButton>
            ))}
          </List>
        </Box>
        <PageBody>
          {loading ? (
            <Loading />
          ) : loadError ? (
            <EmptyState title="Could not load snippets" description={errorMessage(loadError)} />
          ) : visible.length === 0 ? (
            <EmptyState
              icon={<CodeRoundedIcon />}
              title={pkgFilter.kind === "all" ? "No snippets yet" : "Nothing here"}
              description="Save the commands you type over and over, pick the hosts they should run on and run them everywhere at once. Use {{name}} placeholders to be asked for values on run."
              action={
                <Button
                  variant="contained"
                  onClick={() => setDialog({ kind: "edit", snippet: null })}
                >
                  New snippet
                </Button>
              }
            />
          ) : (
            <Stack spacing={1}>
              {visible.map((s) => {
                const targetCount = s.targetHostIds.length;
                return (
                  <EntityCard
                    key={s.id}
                    selected={s.id === selectedId}
                    onClick={() => setSelectedId(s.id === selectedId ? null : s.id)}
                    onDoubleClick={() => setDialog({ kind: "edit", snippet: s })}
                    sx={{ alignItems: "flex-start" }}
                    tile={
                      <IconTile tone="purple">
                        <CodeRoundedIcon />
                      </IconTile>
                    }
                    title={
                      <>
                        {s.label}
                        {s.closeAfterRun && (
                          <Chip label="closes tab" size="small" variant="outlined" sx={{ ml: 1 }} />
                        )}
                      </>
                    }
                    subtitle={<Script script={s.script} />}
                    meta={
                      s.variables.length > 0 || targetCount > 0 ? (
                        <>
                          {targetCount > 0 && (
                            <Chip
                              label={`${targetCount} ${targetCount === 1 ? "target" : "targets"}`}
                              size="small"
                              variant="outlined"
                            />
                          )}
                          {s.variables.map((v) => (
                            <Chip
                              key={v}
                              label={`{{${v}}}`}
                              size="small"
                              sx={{ fontFamily: monoFontFamily }}
                            />
                          ))}
                        </>
                      ) : undefined
                    }
                    trailing={
                      <Button
                        variant="tonal"
                        startIcon={<PlayArrowRoundedIcon />}
                        onClick={(e) => {
                          e.stopPropagation();
                          if (targetCount > 0) runOnTargets(s);
                          else setDialog({ kind: "run", snippet: s });
                        }}
                      >
                        Run
                      </Button>
                    }
                    actions={
                      <>
                        <ToolIconButton
                          title="Edit"
                          onClick={(e) => {
                            e.stopPropagation();
                            setDialog({ kind: "edit", snippet: s });
                          }}
                        >
                          <EditRoundedIcon fontSize="small" />
                        </ToolIconButton>
                        <ToolIconButton
                          title="Delete"
                          onClick={(e) => {
                            e.stopPropagation();
                            setDialog({ kind: "delete", snippet: s });
                          }}
                        >
                          <DeleteOutlineRoundedIcon fontSize="small" />
                        </ToolIconButton>
                      </>
                    }
                  />
                );
              })}
            </Stack>
          )}
        </PageBody>
        {selected && (
          <SnippetPanel
            snippet={selected}
            hosts={hostList}
            packages={pkgList}
            lastRun={lastRun}
            busy={op.isPending}
            onClose={() => setSelectedId(null)}
            onEdit={() => setDialog({ kind: "edit", snippet: selected })}
            onDelete={() => setDialog({ kind: "delete", snippet: selected })}
            onAddTargets={() => setDialog({ kind: "targets", snippet: selected })}
            onRemoveTarget={(hostId) =>
              op.mutate(async () => {
                await ipc.snippetSetTargets(
                  selected.id,
                  selected.targetHostIds.filter((id) => id !== hostId),
                );
                return null;
              })
            }
            onRun={() => runOnTargets(selected)}
            onRunInTerminals={() => setDialog({ kind: "run", snippet: selected })}
          />
        )}
      </Box>

      {vaultId && dialog.kind === "edit" && (
        <SnippetDialog
          vaultId={vaultId}
          packages={packages.data ?? []}
          initial={dialog.snippet}
          defaultPackage={currentPkg}
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={(form) =>
            op.mutate(async () => {
              const saved = await ipc.snippetSave(form);
              setSelectedId(saved.id);
              return null;
            })
          }
        />
      )}
      {dialog.kind === "targets" && (
        <TargetsDialog
          hosts={hostList}
          groups={groups.data ?? []}
          initial={dialog.snippet.targetHostIds}
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={(hostIds) => {
            const id = dialog.snippet.id;
            op.mutate(async () => {
              await ipc.snippetSetTargets(id, hostIds);
              return null;
            });
          }}
        />
      )}
      {dialog.kind === "vars" && (
        <VariablesDialog
          snippet={dialog.snippet}
          description={`Runs on ${dialog.snippet.targetHostIds.length} configured ${
            dialog.snippet.targetHostIds.length === 1 ? "target" : "targets"
          }.`}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={(vars) => launch(dialog.snippet, vars, [], dialog.snippet.targetHostIds)}
        />
      )}
      {dialog.kind === "run" && (
        <RunDialog
          snippet={dialog.snippet}
          busy={false}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={(sessionIds, vars) => launch(dialog.snippet, vars, sessionIds, [])}
        />
      )}
      {dialog.kind === "delete" && (
        <ConfirmDialog
          open
          title="Delete snippet?"
          confirmLabel="Delete"
          danger
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={() => {
            const id = dialog.snippet.id;
            op.mutate(async () => {
              await ipc.snippetDelete(id);
              return "Snippet deleted";
            });
          }}
        >
          <b>{dialog.snippet.label}</b> will be removed, including its targets and host bindings.
        </ConfirmDialog>
      )}
      {vaultId && dialog.kind === "package" && (
        <NameDialog
          open
          title={dialog.pkg ? "Rename package" : "New package"}
          label="Name"
          initial={dialog.pkg?.label ?? ""}
          confirmLabel={dialog.pkg ? "Rename" : "Create"}
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={(label) => {
            const pkg = dialog.pkg;
            op.mutate(async () => {
              await ipc.snippetPackageSave({
                vaultId,
                id: pkg?.id ?? null,
                label,
                parentId: pkg?.parentId ?? null,
              });
              return null;
            });
          }}
        />
      )}
      {dialog.kind === "deletePackage" && (
        <ConfirmDialog
          open
          title="Delete package?"
          confirmLabel="Delete"
          danger
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={() => {
            const id = dialog.pkg.id;
            op.mutate(async () => {
              await ipc.snippetPackageDelete(id);
              if (currentPkg === id) setPkgFilter(ALL);
              return "Package deleted";
            });
          }}
        >
          Snippets inside <b>{dialog.pkg.label}</b> are kept and become unpackaged.
        </ConfirmDialog>
      )}
    </Page>
  );
}
