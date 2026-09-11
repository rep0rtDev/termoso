import { useCallback, useEffect, useState } from "react";
import { Box, Divider } from "@mui/material";
import { useQueryClient } from "@tanstack/react-query";
import { Sidebar, type Section } from "./Sidebar";
import { TabBar } from "./TabBar";
import { HostsPage } from "@/hosts/HostsPage";
import { HistoryPage } from "@/history/HistoryPage";
import { SettingsPage } from "@/settings/SettingsPage";
import { SftpPage } from "@/sftp/SftpPage";
import { ComingSoon } from "./ComingSoon";
import { PromptHost } from "@/prompts/PromptHost";
import { TerminalWorkspace } from "@/terminal/TerminalWorkspace";
import { HOME_TAB, setActiveTab, startTerminalEvents, useTerminal } from "@/terminal/store";
import { startSftpEvents } from "@/sftp/store";

export function AppShell() {
  const [section, setSection] = useState<Section>("hosts");
  const queryClient = useQueryClient();
  const tabs = useTerminal((s) => s.tabs);
  const activeTabId = useTerminal((s) => s.activeTabId);
  const terminalOpen = activeTabId !== HOME_TAB;

  useEffect(() => {
    startTerminalEvents();
    startSftpEvents(queryClient);
  }, [queryClient]);

  const selectSection = useCallback((s: Section) => {
    setSection(s);
    setActiveTab(HOME_TAB);
  }, []);
  const openSftp = useCallback(() => selectSection("sftp"), [selectSection]);

  return (
    <Box sx={{ display: "flex", height: "100%", bgcolor: "background.default" }}>
      <Sidebar section={terminalOpen ? null : section} onSelect={selectSection} />
      <Divider orientation="vertical" flexItem />
      <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        {tabs.length > 0 && <TabBar onOpenSftp={openSftp} />}
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
            display: terminalOpen ? "none" : "flex",
            flexDirection: "column",
          }}
        >
          {section === "hosts" && <HostsPage onOpenSftp={openSftp} />}
          {section === "sftp" && <SftpPage />}
          {section === "history" && <HistoryPage />}
          {section === "settings" && <SettingsPage />}
          {section !== "hosts" &&
            section !== "sftp" &&
            section !== "history" &&
            section !== "settings" && <ComingSoon section={section} />}
        </Box>
      </Box>
      <PromptHost />
    </Box>
  );
}
