import { useMemo, useState } from "react";
import {
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  IconButton,
  List,
  ListItemButton,
  ListItemText,
  Menu,
  MenuItem,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import MoreVertRoundedIcon from "@mui/icons-material/MoreVertRounded";
import ContentPasteRoundedIcon from "@mui/icons-material/ContentPasteRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import TabRoundedIcon from "@mui/icons-material/TabRounded";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import OpenInNewRoundedIcon from "@mui/icons-material/OpenInNewRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import DataObjectRoundedIcon from "@mui/icons-material/DataObjectRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import HistoryRoundedIcon from "@mui/icons-material/HistoryRounded";
import PaletteRoundedIcon from "@mui/icons-material/PaletteRounded";
import InfoOutlinedIcon from "@mui/icons-material/InfoOutlined";
import RocketLaunchOutlinedIcon from "@mui/icons-material/RocketLaunchOutlined";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import RemoveRoundedIcon from "@mui/icons-material/RemoveRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { CommandHistory, HistoryItem, SnippetCard, Uuid } from "@/ipc/types";
import { errorMessage, isPostQuantumKex } from "@/ipc/types";
import * as ipc from "@/ipc/commands";
import {
  keys,
  useCommandHistory,
  useHistory,
  useHosts,
  useSaveSettings,
  useSettings,
  useSnippets,
} from "@/ipc/hooks";
import { useActiveVault } from "@/app/vault";
import { goToSection, requestCreate } from "@/app/navigation";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { useSnackbar } from "@/components/Snackbar";
import { ActionMenu, Loading, Mono, SearchField, type MenuAction } from "@/components/ui";
import { ThemeCard } from "@/settings/ThemeGallery";
import { FontPicker } from "@/settings/FontPicker";
import { SearchBar } from "./SearchBar";
import { VariablesDialog } from "@/snippets/SnippetsPage";
import { RunTargets } from "@/snippets/RunStatus";
import { startRun, summarize, useRuns, watchRun } from "@/snippets/run";
import { AUTO_THEME, terminalThemeById, terminalThemes } from "./themes";
import {
  autocompleteOn,
  copyText,
  dropHistoryCache,
  endOfToday,
  openTerminal,
  pauseSuggestions,
  runCommand,
  setPaneAutocomplete,
  setSidePanel,
  setTabTheme,
  terminalStore,
  useTerminal,
  type Pane,
  type SidePanelTab,
  type TerminalTab,
} from "./store";

export const SIDE_PANEL_WIDTH = 300;

const TABS: { id: SidePanelTab; label: string; icon: React.ReactNode }[] = [
  { id: "search", label: "Terminal", icon: <RocketLaunchOutlinedIcon /> },
  { id: "snippets", label: "Snippets", icon: <DataObjectRoundedIcon /> },
  { id: "history", label: "History", icon: <HistoryRoundedIcon /> },
  { id: "themes", label: "Themes", icon: <PaletteRoundedIcon /> },
  { id: "info", label: "Session info", icon: <InfoOutlinedIcon /> },
];

export function TerminalSidePanel({ tab }: { tab: TerminalTab }) {
  const panel = useTerminal((s) => s.sidePanel);
  const pane = useTerminal((s) => s.panes[tab.activePaneId]);
  if (!panel || !pane) return null;
  return (
    <Box sx={{ flexShrink: 0, display: "flex", minHeight: 0, py: 0.75, pr: 0.75 }}>
      <Box
        sx={{
          width: SIDE_PANEL_WIDTH,
          display: "flex",
          flexDirection: "column",
          minHeight: 0,
          borderRadius: 2,
          bgcolor: "surface.high",
          overflow: "hidden",
        }}
      >
        <Stack direction="row" sx={{ alignItems: "center", gap: 0.5, px: 1.25, pt: 1.25, pb: 0.5 }}>
          {TABS.map((t) => {
            const active = panel === t.id;
            return (
              <Tooltip key={t.id} title={t.label} enterDelay={600}>
                <IconButton
                  size="small"
                  aria-label={t.label}
                  aria-pressed={active}
                  onClick={() => setSidePanel(t.id)}
                  sx={{
                    width: 32,
                    height: 32,
                    borderRadius: 1.5,
                    color: active ? "primary.main" : "text.secondary",
                    bgcolor: active ? "rgba(43,184,132,0.18)" : "transparent",
                    "&:hover": { bgcolor: active ? "rgba(43,184,132,0.26)" : "surface.highest" },
                    "& svg": { fontSize: 18 },
                  }}
                >
                  {t.icon}
                </IconButton>
              </Tooltip>
            );
          })}
          <Box sx={{ flex: 1 }} />
          <IconButton
            size="small"
            onClick={() => setSidePanel(null)}
            aria-label="Close panel"
            sx={{ width: 32, height: 32, borderRadius: 1.5, color: "text.secondary" }}
          >
            <CloseRoundedIcon sx={{ fontSize: 18 }} />
          </IconButton>
        </Stack>
        <Box sx={{ flex: 1, minHeight: 0, overflow: "auto" }}>
          {panel === "search" && <TerminalPanel pane={pane} />}
          {panel === "snippets" && <SnippetsPanel pane={pane} />}
          {panel === "history" && <HistoryPanel pane={pane} />}
          {panel === "themes" && <ThemesPanel tab={tab} pane={pane} />}
          {panel === "info" && <InfoPanel pane={pane} tab={tab} />}
        </Box>
      </Box>
    </Box>
  );
}

/* ------------------------------------------------------------- terminal */

/** Termius' first section: buffer search on top, autocomplete state below. */
function TerminalPanel({ pane }: { pane: Pane }) {
  const settings = useSettings();
  const save = useSaveSettings();
  const snackbar = useSnackbar();
  const paused = useTerminal(
    (s) => s.suggestPausedUntil !== null && s.suggestPausedUntil > Date.now(),
  );
  const on = useTerminal((s) => autocompleteOn(pane.id, s));
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const globalOff = settings.data?.autocomplete === false;
  const label = globalOff
    ? "Disabled"
    : paused
      ? "Paused today"
      : pane.autocomplete
        ? "Enabled"
        : "Off in this tab";

  const setGlobal = (autocomplete: boolean) => {
    if (!settings.data) return;
    save.mutate(
      { ...settings.data, autocomplete },
      { onError: (e) => snackbar.error(errorMessage(e)) },
    );
  };

  return (
    <Stack sx={{ p: 1.5, gap: 1.25 }}>
      <SearchBar key={pane.id} paneId={pane.id} onClose={() => setSidePanel(null)} />
      <Divider />
      <Stack direction="row" sx={{ alignItems: "center", gap: 1, px: 0.5 }}>
        <Typography variant="subtitle2" sx={{ flex: 1 }}>
          Autocomplete
        </Typography>
        <Button
          size="small"
          color="inherit"
          endIcon={<ExpandMoreRoundedIcon />}
          onClick={(e) => setAnchor(e.currentTarget)}
          sx={{ color: on ? "text.primary" : "text.secondary", fontWeight: 500 }}
          aria-label={`Autocomplete: ${label}`}
        >
          {label}
        </Button>
        <Menu open={anchor !== null} anchorEl={anchor} onClose={() => setAnchor(null)}>
          <MenuItem
            selected={on}
            onClick={() => {
              setAnchor(null);
              if (globalOff) setGlobal(true);
              if (paused) pauseSuggestions(null);
              setPaneAutocomplete(pane.id, true);
            }}
          >
            Enabled
          </MenuItem>
          <MenuItem
            selected={!globalOff && !paused && !pane.autocomplete}
            onClick={() => {
              setAnchor(null);
              setPaneAutocomplete(pane.id, false);
            }}
          >
            Off in this tab
          </MenuItem>
          <MenuItem
            selected={paused}
            onClick={() => {
              setAnchor(null);
              pauseSuggestions(endOfToday());
            }}
          >
            Pause until tomorrow
          </MenuItem>
          <MenuItem
            selected={globalOff}
            onClick={() => {
              setAnchor(null);
              setGlobal(false);
            }}
          >
            Disabled everywhere
          </MenuItem>
        </Menu>
      </Stack>
      <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
        Offline suggestions from commands, paths, snippets and your history. Tab inserts, Esc
        dismisses.
      </Typography>
    </Stack>
  );
}

/* ------------------------------------------------------------- snippets */

type SnippetAction = "run" | "paste" | "all" | "targets";

const ACTION_LABEL: Record<SnippetAction, string> = {
  run: "Run in this terminal",
  paste: "Paste into this terminal",
  all: "Run in all tabs",
  targets: "Run on configured targets",
};

function SnippetsPanel({ pane }: { pane: Pane }) {
  const vault = useActiveVault();
  const vaultId = vault.data?.id ?? null;
  const snippets = useSnippets(vaultId);
  const hosts = useHosts(vaultId);
  const qc = useQueryClient();
  const [query, setQuery] = useState("");
  const [menu, setMenu] = useState<{
    snippet: SnippetCard;
    anchor: HTMLElement | null;
    position: { left: number; top: number } | null;
  } | null>(null);
  const [pending, setPending] = useState<{ snippet: SnippetCard; action: SnippetAction } | null>(
    null,
  );
  const [remove, setRemove] = useState<SnippetCard | null>(null);
  const [runId, setRunId] = useState<string | null>(null);
  const run = useRuns((s) => (runId ? s.runs.find((r) => r.id === runId) : undefined));
  const snackbar = useSnackbar();
  const del = useMutation({
    mutationFn: (id: Uuid) => ipc.snippetDelete(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["snippets"] });
      setRemove(null);
      snackbar.notify("Snippet deleted");
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const q = query.trim().toLowerCase();
  const items = (snippets.data ?? []).filter(
    (s) => !q || s.label.toLowerCase().includes(q) || s.script.toLowerCase().includes(q),
  );
  const connected = pane.status === "connected";
  const panes = useTerminal((s) => s.panes);
  const openCount = useMemo(
    () =>
      Object.values(panes).filter((p) => p.status === "connected" && p.protocol !== null).length,
    [panes],
  );

  const execute = (snippet: SnippetCard, action: SnippetAction, vars: Record<string, string>) => {
    const sessionIds =
      action === "all"
        ? Object.values(terminalStore.get().panes)
            .filter((p) => p.status === "connected" && p.protocol !== null)
            .map((p) => p.id)
        : action === "targets"
          ? []
          : [pane.id];
    const started = startRun({
      snippet,
      vars,
      sessionIds,
      hostIds: action === "targets" ? snippet.targetHostIds : [],
      hosts: hosts.data ?? [],
      paste: action === "paste",
    });
    if (started.targets.length === 0) {
      snackbar.error("Nothing to run on");
      return;
    }
    setRunId(started.id);
    watchRun(started.id, (done) => {
      if (done.targets.length === 1 && action !== "targets") return;
      if (done.targets.some((t) => t.state === "failed")) snackbar.error(summarize(done));
      else snackbar.notify(summarize(done));
    });
  };
  const trigger = (snippet: SnippetCard, action: SnippetAction) => {
    if (snippet.variables.length > 0) setPending({ snippet, action });
    else execute(snippet, action, {});
  };

  const menuItems = (s: SnippetCard): MenuAction[] => [
    {
      label: "Run",
      icon: <PlayArrowRoundedIcon fontSize="small" />,
      disabled: !connected,
      onClick: () => trigger(s, "run"),
    },
    {
      label: "Paste",
      icon: <ContentPasteRoundedIcon fontSize="small" />,
      disabled: !connected,
      onClick: () => trigger(s, "paste"),
    },
    {
      label: openCount > 1 ? `Run in all tabs (${openCount})` : "Run in all tabs",
      icon: <TabRoundedIcon fontSize="small" />,
      disabled: openCount === 0,
      onClick: () => trigger(s, "all"),
    },
    {
      label:
        s.targetHostIds.length > 0
          ? `Run on targets (${s.targetHostIds.length})`
          : "Run on targets",
      icon: <DnsRoundedIcon fontSize="small" />,
      disabled: s.targetHostIds.length === 0,
      divider: true,
      onClick: () => trigger(s, "targets"),
    },
    {
      label: "Copy script",
      icon: <ContentCopyRoundedIcon fontSize="small" />,
      onClick: () => void copyText(s.script).then(() => snackbar.notify("Copied")),
    },
    {
      label: "Open in Snippets",
      icon: <OpenInNewRoundedIcon fontSize="small" />,
      divider: true,
      onClick: () => goToSection("snippets"),
    },
    {
      label: "Remove",
      icon: <DeleteOutlineRoundedIcon fontSize="small" />,
      danger: true,
      onClick: () => setRemove(s),
    },
  ];

  return (
    <Stack sx={{ p: 1.5, gap: 1.5 }}>
      <Stack direction="row" sx={{ alignItems: "center", gap: 1 }}>
        <Button
          size="small"
          variant="tonal"
          startIcon={<DataObjectRoundedIcon />}
          onClick={() => requestCreate("snippet")}
          sx={{ flexShrink: 0 }}
        >
          New snippet
        </Button>
        <SearchField value={query} onChange={setQuery} placeholder="Search" width="100%" />
      </Stack>
      {snippets.isPending ? (
        <Loading pt={4} />
      ) : snippets.error ? (
        <EmptyState
          compact
          title="Could not load snippets"
          description={errorMessage(snippets.error)}
        />
      ) : items.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ px: 0.5 }}>
          {q
            ? `Nothing matches “${query}”.`
            : "No snippets yet — add them in the Snippets section."}
        </Typography>
      ) : (
        <List dense disablePadding>
          {items.map((s) => (
            <ListItemButton
              key={s.id}
              disabled={!connected}
              onClick={() => trigger(s, "run")}
              onContextMenu={(e) => {
                e.preventDefault();
                setMenu({
                  snippet: s,
                  anchor: null,
                  position: { left: e.clientX, top: e.clientY },
                });
              }}
              sx={{
                borderRadius: 1.5,
                alignItems: "flex-start",
                gap: 0.5,
                pr: 0.5,
                "&:hover .snippet-more, &.Mui-focusVisible .snippet-more": { opacity: 1 },
              }}
            >
              <ListItemText
                primary={s.label}
                secondary={
                  <Mono secondary sx={{ fontSize: 11, display: "block" }}>
                    {firstLine(s.script)}
                  </Mono>
                }
                slotProps={{
                  primary: { variant: "body2", noWrap: true, sx: { fontWeight: 600 } },
                  secondary: { component: "div", noWrap: true },
                }}
              />
              {s.targetHostIds.length > 0 && (
                <Tooltip title={`${s.targetHostIds.length} configured targets`}>
                  <Chip
                    size="small"
                    variant="outlined"
                    icon={<DnsRoundedIcon />}
                    label={s.targetHostIds.length}
                    sx={{ mt: 0.25, height: 20, "& .MuiChip-label": { px: 0.5 } }}
                  />
                </Tooltip>
              )}
              <IconButton
                className="snippet-more"
                size="small"
                aria-label="Snippet actions"
                onClick={(e) => {
                  e.stopPropagation();
                  setMenu({ snippet: s, anchor: e.currentTarget, position: null });
                }}
                sx={{ opacity: 0, transition: "opacity 120ms", mt: -0.25 }}
              >
                <MoreVertRoundedIcon fontSize="small" />
              </IconButton>
            </ListItemButton>
          ))}
        </List>
      )}
      {!connected && (
        <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
          Snippets run once the terminal is connected. Right-click for more actions.
        </Typography>
      )}
      {run && (
        <Box
          sx={{
            p: 1.25,
            borderRadius: 2,
            bgcolor: "surface.high",
            display: "flex",
            flexDirection: "column",
            gap: 0.75,
          }}
        >
          <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
            <Box sx={{ flex: 1, minWidth: 0 }}>
              <Typography variant="body2" noWrap sx={{ fontWeight: 600 }}>
                {run.label}
              </Typography>
              <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
                {summarize(run)}
              </Typography>
            </Box>
            <IconButton size="small" aria-label="Dismiss" onClick={() => setRunId(null)}>
              <CloseRoundedIcon sx={{ fontSize: 16 }} />
            </IconButton>
          </Box>
          <RunTargets run={run} dense />
        </Box>
      )}
      {menu && (
        <ActionMenu
          anchor={menu.anchor}
          position={menu.position}
          onClose={() => setMenu(null)}
          items={menuItems(menu.snippet)}
        />
      )}
      {pending && (
        <VariablesDialog
          snippet={pending.snippet}
          description={ACTION_LABEL[pending.action]}
          onCancel={() => setPending(null)}
          onConfirm={(vars) => {
            execute(pending.snippet, pending.action, vars);
            setPending(null);
          }}
        />
      )}
      {remove && (
        <ConfirmDialog
          open
          title="Delete snippet?"
          confirmLabel="Delete"
          danger
          busy={del.isPending}
          onCancel={() => setRemove(null)}
          onConfirm={() => del.mutate(remove.id)}
        >
          <b>{remove.label}</b> will be removed, including its targets and host bindings.
        </ConfirmDialog>
      )}
    </Stack>
  );
}

