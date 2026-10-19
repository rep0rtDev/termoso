import { useState } from "react";
import {
  Alert,
  Card,
  CardActionArea,
  CardContent,
  Chip,
  Grid,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TablePagination,
  TableRow,
  TextField,
  Typography,
} from "@mui/material";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import GroupsOutlinedIcon from "@mui/icons-material/GroupsOutlined";
import PersonOutlineRoundedIcon from "@mui/icons-material/PersonOutlineRounded";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { teamsApi, vaultsApi } from "@/api/endpoints";
import { queryKeys, useServerInfo } from "@/api/hooks";
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
  const info = useServerInfo();
  const [page, setPage] = useState(0);
  const [limit, setLimit] = useState(10);
  const [filter, setFilter] = useState("");
  const vaults = useQuery({ queryKey: queryKeys.vaults, queryFn: vaultsApi.list });
  const teams = useQuery({ queryKey: queryKeys.teams, queryFn: teamsApi.list });
  if (vaults.isPending) return <Loading />;
  if (vaults.isError) return <Alert severity="error">{errorMessage(vaults.error)}</Alert>;

  const teamName = (id?: string) => teams.data?.teams.find((t) => t.id === id)?.name;
  const personal =
    info.data?.features.personal_vaults === false
      ? []
      : vaults.data.vaults.filter((v) => v.kind === "personal");
  const team = vaults.data.vaults.filter((v) => v.kind === "team");
  const needle = filter.trim().toLowerCase();
  const filtered = needle
    ? team.filter(
        (v) =>
          v.name.toLowerCase().includes(needle) ||
          (teamName(v.team_id) ?? "").toLowerCase().includes(needle),
      )
    : team;
  const pageRows = filtered.slice(page * limit, page * limit + limit);

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
            <Card>
              <CardContent sx={{ p: 2, "&:last-child": { pb: 1 } }}>
                <Stack
                  direction={{ xs: "column", sm: "row" }}
                  spacing={1}
                  sx={{ alignItems: { sm: "center" }, justifyContent: "space-between", mb: 1.5 }}
                >
                  <Typography variant="overline" color="text.secondary">
                    {trx("Team vaults · {v1}", { v1: team.length })}
                  </Typography>
                  {team.length > 5 && (
                    <TextField
                      size="small"
                      placeholder={tr("Filter by vault or team")}
                      value={filter}
                      onChange={(e) => {
                        setFilter(e.target.value);
                        setPage(0);
                      }}
                      sx={{ width: { xs: "100%", sm: 260 } }}
                    />
                  )}
                </Stack>
                <TableContainer>
                  <Table size="small">
                    <TableHead>
                      <TableRow>
                        <TableCell>{tr("Vault")}</TableCell>
                        <TableCell>{tr("Team")}</TableCell>
                        <TableCell>{tr("Role")}</TableCell>
                        <TableCell sx={{ display: { xs: "none", md: "table-cell" } }} align="right">
                          {tr("Key")}
                        </TableCell>
                        <TableCell sx={{ display: { xs: "none", sm: "table-cell" } }}>
                          {tr("Created")}
                        </TableCell>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {pageRows.map((v) => (
                        <TableRow
                          key={v.id}
                          hover
                          onClick={() => void navigate(`/vaults/${v.id}`)}
                          sx={{ cursor: "pointer" }}
                        >
                          <TableCell sx={{ fontWeight: 500 }}>
                            <Stack direction="row" spacing={0.75} sx={{ alignItems: "center" }}>
                              <span>{v.name}</span>
                              {v.is_default && (
                                <Chip size="small" variant="outlined" label={tr("Default")} />
                              )}
                              {!v.sealed_key && (
                                <Chip size="small" color="warning" label={tr("Key pending")} />
                              )}
                            </Stack>
                          </TableCell>
                          <TableCell>{teamName(v.team_id) ?? "—"}</TableCell>
                          <TableCell>
                            <RoleChip role={v.my_role} />
                          </TableCell>
                          <TableCell
                            sx={{ display: { xs: "none", md: "table-cell" } }}
                            align="right"
                          >
                            v{v.key_version}
                          </TableCell>
                          <TableCell sx={{ display: { xs: "none", sm: "table-cell" } }}>
                            {formatDate(v.created_at)}
                          </TableCell>
                        </TableRow>
                      ))}
                      {pageRows.length === 0 && (
                        <TableRow>
                          <TableCell colSpan={5}>
                            <Typography variant="body2" color="text.secondary">
                              {tr("No vaults match")}
                            </Typography>
                          </TableCell>
                        </TableRow>
                      )}
                    </TableBody>
                  </Table>
                </TableContainer>
                {filtered.length > 10 && (
                  <TablePagination
                    component="div"
                    count={filtered.length}
                    page={page}
                    onPageChange={(_e, p) => setPage(p)}
                    rowsPerPage={limit}
                    onRowsPerPageChange={(e) => {
                      setLimit(Number(e.target.value));
                      setPage(0);
                    }}
                    rowsPerPageOptions={[10, 25, 50]}
                  />
                )}
              </CardContent>
            </Card>
          )}
        </Stack>
      )}
    </>
  );
}
