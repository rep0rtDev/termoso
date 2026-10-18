import { useState } from "react";
import { tr, trx } from "@/i18n";
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
        <AuthTitle title={tr("Invitation unavailable")} />
        <Alert severity="error">{errorMessage(preview.error)}</Alert>
        <Button sx={{ mt: 2 }} variant="contained" onClick={() => navigate("/login")}>
          {tr("Go to sign in")}
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
            {trx("{inviter} invited {v1} to the team as  {v2}.", {
              inviter: inv.inviter,
              v1: <b>{inv.email}</b>,
              v2: <Chip size="small" label={inv.role} sx={{ verticalAlign: "middle" }} />,
            })}
          </>
        }
      />
      <Stack spacing={2}>
        {error && <Alert severity="error">{error}</Alert>}
        {session ? (
          emailMismatch ? (
            <Alert severity="warning">
              {trx(
                "You are signed in as {email}, but this invitation is for {email1}. Sign out and use the invited account to accept it.",
                { email: session.user.email, email1: inv.email },
              )}
            </Alert>
          ) : (
            <Button
              variant="contained"
              size="large"
              onClick={() => accept.mutate()}
              disabled={accept.isPending}
            >
              {tr("Accept invitation")}
            </Button>
          )
        ) : inv.account_exists ? (
          <>
            <Typography variant="body2" color="text.secondary">
              {trx("Sign in with {email} to accept.", { email: inv.email })}
            </Typography>
            <Button
              variant="contained"
              size="large"
              onClick={() => navigate(`/login?next=${encodeURIComponent(`/invite/${token}`)}`)}
            >
              {tr("Sign in")}
            </Button>
          </>
        ) : (
          <>
            <Typography variant="body2" color="text.secondary">
              {trx("Create a Termoso account for {email} to join the team.", { email: inv.email })}
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
              {tr("Create account")}
            </Button>
            <Button
              color="inherit"
              onClick={() => navigate(`/login?next=${encodeURIComponent(`/invite/${token}`)}`)}
            >
              {tr("I already have an account")}
            </Button>
          </>
        )}
        <Divider>{tr("or")}</Divider>
        <Button
          color="inherit"
          href={appInviteLink(token)}
          endIcon={<OpenInNewRoundedIcon />}
          data-testid="open-in-app"
        >
          {tr("Open in the Termoso app")}
        </Button>
      </Stack>
    </>
  );
}
