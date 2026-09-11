import { useMemo } from "react";
import { CssBaseline, ThemeProvider } from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { theme } from "./theme/theme";
import { SnackbarProvider } from "./components/Snackbar";
import { AppShell } from "./app/AppShell";
import { ThemeModeSync } from "./app/ThemeModeSync";

export function App() {
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
