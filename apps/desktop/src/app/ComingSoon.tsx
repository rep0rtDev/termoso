import { Box } from "@mui/material";
import ConstructionRoundedIcon from "@mui/icons-material/ConstructionRounded";
import { EmptyState } from "@/components/EmptyState";
import type { Section } from "./Sidebar";

const titles: Record<Section, string> = {
  hosts: "Hosts",
  sftp: "SFTP",
  forwarding: "Port Forwarding",
  snippets: "Snippets",
  keychain: "Keychain",
  knownHosts: "Known Hosts",
  history: "History",
  logs: "Logs",
  settings: "Settings",
};

export function ComingSoon({ section }: { section: Section }) {
  return (
    <Box sx={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center" }}>
      <EmptyState
        icon={<ConstructionRoundedIcon />}
        title={`${titles[section]} is on its way`}
        description="The Rust engine already supports it; this screen lands in an upcoming desktop build."
      />
    </Box>
  );
}
