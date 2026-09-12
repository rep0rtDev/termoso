import { Box, Divider, IconButton, Tooltip, Typography } from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import AddBoxRoundedIcon from "@mui/icons-material/AddBoxRounded";
import UsbRoundedIcon from "@mui/icons-material/UsbRounded";
import VerticalSplitRoundedIcon from "@mui/icons-material/VerticalSplitRounded";
import HorizontalSplitRoundedIcon from "@mui/icons-material/HorizontalSplitRounded";
import CampaignRoundedIcon from "@mui/icons-material/CampaignRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import DeleteSweepRoundedIcon from "@mui/icons-material/DeleteSweepRounded";
import ViewSidebarRoundedIcon from "@mui/icons-material/ViewSidebarRounded";
import GridViewRoundedIcon from "@mui/icons-material/GridViewRounded";
import ViewListRoundedIcon from "@mui/icons-material/ViewListRounded";
import DriveFileRenameOutlineRoundedIcon from "@mui/icons-material/DriveFileRenameOutlineRounded";
import BookmarkAddedRoundedIcon from "@mui/icons-material/BookmarkAddedRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import type { DragEvent, MouseEvent, ReactNode } from "react";
import { PqBadge, StatusDot } from "@/terminal/TerminalPane";
import {
  HOME_TAB,
  clearBuffer,
  closeTab,
  isWorkspaceTab,
  moveTab,
  openTerminal,
  renameTab,
  resetZoom,
  setActiveTab,
  isSearchOpen,
  setSearchOpen,
  splitActivePane,
  tabTitle,
  terminalStore,
  toggleBroadcast,
  toggleSidePanel,
  toggleTabViewMode,
  useTerminal,
  type TerminalTab,
} from "@/terminal/store";
import { saveTabAsTemplate, useWorkspaces } from "@/terminal/workspaces";
import { ActionMenu, InlineName, type MenuAction } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import { openSftpForSession, useSftp } from "@/sftp/store";
import { useHosts } from "@/ipc/hooks";
import { DistroGlyph, hostIcon } from "@/hosts/HostAvatar";
import { sizes } from "@/theme/theme";
import {
  NEW_TAB,
  SERIAL_TAB,
  SFTP_TAB,
  goHome,
  goToNewTab,
  goToSerial,
  goToSftp,
} from "./navigation";
import { AppMenuButton } from "./AppMenu";
import { TeamBlock } from "./TeamBlock";
import { WindowControls } from "./WindowControls";
import { VaultMenu, useActiveVault } from "./vault";
import { useState } from "react";

/**
 * Persistent top strip, doubling as the window title bar (the native frame is
 * off): Vaults · SFTP · terminal tabs · [+]  ……  pane tools · avatar + team ·
 * window controls. Empty space drags the window; double-click toggles maximize.
 */
