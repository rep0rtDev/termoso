import { useEffect, useMemo, useState, type MouseEvent } from "react";
import {
  Box,
  Breadcrumbs,
  Button,
  Chip,
  IconButton,
  InputAdornment,
  Link,
  Menu,
  MenuItem,
  Popover,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import GridViewRoundedIcon from "@mui/icons-material/GridViewRounded";
import ViewListRoundedIcon from "@mui/icons-material/ViewListRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import CreateNewFolderRoundedIcon from "@mui/icons-material/CreateNewFolderRounded";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import SellOutlinedIcon from "@mui/icons-material/SellOutlined";
import SwapVertRoundedIcon from "@mui/icons-material/SwapVertRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import FolderOpenRoundedIcon from "@mui/icons-material/FolderOpenRounded";
import EditOutlinedIcon from "@mui/icons-material/EditOutlined";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import TabRoundedIcon from "@mui/icons-material/TabRounded";
import DriveFileMoveOutlinedIcon from "@mui/icons-material/DriveFileMoveOutlined";
import LibraryAddOutlinedIcon from "@mui/icons-material/LibraryAddOutlined";
import LinkRoundedIcon from "@mui/icons-material/LinkRounded";
import GroupAddOutlinedIcon from "@mui/icons-material/GroupAddOutlined";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import SelectAllRoundedIcon from "@mui/icons-material/SelectAllRounded";
import { EmptyState } from "@/components/EmptyState";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import {
  ActionMenu,
  Loading,
  SplitButton,
  ToolIconButton,
  Toolbar,
  type MenuAction,
} from "@/components/ui";
import {
  useDefaultVault,
  useDeleteHosts,
  useDuplicateGroup,
  useDuplicateHost,
  useGroups,
  useHosts,
  useSaveSettings,
  useSettings,
  useTags,
} from "@/ipc/hooks";
import type { GroupNode, HostCard, HostsView, Uuid } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { openTerminal } from "@/terminal/store";
import { openSftpForHost } from "@/sftp/store";
import { goToSftp } from "@/app/navigation";
import { HostGrid } from "./HostGrid";
import { HostList } from "./HostList";
import { HostEditPanel } from "./HostEditPanel";
import { DeleteGroupDialog, GroupPanel } from "./GroupPanel";
import { MoveCopyDialog, type MoveCopyRequest } from "./MoveCopyDialog";

type Panel =
  | { mode: "closed" }
  | { mode: "new"; groupId: Uuid | null }
  | { mode: "edit"; id: Uuid }
  | { mode: "group"; id: Uuid | null; parentId: Uuid | null };

type SortKey = "manual" | "label" | "address" | "updated" | "lastConnected";

const comparators: Record<SortKey, (a: HostCard, b: HostCard) => number> = {
  manual: (a, b) => a.sortOrder - b.sortOrder || a.label.localeCompare(b.label),
  label: (a, b) => a.label.localeCompare(b.label),
  address: (a, b) => a.address.localeCompare(b.address),
  updated: (a, b) => b.updatedAt.localeCompare(a.updatedAt),
  lastConnected: (a, b) => {
    if (a.lastConnected && b.lastConnected) {
      return b.lastConnected.localeCompare(a.lastConnected) || a.label.localeCompare(b.label);
    }
    if (a.lastConnected) return -1;
    if (b.lastConnected) return 1;
    return a.label.localeCompare(b.label);
  },
};

const sortLabel: Record<SortKey, string> = {
  manual: "Manual",
  label: "Name",
  address: "Address",
  updated: "Recently edited",
  lastConnected: "Recently connected",
};

type Ctx =
  | { kind: "host"; host: HostCard; left: number; top: number }
  | { kind: "group"; group: GroupNode; left: number; top: number };

/** `user@host:port` → quick-connect target; bare words are treated as hostnames. */
export function parseQuickConnect(input: string) {
  const s = input.trim();
  if (!s) return null;
  const m = /^(?:(?<user>[^@\s]+)@)?(?<host>\[[^\]]+\]|[^:\s]+)(?::(?<port>\d{1,5}))?$/.exec(s);
  const groups = m?.groups;
  const host = groups?.host?.replace(/^\[|\]$/g, "");
  if (!groups || !host) return null;
  const port = groups.port ? Number(groups.port) : null;
  if (port !== null && (port < 1 || port > 65535)) return null;
  return { kind: "quick" as const, address: host, username: groups.user ?? null, port };
}

