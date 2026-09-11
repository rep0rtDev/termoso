import { useMemo, useState } from "react";
import {
  Box,
  Button,
  Checkbox,
  Chip,
  CircularProgress,
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
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { useDefaultVault, usePackages, useSnippets } from "@/ipc/hooks";
import {
  errorMessage,
  type PackageNode,
  type SnippetCard,
  type SnippetForm,
  type Uuid,
} from "@/ipc/types";
import { NameDialog } from "@/sftp/dialogs";
import { closePane, terminalStore, type Pane } from "@/terminal/store";

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
            <TextField
              autoFocus
              label="Label"
              value={f.label}
              onChange={(e) => set("label", e.target.value)}
              sx={{ flex: 1 }}
            />
            <TextField
              select
              label="Package"
              value={f.packageId ?? ""}
              onChange={(e) => set("packageId", e.target.value === "" ? null : e.target.value)}
              sx={{ width: 200 }}
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
          </Stack>
          <TextField
            label="Script"
            value={f.script}
            onChange={(e) => set("script", e.target.value)}
            multiline
            minRows={6}
            maxRows={16}
            helperText={VAR_HINT}
            slotProps={{ htmlInput: { spellCheck: false, style: { fontFamily: "monospace" } } }}
          />
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

function RunDialog({
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
  const [vars, setVars] = useState<Record<string, string>>(() =>
    Object.fromEntries(snippet.variables.map((v) => [v, ""])),
  );
  const toggle = (id: Uuid) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const valid = selected.size > 0 && snippet.variables.every((v) => (vars[v] ?? "").length > 0);

  return (
    <Dialog open onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Run “{snippet.label}”</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 0.5 }}>
          {snippet.variables.length > 0 && (
            <Stack spacing={1.5}>
              <Typography variant="overline" color="text.secondary">
                Variables
              </Typography>
              {snippet.variables.map((v) => (
                <TextField
                  key={v}
                  label={v}
                  size="small"
                  value={vars[v] ?? ""}
                  onChange={(e) => setVars({ ...vars, [v]: e.target.value })}
                />
              ))}
            </Stack>
          )}
          <Typography variant="overline" color="text.secondary">
            Target terminals
          </Typography>
          {panes.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              No connected terminals. Open a host first.
            </Typography>
          ) : (
            <List dense disablePadding>
              {panes.map((p) => (
                <ListItemButton key={p.id} onClick={() => toggle(p.id)} sx={{ borderRadius: 1 }}>
                  <Checkbox edge="start" size="small" checked={selected.has(p.id)} tabIndex={-1} />
                  <ListItemText primary={p.title} secondary={p.subtitle} />
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

type DialogState =
  | { kind: "none" }
  | { kind: "edit"; snippet: SnippetCard | null }
  | { kind: "run"; snippet: SnippetCard }
  | { kind: "delete"; snippet: SnippetCard }
  | { kind: "package"; pkg: PackageNode | null }
  | { kind: "deletePackage"; pkg: PackageNode };

type PkgFilter = { kind: "all" } | { kind: "none" } | { kind: "pkg"; id: Uuid };
const ALL: PkgFilter = { kind: "all" };
const NONE: PkgFilter = { kind: "none" };

export function SnippetsPage() {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vault = useDefaultVault();
  const vaultId = vault.data?.id ?? null;
  const snippets = useSnippets(vaultId);
  const packages = usePackages(vaultId);
  const [pkgFilter, setPkgFilter] = useState<PkgFilter>(ALL);
  const [dialog, setDialog] = useState<DialogState>({ kind: "none" });

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

  return (
    <Page>
      <PageHeader
        title="Snippets"
        description="Reusable commands with {{variables}}, runnable in one or many terminals at once."
        actions={
          <>
            <Button
              startIcon={<CreateNewFolderRoundedIcon />}
              disabled={!vaultId}
              onClick={() => setDialog({ kind: "package", pkg: null })}
            >
              New package
            </Button>
            <Button
              variant="contained"
              startIcon={<AddRoundedIcon />}
              disabled={!vaultId}
              onClick={() => setDialog({ kind: "edit", snippet: null })}
            >
              New snippet
            </Button>
          </>
        }
      />
      <Box sx={{ flex: 1, minHeight: 0, display: "flex" }}>
        <Box
          sx={{
            width: 220,
            flexShrink: 0,
            borderRight: 1,
            borderColor: "divider",
            overflowY: "auto",
            py: 1,
          }}
        >
          <List dense disablePadding>
            <ListItemButton
              selected={pkgFilter.kind === "all"}
              onClick={() => setPkgFilter(ALL)}
              sx={{ mx: 1, borderRadius: 1.5 }}
            >
              <ListItemText primary="All snippets" />
              <Typography variant="caption" color="text.secondary">
                {snippets.data?.length ?? 0}
              </Typography>
            </ListItemButton>
            <ListItemButton
              selected={pkgFilter.kind === "none"}
              onClick={() => setPkgFilter(NONE)}
              sx={{ mx: 1, borderRadius: 1.5 }}
            >
              <ListItemText primary="Unpackaged" />
            </ListItemButton>
            {(packages.data ?? []).length > 0 && <Divider sx={{ my: 1 }} />}
            {(packages.data ?? []).map((p) => (
              <ListItemButton
                key={p.id}
                selected={pkgFilter.kind === "pkg" && pkgFilter.id === p.id}
                onClick={() => setPkgFilter({ kind: "pkg", id: p.id })}
                sx={{ mx: 1, borderRadius: 1.5, pr: 0.5, "&:hover .pkg-actions": { opacity: 1 } }}
              >
                <FolderRoundedIcon fontSize="small" sx={{ mr: 1, color: "text.secondary" }} />
                <ListItemText primary={p.label} slotProps={{ primary: { noWrap: true } }} />
                <Box className="pkg-actions" sx={{ display: "flex", opacity: 0 }}>
                  <IconButton
                    size="small"
                    onClick={(e) => {
                      e.stopPropagation();
                      setDialog({ kind: "package", pkg: p });
                    }}
                  >
                    <EditRoundedIcon sx={{ fontSize: 16 }} />
                  </IconButton>
                  <IconButton
                    size="small"
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
            <Box sx={{ display: "flex", justifyContent: "center", pt: 8 }}>
              <CircularProgress size={28} />
            </Box>
          ) : loadError ? (
            <EmptyState title="Could not load snippets" description={errorMessage(loadError)} />
          ) : visible.length === 0 ? (
            <EmptyState
              icon={<CodeRoundedIcon />}
              title={pkgFilter.kind === "all" ? "No snippets yet" : "Nothing here"}
              description="Save the commands you type over and over and run them in any connected terminal."
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
            <Stack spacing={1} sx={{ mt: 2 }}>
              {visible.map((s) => (
                <Box
                  key={s.id}
                  sx={{
                    border: 1,
                    borderColor: "divider",
                    borderRadius: 2,
                    p: 1.5,
                    display: "flex",
                    gap: 1.5,
                    alignItems: "flex-start",
                    "&:hover": { borderColor: "text.disabled" },
                  }}
                >
                  <Box sx={{ flex: 1, minWidth: 0 }}>
                    <Box sx={{ display: "flex", alignItems: "center", gap: 1, mb: 0.5 }}>
                      <Typography variant="body2" sx={{ fontWeight: 600 }} noWrap>
                        {s.label}
                      </Typography>
                      {s.variables.map((v) => (
                        <Chip
                          key={v}
                          label={`{{${v}}}`}
                          size="small"
                          variant="outlined"
                          sx={{ height: 18, fontSize: 10, fontFamily: "monospace" }}
                        />
                      ))}
                      {s.closeAfterRun && (
                        <Chip
                          label="closes tab"
                          size="small"
                          variant="outlined"
                          sx={{ height: 18, fontSize: 10 }}
                        />
                      )}
                    </Box>
                    <Typography
                      component="pre"
                      variant="body2"
                      sx={{
                        m: 0,
                        fontFamily: "monospace",
                        fontSize: 12,
                        color: "text.secondary",
                        whiteSpace: "pre-wrap",
                        maxHeight: 96,
                        overflow: "hidden",
                      }}
                    >
                      {s.script}
                    </Typography>
                  </Box>
                  <Tooltip title="Run in terminal">
                    <IconButton
                      size="small"
                      color="primary"
                      onClick={() => setDialog({ kind: "run", snippet: s })}
                    >
                      <PlayArrowRoundedIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                  <IconButton size="small" onClick={() => setDialog({ kind: "edit", snippet: s })}>
                    <EditRoundedIcon fontSize="small" />
                  </IconButton>
                  <IconButton
                    size="small"
                    onClick={() => setDialog({ kind: "delete", snippet: s })}
                  >
                    <DeleteOutlineRoundedIcon fontSize="small" />
                  </IconButton>
                </Box>
              ))}
            </Stack>
          )}
        </PageBody>
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
              await ipc.snippetSave(form);
              return null;
            })
          }
        />
      )}
      {dialog.kind === "run" && (
        <RunDialog
          snippet={dialog.snippet}
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={(sessionIds, vars) => {
            const id = dialog.snippet.id;
            op.mutate(async () => {
              const res = await ipc.snippetRun(id, sessionIds, vars);
              if (res.closeAfterRun) {
                for (const sid of res.sessionIds) void closePane(sid);
              }
              return `Sent to ${res.sessionIds.length} terminal(s)`;
            });
          }}
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
          <b>{dialog.snippet.label}</b> will be removed, including its host bindings.
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
