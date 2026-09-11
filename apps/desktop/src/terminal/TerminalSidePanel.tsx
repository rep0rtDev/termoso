import { useMemo, useState } from "react";
import {
  Box,
  Button,
  Chip,
  Divider,
  IconButton,
  List,
  ListItemButton,
  ListItemText,
  Stack,
  Tab,
  Tabs,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import { useMutation } from "@tanstack/react-query";
import type { SnippetCard, Uuid } from "@/ipc/types";
import { errorMessage, isPostQuantumKex } from "@/ipc/types";
import * as ipc from "@/ipc/commands";
import { useDefaultVault, useHistory, useSettings, useSnippets } from "@/ipc/hooks";
import { EmptyState } from "@/components/EmptyState";
import { useSnackbar } from "@/components/Snackbar";
import { Loading, Mono, SearchField } from "@/components/ui";
import { ThemeCard } from "@/settings/ThemeGallery";
import { RunDialog } from "@/snippets/SnippetsPage";
import { AUTO_THEME, terminalThemeById, terminalThemes } from "./themes";
import {
  closePane,
  openTerminal,
  setSidePanel,
  setTabTheme,
  useTerminal,
  type Pane,
  type SidePanelTab,
  type TerminalTab,
} from "./store";

export const SIDE_PANEL_WIDTH = 300;

const TABS: { id: SidePanelTab; label: string }[] = [
  { id: "snippets", label: "Snippets" },
  { id: "history", label: "History" },
  { id: "themes", label: "Themes" },
  { id: "info", label: "Info" },
];

export function TerminalSidePanel({ tab }: { tab: TerminalTab }) {
  const panel = useTerminal((s) => s.sidePanel);
  const pane = useTerminal((s) => s.panes[tab.activePaneId]);
  if (!panel || !pane) return null;
  return (
    <Box
      sx={{
        width: SIDE_PANEL_WIDTH,
        flexShrink: 0,
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
        borderLeft: 1,
        borderColor: "divider",
        bgcolor: "surface.lowest",
      }}
    >
      <Stack
        direction="row"
        sx={{ alignItems: "center", pr: 0.5, borderBottom: 1, borderColor: "divider" }}
      >
        <Tabs
          value={panel}
          onChange={(_, v: SidePanelTab) => setSidePanel(v)}
          variant="fullWidth"
          sx={{ flex: 1, minHeight: 36, "& .MuiTab-root": { minHeight: 36, minWidth: 0, px: 1 } }}
        >
          {TABS.map((t) => (
            <Tab key={t.id} value={t.id} label={t.label} />
          ))}
        </Tabs>
        <IconButton size="small" onClick={() => setSidePanel(null)} aria-label="Close panel">
          <CloseRoundedIcon fontSize="small" />
        </IconButton>
      </Stack>
      <Box sx={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        {panel === "snippets" && <SnippetsPanel pane={pane} />}
        {panel === "history" && <HistoryPanel />}
        {panel === "themes" && <ThemesPanel tab={tab} pane={pane} />}
        {panel === "info" && <InfoPanel pane={pane} tab={tab} />}
      </Box>
    </Box>
  );
}

/* ------------------------------------------------------------- snippets */

function SnippetsPanel({ pane }: { pane: Pane }) {
  const vault = useDefaultVault();
  const snippets = useSnippets(vault.data?.id ?? null);
  const [query, setQuery] = useState("");
  const [run, setRun] = useState<SnippetCard | null>(null);
  const snackbar = useSnackbar();
  const op = useMutation({
    mutationFn: async (job: () => Promise<string>) => job(),
    onSuccess: (msg) => snackbar.notify(msg),
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const q = query.trim().toLowerCase();
  const items = (snippets.data ?? []).filter(
    (s) => !q || s.label.toLowerCase().includes(q) || s.script.toLowerCase().includes(q),
  );
  const connected = pane.status === "connected";

  const execute = (snippet: SnippetCard, sessionIds: Uuid[], vars: Record<string, string>) =>
    op.mutate(async () => {
      const res = await ipc.snippetRun(snippet.id, sessionIds, vars);
      if (res.closeAfterRun) for (const sid of res.sessionIds) void closePane(sid);
      return `Sent to ${res.sessionIds.length} terminal(s)`;
    });

  return (
    <Stack sx={{ p: 1.5, gap: 1.5 }}>
      <SearchField
        value={query}
        onChange={setQuery}
        placeholder="Search snippets"
        width={SIDE_PANEL_WIDTH - 24}
      />
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
              disabled={!connected || op.isPending}
              onClick={() => (s.variables.length > 0 ? setRun(s) : execute(s, [pane.id], {}))}
              sx={{ borderRadius: 1.5, alignItems: "flex-start", gap: 1 }}
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
              <Tooltip
                title={s.variables.length ? "Fill variables and run" : "Run in this terminal"}
              >
                <PlayArrowRoundedIcon fontSize="small" sx={{ mt: 0.5, color: "text.secondary" }} />
              </Tooltip>
            </ListItemButton>
          ))}
        </List>
      )}
      {!connected && (
        <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
          Snippets run once the terminal is connected.
        </Typography>
      )}
      {run && (
        <RunDialog
          snippet={run}
          busy={op.isPending}
          onCancel={() => setRun(null)}
          onConfirm={(ids, vars) => {
            execute(run, ids, vars);
            setRun(null);
          }}
        />
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

function HistoryPanel() {
  const history = useHistory();
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  const items = (history.data ?? []).filter(
    (i) => !q || i.data.label.toLowerCase().includes(q) || i.data.target.toLowerCase().includes(q),
  );
  return (
    <Stack sx={{ p: 1.5, gap: 1.5 }}>
      <SearchField
        value={query}
        onChange={setQuery}
        placeholder="Search history"
        width={SIDE_PANEL_WIDTH - 24}
      />
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
    </Stack>
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

function ThemesPanel({ tab, pane }: { tab: TerminalTab; pane: Pane }) {
  const settings = useSettings();
  const [filter, setFilter] = useState<"all" | "dark" | "light">("all");
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
      <Stack direction="row" sx={{ alignItems: "center", justifyContent: "space-between", gap: 1 }}>
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
      <Stack spacing={0.75}>
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

  return (
    <Stack sx={{ p: 1.5, gap: 1.5 }}>
      <Section title="Session">
        {rows.map(([k, v]) => (
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
    <Box sx={{ bgcolor: "surface.high", borderRadius: 2, p: 1.5 }}>
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
