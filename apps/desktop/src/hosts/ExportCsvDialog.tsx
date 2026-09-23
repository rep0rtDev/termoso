import { useState } from "react";
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Switch,
  Typography,
} from "@mui/material";
import { save as saveFile } from "@tauri-apps/plugin-dialog";
import { useSnackbar } from "@/components/Snackbar";
import { Mono, SettingRow } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { errorMessage, type LocalVault } from "@/ipc/types";
import { tr, trn, trx, msg } from "@/i18n";

const COLUMNS: [string, string][] = [
  ["Groups", msg("group path, joined with /")],
  ["Label", msg("host name")],
  ["Tags", msg("comma separated")],
  ["Hostname/IP", msg("address")],
  ["Protocol", msg("ssh or telnet")],
  ["Port", msg("effective port")],
  ["Username", msg("effective username (own or inherited)")],
  ["Password", "empty unless enabled below"],
];

/** Termius-compatible CSV of the current vault's hosts; keys, certificates and proxies never leave the app. */
export function ExportCsvDialog({
  open,
  vault,
  hostCount,
  onClose,
}: {
  open: boolean;
  vault: LocalVault | null;
  hostCount: number;
  onClose: () => void;
}) {
  const snackbar = useSnackbar();
  const [passwords, setPasswords] = useState(false);
  const [busy, setBusy] = useState(false);

  const close = () => {
    setPasswords(false);
    onClose();
  };

  const run = async () => {
    if (!vault) return;
    const path = await saveFile({
      title: tr("Export hosts to CSV"),
      defaultPath: `termoso-${vault.name.toLowerCase().replace(/\s+/g, "-")}-hosts.csv`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    setBusy(true);
    try {
      const r = await ipc.hostsExportCsv(vault.id, passwords, path);
      snackbar.notify(
        `Exported ${r.hosts} host${r.hosts === 1 ? "" : "s"}${r.passwordsIncluded ? " with passwords" : ""}`,
      );
      close();
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onClose={close} maxWidth="sm" fullWidth>
      <DialogTitle>{tr("Export hosts to CSV")}</DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}>
        <Typography variant="body2" color="text.secondary">
          {trx(
            trn(
              hostCount,
              "{count} host from {vault} in the same column layout Termius and Termoso import. SSH keys, certificates, proxies and jump chains are not part of the file.",
              "{count} hosts from {vault} in the same column layout Termius and Termoso import. SSH keys, certificates, proxies and jump chains are not part of the file.",
            ),
            { vault: <b>{vault?.name ?? "—"}</b> },
          )}
        </Typography>
        <Box
          sx={{
            display: "grid",
            gridTemplateColumns: "auto 1fr",
            columnGap: 2,
            rowGap: 0.25,
            px: 1.5,
            py: 1,
            borderRadius: 2,
            bgcolor: "surface.high",
          }}
        >
          {COLUMNS.map(([name, hint]) => (
            <Box key={name} sx={{ display: "contents" }}>
              <Mono sx={{ fontSize: 12 }}>{name}</Mono>
              <Typography variant="caption" color="text.secondary">
                {tr(hint)}
              </Typography>
            </Box>
          ))}
        </Box>
        <SettingRow
          label={tr("Include passwords")}
          hint={tr("Writes host passwords as plaintext into the file.")}
          control={<Switch checked={passwords} onChange={(e) => setPasswords(e.target.checked)} />}
          last
        />
        {passwords && (
          <Alert severity="warning" variant="outlined">
            {tr(
              "The CSV will contain plaintext passwords. Store it somewhere safe and delete it after importing elsewhere.",
            )}
          </Alert>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={close}>{tr("Cancel")}</Button>
        <Button variant="contained" onClick={run} disabled={busy || !vault || hostCount === 0}>
          {tr("Export…")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
