import { useEffect } from "react";
import { Box } from "@mui/material";
import { SearchBar } from "./SearchBar";
import { SplitView } from "./SplitView";
import { TerminalSidePanel } from "./TerminalSidePanel";
import { focusPane, setSearchOpen, useTerminal } from "./store";

interface Props {
  tabId: string;
}

/** Panes of one tab (nested resizable splits) plus the optional side panel. */
export function TerminalWorkspace({ tabId }: Props) {
  const tab = useTerminal((s) => s.tabs.find((t) => t.id === tabId));
  const visible = useTerminal((s) => s.activeTabId === tabId);
  const activePaneId = tab?.activePaneId;

  useEffect(() => {
    if (visible && activePaneId) focusPane(activePaneId);
  }, [visible, activePaneId]);

  if (!tab) return null;

  return (
    <Box sx={{ flex: 1, minHeight: 0, display: "flex" }}>
      <Box sx={{ position: "relative", flex: 1, minWidth: 0, minHeight: 0, display: "flex" }}>
        <SplitView
          tabId={tabId}
          node={tab.layout}
          activePaneId={tab.activePaneId}
          showFrame={tab.paneIds.length > 1}
        />
        {tab.searchOpen && (
          <SearchBar
            key={tab.activePaneId}
            paneId={tab.activePaneId}
            onClose={() => setSearchOpen(tabId, false)}
          />
        )}
      </Box>
      <TerminalSidePanel tab={tab} />
    </Box>
  );
}
