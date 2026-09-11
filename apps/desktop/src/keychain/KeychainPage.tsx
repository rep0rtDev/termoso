import { useState } from "react";
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  IconButton,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tabs,
  Tooltip,
  Typography,
} from "@mui/material";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import MoreVertRoundedIcon from "@mui/icons-material/MoreVertRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import DriveFileRenameOutlineRoundedIcon from "@mui/icons-material/DriveFileRenameOutlineRounded";
import LockResetRoundedIcon from "@mui/icons-material/LockResetRounded";
import FileDownloadRoundedIcon from "@mui/icons-material/FileDownloadRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import UploadFileRoundedIcon from "@mui/icons-material/UploadFileRounded";
import WarningAmberRoundedIcon from "@mui/icons-material/WarningAmberRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { save as saveFile } from "@tauri-apps/plugin-dialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { useDefaultVault, useIdentities, useSshKeys } from "@/ipc/hooks";
import { errorMessage, type IdentityCard, type KeyCard } from "@/ipc/types";
import { NameDialog } from "@/sftp/dialogs";
import { IdentityDialog } from "./IdentityDialog";
import {
  ExportKeyDialog,
  GenerateKeyDialog,
  ImportKeyDialog,
  PassphraseDialog,
} from "./KeyDialogs";

type KeyDialog =
  | { kind: "none" }
  | { kind: "generate" }
  | { kind: "import" }
  | { kind: "rename"; card: KeyCard }
  | { kind: "passphrase"; card: KeyCard }
  | { kind: "export"; card: KeyCard }
  | { kind: "delete"; card: KeyCard };

type IdDialog =
  | { kind: "none" }
  | { kind: "edit"; card: IdentityCard | null }
  | { kind: "delete"; card: IdentityCard };

async function copy(text: string) {
  await navigator.clipboard.writeText(text);
}

