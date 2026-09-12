import { Box, Button, Divider, Link, Stack, Typography } from "@mui/material";
import { useMutation } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { LogoMark } from "@/components/Logo";
import { useSnackbar } from "@/components/Snackbar";
import { PendingForm, SignInForm, pendingTitle, usePendingLogin } from "@/account/SignIn";
import { useAccount, useSaveSettings } from "@/ipc/hooks";
import { errorMessage, type LoginOutcome, type Settings } from "@/ipc/types";
import { sizes } from "@/theme/theme";
import { WindowControls } from "@/app/WindowControls";

const REPO_URL = "https://github.com/rep0rtDev/termoso";

/**
 * First thing shown while nobody is signed in: sign in to the free cloud, to
 * your own server, or carry on offline — everything works without an account.
 */
export function WelcomeScreen({ settings }: { settings: Settings }) {
  const status = useAccount();
  const snackbar = useSnackbar();
  const save = useSaveSettings();
  const { pending, onOutcome: settle } = usePendingLogin(status);

  const dismiss = useMutation({
    mutationFn: () => save.mutateAsync({ ...settings, welcomeSeen: true }),
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const onOutcome = (o: LoginOutcome | null) => {
    if (o?.step === "done") dismiss.mutate();
    settle(o);
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
          overflowY: "auto",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          px: 3,
          pb: 6,
        }}
      >
        <Stack spacing={3} sx={{ width: 400, maxWidth: "100%", userSelect: "text" }}>
          <Stack spacing={1.5} sx={{ alignItems: "center", textAlign: "center" }}>
            <LogoMark size={52} />
            <Typography variant="h5" sx={{ fontWeight: 600, letterSpacing: "-0.01em" }}>
              {pending ? pendingTitle(pending) : "Welcome to Termoso"}
            </Typography>
            {!pending && (
              <Typography variant="body2" color="text.secondary" sx={{ maxWidth: 340 }}>
                Sign in to keep hosts, keys and snippets in sync across your devices — encrypted
                before they leave this one.
              </Typography>
            )}
          </Stack>

          <Box sx={{ p: 3, borderRadius: 3, bgcolor: "surface.high" }}>
            {pending ? (
              <PendingForm key={pending.step} pending={pending} onOutcome={onOutcome} />
            ) : (
              <SignInForm autoFocus onOutcome={onOutcome} />
            )}
          </Box>

          {!pending && (
            <>
              <Divider>
                <Typography variant="caption" color="text.disabled">
                  or
                </Typography>
              </Divider>
              <Stack spacing={0.75} sx={{ alignItems: "center" }}>
                <Button
                  variant="tonal"
                  size="large"
                  fullWidth
                  disabled={dismiss.isPending}
                  onClick={() => dismiss.mutate()}
                >
                  Continue offline
                </Button>
                <Typography
                  variant="caption"
                  color="text.secondary"
                  sx={{ textAlign: "center", maxWidth: 340 }}
                >
                  No account needed. Everything stays in the encrypted vault on this device; you can
                  sign in later from the account menu.
                </Typography>
              </Stack>
            </>
          )}

          <Typography variant="caption" color="text.disabled" sx={{ textAlign: "center" }}>
            Free · Open source ·{" "}
            <Link
              component="button"
              type="button"
              color="inherit"
              underline="hover"
              onClick={() => void openUrl(REPO_URL)}
            >
              AGPL-3.0 on GitHub
            </Link>
            {" · "}No telemetry
          </Typography>
        </Stack>
      </Box>
    </Box>
  );
}
