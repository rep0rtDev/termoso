import {
  Box,
  CircularProgress,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Tooltip,
  Typography,
} from "@mui/material";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import SwapHorizRoundedIcon from "@mui/icons-material/SwapHorizRounded";
import CodeRoundedIcon from "@mui/icons-material/CodeRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import ArticleRoundedIcon from "@mui/icons-material/ArticleRounded";
import SettingsRoundedIcon from "@mui/icons-material/SettingsRounded";
import CloudOffRoundedIcon from "@mui/icons-material/CloudOffRounded";
import CloudDoneRoundedIcon from "@mui/icons-material/CloudDoneRounded";
import CloudSyncRoundedIcon from "@mui/icons-material/CloudSyncRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import type { ReactNode } from "react";
import { useAccount, useAppInfo } from "@/ipc/hooks";
import type { AccountStatus } from "@/ipc/types";
import { sizes } from "@/theme/theme";
import { useTerminal } from "@/terminal/store";
import { goToSection, goToSettings, goToSftp, isSftpTab, useNav, type Section } from "./navigation";

interface Item {
  id: Section | "sftp";
  label: string;
  icon: ReactNode;
}

const items: Item[] = [
  { id: "hosts", label: "Hosts", icon: <DnsRoundedIcon fontSize="small" /> },
  { id: "sftp", label: "SFTP", icon: <FolderRoundedIcon fontSize="small" /> },
  { id: "keychain", label: "Keychain", icon: <KeyRoundedIcon fontSize="small" /> },
  { id: "forwarding", label: "Port Forwarding", icon: <SwapHorizRoundedIcon fontSize="small" /> },
  { id: "snippets", label: "Snippets", icon: <CodeRoundedIcon fontSize="small" /> },
  { id: "knownHosts", label: "Known Hosts", icon: <VerifiedUserRoundedIcon fontSize="small" /> },
  { id: "logs", label: "Logs", icon: <ArticleRoundedIcon fontSize="small" /> },
];

export function Sidebar() {
  const section = useNav((s) => s.section);
  const sftp = useTerminal((s) => isSftpTab(s.activeTabId));
  const { data: info } = useAppInfo();
  const { data: account } = useAccount();

  return (
    <Box
      component="nav"
      sx={{
        width: sizes.sidebar,
        flexShrink: 0,
        display: "flex",
        flexDirection: "column",
        bgcolor: "surface.lowest",
        borderRight: 1,
        borderColor: "border.light",
      }}
    >
      <List disablePadding sx={{ flex: 1, overflowY: "auto", px: 1, pt: 1 }}>
        {items.map((item) => (
          <NavItem
            key={item.id}
            item={item}
            selected={item.id === "sftp" ? sftp : !sftp && section === item.id}
            onClick={() => (item.id === "sftp" ? goToSftp() : goToSection(item.id))}
          />
        ))}
      </List>
      <Box sx={{ px: 1, pb: 1 }}>
        <NavItem
          item={{
            id: "settings",
            label: "Settings",
            icon: <SettingsRoundedIcon fontSize="small" />,
          }}
          selected={!sftp && section === "settings"}
          onClick={() => goToSettings()}
        />
        <Tooltip title={footerTip(account)} placement="right">
          <ListItemButton
            onClick={() => goToSettings("account")}
            sx={{ mt: 0.5, py: 0.75, gap: 1.25, alignItems: "center" }}
          >
            <FooterIcon account={account} />
            <Box sx={{ minWidth: 0 }}>
              <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
                {account?.account ? account.account.email : "Local vault"}
              </Typography>
              <Typography
                variant="caption"
                color="text.secondary"
                noWrap
                sx={{ display: "block", lineHeight: 1.3 }}
              >
                {footerLine(account, info?.version)}
              </Typography>
            </Box>
          </ListItemButton>
        </Tooltip>
      </Box>
    </Box>
  );
}

function NavItem({
  item,
  selected,
  onClick,
}: {
  item: Item;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <ListItemButton selected={selected} onClick={onClick} sx={{ my: 0.25, minHeight: 34 }}>
      <ListItemIcon sx={{ color: selected ? "text.primary" : "text.secondary" }}>
        {item.icon}
      </ListItemIcon>
      <ListItemText
        primary={item.label}
        slotProps={{
          primary: {
            variant: "body1",
            sx: { fontWeight: 500, color: selected ? "text.primary" : "text.secondary" },
          },
        }}
      />
    </ListItemButton>
  );
}

function footerTip(a: AccountStatus | undefined): string {
  if (!a?.account) return "Offline vault — open Settings › Account to connect to a server";
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
  const sx = { color: "text.secondary" } as const;
  if (!account?.account) return <CloudOffRoundedIcon fontSize="small" sx={sx} />;
  switch (account.sync.state) {
    case "syncing":
      return <CircularProgress size={18} thickness={5} />;
    case "offline":
      return <CloudSyncRoundedIcon fontSize="small" sx={sx} />;
    case "error":
      return <ErrorOutlineRoundedIcon fontSize="small" color="error" />;
    case "idle":
      return <CloudDoneRoundedIcon fontSize="small" color="primary" />;
  }
}
