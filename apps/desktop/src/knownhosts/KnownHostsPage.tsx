import { useMemo, useState } from "react";
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  InputAdornment,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import UploadFileRoundedIcon from "@mui/icons-material/UploadFileRounded";
import FileDownloadRoundedIcon from "@mui/icons-material/FileDownloadRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open as openFile, save as saveFile } from "@tauri-apps/plugin-dialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useKnownHosts } from "@/ipc/hooks";
import { errorMessage, type KnownHostCard } from "@/ipc/types";

type DialogState =
  | { kind: "none" }
  | { kind: "forget"; card: KnownHostCard }
  | { kind: "forgetHost"; hostname: string; count: number }
  | { kind: "paste" };

function PasteDialog({
  busy,
  onCancel,
  onConfirm,
}: {
  busy: boolean;
  onCancel: () => void;
  onConfirm: (text: string) => void;
}) {
  const [text, setText] = useState("");
  return (
    <Dialog open onClose={busy ? undefined : onCancel} maxWidth="sm" fullWidth>
      <DialogTitle>Import known_hosts entries</DialogTitle>
      <DialogContent>
        <TextField
          autoFocus
          fullWidth
          multiline
          minRows={6}
          maxRows={14}
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="example.com ssh-ed25519 AAAA…"
          helperText="OpenSSH known_hosts format, one entry per line. Hashed entries are skipped."
          slotProps={{ htmlInput: { spellCheck: false, style: { fontFamily: "monospace" } } }}
          sx={{ mt: 0.5 }}
        />
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={busy || text.trim().length === 0}
          onClick={() => onConfirm(text)}
        >
          Import
        </Button>
      </DialogActions>
    </Dialog>
  );
}

