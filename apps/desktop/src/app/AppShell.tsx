import { useEffect } from "react";
import { Box } from "@mui/material";
import { useQueryClient } from "@tanstack/react-query";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { HostsPage } from "@/hosts/HostsPage";
import { SettingsPage } from "@/settings/SettingsPage";
import { SftpPage } from "@/sftp/SftpPage";
import { ForwardingPage } from "@/forwarding/ForwardingPage";
import { SnippetsPage } from "@/snippets/SnippetsPage";
import { KeychainPage } from "@/keychain/KeychainPage";
import { KnownHostsPage } from "@/knownhosts/KnownHostsPage";
import { LogsPage } from "@/logs/LogsPage";
import { PromptHost } from "@/prompts/PromptHost";
import { TerminalWorkspace } from "@/terminal/TerminalWorkspace";
import { TerminalOverlays } from "@/terminal/TerminalOverlays";
import { startTerminalHotkeys } from "@/terminal/hotkeys";
import { startTerminalEvents, useTerminal } from "@/terminal/store";
import { startSftpEvents } from "@/sftp/store";
import { startUpdateEvents } from "@/update/store";
import { UpdateBanner } from "@/update/UpdateBanner";
import { useSyncNotices } from "@/ipc/hooks";
import { goToSettings, isHomeTab, isSftpTab, useNav } from "./navigation";

export function AppShell() {
  const section = useNav((s) => s.section);
  const queryClient = useQueryClient();
  const tabs = useTerminal((s) => s.tabs);
  const activeTabId = useTerminal((s) => s.activeTabId);
  const home = isHomeTab(activeTabId);
  const sftp = isSftpTab(activeTabId);

  useSyncNotices();
  useEffect(() => {
    startTerminalEvents();
    startTerminalHotkeys();
    startSftpEvents(queryClient);
    startUpdateEvents();
  }, [queryClient]);

  return (
    <Box
      sx={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        bgcolor: "background.default",
      }}
    >
      <TopBar />
      <Box sx={{ flex: 1, minHeight: 0, display: "flex" }}>
        {home && <Sidebar />}
        <Box
          sx={{
            flex: 1,
            minWidth: 0,
            display: "flex",
            flexDirection: "column",
            bgcolor: "surface.base",
          }}
        >
          {home && <UpdateBanner onOpenSettings={() => goToSettings("updates")} />}
          {tabs.map((t) => (
            <Box
              key={t.id}
              sx={{
                flex: 1,
                minHeight: 0,
                display: t.id === activeTabId ? "flex" : "none",
                flexDirection: "column",
              }}
            >
              <TerminalWorkspace tabId={t.id} />
            </Box>
          ))}
          <Box
            sx={{
              flex: 1,
              minHeight: 0,
              display: sftp ? "flex" : "none",
              flexDirection: "column",
            }}
          >
            <SftpPage />
          </Box>
          <Box
            sx={{
              flex: 1,
              minHeight: 0,
              display: home ? "flex" : "none",
              flexDirection: "column",
            }}
          >
            {section === "hosts" && <HostsPage />}
            {section === "forwarding" && <ForwardingPage />}
            {section === "snippets" && <SnippetsPage />}
            {section === "keychain" && <KeychainPage />}
            {section === "knownHosts" && <KnownHostsPage />}
            {section === "logs" && <LogsPage />}
            {section === "settings" && <SettingsPage />}
          </Box>
        </Box>
      </Box>
      <TerminalOverlays />
      <PromptHost />
    </Box>
  );
}
