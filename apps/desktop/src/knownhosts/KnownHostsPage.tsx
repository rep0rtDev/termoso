import { useMemo, useState } from "react";
import { copyToClipboard } from "@/lib/clipboard";
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import ContentPasteRoundedIcon from "@mui/icons-material/ContentPasteRounded";
import DeleteSweepRoundedIcon from "@mui/icons-material/DeleteSweepRounded";
import UploadFileRoundedIcon from "@mui/icons-material/UploadFileRounded";
import FileDownloadRoundedIcon from "@mui/icons-material/FileDownloadRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open as openFile, save as saveFile } from "@tauri-apps/plugin-dialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import {
  EntityCard,
  IconTile,
  Loading,
  Mono,
  SearchField,
  SplitButton,
  ToolIconButton,
} from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useKnownHosts } from "@/ipc/hooks";
import { errorMessage, type KnownHostCard } from "@/ipc/types";
import { sizes } from "@/theme/theme";

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

  const total = (hosts.data ?? []).length;

  return (
    <Page>
      <PageHeader
        actions={
          <>
            <SplitButton
              label="Import"
              icon={<UploadFileRoundedIcon />}
              onClick={() => op.mutate(importFile)}
              items={[
                {
                  label: "Import known_hosts file…",
                  icon: <UploadFileRoundedIcon fontSize="small" />,
                  onClick: () => op.mutate(importFile),
                },
                {
                  label: "Paste entries…",
                  icon: <ContentPasteRoundedIcon fontSize="small" />,
                  onClick: () => setDialog({ kind: "paste" }),
                },
              ]}
            />
            <Button
              startIcon={<FileDownloadRoundedIcon />}
              disabled={total === 0}
              onClick={() => op.mutate(exportFile)}
            >
              Export
            </Button>
          </>
        }
        trailing={
          <SearchField
            value={filter}
            onChange={setFilter}
            placeholder="Filter by host, type or fingerprint"
            width={280}
          />
        }
      />
      <PageBody>
        {hosts.isPending ? (
          <Loading />
        ) : hosts.error ? (
          <EmptyState title="Could not load known hosts" description={errorMessage(hosts.error)} />
        ) : visible.length === 0 ? (
          <EmptyState
            icon={<VerifiedUserRoundedIcon />}
            title={filter ? "No matches" : "No trusted host keys yet"}
            description="Keys are recorded when you accept a host's fingerprint on first connect, or import ~/.ssh/known_hosts. A changed key is always re-prompted, never accepted silently."
          />
        ) : (
          <Stack spacing={1}>
            {visible.map((h) => {
              const siblings = countFor(h.hostname);
              return (
                <EntityCard
                  key={h.id}
                  dense
                  tile={
                    <IconTile size={sizes.tileSmall}>
                      <VerifiedUserRoundedIcon />
                    </IconTile>
                  }
                  title={<Mono>{h.hostname}</Mono>}
                  subtitle={
                    <>
                      {h.keyType}
                      {" · "}
                      <Mono>{h.fingerprint}</Mono>
                    </>
                  }
                  trailing={
                    <Typography variant="caption" color="text.secondary" noWrap sx={{ px: 0.5 }}>
                      Trusted {new Date(h.updatedAt).toLocaleDateString()}
                    </Typography>
                  }
                  actions={
                    <>
                      <ToolIconButton
                        title="Copy public key line"
                        onClick={() =>
                          op.mutate(async () => {
                            await copyToClipboard(`${h.hostname} ${h.keyType} ${h.publicKey}`);
                            return "Copied";
                          })
                        }
                      >
                        <ContentCopyRoundedIcon fontSize="small" />
                      </ToolIconButton>
                      {siblings > 1 && (
                        <ToolIconButton
                          title={`Forget all ${siblings} keys for this host`}
                          onClick={() =>
                            setDialog({ kind: "forgetHost", hostname: h.hostname, count: siblings })
                          }
                        >
                          <DeleteSweepRoundedIcon fontSize="small" />
                        </ToolIconButton>
                      )}
                      <ToolIconButton
                        title="Forget this key"
                        onClick={() => setDialog({ kind: "forget", card: h })}
                      >
                        <DeleteOutlineRoundedIcon fontSize="small" />
                      </ToolIconButton>
                    </>
                  }
                />
              );
            })}
          </Stack>
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
