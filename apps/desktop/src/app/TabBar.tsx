import { Box, Divider, IconButton, Stack, Tooltip, Typography, alpha } from "@mui/material";
import HomeRoundedIcon from "@mui/icons-material/HomeRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import VerticalSplitRoundedIcon from "@mui/icons-material/VerticalSplitRounded";
import HorizontalSplitRoundedIcon from "@mui/icons-material/HorizontalSplitRounded";
import CampaignRoundedIcon from "@mui/icons-material/CampaignRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
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
import { openSftpForSession } from "@/sftp/store";

interface Props {
  onOpenSftp: () => void;
}

export function TabBar({ onOpenSftp }: Props) {
  const tabs = useTerminal((s) => s.tabs);
  const activeTabId = useTerminal((s) => s.activeTabId);
  const active = tabs.find((t) => t.id === activeTabId);

  return (
    <Stack
      direction="row"

      sx={{
        alignItems: "stretch",
        height: 40,
        flexShrink: 0,
        bgcolor: "background.paper",
        borderBottom: 1,
        borderColor: "divider",
      }}
    >
      <TabButton
        active={activeTabId === HOME_TAB}
        onClick={() => setActiveTab(HOME_TAB)}
        icon={<HomeRoundedIcon fontSize="small" />}
        label="Hosts"
      />
      <Box
        sx={{
          flex: 1,
          minWidth: 0,
          display: "flex",
          overflowX: "auto",
          "&::-webkit-scrollbar": { height: 3 },
        }}
      >
        {tabs.map((t) => (
          <TerminalTabButton key={t.id} tab={t} active={t.id === activeTabId} />
        ))}
        <Tooltip title="New local terminal">
          <IconButton
            size="small"
            onClick={() => openTerminal({ kind: "local" })}
            sx={{ alignSelf: "center", mx: 0.5 }}
            aria-label="New local terminal"
          >
            <AddRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </Box>
      {active && <TabToolbar tab={active} onOpenSftp={onOpenSftp} />}
    </Stack>
  );
}

function TabToolbar({ tab, onOpenSftp }: { tab: TerminalTab; onOpenSftp: () => void }) {
  const pane = useTerminal((s) => s.panes[tab.activePaneId]);
  const canSftp = pane?.protocol === "ssh" && pane.status === "connected";
  return (
    <Stack direction="row" spacing={0.25} sx={{ alignItems: "center", px: 1 }}>
      <Divider orientation="vertical" flexItem sx={{ my: 1, mr: 0.5 }} />
      <Tooltip title="Split right (Ctrl+Shift+D)">
        <IconButton size="small" onClick={() => splitActivePane(tab.id, "row")}>
          <VerticalSplitRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Tooltip title="Split down (Ctrl+Shift+Alt+D)">
        <IconButton size="small" onClick={() => splitActivePane(tab.id, "column")}>
          <HorizontalSplitRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Tooltip title={tab.broadcast ? "Broadcast input: on" : "Broadcast input to all panes"}>
        <span>
          <IconButton
            size="small"
            color={tab.broadcast ? "primary" : "default"}
            disabled={tab.paneIds.length < 2}
            onClick={() => toggleBroadcast(tab.id)}
          >
            <CampaignRoundedIcon fontSize="small" />
          </IconButton>
        </span>
      </Tooltip>
      <Tooltip title="Find (Ctrl+Shift+F)">
        <IconButton
          size="small"
          color={tab.searchOpen ? "primary" : "default"}
          onClick={() => setSearchOpen(tab.id, !tab.searchOpen)}
        >
          <SearchRoundedIcon fontSize="small" />
        </IconButton>
      </Tooltip>
      <Tooltip title="Open SFTP for this connection">
        <span>
          <IconButton
            size="small"
            disabled={!canSftp}
            onClick={() => {
              if (!pane) return;
              openSftpForSession(pane.id, pane.title, pane.hostId);
              onOpenSftp();
            }}
          >
            <FolderCopyRoundedIcon fontSize="small" />
          </IconButton>
        </span>
      </Tooltip>
    </Stack>
  );
}

function TerminalTabButton({ tab, active }: { tab: TerminalTab; active: boolean }) {
  const pane = useTerminal((s) => s.panes[tab.activePaneId]);
  if (!pane) return null;
  return (
    <TabButton
      active={active}
      onClick={() => setActiveTab(tab.id)}
      onMiddleClick={() => closeTab(tab.id)}
      icon={
        pane.protocol === "local" ? (
          <TerminalRoundedIcon fontSize="small" />
        ) : (
          <StatusDot status={pane.status} />
        )
      }
      label={tab.paneIds.length > 1 ? `${pane.title} (+${tab.paneIds.length - 1})` : pane.title}
      onClose={() => closeTab(tab.id)}
    />
  );
}

interface TabButtonProps {
  active: boolean;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  onClose?: () => void;
  onMiddleClick?: () => void;
}

function TabButton({ active, icon, label, onClick, onClose, onMiddleClick }: TabButtonProps) {
  return (
    <Box
      role="tab"
      aria-selected={active}
      onClick={onClick}
      onAuxClick={(e) => {
        if (e.button === 1) onMiddleClick?.();
      }}
      sx={(t) => ({
        display: "flex",
        alignItems: "center",
        gap: 1,
        pl: 1.5,
        pr: onClose ? 0.5 : 1.5,
        minWidth: 0,
        maxWidth: 220,
        flexShrink: 0,
        cursor: "pointer",
        borderRight: 1,
        borderColor: "divider",
        borderBottom: `2px solid ${active ? t.palette.primary.main : "transparent"}`,
        bgcolor: active ? alpha(t.palette.primary.main, 0.08) : "transparent",
        color: active ? "text.primary" : "text.secondary",
        "&:hover": { bgcolor: alpha(t.palette.primary.main, 0.05) },
        "&:hover .tab-close": { opacity: 1 },
      })}
    >
      {icon}
      <Typography variant="body2" noWrap sx={{ fontWeight: active ? 600 : 500 }}>
        {label}
      </Typography>
      {onClose && (
        <IconButton
          className="tab-close"
          size="small"
          onClick={(e) => {
            e.stopPropagation();
            onClose();
          }}
          sx={{ opacity: active ? 0.7 : 0, p: 0.25, ml: 0.25 }}
          aria-label={`Close ${label}`}
        >
          <CloseRoundedIcon sx={{ fontSize: 16 }} />
        </IconButton>
      )}
    </Box>
  );
}
