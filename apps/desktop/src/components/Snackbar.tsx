import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import { Alert, Snackbar, type AlertColor } from "@mui/material";

interface Toast {
  message: string;
  severity: AlertColor;
}

interface SnackbarApi {
  notify: (message: string, severity?: AlertColor) => void;
  error: (message: string) => void;
}

const Ctx = createContext<SnackbarApi | null>(null);

export function SnackbarProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<Toast | null>(null);
  const notify = useCallback((message: string, severity: AlertColor = "success") => {
    setToast({ message, severity });
  }, []);
  const api = useMemo<SnackbarApi>(() => ({ notify, error: (m) => notify(m, "error") }), [notify]);
  return (
    <Ctx.Provider value={api}>
      {children}
      <Snackbar
        open={toast !== null}
        autoHideDuration={4500}
        onClose={() => setToast(null)}
        anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
      >
        <Alert
          variant="filled"
          severity={toast?.severity ?? "info"}
          onClose={() => setToast(null)}
          sx={{ minWidth: 280 }}
        >
          {toast?.message}
        </Alert>
      </Snackbar>
    </Ctx.Provider>
  );
}

export function useSnackbar(): SnackbarApi {
  const api = useContext(Ctx);
  if (!api) throw new Error("SnackbarProvider missing");
  return api;
}
