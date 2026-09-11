import { useEffect } from "react";
import { useColorScheme } from "@mui/material";
import { useSettings } from "@/ipc/hooks";
import { applyTerminalScheme, applyTerminalSettings } from "@/terminal/store";

/** Keeps MUI's color scheme and the terminals in step with the settings stored by Rust. */
export function ThemeModeSync() {
  const { setMode, mode, systemMode } = useColorScheme();
  const { data } = useSettings();
  useEffect(() => {
    if (data) {
      setMode(data.theme);
      applyTerminalSettings(data);
    }
  }, [data, setMode]);
  useEffect(() => {
    const resolved = mode === "system" ? systemMode : mode;
    applyTerminalScheme(resolved === "light" ? "light" : "dark");
  }, [mode, systemMode]);
  return null;
}