export function TopBar() {
  const tabs = useTerminal((s) => s.tabs);
  const activeTabId = useTerminal((s) => s.activeTabId);
  const sftpCount = useSftp((s) => s.order.length);
  const active = tabs.find((t) => t.id === activeTabId);
  const [dragging, setDragging] = useState<string | null>(null);
  const vault = useActiveVault();
  const [vaultMenu, setVaultMenu] = useState<HTMLElement | null>(null);
  const multiVault = vault.vaults.length > 1;

  return (
    <Box
      data-tauri-drag-region
      sx={{
        display: "flex",
        alignItems: "stretch",
        height: sizes.topbar,
        flexShrink: 0,
        bgcolor: "surface.lowest",
        borderBottom: 1,
        borderColor: "border.light",
        pl: 1,
        userSelect: "none",
      }}
    >
      <Box sx={{ display: "flex", alignItems: "center", pr: 0.5 }}>
        <AppMenuButton />
      </Box>
      <TopTab
        active={activeTabId === HOME_TAB}
        onClick={() => setActiveTab(HOME_TAB)}
        icon={<LockRoundedIcon sx={{ fontSize: 16 }} />}
        label="Vaults"
        trailing={
          multiVault ? (
            <Tooltip title={vault.data ? `Vault: ${vault.data.name}` : "Switch vault"}>
              <IconButton
                aria-label="Switch vault"
                aria-haspopup="menu"
                aria-expanded={vaultMenu !== null}
                onClick={(e) => {
                  e.stopPropagation();
                  setVaultMenu(e.currentTarget);
                }}
                sx={{ width: 22, height: 22, mr: -0.5, color: "inherit" }}
              >
                <ExpandMoreRoundedIcon
                  sx={{
                    fontSize: 18,
                    transition: "transform 120ms",
                    transform: vaultMenu ? "rotate(180deg)" : "none",
                  }}
                />
              </IconButton>
            </Tooltip>
          ) : null
        }
      />
      <VaultMenu anchor={vaultMenu} onClose={() => setVaultMenu(null)} />
      <TopTab
        active={activeTabId === SFTP_TAB}
        onClick={goToSftp}
        icon={<FolderCopyRoundedIcon sx={{ fontSize: 16 }} />}
        label={sftpCount > 0 ? `SFTP (${sftpCount})` : "SFTP"}
      />
      {tabs.length > 0 && <Divider orientation="vertical" flexItem sx={{ my: 1.25, mx: 0.5 }} />}
      <Box
        data-tauri-drag-region
        sx={{
          flex: 1,
          minWidth: 0,
          display: "flex",
          alignItems: "stretch",
          overflowX: "auto",
          "&::-webkit-scrollbar": { height: 0 },
        }}
      >
        {tabs.map((t) => (
          <TerminalTopTab
            key={t.id}
            tab={t}
            active={t.id === activeTabId}
            dragging={dragging}
            onDragState={setDragging}
          />
        ))}
        {activeTabId === NEW_TAB && (
          <TopTab
            active
            onClick={goToNewTab}
            onClose={goHome}
            icon={<AddBoxRoundedIcon sx={{ fontSize: 16 }} />}
            label="New Tab"
          />
        )}
        {activeTabId === SERIAL_TAB && (
          <TopTab
            active
            onClick={goToSerial}
            onClose={goHome}
            icon={<UsbRoundedIcon sx={{ fontSize: 16 }} />}
            label="Serial"
          />
        )}
        <Tooltip title="New tab">
          <IconButton
            onClick={goToNewTab}
            sx={{ alignSelf: "center", mx: 0.5, width: 28, height: 28 }}
            aria-label="New tab"
          >
            <AddRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </Box>
      {active && <PaneTools tab={active} />}
      <Box sx={{ display: "flex", alignItems: "center", pl: 0.5, pr: 1 }}>
        {active && <Divider orientation="vertical" flexItem sx={{ my: 1.25, mr: 1 }} />}
        <TeamBlock />
      </Box>
      <WindowControls />
    </Box>
  );
}

function PaneTools({ tab }: { tab: TerminalTab }) {
  const pane = useTerminal((s) => s.panes[tab.activePaneId]);
  const sidePanel = useTerminal((s) => s.sidePanel);
  const searchOpen = useTerminal(isSearchOpen);
  const canSftp = pane?.protocol === "ssh" && pane.status === "connected";
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 0.25, px: 1 }}>
      {tab.zoom !== 1 && (
        <Tooltip title="Reset zoom (Ctrl+0)">
          <Box
            component="button"
            onClick={() => resetZoom(tab.id)}
            sx={{
              all: "unset",
              cursor: "pointer",
              px: 0.75,
              height: 20,
              mr: 0.5,
              borderRadius: 1,
              fontSize: 11,
              fontWeight: 600,
              color: "text.secondary",
              bgcolor: "action.selected",
              "&:hover": { color: "text.primary" },
            }}
          >
            {Math.round(tab.zoom * 100)}%
          </Box>
        </Tooltip>
      )}
      <Tooltip title="Split right (Ctrl+Shift+D)">
        <IconButton onClick={() => splitActivePane(tab.id, "row")}>
          <VerticalSplitRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Tooltip title="Split down (Ctrl+Shift+Alt+D)">
        <IconButton onClick={() => splitActivePane(tab.id, "column")}>
          <HorizontalSplitRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Tooltip
        title={
          tab.viewMode === "split"
            ? "Show terminals as a list (Ctrl+Alt+M)"
            : "Show terminals side by side (Ctrl+Alt+M)"
        }
      >
        <span>
          <IconButton
            disabled={tab.paneIds.length < 2 && tab.viewMode === "split"}
            onClick={() => toggleTabViewMode(tab.id)}
            sx={
              tab.viewMode === "list"
                ? { color: "text.primary", bgcolor: "action.selected" }
                : undefined
            }
          >
            <ViewListRoundedIcon fontSize="small" />
          </IconButton>
        </span>
      </Tooltip>
      <Tooltip title={tab.broadcast ? "Broadcast input: on" : "Broadcast input to all panes"}>
        <span>
          <IconButton
            disabled={tab.paneIds.length < 2}
            onClick={() => toggleBroadcast(tab.id)}
            sx={tab.broadcast ? { color: "primary.main", bgcolor: "action.selected" } : undefined}
          >
            <CampaignRoundedIcon fontSize="small" />
          </IconButton>
        </span>
      </Tooltip>
      <Tooltip title="Find (Ctrl+Shift+F)">
        <IconButton
          onClick={() => setSearchOpen(!searchOpen)}
          sx={searchOpen ? { color: "text.primary", bgcolor: "action.selected" } : undefined}
        >
          <SearchRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Tooltip title="Clear buffer (Ctrl+Shift+K)">
        <IconButton onClick={() => clearBuffer(tab.activePaneId)}>
          <DeleteSweepRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Tooltip title="Open SFTP for this connection">
        <span>
          <IconButton
            disabled={!canSftp}
            onClick={() => {
              if (!pane) return;
              openSftpForSession(pane.id, pane.title, pane.hostId);
              goToSftp();
            }}
          >
            <FolderCopyRoundedIcon fontSize="small" />
          </IconButton>
        </span>
      </Tooltip>
      <Tooltip title="Side panel: snippets, history, themes, info (Ctrl+Shift+B)">
        <IconButton
          onClick={() => toggleSidePanel()}
          sx={sidePanel ? { color: "text.primary", bgcolor: "action.selected" } : undefined}
        >
          <ViewSidebarRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
    </Box>
  );
}

