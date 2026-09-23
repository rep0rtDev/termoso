import { useMemo, useState, type ReactNode } from "react";
import {
  Button,
  Chip,
  IconButton,
  List,
  ListItemButton,
  ListItemText,
  MenuItem,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import DataObjectRoundedIcon from "@mui/icons-material/DataObjectRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { CommandHistory, HistoryItem, Uuid } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import * as ipc from "@/ipc/commands";
import { keys, useCommandHistory, useHistory, useHosts } from "@/ipc/hooks";
import { useActiveVault } from "@/app/vault";
import { scopedTo } from "./scope";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { useSnackbar } from "@/components/Snackbar";
import { Loading, SearchField, SidePanel } from "@/components/ui";
import { copyText, dropHistoryCache, openTerminal } from "@/terminal/store";

export type HostFilter = { kind: "all" } | { kind: "local" } | { kind: "host"; id: Uuid };

export const ALL_HOSTS: HostFilter = { kind: "all" };
export const LOCAL_ONLY: HostFilter = { kind: "local" };

export function relativeTime(iso: string): string {
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

export function duration(secs: number | null): string {
  if (secs === null) return "open";
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m ${secs % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

/** One row per distinct command (latest occurrence wins), newest first. */
export function dedupeCommands(items: readonly HistoryItem<CommandHistory>[]) {
  const byCommand = new Map<string, { latest: HistoryItem<CommandHistory>; ids: Uuid[] }>();
  for (const it of items) {
    const entry = byCommand.get(it.data.command);
    if (entry) entry.ids.push(it.id);
    else byCommand.set(it.data.command, { latest: it, ids: [it.id] });
  }
  return [...byCommand.values()];
}

const matchesHost = (hostId: Uuid | null, filter: HostFilter) =>
  filter.kind === "all" ? true : filter.kind === "local" ? hostId === null : hostId === filter.id;

/**
 * Commands recorded by shell integration, deduplicated, newest first. Hover a row
 * for Save (inline label, as in Termius), Copy and Delete; click runs it when `onRun` is set.
 */
export function CommandHistoryList({
  query,
  host,
  onRun,
  leading,
  hideClear = false,
  emptyHint,
}: {
  query: string;
  host: HostFilter;
  onRun?: (command: string) => void;
  leading?: ReactNode;
  hideClear?: boolean;
  emptyHint?: string;
}) {
  const history = useCommandHistory();
  const hosts = useHosts(null);
  const vault = useActiveVault();
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  const [confirmClear, setConfirmClear] = useState(false);
  const [saving, setSaving] = useState<{ id: Uuid; label: string } | null>(null);

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
      snackbar.notify("Shell history cleared");
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
      await qc.invalidateQueries({ queryKey: ["snippets"] });
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
  const rows = useMemo(
    () =>
      dedupeCommands(
        (history.data ?? []).filter(
          (i) =>
            matchesHost(i.data.host_id, host) && (!q || i.data.command.toLowerCase().includes(q)),
        ),
      ),
    [history.data, host, q],
  );

  const commitSave = (command: string) => {
    if (!saving) return;
    const typed = saving.label.trim();
    const label = typed.length > 0 ? typed : command.split(/\s+/).slice(0, 3).join(" ");
    save.mutate({ label, script: command });
  };

  return (
    <>
      {(leading !== undefined || !hideClear) && (
        <Stack
          direction="row"
          sx={{ alignItems: "center", justifyContent: "space-between", gap: 1 }}
        >
          {leading ?? <span />}
          {!hideClear && (
            <Button
              size="small"
              color="inherit"
              disabled={(history.data ?? []).length === 0}
              onClick={() => setConfirmClear(true)}
              sx={{ flexShrink: 0, whiteSpace: "nowrap" }}
            >
              Delete all
            </Button>
          )}
        </Stack>
      )}
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
            : (emptyHint ??
              (host.kind !== "all"
                ? "No commands recorded here yet."
                : "No commands yet. Bash, zsh and fish sessions record commands as you run them."))}
        </Typography>
      ) : (
        <List dense disablePadding>
          {rows.map(({ latest, ids }) => {
            const command = latest.data.command;
            const where = latest.data.host_id
              ? (hostLabel.get(latest.data.host_id) ?? "Removed host")
              : "Local";
            if (saving?.id === latest.id) {
              return (
                <Stack
                  key={latest.id}
                  component="form"
                  spacing={0.75}
                  onSubmit={(e) => {
                    e.preventDefault();
                    commitSave(command);
                  }}
                  sx={{ p: 1, borderRadius: 1.5, bgcolor: "surface.high" }}
                >
                  <Typography
                    variant="body2"
                    noWrap
                    sx={{ fontFamily: "monospace", fontSize: 12.5 }}
                  >
                    {command}
                  </Typography>
                  <Stack direction="row" spacing={0.75} sx={{ alignItems: "center" }}>
                    <TextField
                      autoFocus
                      placeholder="Set a label"
                      value={saving.label}
                      onChange={(e) => setSaving({ id: latest.id, label: e.target.value })}
                      onKeyDown={(e) => {
                        if (e.key === "Escape") setSaving(null);
                      }}
                      sx={{ flex: 1 }}
                    />
                    <Button
                      size="small"
                      type="submit"
                      variant="contained"
                      disabled={save.isPending}
                    >
                      Done
                    </Button>
                  </Stack>
                </Stack>
              );
            }
            return (
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
                  disabled={!onRun}
                  onClick={() => onRun?.(command)}
                  sx={{
                    borderRadius: 1.5,
                    gap: 1,
                    flex: 1,
                    minWidth: 0,
                    py: 0.5,
                    "&.Mui-disabled": { opacity: 1 },
                  }}
                >
                  <ListItemText
                    primary={command}
                    secondary={`${where} · ${relativeTime(latest.created_at)}${ids.length > 1 ? ` · ×${ids.length}` : ""}`}
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
                    <IconButton
                      size="small"
                      onClick={() => setSaving({ id: latest.id, label: "" })}
                    >
                      <DataObjectRoundedIcon sx={{ fontSize: 16 }} />
                    </IconButton>
                  </Tooltip>
                  <Tooltip title="Copy">
                    <IconButton
                      size="small"
                      onClick={() => {
                        void copyText(command);
                        snackbar.notify("Copied");
                      }}
                    >
                      <ContentCopyRoundedIcon sx={{ fontSize: 16 }} />
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
            );
          })}
        </List>
      )}
      <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
        Stored encrypted on this device; lines that look like they contain a password or token are
        never recorded.
      </Typography>
      <ConfirmDialog
        open={confirmClear}
        title="Delete all shell history?"
        confirmLabel="Delete all"
        danger
        busy={clear.isPending}
        onCancel={() => setConfirmClear(false)}
        onConfirm={() => clear.mutate()}
      >
        Removes every recorded command from this device (and from sync, if enabled).
      </ConfirmDialog>
    </>
  );
}

/** Recent connections: when, where, how long, and why it failed. Click reopens the host. */
export function ConnectionHistoryList({
  query,
  host = ALL_HOSTS,
  hideClear = false,
}: {
  query: string;
  host?: HostFilter;
  hideClear?: boolean;
}) {
  const history = useHistory();
  const vault = useActiveVault();
  const qc = useQueryClient();
  const snackbar = useSnackbar();
  const [confirmClear, setConfirmClear] = useState(false);
  const clear = useMutation({
    mutationFn: () => ipc.historyClearConnections(),
    onSuccess: async () => {
      setConfirmClear(false);
      await qc.invalidateQueries({ queryKey: keys.history });
      snackbar.notify("Connection history cleared");
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const q = query.trim().toLowerCase();
  const items = scopedTo(history.data ?? [], vault.data).filter(
    (i) =>
      matchesHost(i.data.host_id, host) &&
      (!q || i.data.label.toLowerCase().includes(q) || i.data.target.toLowerCase().includes(q)),
  );
  return (
    <>
      {!hideClear && (
        <Stack direction="row" sx={{ justifyContent: "flex-end" }}>
          <Button
            size="small"
            color="inherit"
            disabled={(history.data ?? []).length === 0}
            onClick={() => setConfirmClear(true)}
          >
            Delete all
          </Button>
        </Stack>
      )}
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
            <Tooltip
              key={item.id}
              title={item.data.error ?? ""}
              placement="left"
              disableHoverListener={!item.data.error}
            >
              <ListItemButton
                disabled={!item.data.host_id}
                onClick={() => {
                  if (item.data.host_id) openTerminal({ kind: "host", host_id: item.data.host_id });
                }}
                sx={{ borderRadius: 1.5, gap: 1, "&.Mui-disabled": { opacity: 1 } }}
              >
                {item.data.error ? (
                  <ErrorOutlineRoundedIcon fontSize="small" color="error" />
                ) : (
                  <TerminalRoundedIcon fontSize="small" sx={{ color: "text.secondary" }} />
                )}
                <ListItemText
                  primary={item.data.label}
                  secondary={`${item.data.target} · ${new Date(item.created_at).toLocaleString([], { dateStyle: "short", timeStyle: "short" })} · ${duration(item.data.duration_secs)}`}
                  slotProps={{
                    primary: { variant: "body2", noWrap: true, sx: { fontWeight: 600 } },
                  }}
                />
                <Chip
                  size="small"
                  variant="outlined"
                  color={item.data.error ? "error" : "default"}
                  label={item.data.error ? "Failed" : item.data.protocol.toUpperCase()}
                />
              </ListItemButton>
            </Tooltip>
          ))}
        </List>
      )}
      <ConfirmDialog
        open={confirmClear}
        title="Delete all connection history?"
        confirmLabel="Delete all"
        danger
        busy={clear.isPending}
        onCancel={() => setConfirmClear(false)}
        onConfirm={() => clear.mutate()}
      >
        Removes every recorded connection from this device (and from sync, if enabled).
      </ConfirmDialog>
    </>
  );
}

