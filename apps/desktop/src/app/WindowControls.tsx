import { Box, IconButton, Tooltip } from "@mui/material";
import MinimizeRoundedIcon from "@mui/icons-material/MinimizeRounded";
import CropSquareRoundedIcon from "@mui/icons-material/CropSquareRounded";
import FilterNoneRoundedIcon from "@mui/icons-material/FilterNoneRounded";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState, type ReactNode } from "react";
import { tr } from "@/i18n";

const win = getCurrentWindow();

/** Minimize / maximize / close for the undecorated main window. */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let alive = true;
    const refresh = () => {
      win
        .isMaximized()
        .then((m) => {
          if (alive) setMaximized(m);
        })
        .catch(() => undefined);
    };
    refresh();
    const unlisten = win.onResized(refresh);
    return () => {
      alive = false;
      unlisten.then((f) => f()).catch(() => undefined);
    };
  }, []);

  return (
    <Box sx={{ display: "flex", alignItems: "stretch", flexShrink: 0, ml: 0.5 }}>
      <WindowButton label={tr("Minimize")} onClick={() => void win.minimize()}>
        <MinimizeRoundedIcon sx={{ fontSize: 18, mt: "-6px" }} />
      </WindowButton>
      <WindowButton
        label={maximized ? tr("Restore") : tr("Maximize")}
        onClick={() => void win.toggleMaximize()}
      >
        {maximized ? (
          <FilterNoneRoundedIcon sx={{ fontSize: 13, transform: "scaleX(-1)" }} />
        ) : (
          <CropSquareRoundedIcon sx={{ fontSize: 15 }} />
        )}
      </WindowButton>
      <WindowButton label={tr("Close")} onClick={() => void win.close()} danger>
        <CloseRoundedIcon sx={{ fontSize: 17 }} />
      </WindowButton>
    </Box>
  );
}

function WindowButton({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <Tooltip title={label} enterDelay={600}>
      <IconButton
        onClick={onClick}
        aria-label={label}
        disableRipple
        sx={{
          width: 46,
          height: "100%",
          borderRadius: 0,
          color: "text.secondary",
          "&:hover": danger
            ? { bgcolor: "#C42B1C", color: "#fff" }
            : { bgcolor: "action.hover", color: "text.primary" },
          "&:active": danger ? { bgcolor: "#A5231A" } : { bgcolor: "action.selected" },
        }}
      >
        {children}
      </IconButton>
    </Tooltip>
  );
}