function firstLine(script: string): string {
  return (
    script
      .split("\n")
      .find((l) => l.trim().length > 0)
      ?.trim() ?? ""
  );
}

/* -------------------------------------------------------------- history */

function HistoryPanel({ pane }: { pane: Pane }) {
  const [mode, setMode] = useState<"commands" | "connections">("commands");
  const [query, setQuery] = useState("");
  return (
    <Stack sx={{ p: 1.5, gap: 1.5 }}>
      <ToggleButtonGroup
        exclusive
        fullWidth
        size="small"
        value={mode}
        onChange={(_, v: "commands" | "connections" | null) => v && setMode(v)}
      >
        <ToggleButton value="commands">Commands</ToggleButton>
        <ToggleButton value="connections">Connections</ToggleButton>
      </ToggleButtonGroup>
      <SearchField
        value={query}
        onChange={setQuery}
        placeholder={mode === "commands" ? "Search commands" : "Search connections"}
        width={SIDE_PANEL_WIDTH - 24}
      />
      {mode === "commands" ? (
        <CommandHistoryList pane={pane} query={query} />
      ) : (
        <ConnectionHistoryList query={query} />
      )}
    </Stack>
  );
}

/** One row per distinct command (latest occurrence wins), newest first. */
function dedupeCommands(items: readonly HistoryItem<CommandHistory>[]) {
  const byCommand = new Map<string, { latest: HistoryItem<CommandHistory>; ids: Uuid[] }>();
  for (const it of items) {
    const entry = byCommand.get(it.data.command);
    if (entry) entry.ids.push(it.id);
    else byCommand.set(it.data.command, { latest: it, ids: [it.id] });
  }
  return [...byCommand.values()];
}

