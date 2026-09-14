import { Alert, AlertTitle, Button } from "@mui/material";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "@/api/client";
import { accountApi } from "@/api/endpoints";
import { queryKeys, useAccount } from "@/api/hooks";
import { useSnackbar } from "@/components/Snackbar";
import { formatDateTime, formatRelative } from "@/components/format";

/** Shown on every cabinet page while a destructive "start over" is pending. */
export function ResetScheduledBanner() {
  const account = useAccount();
  const qc = useQueryClient();
  const snack = useSnackbar();
  const cancel = useMutation({
    mutationFn: accountApi.cancelStartOver,
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: queryKeys.account });
      snack.notify("Account reset cancelled");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  const at = account.data?.user.reset_scheduled_for;
  if (!at) return null;
  return (
    <Alert
      severity="error"
      sx={{ mx: { xs: 2, sm: 3, md: 4 }, mt: 2 }}
      action={
        <Button
          color="inherit"
          size="small"
          onClick={() => cancel.mutate()}
          disabled={cancel.isPending}
        >
          Cancel reset
        </Button>
      }
    >
      <AlertTitle>An account reset is scheduled {formatRelative(at)}</AlertTitle>
      Someone requested to start this account over from scratch using the email address. Once it
      completes ({formatDateTime(at)}) the encrypted vault is deleted permanently and every device
      is signed out. If this was not you, cancel it now and review Devices and Security.
    </Alert>
  );
}
