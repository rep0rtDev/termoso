import { useState, type SubmitEvent } from "react";
import { tr, trx } from "@/i18n";
import {
  Alert,
  Chip,
  Button,
  Card,
  CardActionArea,
  CardContent,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Grid,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { teamsApi } from "@/api/endpoints";
import { queryKeys, useServerInfo } from "@/api/hooks";
import { useAuthState } from "@/auth/store";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { RoleChip } from "@/components/RoleChip";
import { useSnackbar } from "@/components/Snackbar";
import { formatDate } from "@/components/format";
import { createDefaultTeamVault } from "@/vaults/keys";

export function TeamsPage() {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const info = useServerInfo();
  const snack = useSnackbar();
  const { session } = useAuthState();
  const teams = useQuery({ queryKey: queryKeys.teams, queryFn: teamsApi.list });
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const create = useMutation({
    mutationFn: async () => {
      const team = await teamsApi.create(name.trim());
      // The team's default vault is created client-side: its key is generated in
      // this tab and sealed to our public key, so the server only sees ciphertext.
      if (session) {
        try {
          await createDefaultTeamVault(team, {
            user_id: session.user.id,
            public_key: session.keys.public_key,
          });
        } catch (e) {
          snack.error(
            tr("Team created, but its default vault was not: {v1}", { v1: errorMessage(e) }),
          );
        }
      }
      return team;
    },
    onSuccess: async (team) => {
      setOpen(false);
      setName("");
      await qc.invalidateQueries({ queryKey: queryKeys.teams });
      await qc.invalidateQueries({ queryKey: queryKeys.vaults });
      void navigate(`/team/${team.id}`);
    },
  });

  if (teams.isPending) return <Loading />;
  if (teams.isError) return <Alert severity="error">{errorMessage(teams.error)}</Alert>;
  const teamsEnabled = info.data?.features.teams ?? true;

  return (
    <>
      <PageHeader
        title={tr("Team")}
        subtitle={tr(
          "Share vaults with teammates. Team data is end-to-end encrypted; the server only relays sealed keys.",
        )}
        actions={
          teamsEnabled && (
            <Button
              variant="contained"
              startIcon={<AddRoundedIcon />}
              onClick={() => setOpen(true)}
            >
              {tr("New team")}
            </Button>
          )
        }
      />
      {!teamsEnabled && teams.data.teams.length === 0 && (
        <Alert severity="info" sx={{ mb: 2 }}>
          {tr(
            "Team creation is disabled on this server. You can still join teams you are invited to.",
          )}
        </Alert>
      )}
      {teams.data.teams.length === 0 ? (
        <EmptyState
          icon={<GroupsRoundedIcon fontSize="inherit" />}
          title={tr("You are not in a team yet")}
          description={tr(
            "Create a team to share hosts, keys and snippets with colleagues, or accept an invitation link.",
          )}
          action={
            teamsEnabled && (
              <Button variant="contained" onClick={() => setOpen(true)}>
                {tr("Create a team")}
              </Button>
            )
          }
        />
      ) : (
        <Grid container spacing={2}>
          {teams.data.teams.map((t) => (
            <Grid key={t.id} size={{ xs: 12, sm: 6, md: 4 }}>
              <Card sx={{ height: "100%" }}>
                <CardActionArea
                  onClick={() => void navigate(`/team/${t.id}`)}
                  sx={{ height: "100%" }}
                >
                  <CardContent>
                    <Stack spacing={1.5}>
                      <Stack
                        direction="row"
                        spacing={1}
                        sx={{ alignItems: "center", justifyContent: "space-between" }}
                      >
                        <Typography variant="h6" noWrap>
                          {t.name}
                        </Typography>
                        <Stack
                          direction="row"
                          spacing={0.5}
                          sx={{ alignItems: "center", flexShrink: 0 }}
                        >
                          {info.data?.home_team?.id === t.id && (
                            <Chip size="small" variant="outlined" label={tr("Home team")} />
                          )}
                          <RoleChip role={t.my_role} />
                        </Stack>
                      </Stack>
                      <Typography variant="body2" color="text.secondary">
                        {trx("{member_count} {v1} · created  {v2}", {
                          member_count: t.member_count,
                          v1: t.member_count === 1 ? "member" : "members",
                          v2: formatDate(t.created_at),
                        })}
                      </Typography>
                    </Stack>
                  </CardContent>
                </CardActionArea>
              </Card>
            </Grid>
          ))}
        </Grid>
      )}

      <Dialog
        open={open}
        onClose={create.isPending ? undefined : () => setOpen(false)}
        maxWidth="xs"
        fullWidth
      >
        <form
          onSubmit={(e: SubmitEvent) => {
            e.preventDefault();
            create.mutate();
          }}
        >
          <DialogTitle>{tr("New team")}</DialogTitle>
          <DialogContent sx={{ display: "grid", gap: 2 }}>
            {create.isError && <Alert severity="error">{errorMessage(create.error)}</Alert>}
            <DialogContentText>
              {tr("You become the owner and can invite members afterwards.")}
            </DialogContentText>
            <TextField
              autoFocus
              label={tr("Team name")}
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={create.isPending}
            />
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button onClick={() => setOpen(false)} color="inherit" disabled={create.isPending}>
              {tr("Cancel")}
            </Button>
            <Button
              type="submit"
              variant="contained"
              disabled={create.isPending || name.trim() === ""}
            >
              {tr("Create")}
            </Button>
          </DialogActions>
        </form>
      </Dialog>
    </>
  );
}
