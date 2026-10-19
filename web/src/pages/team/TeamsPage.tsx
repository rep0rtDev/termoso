import { useMemo, useState, type SubmitEvent } from "react";
import { tr, trn } from "@/i18n";
import {
  Alert,
  Chip,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  InputAdornment,
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
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { teamsApi } from "@/api/endpoints";
import type { Team } from "@/api/types";
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
        <TeamsTable
          teams={teams.data.teams}
          homeTeamId={info.data?.home_team?.id}
          onOpen={(id) => void navigate(`/team/${id}`)}
        />
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

/** Compact flags of the policies a team enforces, for the list. */
function PolicyChips({ t }: { t: Team }) {
  const p = t.policies;
  const chips: string[] = [];
  if ((p && p.mfa !== "off") || t.require_mfa) chips.push(tr("MFA"));
  if (p?.sso_only) chips.push(tr("SSO"));
  if (p?.device_approval || p?.device_admin_approval) chips.push(tr("Devices"));
  if (p?.ip_allowlist.length) chips.push(tr("IP"));
  if (p?.session_recording === "required") chips.push(tr("Recording"));
  if (chips.length === 0)
    return (
      <Typography variant="body2" color="text.secondary">
        —
      </Typography>
    );
  return (
    <Stack direction="row" spacing={0.5} useFlexGap sx={{ flexWrap: "wrap" }}>
      {chips.map((c) => (
        <Chip key={c} size="small" variant="outlined" label={c} sx={{ height: 20 }} />
      ))}
    </Stack>
  );
}

function TeamsTable({
  teams,
  homeTeamId,
  onOpen,
}: {
  teams: Team[];
  homeTeamId: string | undefined;
  onOpen: (id: string) => void;
}) {
  const [q, setQ] = useState("");
  const [page, setPage] = useState(0);
  const [limit, setLimit] = useState(25);
  const filtered = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const list = needle ? teams.filter((t) => t.name.toLowerCase().includes(needle)) : teams;
    return [...list].sort((a, b) =>
      a.id === homeTeamId ? -1 : b.id === homeTeamId ? 1 : a.name.localeCompare(b.name),
    );
  }, [teams, q, homeTeamId]);
  const pageItems = filtered.slice(page * limit, page * limit + limit);
  return (
    <Stack spacing={1.5}>
      {teams.length > 8 && (
        <TextField
          size="small"
          placeholder={tr("Search teams")}
          value={q}
          onChange={(e) => {
            setQ(e.target.value);
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
          sx={{ maxWidth: 360 }}
        />
      )}
      <TableContainer sx={{ bgcolor: "surface.high", borderRadius: 2 }}>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{tr("Team")}</TableCell>
              <TableCell>{tr("Your role")}</TableCell>
              <TableCell align="right">{tr("Members")}</TableCell>
              <TableCell>{tr("Policies")}</TableCell>
              <TableCell>{tr("Created")}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {pageItems.map((t) => (
              <TableRow
                key={t.id}
                hover
                onClick={() => onOpen(t.id)}
                sx={{ cursor: "pointer" }}
                role="link"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === "Enter") onOpen(t.id);
                }}
              >
                <TableCell>
                  <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
                    <Typography variant="body2" sx={{ fontWeight: 600 }}>
                      {t.name}
                    </Typography>
                    {homeTeamId === t.id && (
                      <Chip
                        size="small"
                        variant="outlined"
                        label={tr("Home team")}
                        sx={{ height: 20 }}
                      />
                    )}
                  </Stack>
                </TableCell>
                <TableCell>
                  <RoleChip role={t.my_role} />
                </TableCell>
                <TableCell align="right">
                  {trn(t.member_count, "{count} member", "{count} members")}
                </TableCell>
                <TableCell>
                  <PolicyChips t={t} />
                </TableCell>
                <TableCell sx={{ whiteSpace: "nowrap" }}>{formatDate(t.created_at)}</TableCell>
              </TableRow>
            ))}
            {pageItems.length === 0 && (
              <TableRow>
                <TableCell colSpan={5}>
                  <Typography variant="body2" color="text.secondary" sx={{ py: 1 }}>
                    {tr("No teams match.")}
                  </Typography>
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
        {filtered.length > 10 && (
          <TablePagination
            component="div"
            count={filtered.length}
            page={page}
            onPageChange={(_, p) => setPage(p)}
            rowsPerPage={limit}
            onRowsPerPageChange={(e) => {
              setLimit(Number(e.target.value));
              setPage(0);
            }}
            rowsPerPageOptions={[10, 25, 50]}
          />
        )}
      </TableContainer>
    </Stack>
  );
}
