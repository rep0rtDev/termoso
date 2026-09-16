import { Box } from "@mui/material";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState, type ReactNode } from "react";
import { IS_MAC } from "@/lib/platform";

const win = getCurrentWindow();

export const WINDOW_RADIUS = 10;

/** True while the window is maximized or fullscreen — corners are square then. */
export function useWindowSquare(): boolean {
  const [square, setSquare] = useState(false);
  useEffect(() => {
    let alive = true;
    const refresh = () => {
      Promise.all([win.isMaximized(), win.isFullscreen()])
        .then(([m, f]) => {
          if (alive) setSquare(m || f);
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
  return square;
}

/**
 * On Linux / Windows the window is undecorated and transparent; this draws
 * the actual rounded frame (Termius-style). `body` is clipped with the same
 * radius via `--window-radius` (see app.css) so portals — dialogs, menus,
 * backdrops — never paint over the rounded corners. macOS keeps its native
 * frame (rounded corners, shadow, traffic lights), so nothing is drawn there.
 */
export function WindowFrame({ children }: { children: ReactNode }) {
  const square = useWindowSquare() || IS_MAC;
  const radius = square ? 0 : WINDOW_RADIUS;
  useEffect(() => {
    document.documentElement.style.setProperty("--window-radius", `${radius}px`);
  }, [radius]);
  return (
    <Box
      sx={{
        height: "100%",
        bgcolor: "background.default",
        borderRadius: `${radius}px`,
        boxShadow: square ? "none" : "inset 0 0 0 1px var(--mui-palette-divider)",
      }}
    >
      {children}
    </Box>
  );
}
