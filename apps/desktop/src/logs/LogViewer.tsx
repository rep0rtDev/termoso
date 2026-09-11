import { useEffect, useRef } from "react";
import { Box, useColorScheme } from "@mui/material";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { monoFontFamily } from "@/theme/theme";
import { terminalThemes } from "@/terminal/xtermTheme";
import type { Settings } from "@/ipc/types";

export interface ViewerHandle {
  /** Line index at the top of the viewport (into scrollback + screen). */
  topLine: () => number;
  scrollTo: (line: number) => void;
}

/** Read-only xterm replaying a recorded session. */
export function LogViewer({
  text,
  cols,
  settings,
  onReady,
}: {
  text: string;
  cols: number;
  settings: Settings;
  onReady: (h: ViewerHandle) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const { mode, systemMode } = useColorScheme();
  const scheme = (mode === "system" ? systemMode : mode) === "light" ? "light" : "dark";

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const font = settings.terminalFontFamily.trim();
    const term = new Terminal({
      disableStdin: true,
      convertEol: false,
      cursorBlink: false,
      cursorStyle: "underline",
      cursorInactiveStyle: "none",
      scrollback: 200_000,
      cols,
      fontSize: settings.terminalFontSize,
      fontFamily: font ? `'${font}', ${monoFontFamily}` : monoFontFamily,
      theme: terminalThemes[scheme],
      allowProposedApi: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";
    term.open(el);
    // Keep recorded width so line wrapping matches the original session; only adapt rows.
    const rows = () => Math.max(2, fit.proposeDimensions()?.rows ?? term.rows);
    term.resize(cols, rows());
    term.write(text, () => term.scrollToTop());
    const ro = new ResizeObserver(() => term.resize(cols, rows()));
    ro.observe(el);
    onReady({
      topLine: () => term.buffer.active.viewportY,
      scrollTo: (line) => term.scrollToLine(line),
    });
    return () => {
      ro.disconnect();
      term.dispose();
    };
  }, [text, cols, settings.terminalFontFamily, settings.terminalFontSize, scheme, onReady]);

  return (
    <Box
      ref={host}
      sx={{
        flex: 1,
        minHeight: 0,
        minWidth: 0,
        overflow: "hidden",
        "& .xterm": { height: "100%", p: 1 },
        "& .xterm-viewport": { overflowY: "auto !important" },
      }}
    />
  );
}
