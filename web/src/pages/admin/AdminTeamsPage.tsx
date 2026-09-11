import { useEffect, useState } from "react";
import {
  Alert,
  Box,
  IconButton,
  InputAdornment,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TablePagination,
  TableRow,
  TextField,
  Tooltip,
} from "@mui/material";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "@/api/client";
import { adminApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { AdminTeam } from "@/api/types";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { Section } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { formatDateTime } from "@/components/format";

export function AdminTeamsPage() {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [search, setSearch] = useState("");
  const [q, setQ] = useState("");
  useEffect(() => {
    const t = window.setTimeout(() => setQ(search.trim()), 300);
    return () => window.clearTimeout(t);
  }, [search]);
  const [page, setPage] = useState(0);
  const [limit, setLimit] = useState(25);
  const offset = page * limit;
  const teams = useQuery({
    queryKey: queryKeys.adminTeams(q, offset, limit),
    queryFn: () => adminApi.teams({ q, offset, limit }),
    placeholderData: keepPreviousData,
  });
  const [deleting, setDeleting] = useState<AdminTeam | null>(null);
  const del = useMutation({
    mutationFn: (id: string) => adminApi.deleteTeam(id),
    onSuccess: async () => {
      setDeleting(null);
      await qc.invalidateQueries({ queryKey: ["admin", "teams"] });
      await qc.invalidateQueries({ queryKey: queryKeys.adminStats });
      snack.notify("Team deleted");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  return (
    <>
      <PageHeader
        title="Teams"
        subtitle="All teams on this server. Deleting a team removes its vaults and every record in them."
        actions={
          <TextField
            size="small"
            placeholder="Search by team or owner"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setPage(0);
            }}
            slotProps={{
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchRoundedIcon fontSize="small" />
                  </InputAdornment>
                ),
              },
            }}
            sx={{ minWidth: 280 }}
          />
        }
      />
      <Section
        title={teams.data ? `${teams.data.total.toLocaleString()} teams` : "Teams"}
        disablePadding
      >
        {teams.isPending ? (
          <Loading />
        ) : teams.isError ? (
          <Box sx={{ p: 3 }}>
            <Alert severity="error">{errorMessage(teams.error)}</Alert>
          </Box>
        ) : teams.data.items.length === 0 ? (
          <EmptyState title="No teams match" />
        ) : (
          <>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>Team</TableCell>
                  <TableCell>Owner</TableCell>
                  <TableCell align="right">Members</TableCell>
                  <TableCell align="right">Vaults</TableCell>
                  <TableCell>Created</TableCell>
                  <TableCell align="right" />
                </TableRow>
              </TableHead>
              <TableBody>
                {teams.data.items.map((t) => (
                  <TableRow key={t.id} hover>
                    <TableCell sx={{ fontWeight: 600 }}>{t.name}</TableCell>
                    <TableCell>{t.owner_email}</TableCell>
                    <TableCell align="right">{t.member_count}</TableCell>
                    <TableCell align="right">{t.vault_count}</TableCell>
                    <TableCell>{formatDateTime(t.created_at)}</TableCell>
                    <TableCell align="right">
                      <Tooltip title="Delete team">
                        <IconButton
                          size="small"
                          onClick={() => setDeleting(t)}
                          aria-label="Delete team"
                        >
                          <DeleteOutlineRoundedIcon fontSize="small" />
                        </IconButton>
                      </Tooltip>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            <TablePagination
              component="div"
              count={teams.data.total}
              page={page}
              onPageChange={(_e, p) => setPage(p)}
              rowsPerPage={limit}
              onRowsPerPageChange={(e) => {
                setLimit(Number(e.target.value));
                setPage(0);
              }}
              rowsPerPageOptions={[25, 50, 100]}
            />
          </>
        )}
      </Section>
      <ConfirmDialog
        open={deleting !== null}
        title="Delete team?"
        confirmLabel="Delete"
        danger
        busy={del.isPending}
        onCancel={() => setDeleting(null)}
        onConfirm={() => {
          if (deleting) del.mutate(deleting.id);
        }}
      >
        “{deleting?.name}” (owner {deleting?.owner_email}) with {deleting?.vault_count} vaults and{" "}
        {deleting?.member_count} members is deleted permanently.
      </ConfirmDialog>
    </>
  );
}
