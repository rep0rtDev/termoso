import { useColorScheme } from "@mui/material";
import { useSettings } from "@/ipc/hooks";
import { resolveTerminalTheme, type TerminalTheme } from "./themes";

/** The colour scheme terminals currently render with, for chrome around them. */
export function useTerminalTheme(): TerminalTheme {
  const { mode, systemMode } = useColorScheme();
  const { data } = useSettings();
  const scheme = (mode === "system" ? systemMode : mode) === "light" ? "light" : "dark";
  return resolveTerminalTheme(data?.terminalTheme, scheme);
}
