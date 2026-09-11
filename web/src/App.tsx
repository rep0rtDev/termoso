import { useMemo } from "react";
import { CssBaseline, ThemeProvider } from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router";
import { theme } from "./theme/theme";
import { router } from "./router";
import { configureClient, ApiError } from "./api/client";
import { authStore } from "./auth/store";
import { SnackbarProvider } from "./components/Snackbar";

configureClient({
  token: authStore.token,
  onUnauthorized: () => authStore.signOut(),
});

export function App() {
  const queryClient = useMemo(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            retry: (count, err) => !(err instanceof ApiError) && count < 2,
            staleTime: 15_000,
            refetchOnWindowFocus: false,
          },
        },
      }),
    [],
  );
  return (
    <ThemeProvider theme={theme} defaultMode="dark" modeStorageKey="termoso.theme" noSsr>
      <CssBaseline enableColorScheme />
      <QueryClientProvider client={queryClient}>
        <SnackbarProvider>
          <RouterProvider router={router} />
        </SnackbarProvider>
      </QueryClientProvider>
    </ThemeProvider>
  );
}
