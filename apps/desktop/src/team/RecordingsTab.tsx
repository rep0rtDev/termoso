import { useMemo, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  MenuItem,
  Select,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
import { useMutation } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { tr } from "@/i18n";
import * as ipc from "@/ipc/commands";
import { useLogs, useVaults } from "@/ipc/hooks";
import { errorMessage, type Team, type Uuid } from "@/ipc/types";
import { Loading } from "@/components/ui";
import { EmptyState } from "@/components/EmptyState";
import { useSnackbar } from "@/components/Snackbar";
import { authorName, authorsOf, visibleLogs } from "@/logs/team";
import { Viewer } from "@/logs/LogsPage";
import { Section } from "./Section";
import { can } from "./roles";

function fmtDuration(secs: number | null): string {
  if (secs === null) return "—";
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return m > 0 ? `${m}:${String(s).padStart(2, "0")}` : `0:${String(s).padStart(2, "0")}`;
}

/** Every recording of the team's vaults this device holds a key for, across vaults. */
export function RecordingsTab({ team }: { team: Team }) {
  const snack = useSnackbar();
  const logs = useLogs();
  const vaults = useVaults();
  const [vaultId, setVaultId] = useState<string>("");
  const [authorId, setAuthorId] = useState<Uuid | null>(null);
  const [selectedId, setSelectedId] = useState<Uuid | null>(null);
  const teamVaults = useMemo(
    () => (vaults.data ?? []).filter((v) => v.kind === "team" && v.team_id === team.id),
    [vaults.data, team.id],
  );
  const inTeam = useMemo(() => {
    const ids = new Set(teamVaults.map((v) => v.id));
    return (logs.data ?? []).filter((l) => ids.has(l.vaultId));
  }, [logs.data, teamVaults]);
  const scoped = useMemo(
    () => visibleLogs(inTeam, vaultId === "" ? null : vaultId, null),
    [inTeam, vaultId],
  );
  const authors = useMemo(() => authorsOf(scoped), [scoped]);
  const list = useMemo(() => visibleLogs(scoped, null, authorId), [scoped, authorId]);
  const selected = list.find((l) => l.id === selectedId) ?? null;
  const vaultName = (id: Uuid) => teamVaults.find((v) => v.id === id)?.name ?? id.slice(0, 8);
  const exportAll = useMutation({
    mutationFn: async () => {
      const ndjson = await ipc.teamLogsExport(team.id);
      const path = await save({
        defaultPath: `termoso-recordings-${team.name.replace(/[^\w.-]+/g, "_")}.ndjson`,
        filters: [{ name: "NDJSON", extensions: ["ndjson"] }],
      });
      if (!path) return false;
      await ipc.saveTextFile(path, ndjson);
      return true;
    },
    onSuccess: (saved) => saved && snack.notify(tr("Export downloaded")),
    onError: (e) => snack.error(errorMessage(e)),
  });
  if (logs.isPending || vaults.isPending) return <Loading />;
  return (
    <Stack spacing={2}>
      <Section
        title={tr("Team recordings")}
        description={tr(
          "Recordings of every team vault this device holds a key for, decrypted locally. Vaults you are not a member of stay invisible — the server cannot read them either.",
        )}
        actions={
          <Stack direction="row" spacing={1}>
            {teamVaults.length > 1 && (
              <Select
                size="small"
                displayEmpty
                value={vaultId}
                onChange={(e) => setVaultId(e.target.value)}
                sx={{ minWidth: 160 }}
              >
                <MenuItem value="">{tr("All vaults")}</MenuItem>
                {teamVaults.map((v) => (
                  <MenuItem key={v.id} value={v.id}>
                    {v.name}
                  </MenuItem>
                ))}
              </Select>
            )}
            {authors.length > 1 && (
              <Select
                size="small"
                displayEmpty
                value={authorId ?? ""}
                onChange={(e) => setAuthorId(e.target.value === "" ? null : e.target.value)}
                sx={{ minWidth: 160 }}
              >
                <MenuItem value="">{tr("Everyone")}</MenuItem>
                {authors.map((a) => (
                  <MenuItem key={a.userId} value={a.userId}>
                    {authorName(a)}
                  </MenuItem>
                ))}
              </Select>
            )}
            {can(team, "logs.export") && (
              <Button
                variant="outlined"
                disabled={exportAll.isPending}
                onClick={() => exportAll.mutate()}
              >
                {tr("Export NDJSON")}
              </Button>
            )}
          </Stack>
        }
        disablePadding
      >
        {teamVaults.length === 0 ? (
          <Box sx={{ p: 2 }}>
            <Alert severity="info">
              {tr(
                "You are not a member of any vault of this team, so there is nothing to decrypt here.",
              )}
            </Alert>
          </Box>
        ) : list.length === 0 ? (
          <EmptyState
            title={tr("No recordings")}
            description={tr(
              "Recordings appear after a member connects with session recording on, and after the next sync.",
            )}
            compact
          />
        ) : (
          <Box sx={{ display: "flex", minHeight: 320 }}>
            <TableContainer
              sx={{
                width: selected ? 360 : "100%",
                flexShrink: 0,
                borderRight: selected ? 1 : 0,
                borderColor: "divider",
              }}
            >
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>{tr("Session")}</TableCell>
                    {!selected && <TableCell>{tr("Vault")}</TableCell>}
                    <TableCell>{tr("Author")}</TableCell>
                    <TableCell>{tr("When")}</TableCell>
                    {!selected && <TableCell align="right">{tr("Duration")}</TableCell>}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {list.map((l) => (
                    <TableRow
                      key={l.id}
                      hover
                      selected={l.id === selectedId}
                      sx={{ cursor: "pointer" }}
                      onClick={() => setSelectedId(l.id)}
                    >
                      <TableCell>
                        <Typography variant="body2" noWrap>
                          {l.label || l.target}
                        </Typography>
                        <Typography variant="caption" color="text.secondary" noWrap>
                          {l.target}
                          {!l.completed && (
                            <Chip
                              size="small"
                              color="warning"
                              variant="outlined"
                              label={tr("Live")}
                              sx={{ ml: 1, height: 18 }}
                            />
                          )}
                        </Typography>
                      </TableCell>
                      {!selected && <TableCell>{vaultName(l.vaultId)}</TableCell>}
                      <TableCell>
                        {l.mine ? tr("You") : l.author ? authorName(l.author) : "—"}
                      </TableCell>
                      <TableCell>{new Date(l.startedAt).toLocaleString()}</TableCell>
                      {!selected && (
                        <TableCell align="right">{fmtDuration(l.durationSecs)}</TableCell>
                      )}
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
            {selected && (
              <Box sx={{ flex: 1, minWidth: 0 }}>
                <Viewer
                  key={selected.id}
                  log={selected}
                  onClose={() => setSelectedId(null)}
                  onDeleted={() => setSelectedId(null)}
                />
              </Box>
            )}
          </Box>
        )}
      </Section>
    </Stack>
  );
}
