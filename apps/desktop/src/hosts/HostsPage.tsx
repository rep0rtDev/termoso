import { useMemo, useState } from "react";
import {
  Box,
  Breadcrumbs,
  Button,
  CircularProgress,
  Divider,
  IconButton,
  InputAdornment,
  Link,
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
import BoltRoundedIcon from "@mui/icons-material/BoltRounded";
import { EmptyState } from "@/components/EmptyState";
import { useSnackbar } from "@/components/Snackbar";
import { useDefaultVault, useGroups, useHosts, useSaveSettings, useSettings } from "@/ipc/hooks";
import type { GroupNode, HostCard, HostsView, Uuid } from "@/ipc/types";
import { errorMessage } from "@/ipc/types";
import { openTerminal } from "@/terminal/store";
import { HostGrid } from "./HostGrid";
import { HostList } from "./HostList";
import { HostEditPanel } from "./HostEditPanel";
import { GroupDialog } from "./GroupDialog";

type Editor =
  { mode: "closed" } | { mode: "new"; groupId: Uuid | null } | { mode: "edit"; id: Uuid };

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

interface Props {
  onOpenSftp: () => void;
}

export function HostsPage({ onOpenSftp }: Props) {
  const snackbar = useSnackbar();
  const vault = useDefaultVault();
  const vaultId = vault.data?.id ?? null;
  const hosts = useHosts(vaultId);
  const groups = useGroups(vaultId);
  const settings = useSettings();
  const saveSettings = useSaveSettings();

  const [groupId, setGroupId] = useState<Uuid | null>(null);
  const [search, setSearch] = useState("");
  const [quick, setQuick] = useState("");
  const [editor, setEditor] = useState<Editor>({ mode: "closed" });
  const [groupDialog, setGroupDialog] = useState<{ open: boolean; edit: GroupNode | null }>({
    open: false,
    edit: null,
  });

  const view: HostsView = settings.data?.hostsView ?? "grid";
  const setView = (v: HostsView | null) => {
    if (!v || !settings.data) return;
    saveSettings.mutate(
      { ...settings.data, hostsView: v },
      {
        onError: (e) => snackbar.error(errorMessage(e)),
      },
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
  const searching = q.length > 0;
  const childGroups = useMemo(
    () =>
      searching
        ? []
        : (groups.data ?? [])
            .filter((g) => g.parentId === groupId)
            .sort((a, b) => a.sortOrder - b.sortOrder || a.label.localeCompare(b.label)),
    [groups.data, groupId, searching],
  );
  const visibleHosts = useMemo(() => {
    const all = hosts.data ?? [];
    const scoped = searching
      ? all.filter((h) =>
          [h.label, h.address, h.username, ...h.tags, ...h.groupPath].some((s) =>
            s.toLowerCase().includes(q),
          ),
        )
      : all.filter((h) => h.groupId === groupId);
    return [...scoped].sort((a, b) => a.sortOrder - b.sortOrder || a.label.localeCompare(b.label));
  }, [hosts.data, groupId, q, searching]);

  const openHost = (h: HostCard) => setEditor({ mode: "edit", id: h.id });
  const connectHost = (h: HostCard) => openTerminal({ kind: "host", host_id: h.id });
  const quickTarget = parseQuickConnect(quick);
  const quickConnect = () => {
    if (!quickTarget) return;
    openTerminal(quickTarget);
    setQuick("");
  };
  const selectedId = editor.mode === "edit" ? editor.id : null;
  const loading = vault.isPending || hosts.isPending || groups.isPending;
  const loadError = vault.error ?? hosts.error ?? groups.error;

  return (
    <Box sx={{ display: "flex", flex: 1, minHeight: 0 }}>
      <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <Box
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1.5,
            px: 2.5,
            py: 1.5,
            borderBottom: 1,
            borderColor: "divider",
          }}
        >
          <TextField
            size="small"
            placeholder="Search hosts, tags, groups…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            sx={{ width: 320 }}
            slotProps={{
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchRoundedIcon fontSize="small" />
                  </InputAdornment>
                ),
              },
            }}
          />
          <TextField
            size="small"
            placeholder="Quick connect: user@host:port"
            value={quick}
            onChange={(e) => setQuick(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") quickConnect();
            }}
            error={quick.trim().length > 0 && quickTarget === null}
            sx={{ width: 300 }}
            slotProps={{
              input: {
                sx: { fontFamily: "monospace", fontSize: 13 },
                startAdornment: (
                  <InputAdornment position="start">
                    <BoltRoundedIcon fontSize="small" />
                  </InputAdornment>
                ),
              },
            }}
          />
          <Box sx={{ flex: 1 }} />
          <Tooltip title="Local terminal">
            <IconButton onClick={() => openTerminal({ kind: "local" })}>
              <TerminalRoundedIcon />
            </IconButton>
          </Tooltip>
          <ToggleButtonGroup
            size="small"
            exclusive
            value={view}
            onChange={(_e, v: HostsView | null) => setView(v)}
          >
            <ToggleButton value="grid" aria-label="Grid view">
              <GridViewRoundedIcon fontSize="small" />
            </ToggleButton>
            <ToggleButton value="list" aria-label="List view">
              <ViewListRoundedIcon fontSize="small" />
            </ToggleButton>
          </ToggleButtonGroup>
          <Tooltip title="New group">
            <IconButton
              onClick={() => setGroupDialog({ open: true, edit: null })}
              disabled={!vaultId}
            >
              <CreateNewFolderRoundedIcon />
            </IconButton>
          </Tooltip>
          <Button
            variant="contained"
            startIcon={<AddRoundedIcon />}
            disabled={!vaultId}
            onClick={() => setEditor({ mode: "new", groupId })}
          >
            New Host
          </Button>
        </Box>

        <Box sx={{ px: 2.5, pt: 1.5, display: "flex", alignItems: "center", minHeight: 36 }}>
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
                  onClick={() => setGroupId(g.id)}
                  sx={{ fontWeight: 600, fontSize: 14 }}
                >
                  {g.label}
                </Link>
              );
            })}
          </Breadcrumbs>
          <Box sx={{ flex: 1 }} />
          <Typography variant="caption" color="text.secondary">
            {visibleHosts.length} host{visibleHosts.length === 1 ? "" : "s"}
          </Typography>
        </Box>

        <Box sx={{ flex: 1, minHeight: 0, overflowY: "auto", px: 2.5, pb: 3 }}>
          {loading ? (
            <Box sx={{ display: "flex", justifyContent: "center", pt: 8 }}>
              <CircularProgress size={28} />
            </Box>
          ) : loadError ? (
            <EmptyState title="Could not open the vault" description={errorMessage(loadError)} />
          ) : childGroups.length === 0 && visibleHosts.length === 0 ? (
            <EmptyState
              icon={<DnsRoundedIcon />}
              title={searching ? "Nothing matches" : "No hosts yet"}
              description={
                searching
                  ? "Try a different label, address or tag."
                  : "Add your first server — everything is stored encrypted on this device."
              }
              action={
                searching ? undefined : (
                  <Button
                    variant="contained"
                    startIcon={<AddRoundedIcon />}
                    onClick={() => setEditor({ mode: "new", groupId })}
                  >
                    New Host
                  </Button>
                )
              }
            />
          ) : view === "grid" ? (
            <HostGrid
              groups={childGroups}
              hosts={visibleHosts}
              selectedId={selectedId}
              showPath={searching}
              onOpenGroup={setGroupId}
              onEditGroup={(g) => setGroupDialog({ open: true, edit: g })}
              onOpenHost={openHost}
              onConnectHost={connectHost}
            />
          ) : (
            <HostList
              groups={childGroups}
              hosts={visibleHosts}
              selectedId={selectedId}
              showPath={searching}
              onOpenGroup={setGroupId}
              onEditGroup={(g) => setGroupDialog({ open: true, edit: g })}
              onOpenHost={openHost}
              onConnectHost={connectHost}
            />
          )}
        </Box>
      </Box>

      {editor.mode !== "closed" && vaultId && (
        <>
          <Divider orientation="vertical" flexItem />
          <HostEditPanel
            key={editor.mode === "edit" ? editor.id : "new"}
            vaultId={vaultId}
            hostId={editor.mode === "edit" ? editor.id : null}
            initialGroupId={editor.mode === "new" ? editor.groupId : null}
            onClose={() => setEditor({ mode: "closed" })}
            onOpenSftp={onOpenSftp}
          />
        </>
      )}

      {vaultId && (
        <GroupDialog
          open={groupDialog.open}
          vaultId={vaultId}
          parentId={groupId}
          group={groupDialog.edit}
          onClose={() => setGroupDialog({ open: false, edit: null })}
          onDeleted={(id) => {
            if (groupId === id) setGroupId(groupDialog.edit?.parentId ?? null);
          }}
        />
      )}
    </Box>
  );
}
