import {
  Box,
  Chip,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Tooltip,
  Typography,
} from "@mui/material";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import SwapHorizRoundedIcon from "@mui/icons-material/SwapHorizRounded";
import CodeRoundedIcon from "@mui/icons-material/CodeRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import HistoryRoundedIcon from "@mui/icons-material/HistoryRounded";
import ArticleRoundedIcon from "@mui/icons-material/ArticleRounded";
import SettingsRoundedIcon from "@mui/icons-material/SettingsRounded";
import CloudOffRoundedIcon from "@mui/icons-material/CloudOffRounded";
import type { ReactNode } from "react";
import { Logo } from "@/components/Logo";
import { useAppInfo } from "@/ipc/hooks";

export type Section =
  | "hosts"
  | "sftp"
  | "forwarding"
  | "snippets"
  | "keychain"
  | "knownHosts"
  | "history"
  | "logs"
  | "settings";

interface Item {
  id: Section;
  label: string;
  icon: ReactNode;
  soon?: boolean;
}

const primary: Item[] = [
  { id: "hosts", label: "Hosts", icon: <DnsRoundedIcon /> },
  { id: "sftp", label: "SFTP", icon: <FolderCopyRoundedIcon /> },
  { id: "forwarding", label: "Port Forwarding", icon: <SwapHorizRoundedIcon />, soon: true },
  { id: "snippets", label: "Snippets", icon: <CodeRoundedIcon />, soon: true },
  { id: "keychain", label: "Keychain", icon: <KeyRoundedIcon />, soon: true },
  { id: "knownHosts", label: "Known Hosts", icon: <VerifiedUserRoundedIcon />, soon: true },
];

const secondary: Item[] = [
  { id: "history", label: "History", icon: <HistoryRoundedIcon /> },
  { id: "logs", label: "Logs", icon: <ArticleRoundedIcon />, soon: true },
  { id: "settings", label: "Settings", icon: <SettingsRoundedIcon /> },
];

export const SIDEBAR_WIDTH = 224;

export function Sidebar({
  section,
  onSelect,
}: {
  section: Section | null;
  onSelect: (s: Section) => void;
}) {
  const { data: info } = useAppInfo();
  const render = (item: Item) => (
    <ListItemButton
      key={item.id}
      selected={section === item.id}
      onClick={() => onSelect(item.id)}
      sx={{ borderRadius: 2, mx: 1, my: 0.25, py: 0.75, minHeight: 36 }}
    >
      <ListItemIcon sx={{ minWidth: 34, color: section === item.id ? "primary.main" : "inherit" }}>
        {item.icon}
      </ListItemIcon>
      <ListItemText primary={item.label} slotProps={{ primary: { variant: "body2" } }} />
      {item.soon && (
        <Chip label="soon" size="small" variant="outlined" sx={{ height: 18, fontSize: 10 }} />
      )}
    </ListItemButton>
  );

  return (
    <Box
      component="nav"
      sx={{
        width: SIDEBAR_WIDTH,
        flexShrink: 0,
        display: "flex",
        flexDirection: "column",
        bgcolor: "background.paper",
      }}
    >
      <Box sx={{ px: 2, pt: 2, pb: 1.5 }}>
        <Logo size={28} />
      </Box>
      <List dense disablePadding sx={{ flex: 1, overflowY: "auto" }}>
        {primary.map(render)}
        <Typography
          variant="overline"
          color="text.secondary"
          sx={{ display: "block", px: 3, pt: 2, pb: 0.5, fontSize: 10 }}
        >
          Workspace
        </Typography>
        {secondary.map(render)}
      </List>
      <Box sx={{ px: 2, py: 1.5, borderTop: 1, borderColor: "divider" }}>
        <Tooltip title="Offline vault — account sync arrives in a later build" placement="right">
          <Box sx={{ display: "flex", alignItems: "center", gap: 1, color: "text.secondary" }}>
            <CloudOffRoundedIcon fontSize="small" />
            <Box sx={{ minWidth: 0 }}>
              <Typography variant="body2" noWrap color="text.primary">
                Local vault
              </Typography>
              <Typography variant="caption" noWrap sx={{ display: "block" }}>
                {info ? `v${info.version} · ${info.masterKeySource}` : "…"}
              </Typography>
            </Box>
          </Box>
        </Tooltip>
      </Box>
    </Box>
  );
}
