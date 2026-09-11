import { useEffect, useMemo } from "react";
import { CssBaseline, ThemeProvider } from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { theme } from "./theme/theme";
import { SnackbarProvider } from "./components/Snackbar";
import { AppShell } from "./app/AppShell";
import { ThemeModeSync } from "./app/ThemeModeSync";

/** The webview's native context menu (Back/Forward/Reload…) never makes sense in the app. */
function useSuppressNativeContextMenu() {
  useEffect(() => {
    if (import.meta.env.DEV) return;
    const onMenu = (e: globalThis.MouseEvent) => {
      const t = e.target;
      if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement) return;
      if (t instanceof HTMLElement && t.isContentEditable) return;
      e.preventDefault();
    };
    document.addEventListener("contextmenu", onMenu);
    return () => document.removeEventListener("contextmenu", onMenu);
  }, []);
}

export function App() {
  useSuppressNativeContextMenu();
  const queryClient = useMemo(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: { retry: false, staleTime: 10_000, refetchOnWindowFocus: false },
        },
      }),
    [],
  );
  return (
    <ThemeProvider theme={theme} defaultMode="dark" modeStorageKey="termoso.theme" noSsr>
      <CssBaseline enableColorScheme />
      <QueryClientProvider client={queryClient}>
        <SnackbarProvider>
          <ThemeModeSync />
          <AppShell />
        </SnackbarProvider>
      </QueryClientProvider>
    </ThemeProvider>
  );
}