const TAB_MIME = "application/x-termoso-tab";

function TerminalTopTab({
  tab,
  active,
  dragging,
  onDragState,
}: {
  tab: TerminalTab;
  active: boolean;
  /** Id of the tab currently being dragged, if any. */
  dragging: string | null;
  onDragState: (id: string | null) => void;
}) {
  const pane = useTerminal((s) => s.panes[tab.activePaneId]);
  const title = useTerminal((s) => tabTitle(tab, s));
  const hosts = useHosts(null);
  const snackbar = useSnackbar();
  const templateName = useWorkspaces(
    (s) => s.templates.find((t) => t.id === tab.templateId)?.name ?? null,
  );
  const [over, setOver] = useState<"before" | "after" | null>(null);
  const [menu, setMenu] = useState<{ left: number; top: number } | null>(null);
  const [renaming, setRenaming] = useState(false);
  if (!pane) return null;
  const paneHost = pane.hostId ? hosts.data?.find((h) => h.id === pane.hostId) : null;
  const distro = pane.status === "connected" ? hostIcon(paneHost) : null;
  const workspace = isWorkspaceTab(tab);

  const menuItems: MenuAction[] = [
    {
      label: "Rename",
      icon: <DriveFileRenameOutlineRoundedIcon fontSize="small" />,
      onClick: () => setRenaming(true),
    },
    {
      label: tab.viewMode === "split" ? "Show as list" : "Show side by side",
      icon:
        tab.viewMode === "split" ? (
          <ViewListRoundedIcon fontSize="small" />
        ) : (
          <GridViewRoundedIcon fontSize="small" />
        ),
      onClick: () => toggleTabViewMode(tab.id),
    },
    {
      label: templateName ? `Save to “${templateName}”` : "Save as workspace template",
      icon: <BookmarkAddedRoundedIcon fontSize="small" />,
      onClick: () => {
        const tpl = saveTabAsTemplate(tab.id);
        if (tpl) snackbar.notify(`Workspace “${tpl.name}” saved`);
      },
    },
    {
      label: "Duplicate session",
      icon: <ContentCopyRoundedIcon fontSize="small" />,
      divider: true,
      onClick: () => openTerminal(pane.target),
    },
    {
      label: workspace ? "Close workspace" : "Close",
      icon: <CloseRoundedIcon fontSize="small" />,
      onClick: () => closeTab(tab.id),
    },
  ];

  const onContextMenu = (e: MouseEvent<HTMLElement>) => {
    e.preventDefault();
    setMenu({ left: e.clientX, top: e.clientY });
  };

  const side = (e: DragEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    return e.clientX < r.left + r.width / 2 ? "before" : "after";
  };
  const drag = {
    draggable: true,
    onDragStart: (e: DragEvent<HTMLElement>) => {
      e.dataTransfer.setData(TAB_MIME, tab.id);
      e.dataTransfer.effectAllowed = "move";
      onDragState(tab.id);
    },
    onDragEnd: () => {
      onDragState(null);
      setOver(null);
    },
    onDragOver: (e: DragEvent<HTMLElement>) => {
      if (!dragging || dragging === tab.id) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      setOver(side(e));
    },
    onDragLeave: () => setOver(null),
    onDrop: (e: DragEvent<HTMLElement>) => {
      const id = e.dataTransfer.getData(TAB_MIME) || dragging;
      setOver(null);
      if (!id || id === tab.id) return;
      e.preventDefault();
      const tabs = terminalStore.get().tabs;
      if (side(e) === "before") moveTab(id, tab.id);
      else {
        const idx = tabs.findIndex((t) => t.id === tab.id);
        moveTab(id, tabs[idx + 1]?.id ?? null);
      }
    },
  };

  return (
    <>
      <TopTab
        active={active}
        drag={drag}
        dropSide={over}
        faded={dragging === tab.id}
        onClick={() => setActiveTab(tab.id)}
        onDoubleClick={() => setRenaming(true)}
        onContextMenu={onContextMenu}
        onMiddleClick={() => closeTab(tab.id)}
        icon={
          workspace ? (
            <GridViewRoundedIcon sx={{ fontSize: 16 }} />
          ) : pane.protocol === "local" ? (
            <TerminalRoundedIcon sx={{ fontSize: 16 }} />
          ) : distro ? (
            <DistroGlyph icon={distro} sx={{ fontSize: 15 }} />
          ) : (
            <StatusDot status={pane.status} />
          )
        }
        label={title}
        editor={
          renaming ? (
            <InlineName
              value={tab.name ?? title}
              placeholder="Workspace name"
              onCommit={(name) => {
                setRenaming(false);
                renameTab(tab.id, name.trim() ? name : tab.name);
              }}
              onCancel={() => setRenaming(false)}
              sx={{ width: 140 }}
            />
          ) : null
        }
        trailing={
          !workspace && tab.paneIds.length === 1 ? (
            <PqBadge algorithms={pane.algorithms} size={13} />
          ) : null
        }
        onClose={() => closeTab(tab.id)}
      />
      <ActionMenu position={menu} anchor={null} onClose={() => setMenu(null)} items={menuItems} />
    </>
  );
}