function CommandHistoryList({ pane, query }: { pane: Pane; query: string }) {
  const history = useCommandHistory();
  const hosts = useHosts(null);
  const vault = useActiveVault();
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  const [thisHost, setThisHost] = useState(true);
  const [confirmClear, setConfirmClear] = useState(false);
  const [saving, setSaving] = useState<string | null>(null);

  const refresh = () => {
    dropHistoryCache();
    return qc.invalidateQueries({ queryKey: keys.history });
  };
  const remove = useMutation({
    mutationFn: async (ids: Uuid[]) => {
      for (const id of ids) await ipc.historyDelete(id);
    },
    onSuccess: refresh,
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const clear = useMutation({
    mutationFn: () => ipc.historyClearCommands(),
    onSuccess: async () => {
      setConfirmClear(false);
      await refresh();
      snackbar.notify("Command history cleared");
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const save = useMutation({
    mutationFn: (form: { label: string; script: string }) => {
      if (!vault.data) throw new Error("No vault available");
      return ipc.snippetSave({
        id: null,
        vaultId: vault.data.id,
        label: form.label,
        script: form.script,
        packageId: null,
        closeAfterRun: false,
        sortOrder: 0,
      });
    },
    onSuccess: async () => {
      setSaving(null);
      await qc.invalidateQueries({ queryKey: keys.snippets(null) });
      snackbar.notify("Saved as snippet");
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const hostLabel = useMemo(() => {
    const m = new Map<Uuid, string>();
    for (const h of hosts.data ?? []) m.set(h.id, h.label);
    return m;
  }, [hosts.data]);

  const q = query.trim().toLowerCase();
  const filterHost = thisHost && pane.hostId !== null;
  const rows = useMemo(
    () =>
      dedupeCommands(
        (history.data ?? []).filter(
          (i) =>
            (!filterHost || i.data.host_id === pane.hostId) &&
            (!q || i.data.command.toLowerCase().includes(q)),
        ),
      ),
    [history.data, filterHost, pane.hostId, q],
  );
  const connected = pane.status === "connected";

  return (
    <>
      <Stack direction="row" sx={{ alignItems: "center", justifyContent: "space-between", gap: 1 }}>
        <Chip
          size="small"
          variant={filterHost ? "filled" : "outlined"}
          color={filterHost ? "primary" : "default"}
          label={pane.hostId ? "This host" : "All hosts"}
          disabled={pane.hostId === null}
          onClick={() => setThisHost((v) => !v)}
        />
        <Button
          size="small"
          color="inherit"
          disabled={(history.data ?? []).length === 0}
          onClick={() => setConfirmClear(true)}
        >
          Clear all
        </Button>
      </Stack>
      {history.isPending ? (
        <Loading pt={4} />
      ) : history.error ? (
        <EmptyState
          compact
          title="Could not load history"
          description={errorMessage(history.error)}
        />
      ) : rows.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ px: 0.5 }}>
          {q
            ? `Nothing matches “${query}”.`
            : filterHost
              ? "No commands recorded on this host yet."
              : "No commands yet. Bash, zsh and fish sessions record commands as you run them."}
        </Typography>
      ) : (
        <List dense disablePadding>
          {rows.map(({ latest, ids }) => (
            <Stack
              key={latest.id}
              direction="row"
              sx={{
                alignItems: "center",
                borderRadius: 1.5,
                pr: 0.5,
                "&:hover": { bgcolor: "action.hover" },
                "&:hover .row-actions": { opacity: 1 },
              }}
            >
              <ListItemButton
                disabled={!connected}
                onClick={() => runCommand(pane.id, latest.data.command)}
                sx={{ borderRadius: 1.5, gap: 1, flex: 1, minWidth: 0, py: 0.5 }}
              >
                <ListItemText
                  primary={latest.data.command}
                  secondary={`${
                    latest.data.host_id
                      ? (hostLabel.get(latest.data.host_id) ?? "Removed host")
                      : "Local"
                  } · ${relativeTime(latest.created_at)}${ids.length > 1 ? ` · ×${ids.length}` : ""}`}
                  slotProps={{
                    primary: {
                      variant: "body2",
                      noWrap: true,
                      sx: { fontFamily: "monospace", fontSize: 12.5 },
                    },
                    secondary: { noWrap: true },
                  }}
                />
              </ListItemButton>
              <Stack direction="row" className="row-actions" sx={{ opacity: 0, flexShrink: 0 }}>
                <Tooltip title="Save as snippet">
                  <IconButton size="small" onClick={() => setSaving(latest.data.command)}>
                    <DataObjectRoundedIcon sx={{ fontSize: 16 }} />
                  </IconButton>
                </Tooltip>
                <Tooltip title="Delete">
                  <IconButton
                    size="small"
                    disabled={remove.isPending}
                    onClick={() => remove.mutate(ids)}
                  >
                    <DeleteOutlineRoundedIcon sx={{ fontSize: 16 }} />
                  </IconButton>
                </Tooltip>
              </Stack>
            </Stack>
          ))}
        </List>
      )}
      <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
        Stored encrypted on this device; lines that look like they contain a password or token are
        never recorded.
      </Typography>
      <ConfirmDialog
        open={confirmClear}
        title="Clear command history?"
        confirmLabel="Clear"
        danger
        busy={clear.isPending}
        onCancel={() => setConfirmClear(false)}
        onConfirm={() => clear.mutate()}
      >
        Removes every recorded command from this device (and from sync, if enabled).
      </ConfirmDialog>
      {saving !== null && (
        <SaveSnippetDialog
          script={saving}
          busy={save.isPending}
          onCancel={() => setSaving(null)}
          onConfirm={(label, script) => save.mutate({ label, script })}
        />
      )}
    </>
  );
}

function SaveSnippetDialog({
  script: initialScript,
  busy,
  onCancel,
  onConfirm,
}: {
  script: string;
  busy: boolean;
  onCancel: () => void;
  onConfirm: (label: string, script: string) => void;
}) {
  const [label, setLabel] = useState(initialScript.split(/\s+/).slice(0, 3).join(" "));
  const [script, setScript] = useState(initialScript);
  const ok = label.trim().length > 0 && script.trim().length > 0;
  return (
    <Dialog open onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Save as snippet</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 0.5 }}>
          <TextField
            label="Name"
            size="small"
            autoFocus
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
          <TextField
            label="Script"
            size="small"
            multiline
            minRows={2}
            value={script}
            onChange={(e) => setScript(e.target.value)}
            slotProps={{ input: { sx: { fontFamily: "monospace", fontSize: 12.5 } } }}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button color="inherit" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={!ok || busy}
          onClick={() => onConfirm(label.trim(), script)}
        >
          Save
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function ConnectionHistoryList({ query }: { query: string }) {
  const history = useHistory();
  const q = query.trim().toLowerCase();
  const items = (history.data ?? []).filter(
    (i) => !q || i.data.label.toLowerCase().includes(q) || i.data.target.toLowerCase().includes(q),
  );
  return (
    <>
      {history.isPending ? (
        <Loading pt={4} />
      ) : history.error ? (
        <EmptyState
          compact
          title="Could not load history"
          description={errorMessage(history.error)}
        />
      ) : items.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ px: 0.5 }}>
          {q ? `Nothing matches “${query}”.` : "No connections yet."}
        </Typography>
      ) : (
        <List dense disablePadding>
          {items.map((item) => (
            <ListItemButton
              key={item.id}
              disabled={!item.data.host_id}
              onClick={() => {
                if (item.data.host_id) openTerminal({ kind: "host", host_id: item.data.host_id });
              }}
              sx={{ borderRadius: 1.5, gap: 1 }}
            >
              {item.data.error ? (
                <ErrorOutlineRoundedIcon fontSize="small" color="error" />
              ) : (
                <TerminalRoundedIcon fontSize="small" sx={{ color: "text.secondary" }} />
              )}
              <ListItemText
                primary={item.data.label}
                secondary={`${item.data.target} · ${relativeTime(item.created_at)}`}
                slotProps={{
                  primary: { variant: "body2", noWrap: true, sx: { fontWeight: 600 } },
                  secondary: { noWrap: true },
                }}
              />
            </ListItemButton>
          ))}
        </List>
      )}
    </>
  );
}

function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const m = Math.round(diff / 60_000);
  if (m < 1) return "just now";
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  const d = Math.round(h / 24);
  if (d < 7) return `${d} d ago`;
  return new Date(iso).toLocaleDateString();
}

/* --------------------------------------------------------------- themes */

const TEXT_SIZE_MIN = 8;
const TEXT_SIZE_MAX = 40;

/** Termius layout: Font, Text Size (− value +), then the theme list. */
function ThemesPanel({ tab, pane }: { tab: TerminalTab; pane: Pane }) {
  const settings = useSettings();
  const save = useSaveSettings();
  const snackbar = useSnackbar();
  const [filter, setFilter] = useState<"all" | "dark" | "light">("all");
  const s = settings.data;
  const updateText = (patch: { terminalFontFamily?: string; terminalFontSize?: number }) => {
    if (!s) return;
    save.mutate({ ...s, ...patch }, { onError: (e) => snackbar.error(errorMessage(e)) });
  };
  const size = s?.terminalFontSize ?? 14;
  const list = useMemo(
    () => terminalThemes.filter((t) => filter === "all" || (filter === "dark") === t.dark),
    [filter],
  );
  const inherited = pane.hostTheme ?? settings.data?.terminalTheme ?? AUTO_THEME;
  const inheritedName =
    inherited === AUTO_THEME
      ? "Auto (follows app theme)"
      : (terminalThemeById(inherited)?.name ?? inherited);
  const current = tab.themeOverride ?? inherited;

  return (
    <Stack sx={{ p: 1.5, gap: 1.5 }}>
      <Stack sx={{ gap: 1 }}>
        <Typography variant="subtitle2" sx={{ color: "primary.main", px: 0.5 }}>
          Font
        </Typography>
        <FontPicker
          value={s?.terminalFontFamily ?? ""}
          onChange={(name) => updateText({ terminalFontFamily: name })}
          fullWidth
        />
        <Stack direction="row" sx={{ alignItems: "center", gap: 0.5, px: 0.5 }}>
          <Typography variant="body2" sx={{ flex: 1 }}>
            Text Size
          </Typography>
          <IconButton
            size="small"
            aria-label="Smaller text"
            disabled={!s || size <= TEXT_SIZE_MIN}
            onClick={() => updateText({ terminalFontSize: size - 1 })}
            sx={{ width: 32, height: 32, borderRadius: 1.5, bgcolor: "surface.highest" }}
          >
            <RemoveRoundedIcon sx={{ fontSize: 18 }} />
          </IconButton>
          <Box
            sx={{
              width: 44,
              height: 32,
              borderRadius: 1.5,
              bgcolor: "surface.highest",
              display: "grid",
              placeItems: "center",
              fontSize: 13,
              fontVariantNumeric: "tabular-nums",
            }}
            aria-label="Text size"
          >
            {size}
          </Box>
          <IconButton
            size="small"
            aria-label="Larger text"
            disabled={!s || size >= TEXT_SIZE_MAX}
            onClick={() => updateText({ terminalFontSize: size + 1 })}
            sx={{ width: 32, height: 32, borderRadius: 1.5, bgcolor: "surface.highest" }}
          >
            <AddRoundedIcon sx={{ fontSize: 18 }} />
          </IconButton>
        </Stack>
      </Stack>
      <Divider />
      <Stack direction="row" sx={{ alignItems: "center", justifyContent: "space-between", gap: 1 }}>
        <Typography variant="subtitle2" sx={{ px: 0.5 }}>
          Themes
        </Typography>
        <ToggleButtonGroup
          exclusive
          size="small"
          value={filter}
          onChange={(_, v: "all" | "dark" | "light" | null) => v && setFilter(v)}
        >
          <ToggleButton value="all">All</ToggleButton>
          <ToggleButton value="dark">Dark</ToggleButton>
          <ToggleButton value="light">Light</ToggleButton>
        </ToggleButtonGroup>
        <Button
          size="small"
          color="inherit"
          disabled={tab.themeOverride === null}
          onClick={() => setTabTheme(tab.id, null)}
        >
          Reset
        </Button>
      </Stack>
      <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
        Applies to this tab only. Default for this session:{" "}
        <Box component="span" sx={{ color: "text.primary" }}>
          {inheritedName}
        </Box>
        {pane.hostTheme ? " (from host)" : ""}.
      </Typography>
      <Stack spacing={0.25} sx={{ mx: -0.75 }}>
        {list.map((t) => (
          <ThemeCard
            key={t.id}
            theme={t}
            label={t.name}
            compact
            selected={current === t.id}
            onClick={() => setTabTheme(tab.id, t.id)}
          />
        ))}
      </Stack>
    </Stack>
  );
}

/* ----------------------------------------------------------------- info */

function InfoPanel({ pane, tab }: { pane: Pane; tab: TerminalTab }) {
  const a = pane.algorithms;
  const rows: [string, React.ReactNode][] = [
    ["Name", pane.title],
    ["Target", <Mono key="t">{pane.subtitle || "—"}</Mono>],
    ["Protocol", pane.protocol ? pane.protocol.toUpperCase() : "—"],
    ["State", <StateChip key="s" status={pane.status} message={pane.message} />],
    ["Started", pane.startedAt ? new Date(pane.startedAt).toLocaleString() : "—"],
  ];
  if (pane.via.length > 0) {
    rows.push(["Via", <Mono key="v">{pane.via.join(" → ")}</Mono>]);
  }
  if (tab.paneIds.length > 1) {
    rows.push(["Panes in tab", String(tab.paneIds.length)]);
  }
  rows.push(["Zoom", `${Math.round(tab.zoom * 100)}%`]);

  const shellRows: [string, React.ReactNode][] = [
    ["Shell", pane.shell ? <Mono key="sh">{pane.shell}</Mono> : "unknown"],
    [
      "Integration",
      pane.integration ? (
        <Chip
          key="i"
          size="small"
          color="success"
          variant="outlined"
          label="active"
          sx={{ height: 20 }}
        />
      ) : pane.shell && ["bash", "zsh", "fish"].includes(pane.shell) ? (
        "waiting for prompt"
      ) : (
        "not available"
      ),
    ],
    ["Directory", pane.cwd ? <Mono key="cwd">{pane.cwd}</Mono> : "—"],
    ["Last exit", pane.lastExit === null ? "—" : String(pane.lastExit)],
    ["Suggestions", suggestionsState(pane)],
  ];

  return (
    <Stack sx={{ p: 1.5, gap: 1.5 }}>
      <Section title="Session">
        {rows.map(([k, v]) => (
          <Row key={k} label={k} value={v} />
        ))}
      </Section>
      <Section title="Shell">
        {shellRows.map(([k, v]) => (
          <Row key={k} label={k} value={v} />
        ))}
      </Section>
      {a && (
        <Section
          title="Encryption"
          action={
            isPostQuantumKex(a) ? (
              <Chip size="small" color="primary" variant="outlined" label="Quantum-safe" />
            ) : null
          }
        >
          <Row label="Key exchange" value={<Mono>{a.kex}</Mono>} />
          <Row label="Host key" value={<Mono>{a.hostKey}</Mono>} />
          <Row label="Cipher" value={<Mono>{a.cipher}</Mono>} />
          <Row label="MAC" value={<Mono>{a.mac}</Mono>} />
        </Section>
      )}
      <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
        Credentials and keys are never shown here.
      </Typography>
    </Stack>
  );
}

function suggestionsState(pane: Pane): string {
  if (!pane.autocomplete) return "off for this session";
  return autocompleteOn(pane.id) ? "on" : "paused";
}

function Section({
  title,
  action,
  children,
}: {
  title: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Box sx={{ bgcolor: "surface.highest", borderRadius: 2, p: 1.5 }}>
      <Stack direction="row" sx={{ alignItems: "center", justifyContent: "space-between", mb: 1 }}>
        <Typography variant="subtitle2">{title}</Typography>
        {action}
      </Stack>
      <Stack divider={<Divider flexItem />} spacing={0.75}>
        {children}
      </Stack>
    </Box>
  );
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <Stack direction="row" sx={{ gap: 1, alignItems: "baseline", py: 0.25 }}>
      <Typography variant="caption" color="text.secondary" sx={{ width: 92, flexShrink: 0 }}>
        {label}
      </Typography>
      <Typography
        variant="body2"
        component="div"
        sx={{ flex: 1, minWidth: 0, overflowWrap: "anywhere", fontSize: 12.5 }}
      >
        {value}
      </Typography>
    </Stack>
  );
}

function StateChip({ status, message }: { status: Pane["status"]; message: string | null }) {
  const color =
    status === "connected"
      ? "success"
      : status === "connecting"
        ? "warning"
        : status === "error"
          ? "error"
          : "default";
  return (
    <Tooltip title={message ?? ""}>
      <Chip size="small" color={color} variant="outlined" label={status} sx={{ height: 20 }} />
    </Tooltip>
  );
}
