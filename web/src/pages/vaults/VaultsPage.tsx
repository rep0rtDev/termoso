import {
  Alert,
  Card,
  CardActionArea,
  CardContent,
  Chip,
  Grid,
  Stack,
  Typography,
} from "@mui/material";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { teamsApi, vaultsApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { Vault } from "@/api/types";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { RoleChip } from "@/components/RoleChip";
import { formatDate } from "@/components/format";

export function VaultsPage() {
  const navigate = useNavigate();
  const vaults = useQuery({ queryKey: queryKeys.vaults, queryFn: vaultsApi.list });
  const teams = useQuery({ queryKey: queryKeys.teams, queryFn: teamsApi.list });
  if (vaults.isPending) return <Loading />;
  if (vaults.isError) return <Alert severity="error">{errorMessage(vaults.error)}</Alert>;

  const teamName = (id?: string) => teams.data?.teams.find((t) => t.id === id)?.name;
  const personal = vaults.data.vaults.filter((v) => v.kind === "personal");
  const team = vaults.data.vaults.filter((v) => v.kind === "team");

  const card = (v: Vault) => (
    <Grid key={v.id} size={{ xs: 12, sm: 6, md: 4 }}>
      <Card sx={{ height: "100%" }}>
        <CardActionArea onClick={() => void navigate(`/vaults/${v.id}`)} sx={{ height: "100%" }}>
          <CardContent>
            <Stack spacing={1.5}>
              <Stack
                direction="row"
                spacing={1}
                sx={{ alignItems: "center", justifyContent: "space-between" }}
              >
                <Stack direction="row" spacing={1} sx={{ alignItems: "center", minWidth: 0 }}>
                  {v.kind === "personal" ? (
                    <PersonRoundedIcon color="primary" />
                  ) : (
                    <GroupsRoundedIcon color="secondary" />
                  )}
                  <Typography variant="h6" noWrap>
                    {v.name}
                  </Typography>
                </Stack>
                <RoleChip role={v.my_role} />
              </Stack>
              <Typography variant="body2" color="text.secondary" noWrap>
                {v.kind === "personal" ? "Personal vault" : (teamName(v.team_id) ?? "Team vault")} ·
                key v{v.key_version} · {formatDate(v.created_at)}
              </Typography>
              {!v.sealed_key && (
                <Chip
                  size="small"
                  color="warning"
                  variant="outlined"
                  label="Key pending"
                  sx={{ alignSelf: "flex-start" }}
                />
              )}
            </Stack>
          </CardContent>
        </CardActionArea>
      </Card>
    </Grid>
  );

  return (
    <>
      <PageHeader
        title="Vaults"
        subtitle="Every host, key and snippet lives in a vault. Vault keys are sealed to member public keys — the server only stores ciphertext."
      />
      {vaults.data.vaults.length === 0 ? (
        <EmptyState icon={<LockRoundedIcon fontSize="inherit" />} title="No vaults" />
      ) : (
        <Stack spacing={3}>
          <Grid container spacing={2}>
            {personal.map(card)}
          </Grid>
          {team.length > 0 && (
            <>
              <Typography variant="overline" color="text.secondary">
                Team vaults
              </Typography>
              <Grid container spacing={2}>
                {team.map(card)}
              </Grid>
            </>
          )}
        </Stack>
      )}
    </>
  );
}
