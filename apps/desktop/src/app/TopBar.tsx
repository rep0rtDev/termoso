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
import type { ReactNode } from "react";
import { StatusDot } from "@/terminal/TerminalPane";
import {
  HOME_TAB,
  closeTab,
  openTerminal,
  setActiveTab,
  setSearchOpen,
  splitActivePane,
  toggleBroadcast,
  useTerminal,
  type TerminalTab,
} from "@/terminal/store";
import { openSftpForSession, useSftp } from "@/sftp/store";
import { LogoMark } from "@/components/Logo";
import { ActionMenu } from "@/components/ui";
import { sizes } from "@/theme/theme";
import { SFTP_TAB, goToSftp } from "./navigation";
import { useState } from "react";

/** Persistent top strip: Vaults · SFTP · terminal tabs · [+]  ……  pane tools. */
export function TopBar() {
  const tabs = useTerminal((s) => s.tabs);
  const activeTabId = useTerminal((s) => s.activeTabId);
  const sftpCount = useSftp((s) => s.order.length);
  const active = tabs.find((t) => t.id === activeTabId);
  const [addAnchor, setAddAnchor] = useState<HTMLElement | null>(null);

  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "stretch",
        height: sizes.topbar,
        flexShrink: 0,
        bgcolor: "surface.lowest",
        borderBottom: 1,
        borderColor: "border.light",
        pl: 1,
      }}
    >
      <Box sx={{ display: "flex", alignItems: "center", pr: 1 }}>
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
          <TerminalTopTab key={t.id} tab={t} active={t.id === activeTabId} />
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
    </Box>
  );
}

function PaneTools({ tab }: { tab: TerminalTab }) {
  const pane = useTerminal((s) => s.panes[tab.activePaneId]);
  const canSftp = pane?.protocol === "ssh" && pane.status === "connected";
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 0.25, px: 1 }}>
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
    </Box>
  );
}

function TerminalTopTab({ tab, active }: { tab: TerminalTab; active: boolean }) {
  const pane = useTerminal((s) => s.panes[tab.activePaneId]);
  if (!pane) return null;
  return (
    <TopTab
      active={active}
      onClick={() => setActiveTab(tab.id)}
      onMiddleClick={() => closeTab(tab.id)}
      icon={
        pane.protocol === "local" ? (
          <TerminalRoundedIcon sx={{ fontSize: 16 }} />
        ) : (
          <StatusDot status={pane.status} />
        )
      }
      label={tab.paneIds.length > 1 ? `${pane.title} (+${tab.paneIds.length - 1})` : pane.title}
      onClose={() => closeTab(tab.id)}
    />
  );
}

interface TopTabProps {
  active: boolean;
  icon: ReactNode;
  label: string;
  onClick: () => void;
  onClose?: () => void;
  onMiddleClick?: () => void;
}

function TopTab({ active, icon, label, onClick, onClose, onMiddleClick }: TopTabProps) {
  return (
    <Box
      role="tab"
      aria-selected={active}
      onClick={onClick}
      onAuxClick={(e) => {
        if (e.button === 1) onMiddleClick?.();
      }}
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
