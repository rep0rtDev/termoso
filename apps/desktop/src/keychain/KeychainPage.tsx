import { useState } from "react";
import {
  Button,
  Chip,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Stack,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import EditRoundedIcon from "@mui/icons-material/EditRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import DriveFileRenameOutlineRoundedIcon from "@mui/icons-material/DriveFileRenameOutlineRounded";
import LockResetRoundedIcon from "@mui/icons-material/LockResetRounded";
import FileDownloadRoundedIcon from "@mui/icons-material/FileDownloadRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import UploadFileRoundedIcon from "@mui/icons-material/UploadFileRounded";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import WarningAmberRoundedIcon from "@mui/icons-material/WarningAmberRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { save as saveFile } from "@tauri-apps/plugin-dialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import {
  EntityCard,
  IconTile,
  Loading,
  Mono,
  SplitButton,
  ToolIconButton,
  type MenuAction,
} from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { useDefaultVault, useHosts, useIdentities, useSshKeys } from "@/ipc/hooks";
import { errorMessage, type IdentityCard, type KeyCard } from "@/ipc/types";
import { sizes } from "@/theme/theme";
import { NameDialog } from "@/sftp/dialogs";
import { IdentityDialog } from "./IdentityDialog";
import {
  ExportKeyDialog,
  ExportToHostDialog,
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
  | { kind: "exportToHost"; card: KeyCard }
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
  const hosts = useHosts(vaultId);
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

  const keyList = sshKeys.data ?? [];
  const idList = identities.data ?? [];
  const newKeyItems: MenuAction[] = [
    {
      label: "Generate key",
      icon: <AddRoundedIcon fontSize="small" />,
      onClick: () => setKeyDialog({ kind: "generate" }),
    },
    {
      label: "Import key…",
      icon: <UploadFileRoundedIcon fontSize="small" />,
      onClick: () => setKeyDialog({ kind: "import" }),
    },
    {
      label: "New identity",
      icon: <PersonRoundedIcon fontSize="small" />,
      onClick: () => setIdDialog({ kind: "edit", card: null }),
      divider: true,
    },
  ];

  return (
    <Page>
      <PageHeader
        actions={
          <SplitButton
            label={tab === "keys" ? "New key" : "New identity"}
            icon={<AddRoundedIcon />}
            disabled={!vaultId}
            onClick={() =>
              tab === "keys"
                ? setKeyDialog({ kind: "generate" })
                : setIdDialog({ kind: "edit", card: null })
            }
            items={newKeyItems}
          />
        }
        trailing={
          <ToggleButtonGroup
            exclusive
            value={tab}
            onChange={(_, v: "keys" | "identities" | null) => v && setTab(v)}
          >
            <ToggleButton value="keys">Keys · {keyList.length}</ToggleButton>
            <ToggleButton value="identities">Identities · {idList.length}</ToggleButton>
          </ToggleButtonGroup>
        }
      />
      <PageBody>
        {loading ? (
          <Loading />
        ) : loadError ? (
          <EmptyState title="Could not open the keychain" description={errorMessage(loadError)} />
        ) : tab === "keys" ? (
          keyList.length === 0 ? (
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
            <Stack spacing={1}>
              {keyList.map((k) => (
                <EntityCard
                  key={k.id}
                  dense
                  tile={
                    <IconTile size={sizes.tileSmall} tone={k.unreadable ? "warning" : "neutral"}>
                      {k.unreadable ? <WarningAmberRoundedIcon /> : <KeyRoundedIcon />}
                    </IconTile>
                  }
                  title={
                    <>
                      {k.label}
                      {k.comment && (
                        <Typography
                          component="span"
                          variant="caption"
                          color="text.secondary"
                          sx={{ ml: 1 }}
                        >
                          {k.comment}
                        </Typography>
                      )}
                    </>
                  }
                  subtitle={
                    k.unreadable ? (
                      "Could not parse this key"
                    ) : (
                      <>
                        {k.bits > 0 ? `${k.keyType} ${k.bits}` : k.keyType}
                        {" · "}
                        <Mono>{k.fingerprint}</Mono>
                      </>
                    )
                  }
                  trailing={
                    <>
                      {k.encrypted && (
                        <Chip
                          size="small"
                          variant="outlined"
                          icon={<LockOutlinedIcon />}
                          label={k.hasPassphrase ? "Passphrase stored" : "Asks for passphrase"}
                        />
                      )}
                      {k.usedBy > 0 && (
                        <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
                          {k.usedBy} {k.usedBy === 1 ? "use" : "uses"}
                        </Typography>
                      )}
                      <ToolIconButton
                        title="Copy public key"
                        onClick={() =>
                          run(async () => {
                            await copy(await ipc.keyPublic(k.id));
                            return "Public key copied";
                          })
                        }
                      >
                        <ContentCopyRoundedIcon fontSize="small" />
                      </ToolIconButton>
                      <ToolIconButton
                        title="More"
                        onClick={(e) => setMenu({ anchor: e.currentTarget, card: k })}
                      >
                        <MoreHorizRoundedIcon fontSize="small" />
                      </ToolIconButton>
                    </>
                  }
                />
              ))}
            </Stack>
          )
        ) : idList.length === 0 ? (
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
          <Stack spacing={1}>
            {idList.map((i) => (
              <EntityCard
                key={i.id}
                dense
                onClick={() => setIdDialog({ kind: "edit", card: i })}
                tile={
                  <IconTile size={sizes.tileSmall}>
                    <PersonRoundedIcon />
                  </IconTile>
                }
                title={i.label}
                subtitle={
                  <>
                    <Mono>{i.username}</Mono>
                    {" · "}
                    {i.sshKeyLabel
                      ? `key ${i.sshKeyLabel}`
                      : i.hasPassword
                        ? "password"
                        : "no credentials"}
                  </>
                }
                actions={
                  <>
                    <ToolIconButton
                      title="Edit"
                      onClick={(e) => {
                        e.stopPropagation();
                        setIdDialog({ kind: "edit", card: i });
                      }}
                    >
                      <EditRoundedIcon fontSize="small" />
                    </ToolIconButton>
                    <ToolIconButton
                      title="Delete"
                      onClick={(e) => {
                        e.stopPropagation();
                        setIdDialog({ kind: "delete", card: i });
                      }}
                    >
                      <DeleteOutlineRoundedIcon fontSize="small" />
                    </ToolIconButton>
                  </>
                }
              />
            ))}
          </Stack>
        )}
      </PageBody>

      <Menu open={menu !== null} anchorEl={menu?.anchor} onClose={() => setMenu(null)}>
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
            disabled={menu.card.unreadable}
            onClick={() => {
              setKeyDialog({ kind: "exportToHost", card: menu.card });
              setMenu(null);
            }}
          >
            <ListItemIcon>
              <DnsRoundedIcon fontSize="small" />
            </ListItemIcon>
            <ListItemText>Export to host…</ListItemText>
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
      {keyDialog.kind === "exportToHost" && (
        <ExportToHostDialog
          open
          card={keyDialog.card}
          hosts={hosts.data ?? []}
          busy={op.isPending}
          onCancel={closeKey}
          onConfirm={(host) => {
            const card = keyDialog.card;
            run(async () => {
              const r = await ipc.keyExportToHost(card.id, host.id);
              return r.outcome === "added"
                ? `${card.label} added to authorized_keys on ${r.target}`
                : `${card.label} is already authorized on ${r.target}`;
            });
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
