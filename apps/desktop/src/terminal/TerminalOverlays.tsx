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
import AutoAwesomeOutlinedIcon from "@mui/icons-material/AutoAwesomeOutlined";
import SnoozeRoundedIcon from "@mui/icons-material/SnoozeRounded";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ActionMenu, type MenuAction } from "@/components/ui";
import { MAX_PANES } from "./layout";
import { LinkHoverHint, ReconnectSnackbar } from "./ReconnectSnackbar";
import {
  clearBuffer,
  closeContextMenu,
  confirmPendingClose,
  confirmPendingPaste,
  copySelection,
  endOfToday,
  movePaneToNewTab,
  paneHasSelection,
  pasteClipboard,
  pauseSuggestions,
  requestClosePane,
  selectAll,
  setPaneAutocomplete,
  setSearchOpen,
  setSidePanel,
  splitActivePane,
  useTerminal,
} from "./store";
import { tr, trn } from "@/i18n";

/** App-wide terminal dialogs and the pane context menu; mount once. */
export function TerminalOverlays() {
  const pendingPaste = useTerminal((s) => s.pendingPaste);
  const pendingClose = useTerminal((s) => s.pendingClose);
  const liveClose = useTerminal(
    (s) => s.pendingClose?.filter((id) => s.panes[id]?.status === "connected").length ?? 0,
  );
  const menu = useTerminal((s) => s.contextMenu);
  const tab = useTerminal((s) => {
    const paneId = s.contextMenu?.paneId;
    return paneId ? s.tabs.find((t) => t.paneIds.includes(paneId)) : undefined;
  });
  const paneSuggest = useTerminal((s) =>
    s.contextMenu ? (s.panes[s.contextMenu.paneId]?.autocomplete ?? true) : true,
  );
  const paused = useTerminal(
    (s) => s.suggestPausedUntil !== null && s.suggestPausedUntil > Date.now(),
  );

  const items: MenuAction[] = [];
  if (menu && tab) {
    const id = menu.paneId;
    const full = tab.paneIds.length >= MAX_PANES;
    items.push(
      {
        label: tr("Copy"),
        icon: <ContentCopyRoundedIcon fontSize="small" />,
        disabled: !paneHasSelection(id),
        onClick: () => copySelection(id),
      },
      {
        label: tr("Paste"),
        icon: <ContentPasteRoundedIcon fontSize="small" />,
        onClick: () => pasteClipboard(id),
      },
      {
        label: tr("Select all"),
        icon: <SelectAllRoundedIcon fontSize="small" />,
        onClick: () => selectAll(id),
      },
      {
        label: tr("Clear buffer"),
        icon: <DeleteSweepRoundedIcon fontSize="small" />,
        divider: true,
        onClick: () => clearBuffer(id),
      },
      {
        label: tr("Split right"),
        icon: <VerticalSplitRoundedIcon fontSize="small" />,
        disabled: full,
        onClick: () => splitActivePane(tab.id, "row"),
      },
      {
        label: tr("Split down"),
        icon: <HorizontalSplitRoundedIcon fontSize="small" />,
        disabled: full,
        divider: tab.paneIds.length === 1,
        onClick: () => splitActivePane(tab.id, "column"),
      },
    );
    if (tab.paneIds.length > 1) {
      items.push({
        label: tr("Move to new tab"),
        icon: <OpenInNewRoundedIcon fontSize="small" />,
        divider: true,
        onClick: () => movePaneToNewTab(id),
      });
    }
    items.push(
      {
        label: tr("Search"),
        icon: <SearchRoundedIcon fontSize="small" />,
        onClick: () => setSearchOpen(true),
      },
      {
        label: tr("Themes"),
        icon: <PaletteRoundedIcon fontSize="small" />,
        onClick: () => setSidePanel("themes"),
      },
      {
        label: tr("Session info"),
        icon: <InfoOutlinedIcon fontSize="small" />,
        divider: true,
        onClick: () => setSidePanel("info"),
      },
      {
        label: paneSuggest ? tr("Turn off suggestions here") : tr("Turn on suggestions here"),
        icon: <AutoAwesomeOutlinedIcon fontSize="small" />,
        onClick: () => setPaneAutocomplete(id, !paneSuggest),
      },
      {
        label: paused
          ? tr("Resume suggestions everywhere")
          : tr("Pause suggestions until tomorrow"),
        icon: <SnoozeRoundedIcon fontSize="small" />,
        divider: true,
        onClick: () => pauseSuggestions(paused ? null : endOfToday()),
      },
      {
        label: tr("Close pane"),
        icon: <CloseRoundedIcon fontSize="small" />,
        danger: true,
        onClick: () => requestClosePane(id),
      },
    );
  }

  return (
    <>
      <ReconnectSnackbar />
      <LinkHoverHint />
      <ActionMenu
        anchor={null}
        position={menu ? { left: menu.left, top: menu.top } : null}
        onClose={closeContextMenu}
        items={items}
      />

      <ConfirmDialog
        open={pendingPaste !== null}
        title={tr("Paste multiple lines?")}
        confirmLabel={tr("Paste")}
        onCancel={() => confirmPendingPaste(false)}
        onConfirm={() => confirmPendingPaste(true)}
      >
        {trn(
          pendingPaste ? pendingPaste.text.trimEnd().split(/\r?\n/).length : 0,
          "The clipboard contains {count} line. It will be executed as it is pasted.",
          "The clipboard contains {count} lines. Each line will be executed as it is pasted.",
        )}
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
        title={
          liveClose > 1
            ? tr("Close {liveClose} connected sessions?", { liveClose })
            : tr("Close connected session?")
        }
        confirmLabel={tr("Close")}
        danger
        onCancel={() => confirmPendingClose(false)}
        onConfirm={() => confirmPendingClose(true)}
      >
        {liveClose > 1
          ? tr("These sessions are still connected. Running processes in them will be terminated.")
          : tr("The session is still connected. Running processes in it will be terminated.")}
      </ConfirmDialog>
    </>
  );
}