/** A search string that looks like something you'd connect to rather than filter by. */
function looksLikeTarget(s: string) {
  return /[@:.]/.test(s) || /^\d+$/.test(s) || s === "localhost";
}

/** Deep link that other Termoso clients can open (`termoso://host/<id>`). */
export const hostLink = (h: HostCard) => `termoso://host/${h.id}`;

const isEditable = (t: EventTarget | null) =>
  t instanceof HTMLElement &&
  (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);

export function HostsPage() {
  const snackbar = useSnackbar();
  const vault = useDefaultVault();
  const vaultId = vault.data?.id ?? null;
  const hosts = useHosts(vaultId);
  const groups = useGroups(vaultId);
  const tags = useTags(vaultId);
  const settings = useSettings();
  const saveSettings = useSaveSettings();
  const deleteHosts = useDeleteHosts();
  const duplicateHost = useDuplicateHost();
  const duplicateGroup = useDuplicateGroup();

  const [groupId, setGroupId] = useState<Uuid | null>(null);
  const [search, setSearch] = useState("");
  const [tagFilter, setTagFilter] = useState<string[]>([]);
  const [sort, setSort] = useState<SortKey>("manual");
  const [panel, setPanel] = useState<Panel>({ mode: "closed" });
  const [tagAnchor, setTagAnchor] = useState<HTMLElement | null>(null);
  const [sortAnchor, setSortAnchor] = useState<HTMLElement | null>(null);
  const [ctx, setCtx] = useState<Ctx | null>(null);
  const [checked, setChecked] = useState<ReadonlySet<Uuid>>(() => new Set());
  const [anchorId, setAnchorId] = useState<Uuid | null>(null);
  const [moveCopy, setMoveCopy] = useState<MoveCopyRequest | null>(null);
  const [confirmRemove, setConfirmRemove] = useState<HostCard[] | null>(null);
  const [confirmGroup, setConfirmGroup] = useState<GroupNode | null>(null);

  const view: HostsView = settings.data?.hostsView ?? "grid";
  const setView = (v: HostsView | null) => {
    if (!v || !settings.data) return;
    saveSettings.mutate(
      { ...settings.data, hostsView: v },
      { onError: (e) => snackbar.error(errorMessage(e)) },
    );
  };

  const groupById = useMemo(
    () => new Map((groups.data ?? []).map((g) => [g.id, g])),
    [groups.data],
  );
  const crumbs = useMemo(() => {
    const out: GroupNode[] = [];
    let cur = groupId ? groupById.get(groupId) : undefined;
    while (cur) {
      out.unshift(cur);
      cur = cur.parentId ? groupById.get(cur.parentId) : undefined;
    }
    return out;
  }, [groupId, groupById]);

  const q = search.trim().toLowerCase();
  const filtering = q.length > 0 || tagFilter.length > 0;
  const childGroups = useMemo(
    () =>
      filtering
        ? []
        : (groups.data ?? [])
            .filter((g) => g.parentId === groupId)
            .sort((a, b) => a.sortOrder - b.sortOrder || a.label.localeCompare(b.label)),
    [groups.data, groupId, filtering],
  );
  const visibleHosts = useMemo(() => {
    const all = hosts.data ?? [];
    let scoped = filtering ? all : all.filter((h) => h.groupId === groupId);
    if (q) {
      scoped = scoped.filter((h) =>
        [h.label, h.address, h.username, ...h.tags, ...h.groupPath].some((s) =>
          s.toLowerCase().includes(q),
        ),
      );
    }
    if (tagFilter.length) scoped = scoped.filter((h) => tagFilter.every((t) => h.tags.includes(t)));
    return [...scoped].sort(comparators[sort]);
  }, [hosts.data, groupId, q, tagFilter, filtering, sort]);

  /* ------------------------------------------------------------ selection */

  // Only hosts currently on screen count as selected, so bulk actions never hit stale ids.
  const selectedHosts = useMemo(
    () => visibleHosts.filter((h) => checked.has(h.id)),
    [visibleHosts, checked],
  );
  const visibleChecked = useMemo(
    () => new Set(selectedHosts.map((h) => h.id)) as ReadonlySet<Uuid>,
    [selectedHosts],
  );

  const clearSelection = () => setChecked(new Set());
  const toggleHost = (h: HostCard) => {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(h.id)) next.delete(h.id);
      else next.add(h.id);
      return next;
    });
    setAnchorId(h.id);
  };
  const selectRange = (h: HostCard) => {
    const ids = visibleHosts.map((x) => x.id);
    const from = anchorId ? ids.indexOf(anchorId) : -1;
    const to = ids.indexOf(h.id);
    if (from < 0 || to < 0) return toggleHost(h);
    const [a, b] = from < to ? [from, to] : [to, from];
    setChecked((prev) => new Set([...prev, ...ids.slice(a, b + 1)]));
  };
  const selectAll = () => {
    setChecked(new Set(visibleHosts.map((h) => h.id)));
  };
  const updateSearch = (s: string) => {
    setSearch(s);
    if (visibleChecked.size > 0) clearSelection();
  };
  const updateTagFilter = (fn: (f: string[]) => string[]) => {
    setTagFilter(fn);
    if (visibleChecked.size > 0) clearSelection();
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isEditable(e.target)) return;
      if (e.key === "Escape" && visibleChecked.size > 0) {
        clearSelection();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a" && visibleHosts.length) {
        e.preventDefault();
        setChecked(new Set(visibleHosts.map((h) => h.id)));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [visibleChecked.size, visibleHosts]);

  /* -------------------------------------------------------------- actions */

  const openHost = (h: HostCard, e?: MouseEvent<HTMLElement>) => {
    if (e && (e.ctrlKey || e.metaKey)) return toggleHost(h);
    if (e?.shiftKey) return selectRange(h);
    if (visibleChecked.size > 0) return toggleHost(h);
    setPanel({ mode: "edit", id: h.id });
  };
  const connectHosts = (list: HostCard[], background = false) => {
    list.forEach((h, i) =>
      openTerminal({ kind: "host", host_id: h.id }, { background: background || i > 0 }),
    );
  };
  const connectHost = (h: HostCard) => connectHosts([h]);
  const sftpHost = (h: HostCard) => {
    openSftpForHost(h.id, h.label);
    goToSftp();
  };
  const quickTarget = looksLikeTarget(search.trim()) ? parseQuickConnect(search) : null;
  const onSearchEnter = () => {
    if (visibleHosts.length === 1 && visibleHosts[0]) {
      connectHost(visibleHosts[0]);
      setSearch("");
      return;
    }
    if (quickTarget) {
      openTerminal(quickTarget);
      setSearch("");
    }
  };
  const removeHosts = (list: HostCard[]) => {
    if (!vaultId || list.length === 0) return;
    deleteHosts.mutate(
      { ids: list.map((h) => h.id), vaultId },
      {
        onSuccess: () => {
          snackbar.notify(
            list.length === 1
              ? `Removed ${list[0]?.label ?? "host"}`
              : `Removed ${list.length} hosts`,
            "info",
          );
          if (panel.mode === "edit" && list.some((h) => h.id === panel.id)) {
            setPanel({ mode: "closed" });
          }
          clearSelection();
          setConfirmRemove(null);
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );
  };
  const duplicateHosts = async (list: HostCard[]) => {
    try {
      for (const h of list) await duplicateHost.mutateAsync(h.id);
      snackbar.notify(
        list.length === 1
          ? `Duplicated ${list[0]?.label ?? "host"}`
          : `Duplicated ${list.length} hosts`,
      );
      clearSelection();
    } catch (e) {
      snackbar.error(errorMessage(e));
    }
  };
  const copyText = (text: string, what: string) =>
    navigator.clipboard
      .writeText(text)
      .then(() => snackbar.notify(`${what} copied`))
      .catch(() => snackbar.error("Clipboard is not available"));
  const copyLinks = (list: HostCard[]) =>
    copyText(list.map(hostLink).join("\n"), list.length === 1 ? "Link" : "Links");

  const onHostContext = (h: HostCard, e: MouseEvent<HTMLElement>) => {
    e.preventDefault();
    setCtx({ kind: "host", host: h, left: e.clientX, top: e.clientY });
  };
  const onGroupContext = (g: GroupNode, e: MouseEvent<HTMLElement>) => {
    e.preventDefault();
    setCtx({ kind: "group", group: g, left: e.clientX, top: e.clientY });
  };
  const openGroupPanel = (g: GroupNode) =>
    setPanel({ mode: "group", id: g.id, parentId: g.parentId });
  const onDuplicateGroup = (g: GroupNode) =>
    duplicateGroup.mutate(g.id, {
      onSuccess: (copy) => snackbar.notify(`Duplicated as “${copy.label}”`),
      onError: (e) => snackbar.error(errorMessage(e)),
    });

  /** Context-menu targets: the whole selection when the clicked host is part of it. */
  const ctxTargets = (h: HostCard) =>
    visibleChecked.has(h.id) && visibleChecked.size > 1 ? selectedHosts : [h];

  const hostMenu = (h: HostCard): MenuAction[] => {
    const targets = ctxTargets(h);
    const many = targets.length > 1;
    const n = targets.length;
    return [
      {
        label: many ? `Connect ${n} hosts` : "Connect",
        icon: <PlayArrowRoundedIcon fontSize="small" />,
        onClick: () => connectHosts(targets),
      },
      {
        label: "Add to workspace",
        icon: <TabRoundedIcon fontSize="small" />,
        onClick: () => {
          connectHosts(targets, true);
          snackbar.notify(
            many ? `${n} tabs opened in the background` : "Tab opened in the background",
          );
        },
      },
      {
        label: "Open SFTP",
        icon: <FolderCopyRoundedIcon fontSize="small" />,
        onClick: () => sftpHost(h),
        disabled: many || h.protocol !== "ssh",
        divider: true,
      },
      {
        label: "Edit",
        icon: <EditOutlinedIcon fontSize="small" />,
        onClick: () => setPanel({ mode: "edit", id: h.id }),
        disabled: many,
      },
      {
        label: "Collaborate",
        icon: <GroupAddOutlinedIcon fontSize="small" />,
        onClick: () =>
          snackbar.notify("Sharing needs a team vault — sign in under Settings → Account", "info"),
        divider: true,
      },
      {
        label: "Move to…",
        icon: <DriveFileMoveOutlinedIcon fontSize="small" />,
        onClick: () => setMoveCopy({ kind: "group", hosts: targets }),
      },
      {
        label: "Copy to vault…",
        icon: <LibraryAddOutlinedIcon fontSize="small" />,
        onClick: () => setMoveCopy({ kind: "vault", hosts: targets, move: false }),
      },
      {
        label: "Duplicate",
        icon: <ContentCopyRoundedIcon fontSize="small" />,
        onClick: () => void duplicateHosts(targets),
        divider: true,
      },
      {
        label: many ? "Copy links" : "Copy link",
        icon: <LinkRoundedIcon fontSize="small" />,
        onClick: () => void copyLinks(targets),
      },
      {
        label: "Copy address",
        icon: <ContentCopyRoundedIcon fontSize="small" />,
        onClick: () => void copyText(targets.map((t) => t.address).join("\n"), "Address"),
        divider: true,
      },
      {
        label: many ? `Remove ${n} hosts` : "Remove",
        icon: <DeleteOutlineRoundedIcon fontSize="small" />,
        onClick: () => setConfirmRemove(targets),
        danger: true,
      },
    ];
  };

  const groupMenu = (g: GroupNode): MenuAction[] => [
    {
      label: "Open",
      icon: <FolderOpenRoundedIcon fontSize="small" />,
      onClick: () => setGroupId(g.id),
    },
    {
      label: "Group details",
      icon: <EditOutlinedIcon fontSize="small" />,
      onClick: () => openGroupPanel(g),
      divider: true,
    },
    {
      label: "New host here",
      icon: <DnsRoundedIcon fontSize="small" />,
      onClick: () => setPanel({ mode: "new", groupId: g.id }),
    },
    {
      label: "New sub-group",
      icon: <CreateNewFolderRoundedIcon fontSize="small" />,
      onClick: () => setPanel({ mode: "group", id: null, parentId: g.id }),
      divider: true,
    },
    {
      label: "Duplicate",
      icon: <ContentCopyRoundedIcon fontSize="small" />,
      onClick: () => onDuplicateGroup(g),
      divider: true,
    },
    {
      label: "Remove",
      icon: <DeleteOutlineRoundedIcon fontSize="small" />,
      onClick: () => setConfirmGroup(g),
      danger: true,
    },
  ];

  const selectedId = panel.mode === "edit" ? panel.id : null;
  const loading = vault.isPending || hosts.isPending || groups.isPending;
  const loadError = vault.error ?? hosts.error ?? groups.error;
  const collection = {
    groups: childGroups,
    hosts: visibleHosts,
    selectedId,
    checked: visibleChecked,
    showPath: filtering,
    onOpenGroup: (id: Uuid) => {
      clearSelection();
      setGroupId(id);
    },
    onEditGroup: openGroupPanel,
    onGroupContext,
    onOpenHost: openHost,
    onToggleHost: toggleHost,
    onConnectHost: connectHost,
    onHostContext,
  };

  const selecting = visibleChecked.size > 0;

  return (
    <Box sx={{ display: "flex", flex: 1, minHeight: 0 }}>
      <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <Toolbar
          trailing={
            <>
              <Button
                variant="text"
                size="small"
                startIcon={<SwapVertRoundedIcon />}
                onClick={(e) => setSortAnchor(e.currentTarget)}
                sx={{ color: "text.secondary" }}
              >
                {sortLabel[sort]}
              </Button>
              <ToolIconButton
                title="Filter by tag"
                active={tagFilter.length > 0}
                onClick={(e) => setTagAnchor(e.currentTarget)}
              >
                <SellOutlinedIcon fontSize="small" />
              </ToolIconButton>
              <ToggleButtonGroup
                exclusive
                value={view}
                onChange={(_e, v: HostsView | null) => setView(v)}
              >
                <ToggleButton value="grid" aria-label="Grid view">
                  <GridViewRoundedIcon sx={{ fontSize: 18 }} />
                </ToggleButton>
                <ToggleButton value="list" aria-label="List view">
                  <ViewListRoundedIcon sx={{ fontSize: 18 }} />
                </ToggleButton>
              </ToggleButtonGroup>
            </>
          }
        >
          <SplitButton
            label="New host"
            icon={<AddRoundedIcon />}
            disabled={!vaultId}
            onClick={() => setPanel({ mode: "new", groupId })}
            items={[
              {
                label: "New host",
                icon: <DnsRoundedIcon fontSize="small" />,
                onClick: () => setPanel({ mode: "new", groupId }),
              },
              {
                label: "New group",
                icon: <CreateNewFolderRoundedIcon fontSize="small" />,
                onClick: () => setPanel({ mode: "group", id: null, parentId: groupId }),
              },
            ]}
          />
          <Button
            variant="tonal"
            startIcon={<TerminalRoundedIcon />}
            onClick={() => openTerminal({ kind: "local" })}
          >
            Terminal
          </Button>
        </Toolbar>

        <Box sx={{ px: 3, pt: 2, pb: 1 }}>
          <TextField
            placeholder="Search hosts, or type user@host:port and press Enter to connect"
            value={search}
            onChange={(e) => updateSearch(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onSearchEnter();
              if (e.key === "Escape") updateSearch("");
            }}
            slotProps={{
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchRoundedIcon fontSize="small" />
                  </InputAdornment>
                ),
                endAdornment:
                  quickTarget && visibleHosts.length !== 1 ? (
                    <InputAdornment position="end">
                      <Chip
                        size="small"
                        icon={<PlayArrowRoundedIcon />}
                        label={`Connect to ${quickTarget.address}`}
                        onClick={onSearchEnter}
                        color="primary"
                        sx={{ "& .MuiChip-icon": { fontSize: 16 } }}
                      />
                    </InputAdornment>
                  ) : null,
              },
            }}
          />
          {selecting ? (
            <Box
              sx={{
                display: "flex",
                alignItems: "center",
                mt: 1.5,
                minHeight: 28,
                gap: 0.5,
                pl: 1,
                pr: 0.5,
                py: 0.25,
                borderRadius: 1.5,
                bgcolor: "surface.high",
              }}
            >
              <Typography variant="body2" sx={{ fontWeight: 600, mr: 1 }}>
                {visibleChecked.size} selected
              </Typography>
              <Button
                size="small"
                variant="text"
                color="inherit"
                startIcon={<PlayArrowRoundedIcon />}
                onClick={() => connectHosts(selectedHosts)}
              >
                Connect
              </Button>
              <Button
                size="small"
                variant="text"
                color="inherit"
                startIcon={<DriveFileMoveOutlinedIcon />}
                onClick={() => setMoveCopy({ kind: "group", hosts: selectedHosts })}
              >
                Move to
              </Button>
              <Button
                size="small"
                variant="text"
                color="inherit"
                startIcon={<LibraryAddOutlinedIcon />}
                onClick={() => setMoveCopy({ kind: "vault", hosts: selectedHosts, move: false })}
              >
                Copy to vault
              </Button>
              <Button
                size="small"
                variant="text"
                color="inherit"
                startIcon={<ContentCopyRoundedIcon />}
                onClick={() => void duplicateHosts(selectedHosts)}
              >
                Duplicate
              </Button>
              <Button
                size="small"
                variant="text"
                color="error"
                startIcon={<DeleteOutlineRoundedIcon />}
                onClick={() => setConfirmRemove(selectedHosts)}
              >
                Remove
              </Button>
              <Box sx={{ flex: 1 }} />
              <Tooltip title="Select all (Ctrl+A)">
                <IconButton
                  size="small"
                  aria-label="Select all"
                  onClick={selectAll}
                  disabled={visibleChecked.size === visibleHosts.length}
                >
                  <SelectAllRoundedIcon fontSize="small" />
                </IconButton>
              </Tooltip>
              <Tooltip title="Clear selection (Esc)">
                <IconButton size="small" aria-label="Clear selection" onClick={clearSelection}>
                  <CloseRoundedIcon fontSize="small" />
                </IconButton>
              </Tooltip>
            </Box>
          ) : (
            <Box sx={{ display: "flex", alignItems: "center", mt: 1.5, minHeight: 28, gap: 1 }}>
              {filtering ? (
                <Typography variant="body2" color="text.secondary">
                  {visibleHosts.length} result{visibleHosts.length === 1 ? "" : "s"}
                </Typography>
              ) : (
                <Breadcrumbs>
                  <Link
                    component="button"
                    underline={groupId ? "hover" : "none"}
                    color={groupId ? "text.secondary" : "text.primary"}
                    onClick={() => setGroupId(null)}
                    sx={{ fontWeight: 600, fontSize: 14 }}
                  >
                    All hosts
                  </Link>
                  {crumbs.map((g, i) => {
                    const last = i === crumbs.length - 1;
                    return (
                      <Link
                        key={g.id}
                        component="button"
                        underline={last ? "none" : "hover"}
                        color={last ? "text.primary" : "text.secondary"}
                        onClick={() => (last ? openGroupPanel(g) : setGroupId(g.id))}
                        onContextMenu={(e) => onGroupContext(g, e)}
                        sx={{ fontWeight: 600, fontSize: 14 }}
                      >
                        {g.label}
                      </Link>
                    );
                  })}
                </Breadcrumbs>
              )}
              {!filtering && crumbs.length > 0 && (
                <Tooltip title="Group details">
                  <IconButton
                    size="small"
                    aria-label="Group details"
                    onClick={() => {
                      const g = crumbs[crumbs.length - 1];
                      if (g) openGroupPanel(g);
                    }}
                    sx={{ ml: -0.5 }}
                  >
                    <EditOutlinedIcon sx={{ fontSize: 16 }} />
                  </IconButton>
                </Tooltip>
              )}
              {tagFilter.map((t) => (
                <Chip
                  key={t}
                  size="small"
                  label={t}
                  onDelete={() => updateTagFilter((f) => f.filter((x) => x !== t))}
                />
              ))}
              {tagFilter.length > 0 && (
                <Button size="small" variant="text" onClick={() => updateTagFilter(() => [])}>
                  Clear
                </Button>
              )}
            </Box>
          )}
        </Box>

        <Box sx={{ flex: 1, minHeight: 0, overflowY: "auto", px: 3, pb: 3 }}>
          {loading ? (
            <Loading />
          ) : loadError ? (
            <EmptyState title="Could not open the vault" description={errorMessage(loadError)} />
          ) : childGroups.length === 0 && visibleHosts.length === 0 ? (
            <EmptyState
              icon={<DnsRoundedIcon />}
              title={
                filtering ? "Nothing matches" : groupId ? "This group is empty" : "No hosts yet"
              }
              description={
                filtering
                  ? quickTarget
                    ? `Press Enter to connect to ${quickTarget.address}.`
                    : "Try a different label, address or tag."
                  : "Add your first server — everything is stored encrypted on this device."
              }
              action={
                filtering ? undefined : (
                  <Button
                    variant="contained"
                    startIcon={<AddRoundedIcon />}
                    onClick={() => setPanel({ mode: "new", groupId })}
                  >
                    New host
                  </Button>
                )
              }
            />
          ) : view === "grid" ? (
            <HostGrid {...collection} />
          ) : (
            <HostList {...collection} />
          )}
        </Box>
      </Box>

      {(panel.mode === "new" || panel.mode === "edit") && vaultId && (
        <HostEditPanel
          key={panel.mode === "edit" ? panel.id : "new"}
          vaultId={vaultId}
          hostId={panel.mode === "edit" ? panel.id : null}
          initialGroupId={panel.mode === "new" ? panel.groupId : null}
          onClose={() => setPanel({ mode: "closed" })}
        />
      )}

      {panel.mode === "group" && vaultId && (
        <GroupPanel
          key={panel.id ?? "new-group"}
          vaultId={vaultId}
          groupId={panel.id}
          initialParentId={panel.parentId}
          onClose={() => setPanel({ mode: "closed" })}
          onDeleted={(id, parentId) => {
            if (groupId === id) setGroupId(parentId);
          }}
          onDuplicated={(g) => setPanel({ mode: "group", id: g.id, parentId: g.parentId })}
        />
      )}

      {vaultId && (
        <MoveCopyDialog
          request={moveCopy}
          vaultId={vaultId}
          onClose={() => setMoveCopy(null)}
          onDone={clearSelection}
        />
      )}

      <ConfirmDialog
        open={confirmRemove !== null}
        title={
          confirmRemove && confirmRemove.length > 1
            ? `Remove ${confirmRemove.length} hosts?`
            : `Remove ${confirmRemove?.[0]?.label ?? "host"}?`
        }
        danger
        confirmLabel="Remove"
        busy={deleteHosts.isPending}
        onCancel={() => setConfirmRemove(null)}
        onConfirm={() => confirmRemove && removeHosts(confirmRemove)}
      >
        {confirmRemove && confirmRemove.length > 1 ? (
          <>
            {confirmRemove.slice(0, 6).map((h) => (
              <Typography key={h.id} variant="body2">
                {h.label}
              </Typography>
            ))}
            {confirmRemove.length > 6 && (
              <Typography variant="body2" color="text.secondary">
                …and {confirmRemove.length - 6} more
              </Typography>
            )}
            <Typography variant="body2" sx={{ mt: 1 }}>
              Their inline credentials are removed too. Shared identities and keys stay in the
              Keychain.
            </Typography>
          </>
        ) : (
          "The host and its inline credentials are removed. Shared identities and keys stay in the Keychain."
        )}
      </ConfirmDialog>

      <DeleteGroupDialog
        group={confirmGroup}
        onClose={() => setConfirmGroup(null)}
        onDeleted={(id, parentId) => {
          if (groupId === id) setGroupId(parentId);
          if (panel.mode === "group" && panel.id === id) setPanel({ mode: "closed" });
        }}
      />

      <Popover
        open={Boolean(tagAnchor)}
        anchorEl={tagAnchor}
        onClose={() => setTagAnchor(null)}
        anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
        transformOrigin={{ vertical: "top", horizontal: "right" }}
      >
        <Box sx={{ p: 1.5, maxWidth: 320 }}>
          <Typography variant="caption" color="text.secondary" sx={{ display: "block", mb: 1 }}>
            Filter by tag
          </Typography>
          {(tags.data ?? []).length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              No tags yet — add them in the host editor.
            </Typography>
          ) : (
            <Box sx={{ display: "flex", flexWrap: "wrap", gap: 0.75 }}>
              {(tags.data ?? []).map((t) => {
                const on = tagFilter.includes(t.label);
                return (
                  <Chip
                    key={t.id}
                    size="small"
                    label={t.label}
                    variant={on ? "filled" : "outlined"}
                    color={on ? "primary" : "default"}
                    onClick={() =>
                      updateTagFilter((f) =>
                        on ? f.filter((x) => x !== t.label) : [...f, t.label],
                      )
                    }
                  />
                );
              })}
            </Box>
          )}
        </Box>
      </Popover>

      <Menu
        open={Boolean(sortAnchor)}
        anchorEl={sortAnchor}
        onClose={() => setSortAnchor(null)}
        anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
        transformOrigin={{ vertical: "top", horizontal: "right" }}
        slotProps={{ paper: { sx: { minWidth: 200 } } }}
      >
        {(Object.keys(sortLabel) as SortKey[]).map((k) => (
          <MenuItem
            key={k}
            selected={sort === k}
            onClick={() => {
              setSort(k);
              setSortAnchor(null);
            }}
          >
            {sortLabel[k]}
          </MenuItem>
        ))}
      </Menu>

      <ActionMenu
        anchor={null}
        position={ctx ? { left: ctx.left, top: ctx.top } : null}
        onClose={() => setCtx(null)}
        items={ctx ? (ctx.kind === "host" ? hostMenu(ctx.host) : groupMenu(ctx.group)) : []}
      />
    </Box>
  );
}
