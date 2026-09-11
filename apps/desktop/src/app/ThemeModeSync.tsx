import { useEffect } from "react";
import { useColorScheme } from "@mui/material";
import { useSettings } from "@/ipc/hooks";

/** Keeps MUI's color scheme in step with the `theme` setting stored by Rust. */
export function ThemeModeSync() {
  const { setMode } = useColorScheme();
  const { data } = useSettings();
  useEffect(() => {
    if (data) setMode(data.theme);
  }, [data, setMode]);
  return null;
}
