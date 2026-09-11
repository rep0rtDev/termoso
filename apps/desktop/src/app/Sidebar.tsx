import {
  Box,
  Chip,
  CircularProgress,
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
import CloudDoneRoundedIcon from "@mui/icons-material/CloudDoneRounded";
import CloudSyncRoundedIcon from "@mui/icons-material/CloudSyncRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import type { ReactNode } from "react";
import { Logo } from "@/components/Logo";
import { useAccount, useAppInfo } from "@/ipc/hooks";
import type { AccountStatus } from "@/ipc/types";

export type Section =
  | "hosts"
  | "sftp"
  | "forwarding"
  | "snippets"
  | "keychain"
  | "knownHosts"
  | "history"
  | "logs"
  | "account"
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
  { id: "forwarding", label: "Port Forwarding", icon: <SwapHorizRoundedIcon /> },
  { id: "snippets", label: "Snippets", icon: <CodeRoundedIcon /> },
  { id: "keychain", label: "Keychain", icon: <KeyRoundedIcon /> },
  { id: "knownHosts", label: "Known Hosts", icon: <VerifiedUserRoundedIcon /> },
];

const secondary: Item[] = [
  { id: "history", label: "History", icon: <HistoryRoundedIcon /> },
  { id: "logs", label: "Logs", icon: <ArticleRoundedIcon /> },
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
  const { data: account } = useAccount();
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
      <Box sx={{ borderTop: 1, borderColor: "divider", p: 1 }}>
        <Tooltip title={footerTip(account)} placement="right">
          <ListItemButton
            selected={section === "account"}
            onClick={() => onSelect("account")}
            sx={{ borderRadius: 2, py: 0.75, gap: 1, color: "text.secondary" }}
          >
            <FooterIcon account={account} />
            <Box sx={{ minWidth: 0 }}>
              <Typography variant="body2" noWrap color="text.primary">
                {account?.account ? account.account.email : "Local vault"}
              </Typography>
              <Typography variant="caption" noWrap sx={{ display: "block" }}>
                {footerLine(account, info?.version)}
              </Typography>
            </Box>
          </ListItemButton>
        </Tooltip>
      </Box>
    </Box>
  );
}

function footerTip(a: AccountStatus | undefined): string {
  if (!a?.account) return "Offline vault — click to connect to a Termoso server";
  const s = a.sync;
  if (s.state === "error") return s.lastError ?? "Sync error";
  if (s.state === "syncing") return "Syncing…";
  if (s.state === "offline") return "Server unreachable — working offline";
  return s.lastSyncAt ? `Synced ${new Date(s.lastSyncAt).toLocaleString()}` : "Signed in";
}

function footerLine(a: AccountStatus | undefined, version: string | undefined): string {
  const v = version ? `v${version}` : "…";
  if (!a?.account) return `${v} · not synced`;
  switch (a.sync.state) {
    case "syncing":
      return `${v} · syncing`;
    case "offline":
      return `${v} · offline`;
    case "error":
      return `${v} · sync error`;
    case "idle":
      return `${v} · ${a.sync.realtime ? "live" : "synced"}`;
  }
}

function FooterIcon({ account }: { account: AccountStatus | undefined }) {
  if (!account?.account) return <CloudOffRoundedIcon fontSize="small" />;
  switch (account.sync.state) {
    case "syncing":
      return <CircularProgress size={18} thickness={5} />;
    case "offline":
      return <CloudSyncRoundedIcon fontSize="small" />;
    case "error":
      return <ErrorOutlineRoundedIcon fontSize="small" color="error" />;
    case "idle":
      return <CloudDoneRoundedIcon fontSize="small" color="primary" />;
  }
}
