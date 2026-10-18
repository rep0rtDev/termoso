import { useEffect, useRef } from "react";
import Box from "@mui/material/Box";
import { useColorScheme } from "@mui/material/styles";

declare global {
  interface Window {
    turnstile?: {
      render(el: HTMLElement, opts: Record<string, unknown>): string;
      remove(id: string): void;
    };
  }
}

const SCRIPT = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";
let loading: Promise<void> | null = null;

function loadScript(): Promise<void> {
  if (window.turnstile) return Promise.resolve();
  loading ??= new Promise((resolve, reject) => {
    const s = document.createElement("script");
    s.src = SCRIPT;
    s.async = true;
    s.onload = () => resolve();
    s.onerror = () => {
      loading = null;
      reject(new Error("Turnstile failed to load"));
    };
    document.head.appendChild(s);
  });
  return loading;
}

/**
 * Cloudflare Turnstile widget. Calls `onToken` with a fresh token (or `null`
 * when it expires or fails). Tokens are single-use: remount the widget
 * (change `key`) after a request that consumed one.
 */
export function Turnstile({
  siteKey,
  onToken,
}: {
  siteKey: string;
  onToken: (token: string | null) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const cb = useRef(onToken);
  const { mode, systemMode } = useColorScheme();
  const theme = (mode === "system" ? systemMode : mode) ?? "auto";
  useEffect(() => {
    cb.current = onToken;
  }, [onToken]);
  useEffect(() => {
    let id: string | undefined;
    let cancelled = false;
    loadScript()
      .then(() => {
        const el = ref.current;
        if (cancelled || !el || !window.turnstile) return;
        id = window.turnstile.render(el, {
          sitekey: siteKey,
          theme,
          callback: (t: string) => cb.current(t),
          "expired-callback": () => cb.current(null),
          "error-callback": () => cb.current(null),
        });
      })
      .catch(() => cb.current(null));
    return () => {
      cancelled = true;
      if (id) window.turnstile?.remove(id);
    };
  }, [siteKey, theme]);
  return <Box ref={ref} sx={{ minHeight: 65, display: "flex", justifyContent: "center" }} />;
}
