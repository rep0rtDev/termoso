import { useState, type SubmitEvent } from "react";
import {
  Alert,
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
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { RoleChip } from "@/components/RoleChip";
import { formatDate } from "@/components/format";

export function TeamsPage() {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const info = useServerInfo();
  const teams = useQuery({ queryKey: queryKeys.teams, queryFn: teamsApi.list });
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const create = useMutation({
    mutationFn: () => teamsApi.create(name.trim()),
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
        title="Team"
        subtitle="Share vaults with teammates. Team data is end-to-end encrypted; the server only relays sealed keys."
        actions={
          teamsEnabled && (
            <Button
              variant="contained"
              startIcon={<AddRoundedIcon />}
              onClick={() => setOpen(true)}
            >
              New team
            </Button>
          )
        }
      />
      {!teamsEnabled && teams.data.teams.length === 0 && (
        <Alert severity="info" sx={{ mb: 2 }}>
          Team creation is disabled on this server. You can still join teams you are invited to.
        </Alert>
      )}
      {teams.data.teams.length === 0 ? (
        <EmptyState
          icon={<GroupsRoundedIcon fontSize="inherit" />}
          title="You are not in a team yet"
          description="Create a team to share hosts, keys and snippets with colleagues, or accept an invitation link."
          action={
            teamsEnabled && (
              <Button variant="contained" onClick={() => setOpen(true)}>
                Create a team
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
                        <RoleChip role={t.my_role} />
                      </Stack>
                      <Typography variant="body2" color="text.secondary">
                        {t.member_count} {t.member_count === 1 ? "member" : "members"} · created{" "}
                        {formatDate(t.created_at)}
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
          <DialogTitle>New team</DialogTitle>
          <DialogContent sx={{ display: "grid", gap: 2 }}>
            {create.isError && <Alert severity="error">{errorMessage(create.error)}</Alert>}
            <DialogContentText>
              You become the owner and can invite members afterwards.
            </DialogContentText>
            <TextField
              autoFocus
              label="Team name"
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={create.isPending}
            />
          </DialogContent>
          <DialogActions sx={{ px: 3, pb: 2 }}>
            <Button onClick={() => setOpen(false)} color="inherit" disabled={create.isPending}>
              Cancel
            </Button>
            <Button
              type="submit"
              variant="contained"
              disabled={create.isPending || name.trim() === ""}
            >
              Create
            </Button>
          </DialogActions>
        </form>
      </Dialog>
    </>
  );
}
