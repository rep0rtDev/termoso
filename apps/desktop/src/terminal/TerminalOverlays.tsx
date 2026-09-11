import { Box } from "@mui/material";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import ContentPasteRoundedIcon from "@mui/icons-material/ContentPasteRounded";
import SelectAllRoundedIcon from "@mui/icons-material/SelectAllRounded";
import DeleteSweepRoundedIcon from "@mui/icons-material/DeleteSweepRounded";
import VerticalSplitRoundedIcon from "@mui/icons-material/VerticalSplitRounded";
import HorizontalSplitRoundedIcon from "@mui/icons-material/HorizontalSplitRounded";
import OpenInNewRoundedIcon from "@mui/icons-material/OpenInNewRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import PaletteRoundedIcon from "@mui/icons-material/PaletteRounded";
import InfoOutlinedIcon from "@mui/icons-material/InfoOutlined";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ActionMenu, type MenuAction } from "@/components/ui";
import { MAX_PANES } from "./layout";
import {
  clearBuffer,
  closeContextMenu,
  confirmPendingClose,
  confirmPendingPaste,
  copySelection,
  movePaneToNewTab,
  paneHasSelection,
  pasteClipboard,
  requestClosePane,
  selectAll,
  setSearchOpen,
  setSidePanel,
  splitActivePane,
  useTerminal,
} from "./store";

/** App-wide terminal dialogs and the pane context menu; mount once. */
export function TerminalOverlays() {
  const pendingPaste = useTerminal((s) => s.pendingPaste);
  const pendingClose = useTerminal((s) => s.pendingClose);
  const menu = useTerminal((s) => s.contextMenu);
  const tab = useTerminal((s) => {
    const paneId = s.contextMenu?.paneId;
    return paneId ? s.tabs.find((t) => t.paneIds.includes(paneId)) : undefined;
  });

  const items: MenuAction[] = [];
  if (menu && tab) {
    const id = menu.paneId;
    const full = tab.paneIds.length >= MAX_PANES;
    items.push(
      {
        label: "Copy",
        icon: <ContentCopyRoundedIcon fontSize="small" />,
        disabled: !paneHasSelection(id),
        onClick: () => copySelection(id),
      },
      {
        label: "Paste",
        icon: <ContentPasteRoundedIcon fontSize="small" />,
        onClick: () => pasteClipboard(id),
      },
      {
        label: "Select all",
        icon: <SelectAllRoundedIcon fontSize="small" />,
        onClick: () => selectAll(id),
      },
      {
        label: "Clear buffer",
        icon: <DeleteSweepRoundedIcon fontSize="small" />,
        divider: true,
        onClick: () => clearBuffer(id),
      },
      {
        label: "Split right",
        icon: <VerticalSplitRoundedIcon fontSize="small" />,
        disabled: full,
        onClick: () => splitActivePane(tab.id, "row"),
      },
      {
        label: "Split down",
        icon: <HorizontalSplitRoundedIcon fontSize="small" />,
        disabled: full,
        divider: tab.paneIds.length === 1,
        onClick: () => splitActivePane(tab.id, "column"),
      },
    );
    if (tab.paneIds.length > 1) {
      items.push({
        label: "Move to new tab",
        icon: <OpenInNewRoundedIcon fontSize="small" />,
        divider: true,
        onClick: () => movePaneToNewTab(id),
      });
    }
    items.push(
      {
        label: "Search",
        icon: <SearchRoundedIcon fontSize="small" />,
        onClick: () => setSearchOpen(tab.id, true),
      },
      {
        label: "Themes",
        icon: <PaletteRoundedIcon fontSize="small" />,
        onClick: () => setSidePanel("themes"),
      },
      {
        label: "Session info",
        icon: <InfoOutlinedIcon fontSize="small" />,
        divider: true,
        onClick: () => setSidePanel("info"),
      },
      {
        label: "Close pane",
        icon: <CloseRoundedIcon fontSize="small" />,
        danger: true,
        onClick: () => requestClosePane(id),
      },
    );
  }

  return (
    <>
      <ActionMenu
        anchor={null}
        position={menu ? { left: menu.left, top: menu.top } : null}
        onClose={closeContextMenu}
        items={items}
      />

      <ConfirmDialog
        open={pendingPaste !== null}
        title="Paste multiple lines?"
        confirmLabel="Paste"
        onCancel={() => confirmPendingPaste(false)}
        onConfirm={() => confirmPendingPaste(true)}
      >
        The clipboard contains{" "}
        {pendingPaste ? pendingPaste.text.trimEnd().split(/\r?\n/).length : 0} lines. Each line will
        be executed as it is pasted.
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
    </>
  );
}
