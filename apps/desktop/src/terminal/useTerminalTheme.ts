import { useColorScheme } from "@mui/material";
import { useSettings } from "@/ipc/hooks";
import type { Uuid } from "@/ipc/types";
import { resolveTerminalTheme, type TerminalTheme } from "./themes";
import { useTerminal } from "./store";

function useScheme(): "dark" | "light" {
  const { mode, systemMode } = useColorScheme();
  return (mode === "system" ? systemMode : mode) === "light" ? "light" : "dark";
}

/** The colour scheme terminals render with by default, for chrome around them. */
export function useTerminalTheme(): TerminalTheme {
  const scheme = useScheme();
  const { data } = useSettings();
  return resolveTerminalTheme(data?.terminalTheme, scheme);
}

/** Scheme id a pane renders with: tab override → host scheme → app setting. */
export function usePaneThemeId(paneId: Uuid): string {
  const { data } = useSettings();
  const hostTheme = useTerminal((s) => s.panes[paneId]?.hostTheme ?? null);
  const override = useTerminal(
    (s) => s.tabs.find((t) => t.paneIds.includes(paneId))?.themeOverride ?? null,
  );
  return override ?? hostTheme ?? data?.terminalTheme ?? "auto";
}

export function usePaneTheme(paneId: Uuid): TerminalTheme {
  const scheme = useScheme();
  return resolveTerminalTheme(usePaneThemeId(paneId), scheme);
}