interface TopTabProps {
  active: boolean;
  icon: ReactNode;
  label: string;
  /** Replaces the label while a rename is in progress. */
  editor?: ReactNode;
  trailing?: ReactNode;
  onClick: () => void;
  onDoubleClick?: () => void;
  onContextMenu?: (e: MouseEvent<HTMLElement>) => void;
  onClose?: () => void;
  onMiddleClick?: () => void;
  /** HTML5 drag handlers for reorderable tabs. */
  drag?: DragHandlers & { draggable: boolean };
  dropSide?: "before" | "after" | null;
  faded?: boolean;
}

interface DragHandlers {
  onDragStart: (e: DragEvent<HTMLElement>) => void;
  onDragEnd: () => void;
  onDragOver: (e: DragEvent<HTMLElement>) => void;
  onDragLeave: () => void;
  onDrop: (e: DragEvent<HTMLElement>) => void;
}

function TopTab({
  active,
  icon,
  label,
  editor,
  trailing,
  onClick,
  onDoubleClick,
  onContextMenu,
  onClose,
  onMiddleClick,
  drag,
  dropSide,
  faded,
}: TopTabProps) {
  return (
    <Box
      role="tab"
      aria-selected={active}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      onAuxClick={(e) => {
        if (e.button === 1) onMiddleClick?.();
      }}
      {...drag}
      sx={{
        display: "flex",
        alignItems: "center",
        alignSelf: "center",
        gap: 0.75,
        height: 30,
        pl: 1.25,
        pr: onClose ? 0.5 : 1.25,
        mx: 0.25,
        minWidth: 0,
        maxWidth: 220,
        flexShrink: 0,
        borderRadius: 1.5,
        cursor: "default",
        opacity: faded ? 0.4 : 1,
        boxShadow:
          dropSide === "before"
            ? "-2px 0 0 0 var(--mui-palette-primary-main)"
            : dropSide === "after"
              ? "2px 0 0 0 var(--mui-palette-primary-main)"
              : "none",
        bgcolor: active ? "surface.highest" : "transparent",
        color: active ? "text.primary" : "text.secondary",
        "&:hover": { bgcolor: active ? "surface.highest" : "action.hover", color: "text.primary" },
        "&:hover .tab-close": { opacity: 1 },
        "& > svg": { color: active ? "text.primary" : "text.secondary" },
      }}
    >
      {icon}
      {editor ?? (
        <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
          {label}
        </Typography>
      )}
      {trailing}
      {onClose && (
        <IconButton
          className="tab-close"
          onClick={(e) => {
            e.stopPropagation();
            onClose();
          }}
          sx={{ opacity: active ? 0.7 : 0, width: 20, height: 20, ml: 0.25 }}
          aria-label={`Close ${label}`}
        >
          <CloseRoundedIcon sx={{ fontSize: 14 }} />
        </IconButton>
      )}
    </Box>
  );
}
