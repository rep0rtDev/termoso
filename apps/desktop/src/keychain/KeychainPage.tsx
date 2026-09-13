import { useMemo, useState, type MouseEvent, type ReactElement } from "react";
import {
  Box,
  Button,
  Chip,
  Stack,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import AutoFixHighRoundedIcon from "@mui/icons-material/AutoFixHighRounded";
import BadgeOutlinedIcon from "@mui/icons-material/BadgeOutlined";
import WorkspacePremiumOutlinedIcon from "@mui/icons-material/WorkspacePremiumOutlined";
import UsbRoundedIcon from "@mui/icons-material/UsbRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import GridViewRoundedIcon from "@mui/icons-material/GridViewRounded";
import ViewListRoundedIcon from "@mui/icons-material/ViewListRounded";
import EditOutlinedIcon from "@mui/icons-material/EditOutlined";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import LockResetRoundedIcon from "@mui/icons-material/LockResetRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import FileDownloadRoundedIcon from "@mui/icons-material/FileDownloadRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import DriveFileMoveOutlinedIcon from "@mui/icons-material/DriveFileMoveOutlined";
import LibraryAddOutlinedIcon from "@mui/icons-material/LibraryAddOutlined";
import GroupAddRoundedIcon from "@mui/icons-material/GroupAddRounded";
import WarningAmberRoundedIcon from "@mui/icons-material/WarningAmberRounded";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { save as saveFile } from "@tauri-apps/plugin-dialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page } from "@/components/PageHeader";
import {
  ActionMenu,
  CardGrid,
  EntityCard,
  Loading,
  SearchField,
  SectionTitle,
  SplitButton,
  Toolbar,
  ToolIconButton,
  type MenuAction,
} from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import {
  useHosts,
  useIdentities,
  useSaveSettings,
  useSettings,
  useSshKeys,
  useVaults,
} from "@/ipc/hooks";
import { openCollaboration, useActiveVault, ViewOnlyChip } from "@/app/vault";
import {
  errorMessage,
  type HostsView,
  type IdentityCard,
  type KeyCard,
  type Uuid,
} from "@/ipc/types";
import { sizes } from "@/theme/theme";
import { ExportKeyDialog, ExportToHostDialog, PassphraseDialog } from "./KeyDialogs";
import {
  EditKeyPanel,
  Fido2Panel,
  GenerateKeyPanel,
  IdentityPanel,
  IdentityTile,
  KeyTile,
  NewKeyPanel,
} from "./KeychainPanels";
import {
  certificateState,
  filterIdentities,
  filterKeys,
  identitySubtitle,
  keyTypeLabel,
} from "./model";

type Panel =
  | { kind: "none" }
  | { kind: "newKey"; certificate: boolean }
  | { kind: "generate" }
  | { kind: "editKey"; id: string }
  | { kind: "identity"; id: string | null }
  | { kind: "fido2" };

type Dialog =
  | { kind: "none" }
  | { kind: "passphrase"; card: KeyCard }
  | { kind: "export"; card: KeyCard }
  | { kind: "exportToHost"; card: KeyCard }
  | { kind: "deleteKey"; card: KeyCard }
  | { kind: "deleteIdentity"; card: IdentityCard };

type Ctx =
  | { kind: "key"; card: KeyCard; left: number; top: number }
  | { kind: "identity"; card: IdentityCard; left: number; top: number };

async function copy(text: string) {
  await navigator.clipboard.writeText(text);
}

export function KeychainPage() {
  const vault = useActiveVault();
  return <KeychainBody key={vault.data?.id ?? ""} vault={vault} />;
}

/** Keyed by vault id so panels, dialogs and search reset when the vault changes. */
function KeychainBody({ vault }: { vault: ReturnType<typeof useActiveVault> }) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vaultId = vault.data?.id ?? null;
  const vaultName = vault.data?.name ?? "Vault";
  const readOnly = vault.readOnly;
  const sshKeys = useSshKeys(vaultId);
  const identities = useIdentities(vaultId);
  const hosts = useHosts(vaultId);
  const vaults = useVaults();
  const settings = useSettings();
  const saveSettings = useSaveSettings();

  const [panel, setPanel] = useState<Panel>({ kind: "none" });
  const [dialog, setDialog] = useState<Dialog>({ kind: "none" });
  const [ctx, setCtx] = useState<Ctx | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [query, setQuery] = useState("");

  const view: HostsView = settings.data?.keychainView ?? "grid";
  const setView = (v: HostsView | null) => {
    if (!v || !settings.data) return;
    saveSettings.mutate(
      { ...settings.data, keychainView: v },
      { onError: (e) => snackbar.error(errorMessage(e)) },
    );
  };

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["sshKeys"] });
    void qc.invalidateQueries({ queryKey: ["identities"] });
    void qc.invalidateQueries({ queryKey: ["hosts"] });
  };
  const closeDialog = () => setDialog({ kind: "none" });
  const closePanel = () => setPanel({ kind: "none" });

  /** Card / menu actions: errors go to the snackbar. */
  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      refresh();
      closeDialog();
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  const run = (job: () => Promise<string | null>) => op.mutate(job);

  /** Panel saves: errors stay inline in the panel (wrong passphrase, bad certificate…). */
  const panelOp = useMutation({
    mutationFn: async (job: () => Promise<{ msg: string | null; next: Panel }>) => job(),
    onSuccess: ({ msg, next }) => {
      refresh();
      setPanel(next);
      if (msg) snackbar.notify(msg);
    },
  });
  const panelError = panelOp.isError ? errorMessage(panelOp.error) : null;
  const runPanel = (job: () => Promise<{ msg: string | null; next: Panel }>) => panelOp.mutate(job);
  const openPanel = (p: Panel) => {
    panelOp.reset();
    setPanel(p);
  };

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
  const keyList = useMemo(() => sshKeys.data ?? [], [sshKeys.data]);
  const idList = useMemo(() => identities.data ?? [], [identities.data]);
  const shownKeys = useMemo(() => filterKeys(keyList, query), [keyList, query]);
  const shownIds = useMemo(() => filterIdentities(idList, query), [idList, query]);
  const editing =
    panel.kind === "editKey" ? (keyList.find((k) => k.id === panel.id) ?? null) : null;
  const editingIdentity =
    panel.kind === "identity" && panel.id !== null
      ? (idList.find((i) => i.id === panel.id) ?? null)
      : null;

  const vaultTargets = (
    card: { id: Uuid; vaultId: Uuid; label: string },
    move: boolean,
    copyTo: (id: Uuid, vaultId: Uuid, move: boolean) => Promise<unknown>,
    panelKind: "editKey" | "identity",
  ): MenuAction[] => {
    const others = (vaults.data ?? []).filter((v) => v.id !== card.vaultId);
    if (others.length === 0) return [{ label: "No other vaults", disabled: true }];
    return others.map((v) => ({
      label: v.name,
      icon: v.unlocked ? undefined : <LockOutlinedIcon fontSize="small" />,
      disabled: !v.unlocked || v.role === "viewer",
      onClick: () =>
        run(async () => {
          await copyTo(card.id, v.id, move);
          if (move && panel.kind === panelKind && panel.id === card.id) closePanel();
          return `${card.label} ${move ? "moved" : "copied"} to ${v.name}`;
        }),
    }));
  };

  const keyMenu = (card: KeyCard, inPanel: boolean): MenuAction[] => [
    ...(inPanel
      ? []
      : [
          {
            label: "Edit",
            icon: <EditOutlinedIcon fontSize="small" />,
            onClick: () => openPanel({ kind: "editKey", id: card.id }),
          },
        ]),
    {
      label: "Copy public key",
      icon: <ContentCopyRoundedIcon fontSize="small" />,
      disabled: card.unreadable,
      onClick: () =>
        run(async () => {
          await copy(await ipc.keyPublic(card.id));
          return "Public key copied";
        }),
    },
    {
      label: "Export to host…",
      icon: <DnsRoundedIcon fontSize="small" />,
      disabled: card.unreadable,
      onClick: () => setDialog({ kind: "exportToHost", card }),
    },
    {
      label: "Export private key…",
      icon: <FileDownloadRoundedIcon fontSize="small" />,
      disabled: card.unreadable,
      onClick: () => setDialog({ kind: "export", card }),
    },
    {
      label: "Change passphrase…",
      icon: <LockResetRoundedIcon fontSize="small" />,
      disabled: card.unreadable || readOnly,
      onClick: () => setDialog({ kind: "passphrase", card }),
      divider: true,
    },
    {
      label: "Collaborate",
      icon: <GroupAddRoundedIcon fontSize="small" />,
      disabled: vault.data?.kind !== "team",
      onClick: () => openCollaboration(vault.data),
    },
    {
      label: "Move to",
      icon: <DriveFileMoveOutlinedIcon fontSize="small" />,
      disabled: readOnly,
      items: vaultTargets(card, true, ipc.keyCopyToVault, "editKey"),
    },
    {
      label: "Copy to",
      icon: <LibraryAddOutlinedIcon fontSize="small" />,
      items: vaultTargets(card, false, ipc.keyCopyToVault, "editKey"),
    },
    {
      label: "Remove",
      icon: <DeleteOutlineRoundedIcon fontSize="small" />,
      danger: true,
      disabled: readOnly,
      onClick: () => setDialog({ kind: "deleteKey", card }),
    },
  ];

  const identityMenu = (card: IdentityCard, inPanel: boolean): MenuAction[] => [
    ...(inPanel
      ? []
      : [
          {
            label: "Edit",
            icon: <EditOutlinedIcon fontSize="small" />,
            onClick: () => openPanel({ kind: "identity", id: card.id }),
            divider: true,
          },
        ]),
    {
      label: "Collaborate",
      icon: <GroupAddRoundedIcon fontSize="small" />,
      disabled: vault.data?.kind !== "team",
      onClick: () => openCollaboration(vault.data),
    },
    {
      label: "Move to",
      icon: <DriveFileMoveOutlinedIcon fontSize="small" />,
      disabled: readOnly,
      items: vaultTargets(card, true, ipc.identityCopyToVault, "identity"),
    },
    {
      label: "Copy to",
      icon: <LibraryAddOutlinedIcon fontSize="small" />,
      items: vaultTargets(card, false, ipc.identityCopyToVault, "identity"),
    },
    {
      label: "Remove",
      icon: <DeleteOutlineRoundedIcon fontSize="small" />,
      danger: true,
      disabled: readOnly,
      onClick: () => setDialog({ kind: "deleteIdentity", card }),
    },
  ];

  const onKeyContext = (e: MouseEvent<HTMLElement>, card: KeyCard) => {
    e.preventDefault();
    setCtx({ kind: "key", card, left: e.clientX, top: e.clientY });
  };
  const onIdContext = (e: MouseEvent<HTMLElement>, card: IdentityCard) => {
    e.preventDefault();
    setCtx({ kind: "identity", card, left: e.clientX, top: e.clientY });
  };

  const keyTrailing = (k: KeyCard) => {
    const cs = certificateState(k.certificate, k.certificateUnreadable);
    const certLabel =
      cs === "valid"
        ? "Certificate"
        : cs === "expired"
          ? "Certificate expired"
          : cs === "not_yet"
            ? "Certificate not yet valid"
            : "Certificate unreadable";
    return (
      <>
        {k.unreadable && <WarningAmberRoundedIcon fontSize="small" color="warning" />}
        {cs &&
          (view === "list" ? (
            <Chip
              size="small"
              variant="outlined"
              color={cs === "valid" ? "success" : "warning"}
              icon={<WorkspacePremiumOutlinedIcon />}
              label={certLabel}
            />
          ) : (
            <WorkspacePremiumOutlinedIcon
              fontSize="small"
              color={cs === "valid" ? "success" : "warning"}
              titleAccess={certLabel}
            />
          ))}
        {k.encrypted && (
          <LockOutlinedIcon
            fontSize="small"
            sx={{ color: "text.disabled" }}
            titleAccess={k.hasPassphrase ? "Passphrase remembered" : "Asks for passphrase"}
          />
        )}
      </>
    );
  };

  const keyCard = (k: KeyCard) => (
    <EntityCard
      key={k.id}
      dense={view === "list"}
      tile={<KeyTile card={k} size={view === "list" ? sizes.tileSmall : sizes.tile} />}
      title={k.label}
      subtitle={
        <>
          {keyTypeLabel(k)}
          {k.comment && view === "list" && (
            <Typography component="span" variant="caption" color="text.secondary" sx={{ ml: 1 }}>
              {k.comment}
            </Typography>
          )}
        </>
      }
      trailing={keyTrailing(k)}
      actions={
        <ToolIconButton
          title="Edit"
          onClick={(e) => {
            e.stopPropagation();
            openPanel({ kind: "editKey", id: k.id });
          }}
        >
          <EditOutlinedIcon fontSize="small" />
        </ToolIconButton>
      }
      selected={panel.kind === "editKey" && panel.id === k.id}
      onClick={() => openPanel({ kind: "editKey", id: k.id })}
      onContextMenu={(e) => onKeyContext(e, k)}
    />
  );

  const identityCard = (i: IdentityCard) => (
    <EntityCard
      key={i.id}
      dense={view === "list"}
      tile={<IdentityTile size={view === "list" ? sizes.tileSmall : sizes.tile} />}
      title={i.label}
      subtitle={identitySubtitle(i)}
      actions={
        <ToolIconButton
          title="Edit"
          onClick={(e) => {
            e.stopPropagation();
            openPanel({ kind: "identity", id: i.id });
          }}
        >
          <EditOutlinedIcon fontSize="small" />
        </ToolIconButton>
      }
      selected={panel.kind === "identity" && panel.id === i.id}
      onClick={() => openPanel({ kind: "identity", id: i.id })}
      onContextMenu={(e) => onIdContext(e, i)}
    />
  );

  const wrap = (cards: ReactElement[]) =>
    view === "grid" ? <CardGrid min={300}>{cards}</CardGrid> : <Stack spacing={1}>{cards}</Stack>;

  const newItems: MenuAction[] = [
    {
      label: "Generate key",
      icon: <AutoFixHighRoundedIcon fontSize="small" />,
      onClick: () => openPanel({ kind: "generate" }),
    },
    {
      label: "New identity",
      icon: <BadgeOutlinedIcon fontSize="small" />,
      onClick: () => openPanel({ kind: "identity", id: null }),
    },
  ];

  const empty = keyList.length === 0 && idList.length === 0;

  return (
    <Box sx={{ display: "flex", flex: 1, minHeight: 0 }}>
      <Page>
        <Toolbar
          trailing={
            <>
              {searchOpen ? (
                <SearchField
                  autoFocus
                  value={query}
                  onChange={setQuery}
                  placeholder="Search keys and identities"
                />
              ) : null}
              <ToolIconButton
                title="Search"
                active={searchOpen}
                onClick={() => {
                  if (searchOpen) setQuery("");
                  setSearchOpen((v) => !v);
                }}
              >
                <SearchRoundedIcon fontSize="small" />
              </ToolIconButton>
              <ToggleButtonGroup
                exclusive
                value={view}
                onChange={(_e, v: HostsView | null) => setView(v)}
              >
                <ToggleButton value="grid" aria-label="Grid view">
                  <GridViewRoundedIcon sx={{ fontSize: 18 }} />
                </ToggleButton>
                <ToggleButton value="list" aria-label="List view">
                  <ViewListRoundedIcon sx={{ fontSize: 18 }} />
                </ToggleButton>
              </ToggleButtonGroup>
            </>
          }
        >
          <SplitButton
            label="New key"
            icon={<AddRoundedIcon />}
            disabled={!vaultId || readOnly}
            onClick={() => openPanel({ kind: "newKey", certificate: false })}
            items={newItems}
          />
          <Button
            variant="tonal"
            startIcon={<WorkspacePremiumOutlinedIcon />}
            disabled={!vaultId || readOnly}
            onClick={() => openPanel({ kind: "newKey", certificate: true })}
          >
            Certificate
          </Button>
          <Button
            variant="text"
            color="inherit"
            startIcon={<UsbRoundedIcon />}
            disabled={!vaultId || readOnly}
            onClick={() => openPanel({ kind: "fido2" })}
          >
            FIDO2
          </Button>
          {readOnly && <ViewOnlyChip sx={{ ml: 1 }} />}
        </Toolbar>

        <Box sx={{ flex: 1, minHeight: 0, overflowY: "auto", px: 3, py: 2 }}>
          {loading ? (
            <Loading />
          ) : loadError ? (
            <EmptyState title="Could not open the keychain" description={errorMessage(loadError)} />
          ) : empty ? (
            <EmptyState
              icon={<KeyRoundedIcon />}
              title="No keys yet"
              description="Paste or drop a private key (OpenSSH, PEM, PuTTY .ppk), generate a new one, or attach a certificate. Everything is stored encrypted with your master key."
              action={
                <Stack direction="row" spacing={1}>
                  <Button
                    variant="contained"
                    onClick={() => openPanel({ kind: "newKey", certificate: false })}
                  >
                    New key
                  </Button>
                  <Button variant="tonal" onClick={() => openPanel({ kind: "generate" })}>
                    Generate key
                  </Button>
                </Stack>
              }
            />
          ) : (
            <Stack spacing={3}>
              {(shownKeys.length > 0 || !query) && (
                <Box>
                  <SectionTitle>Keys</SectionTitle>
                  {shownKeys.length === 0 ? (
                    <Typography variant="body2" color="text.secondary">
                      No keys yet — use New key or Generate key above.
                    </Typography>
                  ) : (
                    wrap(shownKeys.map(keyCard))
                  )}
                </Box>
              )}
              {(shownIds.length > 0 || !query) && (
                <Box>
                  <SectionTitle>Identities</SectionTitle>
                  {shownIds.length === 0 ? (
                    <Typography variant="body2" color="text.secondary">
                      An identity bundles a username with a password, key or certificate so several
                      hosts can share it.{" "}
                      <Button
                        size="small"
                        variant="text"
                        onClick={() => openPanel({ kind: "identity", id: null })}
                      >
                        New identity
                      </Button>
                    </Typography>
                  ) : (
                    wrap(shownIds.map(identityCard))
                  )}
                </Box>
              )}
              {query && shownKeys.length === 0 && shownIds.length === 0 && (
                <EmptyState
                  compact
                  title="Nothing matches"
                  description={`No key or identity matches “${query}”.`}
                />
              )}
            </Stack>
          )}
        </Box>
      </Page>

      {vaultId && panel.kind === "newKey" && (
        <NewKeyPanel
          key={panel.certificate ? "cert" : "key"}
          vaultId={vaultId}
          vaultName={vaultName}
          focusCertificate={panel.certificate}
          busy={panelOp.isPending}
          error={panelError}
          onClose={closePanel}
          onImport={(form) =>
            runPanel(async () => {
              const k = await ipc.keyImport(form);
              return { msg: `Imported ${k.label}`, next: { kind: "editKey", id: k.id } };
            })
          }
          onImportFile={(args) =>
            runPanel(async () => {
              const k = await ipc.keyImportFile(args);
              return { msg: `Imported ${k.label}`, next: { kind: "editKey", id: k.id } };
            })
          }
        />
      )}
      {vaultId && panel.kind === "generate" && (
        <GenerateKeyPanel
          vaultId={vaultId}
          vaultName={vaultName}
          busy={panelOp.isPending}
          error={panelError}
          onClose={closePanel}
          onGenerate={(form) =>
            runPanel(async () => {
              const k = await ipc.keyGenerate(form);
              return { msg: `Generated ${k.label}`, next: { kind: "editKey", id: k.id } };
            })
          }
        />
      )}
      {panel.kind === "editKey" && editing && (
        <EditKeyPanel
          key={editing.id}
          card={editing}
          vaultName={vaultName}
          busy={panelOp.isPending || op.isPending}
          readOnly={readOnly}
          error={panelError}
          menu={keyMenu(editing, true)}
          onClose={closePanel}
          onRename={(label) =>
            runPanel(async () => {
              await ipc.keyRename(editing.id, label);
              return { msg: null, next: panel };
            })
          }
          onSetCertificate={(text) =>
            runPanel(async () => {
              await ipc.keySetCertificate(editing.id, text);
              return {
                msg: text === null ? "Certificate removed" : "Certificate attached",
                next: panel,
              };
            })
          }
          onSetCertificateFile={(path) =>
            runPanel(async () => {
              await ipc.keySetCertificateFile(editing.id, path);
              return { msg: "Certificate attached", next: panel };
            })
          }
          onExportToHost={() => setDialog({ kind: "exportToHost", card: editing })}
          onExportPrivate={() => setDialog({ kind: "export", card: editing })}
          onChangePassphrase={() => setDialog({ kind: "passphrase", card: editing })}
        />
      )}
      {vaultId && panel.kind === "identity" && (panel.id === null || editingIdentity) && (
        <IdentityPanel
          key={panel.id ?? "new"}
          vaultId={vaultId}
          vaultName={vaultName}
          initial={editingIdentity}
          keys={keyList}
          busy={panelOp.isPending}
          readOnly={readOnly}
          error={panelError}
          menu={editingIdentity ? identityMenu(editingIdentity, true) : []}
          onClose={closePanel}
          onNewKey={() => openPanel({ kind: "newKey", certificate: false })}
          onSave={(form) =>
            runPanel(async () => {
              const saved = await ipc.identitySave(form);
              return {
                msg: form.id ? null : `Identity ${saved.label} created`,
                next: { kind: "identity", id: saved.id },
              };
            })
          }
        />
      )}
      {panel.kind === "fido2" && <Fido2Panel vaultName={vaultName} onClose={closePanel} />}

      <ActionMenu
        anchor={null}
        position={ctx ? { left: ctx.left, top: ctx.top } : null}
        onClose={() => setCtx(null)}
        items={
          ctx === null
            ? []
            : ctx.kind === "key"
              ? keyMenu(ctx.card, false)
              : identityMenu(ctx.card, false)
        }
      />

      {dialog.kind === "passphrase" && (
        <PassphraseDialog
          open
          card={dialog.card}
          busy={op.isPending}
          onCancel={closeDialog}
          onConfirm={(args) => {
            const id = dialog.card.id;
            run(async () => {
              await ipc.keyChangePassphrase({ id, ...args });
              return "Passphrase updated";
            });
          }}
        />
      )}
      {dialog.kind === "export" && (
        <ExportKeyDialog
          open
          card={dialog.card}
          busy={op.isPending}
          onCancel={closeDialog}
          onConfirm={(args) => {
            const card = dialog.card;
            run(() => exportKey(card, args));
          }}
        />
      )}
      {dialog.kind === "exportToHost" && (
        <ExportToHostDialog
          open
          card={dialog.card}
          hosts={hosts.data ?? []}
          busy={op.isPending}
          onCancel={closeDialog}
          onConfirm={(host) => {
            const card = dialog.card;
            run(async () => {
              const r = await ipc.keyExportToHost(card.id, host.id);
              return r.outcome === "added"
                ? `${card.label} added to authorized_keys on ${r.target}`
                : `${card.label} is already authorized on ${r.target}`;
            });
          }}
        />
      )}
      {dialog.kind === "deleteKey" && (
        <ConfirmDialog
          open
          title="Remove key?"
          confirmLabel="Remove"
          danger
          busy={op.isPending}
          onCancel={closeDialog}
          onConfirm={() => {
            const card = dialog.card;
            run(async () => {
              await ipc.keyDelete(card.id);
              if (panel.kind === "editKey" && panel.id === card.id) closePanel();
              return `Removed ${card.label}`;
            });
          }}
        >
          <b>{dialog.card.label}</b>
          {dialog.card.certificate ? " and its certificate" : ""} will be removed from the vault
          {dialog.card.usedBy > 0 &&
            ` and detached from ${dialog.card.usedBy} host(s) / identit(ies)`}
          . This cannot be undone.
        </ConfirmDialog>
      )}
      {dialog.kind === "deleteIdentity" && (
        <ConfirmDialog
          open
          title="Remove identity?"
          confirmLabel="Remove"
          danger
          busy={op.isPending}
          onCancel={closeDialog}
          onConfirm={() => {
            const card = dialog.card;
            run(async () => {
              await ipc.identityDelete(card.id);
              if (panel.kind === "identity" && panel.id === card.id) closePanel();
              return `Removed ${card.label}`;
            });
          }}
        >
          Hosts using <b>{dialog.card.label}</b> will fall back to inline credentials.
        </ConfirmDialog>
      )}
    </Box>
  );
}
