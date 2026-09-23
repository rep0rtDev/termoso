import { useEffect, useRef } from "react";
import { Box, Typography } from "@mui/material";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import TuneRoundedIcon from "@mui/icons-material/TuneRounded";
import AccountTreeRoundedIcon from "@mui/icons-material/AccountTreeRounded";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import InsertDriveFileOutlinedIcon from "@mui/icons-material/InsertDriveFileOutlined";
import HistoryRoundedIcon from "@mui/icons-material/HistoryRounded";
import DataObjectRoundedIcon from "@mui/icons-material/DataObjectRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import type { Uuid } from "@/ipc/types";
import type { Suggestion } from "./autocomplete";
import { acceptSuggest, selectSuggest, useTerminal } from "./store";
import { tr } from "@/i18n";

const WIDTH = 360;

function iconFor(s: Suggestion) {
  const sx = { fontSize: 15, color: "text.secondary", flexShrink: 0 } as const;
  switch (s.kind) {
    case "command":
      return <TerminalRoundedIcon sx={sx} />;
    case "option":
      return <TuneRoundedIcon sx={sx} />;
    case "subcommand":
      return <AccountTreeRoundedIcon sx={sx} />;
    case "path":
      return s.desc === "directory" ? (
        <FolderRoundedIcon sx={sx} />
      ) : (
        <InsertDriveFileOutlinedIcon sx={sx} />
      );
    case "history":
      return <HistoryRoundedIcon sx={sx} />;
    case "snippet":
      return <DataObjectRoundedIcon sx={sx} />;
    case "identity":
      return <KeyRoundedIcon sx={{ ...sx, color: "primary.main" }} />;
  }
}

/** Completion list anchored to the cursor of `paneId`; keyboard handling lives in the store. */
export function AutocompletePopup({ paneId }: { paneId: Uuid }) {
  const sg = useTerminal((s) => (s.suggest?.paneId === paneId ? s.suggest : null));
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!sg) return;
    const el = listRef.current?.children[sg.selected];
    if (el instanceof HTMLElement) el.scrollIntoView({ block: "nearest" });
  }, [sg]);

  if (!sg) return null;
  const width = Math.min(WIDTH, Math.max(200, sg.paneWidth - 8));
  const left = Math.max(4, Math.min(sg.left, sg.paneWidth - width - 4));
  const pos = sg.above
    ? { bottom: `calc(100% - ${sg.top - 2}px)` }
    : { top: sg.top + sg.cellHeight + 2 };

  return (
    <Box
      ref={listRef}
      role="listbox"
      onMouseDown={(e) => e.preventDefault()}
      sx={{
        position: "absolute",
        left,
        ...pos,
        width,
        maxHeight: 240,
        overflowY: "auto",
        zIndex: 5,
        py: 0.5,
        bgcolor: "surface.high",
        border: 1,
        borderColor: "divider",
        borderRadius: 1.5,
        boxShadow: "0 8px 24px rgba(0,0,0,0.35)",
      }}
    >
      {sg.items.map((item, i) => (
        <Box
          key={`${item.kind}:${item.label}`}
          role="option"
          aria-selected={i === sg.selected}
          onMouseEnter={() => selectSuggest(i)}
          onClick={() => acceptSuggest(i)}
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1,
            px: 1.25,
            height: 28,
            cursor: "pointer",
            bgcolor: i === sg.selected ? "action.selected" : "transparent",
            "&:hover": { bgcolor: "action.hover" },
          }}
        >
          {iconFor(item)}
          <Typography
            component="span"
            noWrap
            sx={{ fontFamily: "monospace", fontSize: 12.5, flex: 1, minWidth: 0 }}
          >
            {item.label}
          </Typography>
          {item.desc && (
            <Typography
              component="span"
              variant="caption"
              color="text.secondary"
              noWrap
              sx={{ maxWidth: "45%", flexShrink: 0 }}
            >
              {item.desc}
            </Typography>
          )}
        </Box>
      ))}
      <Box
        sx={{
          display: "flex",
          gap: 1.5,
          px: 1.25,
          pt: 0.5,
          mt: 0.25,
          borderTop: 1,
          borderColor: "divider",
        }}
      >
        <Typography variant="caption" color="text.disabled">
          {tr("↑↓ select")}
        </Typography>
        <Typography variant="caption" color="text.disabled">
          {tr("Tab insert")}
        </Typography>
        <Typography variant="caption" color="text.disabled">
          {tr("Esc dismiss")}
        </Typography>
      </Box>
    </Box>
  );
}
