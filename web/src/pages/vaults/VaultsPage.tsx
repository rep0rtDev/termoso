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
import GroupsOutlinedIcon from "@mui/icons-material/GroupsOutlined";
import PersonOutlineRoundedIcon from "@mui/icons-material/PersonOutlineRounded";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { teamsApi, vaultsApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { Vault } from "@/api/types";
import { EmptyState } from "@/components/EmptyState";
import { IconTile } from "@/components/IconTile";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { RoleChip } from "@/components/RoleChip";
import { formatDate } from "@/components/format";
import { tr, trx } from "@/i18n";

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
          <CardContent sx={{ p: 2, "&:last-child": { pb: 2 } }}>
            <Stack
              direction="row"
              spacing={1.5}
              sx={{ alignItems: "center", flexWrap: "wrap", rowGap: 1 }}
            >
              <IconTile>
                {v.kind === "personal" ? <PersonOutlineRoundedIcon /> : <GroupsOutlinedIcon />}
              </IconTile>
              <Stack sx={{ minWidth: 140, flex: 1 }}>
                <Typography variant="body1" sx={{ fontWeight: 500 }} noWrap>
                  {v.name}
                </Typography>
                <Typography variant="body2" color="text.secondary" noWrap>
                  {trx("{v1}  · key v{key_version} · {v2}", {
                    v1:
                      v.kind === "personal"
                        ? tr("Personal vault")
                        : (teamName(v.team_id) ?? tr("Team vault")),
                    key_version: v.key_version,
                    v2: formatDate(v.created_at),
                  })}
                </Typography>
              </Stack>
              <Stack
                direction="row"
                spacing={0.5}
                sx={{ alignItems: "center", flexShrink: 0, ml: "auto" }}
              >
                {v.is_default && <Chip size="small" variant="outlined" label={tr("Default")} />}
                {!v.sealed_key && <Chip size="small" color="warning" label={tr("Key pending")} />}
                <RoleChip role={v.my_role} />
              </Stack>
            </Stack>
          </CardContent>
        </CardActionArea>
      </Card>
    </Grid>
  );

  return (
    <>
      <PageHeader
        title={tr("Vaults")}
        subtitle={tr(
          "Every host, key and snippet lives in a vault. Vault keys are sealed to member public keys — the server only stores ciphertext.",
        )}
      />
      {vaults.data.vaults.length === 0 ? (
        <EmptyState icon={<LockRoundedIcon fontSize="inherit" />} title={tr("No vaults")} />
      ) : (
        <Stack spacing={3}>
          <Grid container spacing={2}>
            {personal.map(card)}
          </Grid>
          {team.length > 0 && (
            <>
              <Typography variant="overline" color="text.secondary">
                {tr("Team vaults")}
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
