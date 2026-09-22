import { useState, type SyntheticEvent } from "react";
import { Box, Button, Link, Stack, TextField, Typography } from "@mui/material";
import { useMutation } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { LogoMark } from "@/components/Logo";
import * as ipc from "@/ipc/commands";
import { errorMessage, isDesktopError } from "@/ipc/types";
import { sizes } from "@/theme/theme";
import { WindowControls } from "./WindowControls";

export const RECOVERY_DOC_URL =
  "https://github.com/rep0rtDev/termoso/blob/main/docs/ARCHITECTURE.md#desktop-master-password-and-app-lock";

/**
 * Shown while the vault is locked: at start-up of a password-protected
 * profile, after "Lock now" and after the inactivity timer. Nothing behind it
 * is mounted — the store is closed, so there is nothing to show.
 */
export function LockScreen() {
  const [password, setPassword] = useState("");
  const unlock = useMutation({
    mutationFn: (pw: string) => ipc.vaultUnlock(pw),
    onSettled: () => setPassword(""),
  });
  const wrong = isDesktopError(unlock.error) && unlock.error.kind === "wrong_password";
  const submit = (e: SyntheticEvent) => {
    e.preventDefault();
    if (password.length === 0 || unlock.isPending) return;
    unlock.mutate(password);
  };

  return (
    <Box
      sx={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
        bgcolor: "surface.lowest",
        userSelect: "none",
      }}
    >
      <Box
        data-tauri-drag-region
        sx={{ display: "flex", alignItems: "stretch", height: sizes.topbar, flexShrink: 0 }}
      >
        <Box sx={{ flex: 1 }} data-tauri-drag-region />
        <WindowControls />
      </Box>

      <Box
        data-tauri-drag-region
        sx={{
          flex: 1,
          minHeight: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          px: 3,
          pb: 6,
        }}
      >
        <Stack
          component="form"
          onSubmit={submit}
          spacing={3}
          sx={{ width: 360, maxWidth: "100%", userSelect: "text" }}
        >
          <Stack spacing={1.5} sx={{ alignItems: "center", textAlign: "center" }}>
            <LogoMark size={52} />
            <Typography variant="h5" sx={{ fontWeight: 600, letterSpacing: "-0.01em" }}>
              Vault locked
            </Typography>
            <Typography variant="body2" color="text.secondary">
              Enter your master password to open hosts, keys and snippets.
            </Typography>
          </Stack>

          <Box sx={{ p: 3, borderRadius: 3, bgcolor: "surface.high" }}>
            <Stack spacing={2}>
              <TextField
                autoFocus
                fullWidth
                type="password"
                label="Master password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                error={unlock.isError}
                helperText={
                  unlock.isError ? (wrong ? "Wrong password" : errorMessage(unlock.error)) : " "
                }
                disabled={unlock.isPending}
                slotProps={{ htmlInput: { autoComplete: "current-password", spellCheck: false } }}
              />
              <Button
                type="submit"
                variant="contained"
                fullWidth
                disabled={password.length === 0 || unlock.isPending}
              >
                {unlock.isPending ? "Unlocking…" : "Unlock"}
              </Button>
            </Stack>
          </Box>

          <Typography variant="caption" color="text.secondary" sx={{ textAlign: "center" }}>
            Forgot it? The password cannot be reset — restore an encrypted vault backup instead.{" "}
            <Link
              component="button"
              type="button"
              variant="caption"
              onClick={() => void openUrl(RECOVERY_DOC_URL)}
            >
              Learn more
            </Link>
          </Typography>
        </Stack>
      </Box>
    </Box>
  );
}