export function KnownHostsPage() {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const hosts = useKnownHosts();
  const [filter, setFilter] = useState("");
  const [dialog, setDialog] = useState<DialogState>({ kind: "none" });

  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      void qc.invalidateQueries({ queryKey: keys.knownHosts });
      setDialog({ kind: "none" });
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const visible = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const all = hosts.data ?? [];
    if (!q) return all;
    return all.filter(
      (h) =>
        h.hostname.toLowerCase().includes(q) ||
        h.fingerprint.toLowerCase().includes(q) ||
        h.keyType.toLowerCase().includes(q),
    );
  }, [hosts.data, filter]);

  const countFor = (hostname: string) =>
    (hosts.data ?? []).filter((h) => h.hostname === hostname).length;

  const importFile = async () => {
    const def = await ipc.knownHostsDefaultPath();
    const picked = await openFile({
      multiple: false,
      directory: false,
      title: "Import known_hosts",
      defaultPath: def ?? undefined,
    });
    if (typeof picked !== "string") return null;
    const rep = await ipc.knownHostsImportFile(picked);
    return `Imported ${rep.added} new host key(s)`;
  };
  const exportFile = async () => {
    const path = await saveFile({ title: "Export known_hosts", defaultPath: "known_hosts" });
    if (path === null) return null;
    const n = await ipc.knownHostsExportFile(path);
    return `Exported ${n} entries to ${path}`;
  };

  return (
    <Page>
      <PageHeader
        title="Known Hosts"
        description="Host keys you have trusted. A changed key is always re-prompted, never accepted silently."
        actions={
          <>
            <Button startIcon={<UploadFileRoundedIcon />} onClick={() => op.mutate(importFile)}>
              Import file
            </Button>
            <Button onClick={() => setDialog({ kind: "paste" })}>Paste</Button>
            <Button
              startIcon={<FileDownloadRoundedIcon />}
              disabled={(hosts.data ?? []).length === 0}
              onClick={() => op.mutate(exportFile)}
            >
              Export
            </Button>
          </>
        }
      />
      <Box sx={{ px: 2.5, pt: 1.5 }}>
        <TextField
          size="small"
          placeholder="Filter by host, type or fingerprint"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          sx={{ width: 360 }}
          slotProps={{
            input: {
              startAdornment: (
                <InputAdornment position="start">
                  <SearchRoundedIcon fontSize="small" />
                </InputAdornment>
              ),
            },
          }}
        />
      </Box>
      <PageBody>
        {hosts.isPending ? (
          <Box sx={{ display: "flex", justifyContent: "center", pt: 8 }}>
            <CircularProgress size={28} />
          </Box>
        ) : hosts.error ? (
          <EmptyState title="Could not load known hosts" description={errorMessage(hosts.error)} />
        ) : visible.length === 0 ? (
          <EmptyState
            icon={<VerifiedUserRoundedIcon />}
            title={filter ? "No matches" : "No trusted host keys yet"}
            description="Keys are recorded when you accept a host's fingerprint on first connect, or import ~/.ssh/known_hosts."
          />
        ) : (
          <Table size="small" sx={{ mt: 1 }}>
            <TableHead>
              <TableRow sx={{ "& th": { color: "text.secondary", fontWeight: 600 } }}>
                <TableCell>Host</TableCell>
                <TableCell>Type</TableCell>
                <TableCell>Fingerprint</TableCell>
                <TableCell>Trusted</TableCell>
                <TableCell padding="checkbox" />
                <TableCell padding="checkbox" />
              </TableRow>
            </TableHead>
            <TableBody>
              {visible.map((h) => (
                <TableRow key={h.id} hover>
                  <TableCell>
                    <Typography
                      variant="body2"
                      sx={{ fontWeight: 600, fontFamily: "monospace", cursor: "pointer" }}
                      noWrap
                      onClick={() =>
                        setDialog({
                          kind: "forgetHost",
                          hostname: h.hostname,
                          count: countFor(h.hostname),
                        })
                      }
                      title="Forget every key for this host"
                    >
                      {h.hostname}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Chip size="small" variant="outlined" label={h.keyType} />
                  </TableCell>
                  <TableCell>
                    <Typography
                      variant="body2"
                      sx={{ fontFamily: "monospace", fontSize: 12 }}
                      noWrap
                      title={h.fingerprint}
                    >
                      {h.fingerprint}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary" noWrap>
                      {new Date(h.updatedAt).toLocaleString()}
                    </Typography>
                  </TableCell>
                  <TableCell padding="checkbox">
                    <Tooltip title="Copy public key line">
                      <IconButton
                        size="small"
                        onClick={() =>
                          op.mutate(async () => {
                            await navigator.clipboard.writeText(
                              `${h.hostname} ${h.keyType} ${h.publicKey}`,
                            );
                            return "Copied";
                          })
                        }
                      >
                        <ContentCopyRoundedIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                  </TableCell>
                  <TableCell padding="checkbox">
                    <Tooltip title="Forget this key">
                      <IconButton
                        size="small"
                        onClick={() => setDialog({ kind: "forget", card: h })}
                      >
                        <DeleteOutlineRoundedIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </PageBody>

      {dialog.kind === "paste" && (
        <PasteDialog
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={(text) =>
            op.mutate(async () => {
              const rep = await ipc.knownHostsImportText(text);
              return `Imported ${rep.added} new host key(s)`;
            })
          }
        />
      )}
      {dialog.kind === "forget" && (
        <ConfirmDialog
          open
          title="Forget host key?"
          confirmLabel="Forget"
          danger
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={() => {
            const id = dialog.card.id;
            op.mutate(async () => {
              await ipc.knownHostForget(id);
              return null;
            });
          }}
        >
          <Stack spacing={0.5}>
            <span>
              The {dialog.card.keyType} key for <b>{dialog.card.hostname}</b> will be forgotten. You
              will be asked to verify the fingerprint on the next connection.
            </span>
            <Typography variant="caption" sx={{ fontFamily: "monospace" }}>
              {dialog.card.fingerprint}
            </Typography>
          </Stack>
        </ConfirmDialog>
      )}
      {dialog.kind === "forgetHost" && (
        <ConfirmDialog
          open
          title="Forget all keys for host?"
          confirmLabel="Forget all"
          danger
          busy={op.isPending}
          onCancel={() => setDialog({ kind: "none" })}
          onConfirm={() => {
            const hostname = dialog.hostname;
            op.mutate(async () => {
              const n = await ipc.knownHostForgetHost(hostname);
              return `Forgot ${n} key(s)`;
            });
          }}
        >
          {dialog.count} key(s) recorded for <b>{dialog.hostname}</b> will be forgotten.
        </ConfirmDialog>
      )}
    </Page>
  );
}
