import { useState } from "react";
import { Box, Divider } from "@mui/material";
import { Sidebar, type Section } from "./Sidebar";
import { HostsPage } from "@/hosts/HostsPage";
import { HistoryPage } from "@/history/HistoryPage";
import { SettingsPage } from "@/settings/SettingsPage";
import { ComingSoon } from "./ComingSoon";

export function AppShell() {
  const [section, setSection] = useState<Section>("hosts");
  return (
    <Box sx={{ display: "flex", height: "100%", bgcolor: "background.default" }}>
      <Sidebar section={section} onSelect={setSection} />
      <Divider orientation="vertical" flexItem />
      <Box sx={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        {section === "hosts" && <HostsPage />}
        {section === "history" && <HistoryPage />}
        {section === "settings" && <SettingsPage />}
        {section !== "hosts" && section !== "history" && section !== "settings" && (
          <ComingSoon section={section} />
        )}
      </Box>
    </Box>
  );
}
