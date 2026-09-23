import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { Alert, Snackbar, type AlertColor } from "@mui/material";
import { tr } from "@/i18n";

interface Toast {
  message: string;
  severity: AlertColor;
}

interface SnackbarApi {
  notify: (message: string, severity?: AlertColor) => void;
  error: (message: string) => void;
}

const Ctx = createContext<SnackbarApi | null>(null);

let mounted: ((message: string, severity: AlertColor) => void) | null = null;

/** Show a toast from outside React (commands, event handlers). */
export function toast(message: string, severity: AlertColor = "success") {
  mounted?.(message, severity);
}

export function SnackbarProvider({ children }: { children: ReactNode }) {
  const [current, setCurrent] = useState<Toast | null>(null);
  const notify = useCallback((message: string, severity: AlertColor = "success") => {
    setCurrent({ message, severity });
  }, []);
  useEffect(() => {
    mounted = notify;
    return () => {
      mounted = null;
    };
  }, [notify]);
  const api = useMemo<SnackbarApi>(() => ({ notify, error: (m) => notify(m, "error") }), [notify]);
  return (
    <Ctx.Provider value={api}>
      {children}
      <Snackbar
        open={current !== null}
        autoHideDuration={4500}
        onClose={() => setCurrent(null)}
        anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
      >
        <Alert
          variant="filled"
          severity={current?.severity ?? "info"}
          onClose={() => setCurrent(null)}
          sx={{ minWidth: 280 }}
        >
          {current?.message}
        </Alert>
      </Snackbar>
    </Ctx.Provider>
  );
}

export function useSnackbar(): SnackbarApi {
  const api = useContext(Ctx);
  if (!api) throw new Error(tr("SnackbarProvider missing"));
  return api;
}
