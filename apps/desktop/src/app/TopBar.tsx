import { Box, Divider, IconButton, Tooltip, Typography } from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import VerticalSplitRoundedIcon from "@mui/icons-material/VerticalSplitRounded";
import HorizontalSplitRoundedIcon from "@mui/icons-material/HorizontalSplitRounded";
import CampaignRoundedIcon from "@mui/icons-material/CampaignRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import DeleteSweepRoundedIcon from "@mui/icons-material/DeleteSweepRounded";
import ViewSidebarRoundedIcon from "@mui/icons-material/ViewSidebarRounded";
import type { DragEvent, ReactNode } from "react";
import { PqBadge, StatusDot } from "@/terminal/TerminalPane";
import {
  HOME_TAB,
  clearBuffer,
  closeTab,
  moveTab,
  openTerminal,
  resetZoom,
  setActiveTab,
  setSearchOpen,
  splitActivePane,
  terminalStore,
  toggleBroadcast,
  toggleSidePanel,
  useTerminal,
  type TerminalTab,
} from "@/terminal/store";
import { openSftpForSession, useSftp } from "@/sftp/store";
import { useHosts } from "@/ipc/hooks";
import { distroIcon } from "@/hosts/distroIcons";
import { DistroGlyph } from "@/hosts/HostAvatar";
import { LogoMark } from "@/components/Logo";
import { ActionMenu } from "@/components/ui";
import { sizes } from "@/theme/theme";
import { SFTP_TAB, goToSftp } from "./navigation";
import { WindowControls } from "./WindowControls";
import { useState } from "react";

/**
 * Persistent top strip, doubling as the window title bar (the native frame is
 * off): Vaults · SFTP · terminal tabs · [+]  ……  pane tools · window controls.
 * Empty space drags the window; double-click toggles maximize.
 */
export function TopBar() {
  const tabs = useTerminal((s) => s.tabs);
  const activeTabId = useTerminal((s) => s.activeTabId);
  const sftpCount = useSftp((s) => s.order.length);
  const active = tabs.find((t) => t.id === activeTabId);
  const [addAnchor, setAddAnchor] = useState<HTMLElement | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);

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
      <Box
        data-tauri-drag-region
        sx={{ display: "flex", alignItems: "center", pr: 1, "& svg": { pointerEvents: "none" } }}
      >
        <LogoMark size={22} />
      </Box>
      <TopTab
        active={activeTabId === HOME_TAB}
        onClick={() => setActiveTab(HOME_TAB)}
        icon={<LockRoundedIcon sx={{ fontSize: 16 }} />}
        label="Vaults"
      />
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
        <Tooltip title="New terminal">
          <IconButton
            onClick={(e) => setAddAnchor(e.currentTarget)}
            sx={{ alignSelf: "center", mx: 0.5, width: 28, height: 28 }}
            aria-label="New terminal"
          >
            <AddRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
        <ActionMenu
          anchor={addAnchor}
          onClose={() => setAddAnchor(null)}
          items={[
            {
              label: "Local terminal",
              icon: <TerminalRoundedIcon fontSize="small" />,
              onClick: () => openTerminal({ kind: "local" }),
            },
          ]}
        />
      </Box>
      {active && <PaneTools tab={active} />}
      <WindowControls />
    </Box>
  );
}

function PaneTools({ tab }: { tab: TerminalTab }) {
  const pane = useTerminal((s) => s.panes[tab.activePaneId]);
  const sidePanel = useTerminal((s) => s.sidePanel);
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
          onClick={() => setSearchOpen(tab.id, !tab.searchOpen)}
          sx={tab.searchOpen ? { color: "text.primary", bgcolor: "action.selected" } : undefined}
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
  const hosts = useHosts(null);
  const [over, setOver] = useState<"before" | "after" | null>(null);
  if (!pane) return null;
  const os = pane.hostId ? hosts.data?.find((h) => h.id === pane.hostId)?.osName : null;
  const distro = pane.status === "connected" ? distroIcon(os) : null;

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
    <TopTab
      active={active}
      drag={drag}
      dropSide={over}
      faded={dragging === tab.id}
      onClick={() => setActiveTab(tab.id)}
      onMiddleClick={() => closeTab(tab.id)}
      icon={
        pane.protocol === "local" ? (
          <TerminalRoundedIcon sx={{ fontSize: 16 }} />
        ) : distro ? (
          <DistroGlyph icon={distro} sx={{ fontSize: 15 }} />
        ) : (
          <StatusDot status={pane.status} />
        )
      }
      label={tab.paneIds.length > 1 ? `${pane.title} (+${tab.paneIds.length - 1})` : pane.title}
      trailing={
        tab.paneIds.length === 1 ? <PqBadge algorithms={pane.algorithms} size={13} /> : null
      }
      onClose={() => closeTab(tab.id)}
    />
  );
}

interface TopTabProps {
  active: boolean;
  icon: ReactNode;
  label: string;
  trailing?: ReactNode;
  onClick: () => void;
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
  trailing,
  onClick,
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
        boxShadow: (t) =>
          dropSide === "before"
            ? `-2px 0 0 0 ${t.palette.primary.main}`
            : dropSide === "after"
              ? `2px 0 0 0 ${t.palette.primary.main}`
              : "none",
        bgcolor: active ? "surface.highest" : "transparent",
        color: active ? "text.primary" : "text.secondary",
        "&:hover": { bgcolor: active ? "surface.highest" : "action.hover", color: "text.primary" },
        "&:hover .tab-close": { opacity: 1 },
        "& > svg": { color: active ? "text.primary" : "text.secondary" },
      }}
    >
      {icon}
      <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
        {label}
      </Typography>
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