/**
 * Termius' Shell History: a right-side panel on the Snippets page listing every recorded
 * command, filterable by host, with Save (→ snippet) on hover and Delete all in the menu.
 */
export function ShellHistoryPanel({ onClose }: { onClose: () => void }) {
  const hosts = useHosts(null);
  const [mode, setMode] = useState<"commands" | "connections">("commands");
  const [query, setQuery] = useState("");
  const [host, setHost] = useState<HostFilter>(ALL_HOSTS);
  const hostValue = host.kind === "all" ? "all" : host.kind === "local" ? "local" : host.id;
  const pickHost = (v: string) =>
    setHost(v === "all" ? ALL_HOSTS : v === "local" ? LOCAL_ONLY : { kind: "host", id: v });

  return (
    <SidePanel title="Shell History" onClose={onClose}>
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
          width="100%"
        />
        <TextField select value={hostValue} onChange={(e) => pickHost(e.target.value)} fullWidth>
          <MenuItem value="all">All hosts</MenuItem>
          <MenuItem value="local">Local terminal</MenuItem>
          {(hosts.data ?? []).map((h) => (
            <MenuItem key={h.id} value={h.id}>
              {h.label}
            </MenuItem>
          ))}
        </TextField>
        {mode === "commands" ? (
          <CommandHistoryList
            query={query}
            host={host}
            leading={
              <Typography variant="caption" color="text.secondary">
                Hover a command to save it as a snippet
              </Typography>
            }
          />
        ) : (
          <ConnectionHistoryList query={query} host={host} />
        )}
      </Stack>
    </SidePanel>
  );
}
