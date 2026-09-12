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
import { startTerminalEvents, useTerminal } from "@/terminal/store";
import { startWorkspaces } from "@/terminal/workspaces";
import { RestoreBanner } from "@/terminal/RestoreBanner";
import { startSftpEvents } from "@/sftp/store";
import { startUpdateEvents } from "@/update/store";
import { UpdateBanner } from "@/update/UpdateBanner";
import { useAccount, useSettings, useSyncNotices } from "@/ipc/hooks";
import { WelcomeScreen } from "@/welcome/WelcomeScreen";
import { RecoveryPrompt } from "@/account/SignIn";
import { goToSettings, isHomeTab, isNewTab, isSerialTab, isSftpTab, useNav } from "./navigation";
import { NewTabPage } from "./NewTabPage";
import { SerialPage } from "@/hosts/SerialPage";
import { startCommands } from "./commands";
import { applyShortcutOverrides } from "./shortcuts";
import { CommandPalette } from "./CommandPalette";
import { startDeepLinks } from "./deepLinks";

export function AppShell() {
  const section = useNav((s) => s.section);
  const queryClient = useQueryClient();
  const tabs = useTerminal((s) => s.tabs);
  const activeTabId = useTerminal((s) => s.activeTabId);
  const home = isHomeTab(activeTabId);
  const sftp = isSftpTab(activeTabId);
  const newTab = isNewTab(activeTabId);
  const serial = isSerialTab(activeTabId);
  const account = useAccount();
  const settings = useSettings();

  useSyncNotices();
  useEffect(() => {
    startTerminalEvents(queryClient);
    startWorkspaces();
    startSftpEvents(queryClient);
    startUpdateEvents();
    const stopCommands = startCommands();
    const stopLinks = startDeepLinks();
    return () => {
      stopCommands();
      stopLinks();
    };
  }, [queryClient]);
  useEffect(() => {
    if (settings.data) applyShortcutOverrides(settings.data.shortcuts);
  }, [settings.data]);

  if (account.isPending || settings.isPending) {
    return <Box sx={{ height: "100%", bgcolor: "surface.lowest" }} />;
  }
  if (settings.data && account.data && !account.data.account && !settings.data.welcomeSeen) {
    return (
      <>
        <WelcomeScreen settings={settings.data} />
        <RecoveryPrompt />
      </>
    );
  }

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
        {(home || sftp) && <Sidebar />}
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
          {home && <RestoreBanner />}
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
          {newTab && <NewTabPage />}
          {serial && <SerialPage />}
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
      <CommandPalette />
      <PromptHost />
      <RecoveryPrompt />
    </Box>
  );
}
