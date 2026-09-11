import { useEffect } from "react";
import { Box } from "@mui/material";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { TerminalPane } from "./TerminalPane";
import { SearchBar } from "./SearchBar";
import {
  confirmPendingClose,
  confirmPendingPaste,
  requestClosePane,
  setSearchOpen,
  splitActivePane,
  useTerminal,
} from "./store";

interface Props {
  tabId: string;
}

/** Panes of one tab, laid out as a row/column split (grid beyond two). */
export function TerminalWorkspace({ tabId }: Props) {
  const tab = useTerminal((s) => s.tabs.find((t) => t.id === tabId));
  const pendingPaste = useTerminal((s) => s.pendingPaste);
  const pendingClose = useTerminal((s) => s.pendingClose);

  useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      const ctrl = ev.ctrlKey || ev.metaKey;
      if (!ctrl || !ev.shiftKey || !tab) return;
      switch (ev.code) {
        case "KeyF":
          ev.preventDefault();
          setSearchOpen(tabId, true);
          break;
        case "KeyD":
          ev.preventDefault();
          splitActivePane(tabId, ev.altKey ? "column" : "row");
          break;
        case "KeyW":
          ev.preventDefault();
          requestClosePane(tab.activePaneId);
          break;
        default:
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tab, tabId]);

  if (!tab) return null;
  const many = tab.paneIds.length > 2;

  return (
    <Box sx={{ position: "relative", flex: 1, minHeight: 0, display: "flex" }}>
      <Box
        sx={{
          flex: 1,
          minWidth: 0,
          minHeight: 0,
          display: many ? "grid" : "flex",
          flexDirection: tab.direction,
          gridTemplateColumns: many ? "repeat(2, minmax(0, 1fr))" : undefined,
          gridAutoRows: many ? "minmax(0, 1fr)" : undefined,
          gap: tab.paneIds.length > 1 ? "2px" : 0,
          bgcolor: "divider",
        }}
      >
        {tab.paneIds.map((id) => (
          <TerminalPane
            key={id}
            paneId={id}
            active={tab.activePaneId === id}
            showFrame={tab.paneIds.length > 1}
          />
        ))}
      </Box>
      {tab.searchOpen && (
        <SearchBar
          key={tab.activePaneId}
          paneId={tab.activePaneId}
          onClose={() => setSearchOpen(tabId, false)}
        />
      )}

      <ConfirmDialog
        open={pendingPaste !== null}
        title="Paste multiple lines?"
        confirmLabel="Paste"
        onCancel={() => confirmPendingPaste(false)}
        onConfirm={() => confirmPendingPaste(true)}
      >
        The clipboard contains {pendingPaste ? pendingPaste.text.split(/\r?\n/).length : 0} lines.
        Each line will be executed as it is pasted.
        <Box
          component="pre"
          sx={{
            mt: 1.5,
            p: 1,
            maxHeight: 160,
            overflow: "auto",
            bgcolor: "background.default",
            borderRadius: 1,
            fontSize: 12,
            fontFamily: "monospace",
            whiteSpace: "pre-wrap",
            userSelect: "text",
          }}
        >
          {pendingPaste?.text}
        </Box>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingClose !== null}
        title="Close connected session?"
        confirmLabel="Close"
        danger
        onCancel={() => confirmPendingClose(false)}
        onConfirm={() => confirmPendingClose(true)}
      >
        The session is still connected. Running processes in it will be terminated.
      </ConfirmDialog>
    </Box>
  );
}
