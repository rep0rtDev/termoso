import { useMemo, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import {
  Box,
  Button,
  IconButton,
  InputAdornment,
  List,
  ListItemButton,
  ListItemText,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import BoltRoundedIcon from "@mui/icons-material/BoltRounded";
import HistoryRoundedIcon from "@mui/icons-material/HistoryRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import GridViewRoundedIcon from "@mui/icons-material/GridViewRounded";
import ViewListRoundedIcon from "@mui/icons-material/ViewListRounded";
import RestoreRoundedIcon from "@mui/icons-material/RestoreRounded";
import DriveFileRenameOutlineRoundedIcon from "@mui/icons-material/DriveFileRenameOutlineRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import AddBoxOutlinedIcon from "@mui/icons-material/AddBoxOutlined";
import type { HostCard, OpenTarget, VaultConnection, WorkspaceTemplate } from "@/ipc/types";
import { useHistory, useHosts } from "@/ipc/hooks";
import { scopedTo } from "@/history/scope";
import { useActiveVault } from "./vault";
import { openTerminal } from "@/terminal/store";
import {
  createTemplate,
  deleteTemplate,
  dismissPrevious,
  layoutFromTargets,
  layoutTargets,
  openTemplate,
  renameTemplate,
  restorePrevious,
  setEditingTemplate,
  snapshotConnections,
  useWorkspaces,
} from "@/terminal/workspaces";
import {
  ActionMenu,
  CheckTile,
  IconTile,
  InlineName,
  SectionCard,
  type MenuAction,
} from "@/components/ui";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import { HostAvatar } from "@/hosts/HostAvatar";
import { hostTarget } from "@/hosts/HostGrid";
import { openHost } from "@/hosts/open";
import { looksLikeTarget, parseQuickConnect, quickFromHistory, quickLabel } from "@/hosts/links";
import { relativeTime } from "@/hosts/HostList";
import { sizes } from "@/theme/theme";
import { goHome, requestCreate } from "./navigation";
import { tr, trn } from "@/i18n";

const RECENT_MAX = 8;

/** Latest attempt per host / target, newest first. */
function recentTargets(items: VaultConnection[]) {
  const seen = new Set<string>();
  const out: VaultConnection[] = [];
  for (const it of items) {
    const key = it.data.host_id ?? `target:${it.data.target}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(it);
    if (out.length >= RECENT_MAX) break;
  }
  return out;
}

function hostMatches(h: HostCard, q: string) {
  const s = q.toLowerCase();
  return (
    h.label.toLowerCase().includes(s) ||
    h.address.toLowerCase().includes(s) ||
    h.tags.some((t) => t.toLowerCase().includes(s))
  );
}

const connectionsN = (n: number) => trn(n, "{count} connection", "{count} connections");
const tabsN = (n: number) => trn(n, "{count} tab", "{count} tabs");

const rowText = {
  primary: { variant: "body2" as const, noWrap: true, sx: { fontWeight: 600 } },
  secondary: { noWrap: true },
};

/**
 * What the `+` after the session tabs opens (as in Termius): find a host or
 * type an address to connect, reopen what was open last time, launch a saved
 * workspace, jump back into recent connections (tick several to turn them into
 * a workspace), or start a local shell. Picking anything replaces this page.
 */
export function NewTabPage() {
  const vault = useActiveVault();
  const hosts = useHosts(vault.data?.id ?? null);
  const history = useHistory();
  const [query, setQuery] = useState("");

  const q = query.trim();
  const matches = useMemo(
    () => (q ? (hosts.data ?? []).filter((h) => hostMatches(h, q)).slice(0, 8) : []),
    [hosts.data, q],
  );
  const quick = q && looksLikeTarget(q) && matches.length !== 1 ? parseQuickConnect(q) : null;
  const recent = useMemo(
    () => recentTargets(scopedTo(history.data ?? [], vault.data)),
    [history.data, vault.data],
  );

  const connect = (h: HostCard) => openHost(h);
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      if (q) setQuery("");
      else goHome();
      return;
    }
    if (e.key !== "Enter") return;
    if (matches[0] && (matches.length === 1 || !quick)) connect(matches[0]);
    else if (quick) openTerminal(quick);
  };

  return (
    <Box
      sx={{
        flex: 1,
        minHeight: 0,
        overflowY: "auto",
        display: "flex",
        justifyContent: "center",
        px: 3,
        py: 6,
        bgcolor: "surface.base",
      }}
    >
      <Stack spacing={3} sx={{ width: 560, maxWidth: "100%", alignSelf: "flex-start" }}>
        <Box>
          <TextField
            autoFocus
            fullWidth
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKey}
            placeholder={tr("Search hosts or type user@host:port to connect")}
            slotProps={{
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchRoundedIcon fontSize="small" />
                  </InputAdornment>
                ),
              },
            }}
            sx={{ "& .MuiOutlinedInput-root": { height: sizes.input } }}
          />
          {(matches.length > 0 || quick) && (
            <SectionCard sx={{ mt: 1, p: 0.5 }}>
              <List dense disablePadding>
                {matches.map((h) => (
                  <ListItemButton
                    key={h.id}
                    onClick={() => connect(h)}
                    sx={{ borderRadius: 1.5, gap: 1.25 }}
                  >
                    <HostAvatar host={h} size={sizes.tileSmall} />
                    <ListItemText primary={h.label} secondary={hostTarget(h)} slotProps={rowText} />
                  </ListItemButton>
                ))}
                {quick && (
                  <ListItemButton
                    onClick={() => openTerminal(quick)}
                    sx={{ borderRadius: 1.5, gap: 1.25 }}
                  >
                    <IconTile size={sizes.tileSmall} tone="accent">
                      <BoltRoundedIcon />
                    </IconTile>
                    <ListItemText
                      primary={tr("Connect to {quickLabel}", { quickLabel: quickLabel(quick) })}
                      secondary={`Quick connect · ${quick.protocol === "telnet" ? "Telnet" : "SSH"} · not saved`}
                      slotProps={rowText}
                    />
                  </ListItemButton>
                )}
              </List>
            </SectionCard>
          )}
        </Box>

        <Stack direction="row" spacing={1}>
          <Button
            variant="tonal"
            startIcon={<TerminalRoundedIcon />}
            onClick={() => openTerminal({ kind: "local" })}
          >
            {tr("Local terminal")}
          </Button>
          <Button
            variant="text"
            color="inherit"
            startIcon={<DnsRoundedIcon />}
            onClick={() => requestCreate("host")}
          >
            {tr("New host")}
          </Button>
        </Stack>

        <PreviousSession />
        <WorkspaceTemplates />
        <RecentConnections recent={recent} hosts={hosts.data ?? []} pending={history.isPending} />
      </Stack>
    </Box>
  );
}

function SectionHeading({
  icon,
  children,
  action,
}: {
  icon: ReactNode;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <Box sx={{ display: "flex", alignItems: "center", mb: 0.5, minHeight: 28 }}>
      <Typography
        variant="overline"
        color="text.secondary"
        sx={{ display: "flex", alignItems: "center", gap: 0.75, flex: 1 }}
      >
        {icon}
        {children}
      </Typography>
      {action}
    </Box>
  );
}

/** Offer to reopen the tabs that were open when the app last closed. */
export function PreviousSession() {
  const previous = useWorkspaces((s) => s.previous);
  const snackbar = useSnackbar();
  if (!previous) return null;
  const connections = snapshotConnections(previous);
  const names = previous.tabs
    .map((t) => t.name ?? connectionsN(layoutTargets(t.layout).length))
    .slice(0, 3);
  return (
    <SectionCard sx={{ flexDirection: "row", alignItems: "center", gap: 1.5, p: 1.5 }}>
      <IconTile size={sizes.tile} tone="accent">
        <RestoreRoundedIcon />
      </IconTile>
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body2" sx={{ fontWeight: 600 }}>
          {tr("Previous session · {connections} in {tabs}", {
            connections: connectionsN(connections),
            tabs: tabsN(previous.tabs.length),
          })}
        </Typography>
        <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
          {names.join(" · ")}
          {previous.tabs.length > 3
            ? " · " + tr("+{value} more", { value: previous.tabs.length - 3 })
            : ""}{" "}
          · {tr("saved {when}", { when: relativeTime(previous.savedAt) })}
        </Typography>
      </Box>
      <Button variant="text" color="inherit" onClick={dismissPrevious}>
        {tr("Dismiss")}
      </Button>
      <Button
        variant="contained"
        onClick={() => {
          const n = restorePrevious();
          snackbar.notify(tr("Restoring {connections}", { connections: connectionsN(n) }));
        }}
      >
        {tr("Restore")}
      </Button>
    </SectionCard>
  );
}

function WorkspaceTemplates() {
  const templates = useWorkspaces((s) => s.templates);
  const editingId = useWorkspaces((s) => s.editingId);
  const [menu, setMenu] = useState<{ tpl: WorkspaceTemplate; left: number; top: number } | null>(
    null,
  );
  const [confirm, setConfirm] = useState<WorkspaceTemplate | null>(null);
  if (templates.length === 0) return null;

  const menuItems = (tpl: WorkspaceTemplate): MenuAction[] => [
    {
      label: tr("Open"),
      icon: <GridViewRoundedIcon fontSize="small" />,
      onClick: () => openTemplate(tpl.id),
    },
    {
      label: tr("Rename"),
      icon: <DriveFileRenameOutlineRoundedIcon fontSize="small" />,
      onClick: () => setEditingTemplate(tpl.id),
    },
    {
      label: tr("Delete"),
      icon: <DeleteOutlineRoundedIcon fontSize="small" />,
      danger: true,
      onClick: () => setConfirm(tpl),
    },
  ];

  return (
    <Box>
      <SectionHeading icon={<GridViewRoundedIcon sx={{ fontSize: 16 }} />}>
        {tr("Workspace templates")}
      </SectionHeading>
      <SectionCard sx={{ p: 0.5 }}>
        <List dense disablePadding>
          {templates.map((tpl) => {
            const targets = layoutTargets(tpl.layout);
            const editing = editingId === tpl.id;
            const onContext = (e: MouseEvent<HTMLElement>) => {
              e.preventDefault();
              setMenu({ tpl, left: e.clientX, top: e.clientY });
            };
            return (
              <ListItemButton
                key={tpl.id}
                onClick={() => {
                  if (!editing) openTemplate(tpl.id);
                }}
                onContextMenu={onContext}
                sx={{
                  borderRadius: 1.5,
                  gap: 1.25,
                  "&:hover .tpl-actions, &:focus-within .tpl-actions": { opacity: 1 },
                }}
              >
                <IconTile size={sizes.tileSmall}>
                  {tpl.viewMode === "list" ? <ViewListRoundedIcon /> : <GridViewRoundedIcon />}
                </IconTile>
                {editing ? (
                  <Box sx={{ flex: 1, minWidth: 0, py: 0.5 }}>
                    <InlineName
                      value={tpl.name}
                      placeholder={tr("Workspace name")}
                      onCommit={(name) => renameTemplate(tpl.id, name)}
                      onCancel={() => setEditingTemplate(null)}
                      sx={{ width: "100%" }}
                    />
                  </Box>
                ) : (
                  <ListItemText
                    primary={tpl.name}
                    secondary={`${connectionsN(targets.length)} · ${
                      tpl.viewMode === "list" ? "list" : "side by side"
                    }`}
                    slotProps={rowText}
                  />
                )}
                <Stack
                  direction="row"
                  className="tpl-actions"
                  sx={{
                    opacity: 0,
                    transition: "opacity 100ms",
                    display: editing ? "none" : "flex",
                  }}
                >
                  <Tooltip title={tr("Rename")}>
                    <IconButton
                      size="small"
                      aria-label={tr("Rename {name}", { name: tpl.name })}
                      onClick={(e) => {
                        e.stopPropagation();
                        setEditingTemplate(tpl.id);
                      }}
                    >
                      <DriveFileRenameOutlineRoundedIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                  <Tooltip title={tr("Delete")}>
                    <IconButton
                      size="small"
                      aria-label={tr("Delete {name}", { name: tpl.name })}
                      onClick={(e) => {
                        e.stopPropagation();
                        setConfirm(tpl);
                      }}
                    >
                      <DeleteOutlineRoundedIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                </Stack>
              </ListItemButton>
            );
          })}
        </List>
      </SectionCard>
      <ActionMenu
        anchor={null}
        position={menu ? { left: menu.left, top: menu.top } : null}
        onClose={() => setMenu(null)}
        items={menu ? menuItems(menu.tpl) : []}
      />
      <ConfirmDialog
        open={confirm !== null}
        title={tr("Delete workspace template?")}
        confirmLabel={tr("Delete")}
        danger
        onCancel={() => setConfirm(null)}
        onConfirm={() => {
          if (confirm) deleteTemplate(confirm.id);
          setConfirm(null);
        }}
      >
        {confirm
          ? tr("“{name}” will be removed from this list. Open tabs are not affected.", {
              name: confirm.name,
            })
          : ""}
      </ConfirmDialog>
    </Box>
  );
}

function RecentConnections({
  recent,
  hosts,
  pending,
}: {
  recent: VaultConnection[];
  hosts: HostCard[];
  pending: boolean;
}) {
  const [checked, setChecked] = useState<Set<string>>(() => new Set());
  const rows = useMemo(
    () =>
      recent.map((it) => {
        const host = it.data.host_id ? hosts.find((h) => h.id === it.data.host_id) : undefined;
        const target: OpenTarget | null = host
          ? { kind: "host", host_id: host.id, vault_id: host.vaultId }
          : it.data.protocol === "local"
            ? { kind: "local" }
            : quickFromHistory(it.data.target, it.data.protocol);
        return { it, host, target };
      }),
    [recent, hosts],
  );
  const selected = rows.filter((r) => checked.has(r.it.id) && r.target);
  const selectedTargets = selected.flatMap((r) => (r.target ? [r.target] : []));
  const anyChecked = selected.length > 0;

  const toggle = (id: string) =>
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const createWorkspace = () => {
    const layout = layoutFromTargets(selectedTargets);
    if (!layout) return;
    createTemplate(layout);
    setChecked(new Set());
  };
  const restoreSelected = () => {
    selectedTargets.forEach((t, i) => openTerminal(t, { background: i > 0 }));
    setChecked(new Set());
  };

  return (
    <Box>
      <SectionHeading
        icon={<HistoryRoundedIcon sx={{ fontSize: 16 }} />}
        action={
          anyChecked ? (
            <Stack direction="row" spacing={0.5}>
              <Button
                size="small"
                variant="text"
                color="inherit"
                startIcon={<AddBoxOutlinedIcon />}
                onClick={createWorkspace}
              >
                {tr("Create a workspace")}
              </Button>
              <Button
                size="small"
                variant="tonal"
                startIcon={<RestoreRoundedIcon />}
                onClick={restoreSelected}
              >
                {tr("Restore")} {selected.length > 1 ? `(${selected.length})` : ""}
              </Button>
            </Stack>
          ) : null
        }
      >
        {tr("Recent connections")}
      </SectionHeading>
      {rows.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          {pending ? tr("Loading…") : tr("Hosts you connect to will show up here.")}
        </Typography>
      ) : (
        <SectionCard sx={{ p: 0.5 }}>
          <List dense disablePadding>
            {rows.map(({ it, host, target }) => {
              const isChecked = checked.has(it.id);
              return (
                <ListItemButton
                  key={it.id}
                  disabled={!target}
                  selected={isChecked}
                  onClick={() => {
                    if (anyChecked) toggle(it.id);
                    else if (target) openTerminal(target);
                  }}
                  sx={{ borderRadius: 1.5, gap: 1.25 }}
                >
                  <Box
                    role="checkbox"
                    aria-checked={isChecked}
                    aria-label={tr("Select {value}", { value: host?.label ?? it.data.label })}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (target) toggle(it.id);
                    }}
                    sx={{ display: "flex", flexShrink: 0 }}
                  >
                    <CheckTile
                      size={sizes.tileSmall}
                      checked={isChecked}
                      hoverHint={!!target}
                      tile={
                        host ? (
                          <HostAvatar host={host} size={sizes.tileSmall} />
                        ) : (
                          <IconTile
                            size={sizes.tileSmall}
                            tone={it.data.error ? "danger" : "neutral"}
                          >
                            {it.data.error ? (
                              <ErrorOutlineRoundedIcon />
                            ) : it.data.protocol === "local" ? (
                              <TerminalRoundedIcon />
                            ) : (
                              <DnsRoundedIcon />
                            )}
                          </IconTile>
                        )
                      }
                    />
                  </Box>
                  <ListItemText
                    primary={host?.label ?? it.data.label}
                    secondary={`${it.data.target} · ${relativeTime(it.created_at)}`}
                    slotProps={rowText}
                  />
                </ListItemButton>
              );
            })}
          </List>
        </SectionCard>
      )}
    </Box>
  );
}
