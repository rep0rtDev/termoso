import { useState } from "react";
import { Alert, Button, Chip, Divider, Stack, Typography } from "@mui/material";
import OpenInNewRoundedIcon from "@mui/icons-material/OpenInNewRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Navigate, useNavigate, useParams } from "react-router";
import { errorMessage } from "@/api/client";
import { teamsApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import { useAuthState } from "@/auth/store";
import { Loading } from "@/components/Loading";
import { AuthTitle } from "./common";
import { appInviteLink } from "./appLinks";

export function InvitePage() {
  const { token = "" } = useParams();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const { session } = useAuthState();
  const [error, setError] = useState<string | null>(null);

  const preview = useQuery({
    queryKey: ["invite", token],
    queryFn: () => teamsApi.invitePreview(token),
    enabled: token.length > 0,
    retry: false,
  });

  const accept = useMutation({
    mutationFn: () => teamsApi.acceptInvite(token),
    onSuccess: async (team) => {
      await qc.invalidateQueries({ queryKey: queryKeys.teams });
      await qc.invalidateQueries({ queryKey: queryKeys.vaults });
      void navigate(`/team/${team.id}`, { replace: true });
    },
    onError: (e) => setError(errorMessage(e)),
  });

  if (!token) return <Navigate to="/login" replace />;
  if (preview.isPending) return <Loading />;
  if (preview.isError) {
    return (
      <>
        <AuthTitle title="Invitation unavailable" />
        <Alert severity="error">{errorMessage(preview.error)}</Alert>
        <Button sx={{ mt: 2 }} variant="contained" onClick={() => navigate("/login")}>
          Go to sign in
        </Button>
      </>
    );
  }

  const inv = preview.data;
  const emailMismatch =
    session !== null && session.user.email.toLowerCase() !== inv.email.toLowerCase();

  return (
    <>
      <AuthTitle
        title={`Join ${inv.team_name}`}
        subtitle={
          <>
            {inv.inviter} invited <b>{inv.email}</b> to the team as{" "}
            <Chip size="small" label={inv.role} sx={{ verticalAlign: "middle" }} />.
          </>
        }
      />
      <Stack spacing={2}>
        {error && <Alert severity="error">{error}</Alert>}
        {session ? (
          emailMismatch ? (
            <Alert severity="warning">
              You are signed in as {session.user.email}, but this invitation is for {inv.email}.
              Sign out and use the invited account to accept it.
            </Alert>
          ) : (
            <Button
              variant="contained"
              size="large"
              onClick={() => accept.mutate()}
              disabled={accept.isPending}
            >
              Accept invitation
            </Button>
          )
        ) : inv.account_exists ? (
          <>
            <Typography variant="body2" color="text.secondary">
              Sign in with {inv.email} to accept.
            </Typography>
            <Button
              variant="contained"
              size="large"
              onClick={() => navigate(`/login?next=${encodeURIComponent(`/invite/${token}`)}`)}
            >
              Sign in
            </Button>
          </>
        ) : (
          <>
            <Typography variant="body2" color="text.secondary">
              Create a Termoso account for {inv.email} to join the team.
            </Typography>
            <Button
              variant="contained"
              size="large"
              onClick={() =>
                void navigate(`/signup?next=${encodeURIComponent(`/invite/${token}`)}`, {
                  state: { inviteToken: token, lockedEmail: inv.email },
                })
              }
            >
              Create account
            </Button>
            <Button
              color="inherit"
              onClick={() => navigate(`/login?next=${encodeURIComponent(`/invite/${token}`)}`)}
            >
              I already have an account
            </Button>
          </>
        )}
        <Divider>or</Divider>
        <Button
          color="inherit"
          href={appInviteLink(token)}
          endIcon={<OpenInNewRoundedIcon />}
          data-testid="open-in-app"
        >
          Open in the Termoso app
        </Button>
      </Stack>
    </>
  );
}