export function KeychainPage() {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vault = useDefaultVault();
  const vaultId = vault.data?.id ?? null;
  const sshKeys = useSshKeys(vaultId);
  const identities = useIdentities(vaultId);
  const [tab, setTab] = useState<"keys" | "identities">("keys");
  const [keyDialog, setKeyDialog] = useState<KeyDialog>({ kind: "none" });
  const [idDialog, setIdDialog] = useState<IdDialog>({ kind: "none" });
  const [menu, setMenu] = useState<{ anchor: HTMLElement; card: KeyCard } | null>(null);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["sshKeys"] });
    void qc.invalidateQueries({ queryKey: ["identities"] });
    void qc.invalidateQueries({ queryKey: ["hosts"] });
  };
  const closeKey = () => setKeyDialog({ kind: "none" });
  const closeId = () => setIdDialog({ kind: "none" });

  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      refresh();
      closeKey();
      closeId();
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const run = (job: () => Promise<string | null>) => op.mutate(job);

  const exportKey = async (
    card: KeyCard,
    args: {
      mode: "file" | "clipboard";
      passphrase: string | null;
      exportPassphrase: string | null;
    },
  ) => {
    if (args.mode === "clipboard") {
      const text = await ipc.keyExport({
        id: card.id,
        passphrase: args.passphrase,
        exportPassphrase: args.exportPassphrase,
      });
      await copy(text);
      return "Private key copied to clipboard";
    }
    const path = await saveFile({
      title: "Export private key",
      defaultPath: card.label.replace(/[^\w.-]+/g, "_"),
    });
    if (path === null) return null;
    await ipc.keyExportFile({
      id: card.id,
      path,
      passphrase: args.passphrase,
      exportPassphrase: args.exportPassphrase,
    });
    return `Saved to ${path}`;
  };

  const loading = vault.isPending || sshKeys.isPending || identities.isPending;
  const loadError = vault.error ?? sshKeys.error ?? identities.error;

  return (
    <Page>
      <PageHeader
        title="Keychain"
        description="SSH keys and identities. Private keys never leave the encrypted vault unless you export them."
        actions={
          tab === "keys" ? (
            <>
              <Button
                startIcon={<UploadFileRoundedIcon />}
                onClick={() => setKeyDialog({ kind: "import" })}
                disabled={!vaultId}
              >
                Import
              </Button>
              <Button
                variant="contained"
                startIcon={<AddRoundedIcon />}
                onClick={() => setKeyDialog({ kind: "generate" })}
                disabled={!vaultId}
              >
                Generate
              </Button>
            </>
          ) : (
            <Button
              variant="contained"
              startIcon={<AddRoundedIcon />}
              onClick={() => setIdDialog({ kind: "edit", card: null })}
              disabled={!vaultId}
            >
              New identity
            </Button>
          )
        }
      />
      <Tabs
        value={tab}
        onChange={(_, v: "keys" | "identities") => setTab(v)}
        sx={{ px: 2.5, borderBottom: 1, borderColor: "divider", minHeight: 40 }}
      >
        <Tab value="keys" label={`Keys (${sshKeys.data?.length ?? 0})`} sx={{ minHeight: 40 }} />
        <Tab
          value="identities"
          label={`Identities (${identities.data?.length ?? 0})`}
          sx={{ minHeight: 40 }}
        />
      </Tabs>
      <PageBody>
        {loading ? (
          <Box sx={{ display: "flex", justifyContent: "center", pt: 8 }}>
            <CircularProgress size={28} />
          </Box>
        ) : loadError ? (
          <EmptyState title="Could not open the keychain" description={errorMessage(loadError)} />
        ) : tab === "keys" ? (
          (sshKeys.data ?? []).length === 0 ? (
            <EmptyState
              icon={<KeyRoundedIcon />}
              title="No keys yet"
              description="Generate an Ed25519 key or import an existing one. Keys are stored encrypted with your master key."
              action={
                <Button variant="contained" onClick={() => setKeyDialog({ kind: "generate" })}>
                  Generate key
                </Button>
              }
            />
          ) : (
            <Table size="small" sx={{ mt: 1 }}>
              <TableHead>
                <TableRow sx={{ "& th": { color: "text.secondary", fontWeight: 600 } }}>
                  <TableCell>Label</TableCell>
                  <TableCell>Type</TableCell>
                  <TableCell>Fingerprint</TableCell>
                  <TableCell>Passphrase</TableCell>
                  <TableCell align="right">Used by</TableCell>
                  <TableCell padding="checkbox" />
                </TableRow>
              </TableHead>
              <TableBody>
                {(sshKeys.data ?? []).map((k) => (
                  <TableRow key={k.id} hover>
                    <TableCell>
                      <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
                        <Typography variant="body2" sx={{ fontWeight: 600 }} noWrap>
                          {k.label}
                        </Typography>
                        {k.unreadable && (
                          <Tooltip title="Could not parse this key">
                            <WarningAmberRoundedIcon color="warning" fontSize="small" />
                          </Tooltip>
                        )}
                        {k.comment && (
                          <Typography variant="caption" color="text.secondary" noWrap>
                            {k.comment}
                          </Typography>
                        )}
                      </Box>
                    </TableCell>
                    <TableCell>
                      <Chip
                        size="small"
                        variant="outlined"
                        label={k.bits > 0 ? `${k.keyType} ${k.bits}` : k.keyType}
                      />
                    </TableCell>
                    <TableCell>
                      <Typography
                        variant="body2"
                        sx={{ fontFamily: "monospace", fontSize: 12 }}
                        noWrap
                        title={k.fingerprint}
                      >
                        {k.fingerprint}
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Typography variant="body2" color="text.secondary">
                        {!k.encrypted ? "none" : k.hasPassphrase ? "stored" : "ask on use"}
                      </Typography>
                    </TableCell>
                    <TableCell align="right">
                      <Typography variant="body2" color="text.secondary">
                        {k.usedBy}
                      </Typography>
                    </TableCell>
                    <TableCell padding="checkbox">
                      <IconButton
                        size="small"
                        onClick={(e) => setMenu({ anchor: e.currentTarget, card: k })}
                      >
                        <MoreVertRoundedIcon fontSize="small" />
                      </IconButton>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )
        ) : (identities.data ?? []).length === 0 ? (
          <EmptyState
            icon={<PersonRoundedIcon />}
            title="No identities"
            description="An identity bundles a username with a password or key so several hosts can share it."
            action={
              <Button variant="contained" onClick={() => setIdDialog({ kind: "edit", card: null })}>
                New identity
              </Button>
            }
          />
        ) : (
          <Table size="small" sx={{ mt: 1 }}>
            <TableHead>
              <TableRow sx={{ "& th": { color: "text.secondary", fontWeight: 600 } }}>
                <TableCell>Label</TableCell>
                <TableCell>Username</TableCell>
                <TableCell>Password</TableCell>
                <TableCell>Key</TableCell>
                <TableCell padding="checkbox" />
              </TableRow>
            </TableHead>
            <TableBody>
              {(identities.data ?? []).map((i) => (
                <TableRow
                  key={i.id}
                  hover
                  sx={{ cursor: "pointer" }}
                  onClick={() => setIdDialog({ kind: "edit", card: i })}
                >
                  <TableCell>
                    <Typography variant="body2" sx={{ fontWeight: 600 }} noWrap>
                      {i.label}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontFamily: "monospace" }} noWrap>
                      {i.username}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary">
                      {i.hasPassword ? "stored" : "—"}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary" noWrap>
                      {i.sshKeyLabel ?? "—"}
                    </Typography>
                  </TableCell>
                  <TableCell padding="checkbox">
                    <IconButton
                      size="small"
                      onClick={(e) => {
                        e.stopPropagation();
                        setIdDialog({ kind: "delete", card: i });
                      }}
                    >
                      <DeleteOutlineRoundedIcon fontSize="small" />
                    </IconButton>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </PageBody>

      <Menu open={menu !== null} anchorEl={menu?.anchor} onClose={() => setMenu(null)}>
        {menu && (
          <MenuItem
            onClick={() => {
              const card = menu.card;
              setMenu(null);
              run(async () => {
                await copy(await ipc.keyPublic(card.id));
                return "Public key copied";
              });
            }}
          >
            <ListItemIcon>
              <ContentCopyRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Copy public key</ListItemText>
          </MenuItem>
        )}
        {menu && (
          <MenuItem
            onClick={() => {
              setKeyDialog({ kind: "rename", card: menu.card });
              setMenu(null);
            }}
          >
            <ListItemIcon>
              <DriveFileRenameOutlineRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Rename</ListItemText>
          </MenuItem>
        )}
        {menu && (
          <MenuItem
            disabled={menu.card.unreadable}
            onClick={() => {
              setKeyDialog({ kind: "passphrase", card: menu.card });
              setMenu(null);
            }}
          >
            <ListItemIcon>
              <LockResetRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Change passphrase</ListItemText>
          </MenuItem>
        )}
        {menu && (
          <MenuItem
            disabled={menu.card.unreadable}
            onClick={() => {
              setKeyDialog({ kind: "export", card: menu.card });
              setMenu(null);
            }}
          >
            <ListItemIcon>
              <FileDownloadRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Export private key…</ListItemText>
          </MenuItem>
        )}
        {menu && (
          <MenuItem
            onClick={() => {
              setKeyDialog({ kind: "delete", card: menu.card });
              setMenu(null);
            }}
            sx={{ color: "error.main" }}
          >
            <ListItemIcon>
              <DeleteOutlineRoundedIcon fontSize="small" color="error" />
            </ListItemIcon>
            <ListItemText>Delete</ListItemText>
          </MenuItem>
        )}
      </Menu>

      {vaultId && keyDialog.kind === "generate" && (
        <GenerateKeyDialog
          open
          vaultId={vaultId}
          busy={op.isPending}
          onCancel={closeKey}
          onConfirm={(form) =>
            run(async () => {
              const k = await ipc.keyGenerate(form);
              return `Generated ${k.label}`;
            })
          }
        />
      )}
      {vaultId && keyDialog.kind === "import" && (
        <ImportKeyDialog
          open
          vaultId={vaultId}
          busy={op.isPending}
          onCancel={closeKey}
          onConfirm={(form) =>
            run(async () => {
              const k = await ipc.keyImport(form);
              return `Imported ${k.label}`;
            })
          }
          onConfirmFile={(args) =>
            run(async () => {
              const k = await ipc.keyImportFile(args);
              return `Imported ${k.label}`;
            })
          }
        />
      )}
      {keyDialog.kind === "rename" && (
        <NameDialog
          open
          title="Rename key"
          label="Label"
          initial={keyDialog.card.label}
          confirmLabel="Rename"
          busy={op.isPending}
          onCancel={closeKey}
          onConfirm={(label) => {
            const id = keyDialog.card.id;
            run(async () => {
              await ipc.keyRename(id, label);
              return null;
            });
          }}
        />
      )}
      {keyDialog.kind === "passphrase" && (
        <PassphraseDialog
          open
          card={keyDialog.card}
          busy={op.isPending}
          onCancel={closeKey}
          onConfirm={(args) => {
            const id = keyDialog.card.id;
            run(async () => {
              await ipc.keyChangePassphrase({ id, ...args });
              return "Passphrase updated";
            });
          }}
        />
      )}
      {keyDialog.kind === "export" && (
        <ExportKeyDialog
          open
          card={keyDialog.card}
          busy={op.isPending}
          onCancel={closeKey}
          onConfirm={(args) => {
            const card = keyDialog.card;
            run(() => exportKey(card, args));
          }}
        />
      )}
      {keyDialog.kind === "delete" && (
        <ConfirmDialog
          open
          title="Delete key?"
          confirmLabel="Delete"
          danger
          busy={op.isPending}
          onCancel={closeKey}
          onConfirm={() => {
            const card = keyDialog.card;
            run(async () => {
              await ipc.keyDelete(card.id);
              return `Deleted ${card.label}`;
            });
          }}
        >
          <b>{keyDialog.card.label}</b> will be removed from the vault
          {keyDialog.card.usedBy > 0 &&
            ` and detached from ${keyDialog.card.usedBy} host(s) / identit(ies)`}
          . This cannot be undone.
        </ConfirmDialog>
      )}

      {vaultId && idDialog.kind === "edit" && (
        <IdentityDialog
          open
          vaultId={vaultId}
          initial={idDialog.card}
          keys={sshKeys.data ?? []}
          busy={op.isPending}
          onCancel={closeId}
          onConfirm={(form) =>
            run(async () => {
              await ipc.identitySave(form);
              return null;
            })
          }
        />
      )}
      {idDialog.kind === "delete" && (
        <ConfirmDialog
          open
          title="Delete identity?"
          confirmLabel="Delete"
          danger
          busy={op.isPending}
          onCancel={closeId}
          onConfirm={() => {
            const card = idDialog.card;
            run(async () => {
              await ipc.identityDelete(card.id);
              return `Deleted ${card.label}`;
            });
          }}
        >
          Hosts using <b>{idDialog.card.label}</b> will fall back to inline credentials.
        </ConfirmDialog>
      )}
    </Page>
  );
}
