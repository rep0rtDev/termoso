import { useEffect, useMemo, useState, type MouseEvent } from "react";
import {
  Box,
  Button,
  CircularProgress,
  MenuItem,
  Menu,
  Stack,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import SwapHorizRoundedIcon from "@mui/icons-material/SwapHorizRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import StopRoundedIcon from "@mui/icons-material/StopRounded";
import EditOutlinedIcon from "@mui/icons-material/EditOutlined";
import GroupAddRoundedIcon from "@mui/icons-material/GroupAddRounded";
import DriveFileMoveOutlinedIcon from "@mui/icons-material/DriveFileMoveOutlined";
import LibraryAddOutlinedIcon from "@mui/icons-material/LibraryAddOutlined";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import GridViewRoundedIcon from "@mui/icons-material/GridViewRounded";
import ViewListRoundedIcon from "@mui/icons-material/ViewListRounded";
import SwapVertRoundedIcon from "@mui/icons-material/SwapVertRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Page } from "@/components/PageHeader";
import {
  ActionMenu,
  CardGrid,
  EntityCard,
  Loading,
  SplitButton,
  Toolbar,
  ToolIconButton,
  type MenuAction,
} from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useHosts, usePfRules, useSaveSettings, useSettings, useVaults } from "@/ipc/hooks";
import { openCollaboration, useActiveVault, ViewOnlyChip } from "@/app/vault";
import { useForwardRequests } from "@/app/navigation";
import {
  errorMessage,
  type HostsView,
  type PfKind,
  type PfRuleCard,
  type PfRuleForm,
  type Uuid,
} from "@/ipc/types";
import { KIND_NAME, emptyRuleForm, routeLine, ruleTitle, ruleToForm } from "./model";
import { RuleEditor, RuleTile, RuleWizard } from "./ForwardingPanels";

type Panel =
  | { mode: "closed" }
  | { mode: "wizard"; form: PfRuleForm }
  | { mode: "new"; form: PfRuleForm }
  | { mode: "edit"; id: Uuid; form: PfRuleForm };

type SortKey = "az" | "za" | "newest" | "oldest";

const sortLabel: Record<SortKey, string> = {
  az: "A-Z",
  za: "Z-A",
  newest: "Newest to oldest",
  oldest: "Oldest to newest",
};

const comparators: Record<SortKey, (a: PfRuleCard, b: PfRuleCard) => number> = {
  az: (a, b) => ruleTitle(a).localeCompare(ruleTitle(b)),
  za: (a, b) => ruleTitle(b).localeCompare(ruleTitle(a)),
  newest: (a, b) => b.updatedAt.localeCompare(a.updatedAt),
  oldest: (a, b) => a.updatedAt.localeCompare(b.updatedAt),
};

/** Right-hand status of a card: spinner while (re)connecting, warning when the last run failed. */
function RuleStatus({ r }: { r: PfRuleCard }) {
  const rt = r.runtime;
  if (rt.state === "starting") return <CircularProgress size={16} />;
  if (rt.state === "reconnecting") {
    return (
      <Tooltip
        title={`Reconnecting, attempt ${rt.attempt}${rt.lastError ? ` — ${rt.lastError}` : ""}`}
      >
        <Box sx={{ display: "flex", alignItems: "center", gap: 0.75 }}>
          <CircularProgress size={16} color="warning" />
          <Typography variant="caption" color="warning.main">
            Reconnecting…
          </Typography>
        </Box>
      </Tooltip>
    );
  }
  if (rt.state === "stopped" && rt.lastError) {
    return (
      <Tooltip title={rt.lastError}>
        <ErrorOutlineRoundedIcon color="error" sx={{ fontSize: 18 }} />
      </Tooltip>
    );
  }
  return null;
}

export function ForwardingPage() {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vault = useActiveVault();
  const vaultId = vault.data?.id ?? null;
  const vaultName = vault.data?.name ?? "";
  const readOnly = vault.readOnly;
  const rules = usePfRules(vaultId);
  const hosts = useHosts(vaultId);
  const vaults = useVaults();
  const settings = useSettings();
  const saveSettings = useSaveSettings();

  const [panel, setPanel] = useState<Panel>({ mode: "closed" });
  const [selectedId, setSelectedId] = useState<Uuid | null>(null);
  const [ctx, setCtx] = useState<{ rule: PfRuleCard; left: number; top: number } | null>(null);
  const [sort, setSort] = useState<SortKey>("newest");
  const [sortAnchor, setSortAnchor] = useState<HTMLElement | null>(null);
  const [confirmRemove, setConfirmRemove] = useState<PfRuleCard | null>(null);
  const [busyId, setBusyId] = useState<Uuid | null>(null);

  const openNew = (kind: PfKind, hostId: Uuid | null = null) => {
    if (!vaultId) return;
    setPanel({ mode: "new", form: emptyRuleForm(vaultId, kind, hostId) });
  };
  const openWizard = (kind: PfKind = "local") => {
    if (!vaultId) return;
    setPanel({ mode: "wizard", form: emptyRuleForm(vaultId, kind, null) });
  };
  const openEdit = (r: PfRuleCard) => {
    setSelectedId(r.id);
    setPanel({ mode: "edit", id: r.id, form: ruleToForm(r) });
  };
  useForwardRequests((hostId) => openNew("local", hostId));

  useEffect(() => {
    let active = true;
    const un = ipc.onForwardEvent((ev) => {
      if (!active) return;
      qc.setQueriesData<PfRuleCard[]>({ queryKey: ["pfRules"] }, (old) =>
        old?.map((r) => (r.id === ev.id ? { ...r, runtime: ev.runtime } : r)),
      );
    });
    return () => {
      active = false;
      void un.then((f) => f());
    };
  }, [qc]);

  const anyLive = (rules.data ?? []).some((r) => r.runtime.state !== "stopped");
  useEffect(() => {
    if (!anyLive) return;
    const timer = setInterval(() => {
      ipc
        .pfRuntimes()
        .then((live) =>
          qc.setQueriesData<PfRuleCard[]>({ queryKey: ["pfRules"] }, (old) =>
            old?.map((r) => {
              const runtime = live[r.id];
              return runtime ? { ...r, runtime } : r;
            }),
          ),
        )
        .catch(() => undefined);
    }, 2000);
    return () => clearInterval(timer);
  }, [anyLive, qc]);

  const view: HostsView = settings.data?.forwardingView ?? "grid";
  const setView = (v: HostsView | null) => {
    if (!v || !settings.data) return;
    saveSettings.mutate(
      { ...settings.data, forwardingView: v },
      { onError: (e) => snackbar.error(errorMessage(e)) },
    );
  };

  const invalidate = () => void qc.invalidateQueries({ queryKey: ["pfRules"] });
  /** Put a fresh card into the cache right away so the panel can point at it before the refetch lands. */
  const upsert = (card: PfRuleCard) =>
    qc.setQueryData<PfRuleCard[]>(keys.pfRules(vaultId), (old) =>
      old
        ? old.some((r) => r.id === card.id)
          ? old.map((r) => (r.id === card.id ? card : r))
          : [...old, card]
        : old,
    );
  const op = useMutation({
    mutationFn: async (job: () => Promise<string | null>) => job(),
    onSuccess: (msg) => {
      invalidate();
      if (msg) snackbar.notify(msg);
    },
    onError: (e) => snackbar.error(errorMessage(e)),
    onSettled: () => setBusyId(null),
  });

  const toggle = (r: PfRuleCard) => {
    if (busyId) return;
    setBusyId(r.id);
    op.mutate(async () => {
      if (r.runtime.state === "stopped") await ipc.pfStart(r.id);
      else await ipc.pfStop(r.id);
      return null;
    });
  };

  const save = useMutation({
    mutationFn: (form: PfRuleForm) => ipc.pfSave(form),
    onSuccess: (card) => {
      upsert(card);
      invalidate();
      setSelectedId(card.id);
      setPanel({ mode: "edit", id: card.id, form: ruleToForm(card) });
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const duplicate = (r: PfRuleCard) =>
    op.mutate(async () => {
      const copy = await ipc.pfDuplicate(r.id);
      upsert(copy);
      openEdit(copy);
      return null;
    });

  const remove = (r: PfRuleCard) =>
    op.mutate(async () => {
      await ipc.pfDelete(r.id);
      setConfirmRemove(null);
      if (selectedId === r.id) setSelectedId(null);
      if (panel.mode === "edit" && panel.id === r.id) setPanel({ mode: "closed" });
      return null;
    });

  const transfer = (r: PfRuleCard, targetVaultId: Uuid, move: boolean) =>
    op.mutate(async () => {
      await ipc.pfCopyToVault(r.id, targetVaultId, move);
      if (move && panel.mode === "edit" && panel.id === r.id) setPanel({ mode: "closed" });
      const target = (vaults.data ?? []).find((v) => v.id === targetVaultId);
      return `${move ? "Moved" : "Copied"} to ${target?.name ?? "vault"}`;
    });

  const vaultTargets = (r: PfRuleCard, move: boolean): MenuAction[] => {
    const others = (vaults.data ?? []).filter((v) => v.id !== r.vaultId);
    if (others.length === 0) return [{ label: "No other vaults", disabled: true }];
    return others.map((v) => ({
      label: v.name,
      icon: v.unlocked ? undefined : <LockOutlinedIcon fontSize="small" />,
      disabled: !v.unlocked || v.role === "viewer",
      onClick: () => transfer(r, v.id, move),
    }));
  };

  const ruleMenu = (r: PfRuleCard): MenuAction[] => {
    const running = r.runtime.state !== "stopped";
    return [
      {
        label: running ? "Disconnect" : "Connect",
        icon: running ? (
          <StopRoundedIcon fontSize="small" />
        ) : (
          <PlayArrowRoundedIcon fontSize="small" />
        ),
        onClick: () => toggle(r),
      },
      {
        label: "Edit",
        icon: <EditOutlinedIcon fontSize="small" />,
        onClick: () => openEdit(r),
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
        items: vaultTargets(r, true),
      },
      {
        label: "Copy to",
        icon: <LibraryAddOutlinedIcon fontSize="small" />,
        items: vaultTargets(r, false),
      },
      {
        label: "Duplicate",
        icon: <ContentCopyRoundedIcon fontSize="small" />,
        disabled: readOnly,
        onClick: () => duplicate(r),
      },
      {
        label: "Remove",
        icon: <DeleteOutlineRoundedIcon fontSize="small" />,
        disabled: readOnly,
        onClick: () => setConfirmRemove(r),
        danger: true,
      },
    ];
  };

  const onContext = (e: MouseEvent<HTMLElement>, r: PfRuleCard) => {
    e.preventDefault();
    setSelectedId(r.id);
    setCtx({ rule: r, left: e.clientX, top: e.clientY });
  };

  const sorted = useMemo(() => [...(rules.data ?? [])].sort(comparators[sort]), [rules.data, sort]);
  const editing = panel.mode === "edit" ? (sorted.find((r) => r.id === panel.id) ?? null) : null;

  const loading = vault.isPending || rules.isPending || hosts.isPending;
  const loadError = vault.error ?? rules.error ?? hosts.error;

  const card = (r: PfRuleCard) => {
    const running = r.runtime.state === "running";
    return (
      <EntityCard
        key={r.id}
        tile={<RuleTile kind={r.kind} active={running} />}
        title={ruleTitle(r)}
        subtitle={routeLine(r)}
        trailing={busyId === r.id ? <CircularProgress size={16} /> : <RuleStatus r={r} />}
        actions={
          <ToolIconButton
            title="Edit"
            onClick={(e) => {
              e.stopPropagation();
              openEdit(r);
            }}
          >
            <EditOutlinedIcon fontSize="small" />
          </ToolIconButton>
        }
        selected={selectedId === r.id}
        onClick={() => {
          setSelectedId(r.id);
          if (panel.mode === "edit") openEdit(r);
        }}
        onDoubleClick={() => toggle(r)}
        onContextMenu={(e) => onContext(e, r)}
      />
    );
  };

  const newItems: MenuAction[] = (["local", "remote", "dynamic"] as PfKind[]).map((k) => ({
    label: `${KIND_NAME[k]} Forwarding`,
    icon: <RuleTile kind={k} size={20} />,
    onClick: () => openNew(k),
  }));

  return (
    <Box sx={{ display: "flex", flex: 1, minHeight: 0 }}>
      <Page>
        <Toolbar
          trailing={
            <>
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
              <Button
                variant="text"
                size="small"
                startIcon={<SwapVertRoundedIcon />}
                onClick={(e) => setSortAnchor(e.currentTarget)}
                sx={{ color: "text.secondary" }}
              >
                {sortLabel[sort]}
              </Button>
            </>
          }
        >
          <SplitButton
            label="New forwarding"
            icon={<AddRoundedIcon />}
            disabled={!vaultId || readOnly}
            onClick={() => openWizard()}
            items={newItems}
          />
          {readOnly && <ViewOnlyChip sx={{ ml: 1 }} />}
        </Toolbar>

        <Box sx={{ flex: 1, minHeight: 0, overflowY: "auto", px: 3, py: 2 }}>
          {loading ? (
            <Loading />
          ) : loadError ? (
            <Typography color="error">{errorMessage(loadError)}</Typography>
          ) : sorted.length === 0 ? (
            <EmptyState
              icon={<SwapHorizRoundedIcon />}
              title="No port forwarding rules"
              description="Forward a local port through an SSH host, expose a local service on the remote side, or run a SOCKS5 proxy."
              action={
                vaultId && (
                  <Button
                    variant="tonal"
                    startIcon={<AddRoundedIcon />}
                    onClick={() => openWizard()}
                  >
                    New forwarding
                  </Button>
                )
              }
            />
          ) : (
            <Box>
              <Typography variant="subtitle2" sx={{ mb: 1.5 }}>
                Port Forwarding
              </Typography>
              {view === "grid" ? (
                <CardGrid min={260}>{sorted.map(card)}</CardGrid>
              ) : (
                <Stack spacing={1}>{sorted.map(card)}</Stack>
              )}
            </Box>
          )}
        </Box>
      </Page>

      {panel.mode === "wizard" && vaultId && (
        <RuleWizard
          key="wizard"
          form={panel.form}
          hosts={hosts.data ?? []}
          vaultName={vaultName}
          saving={save.isPending}
          onChange={(form) => setPanel({ mode: "wizard", form })}
          onSave={() => save.mutate(panel.form)}
          onSkip={() => setPanel({ mode: "new", form: panel.form })}
          onClose={() => setPanel({ mode: "closed" })}
        />
      )}

      {(panel.mode === "new" || (panel.mode === "edit" && editing)) && vaultId && (
        <RuleEditor
          key={panel.mode === "edit" ? panel.id : "new"}
          form={panel.form}
          rule={editing}
          hosts={hosts.data ?? []}
          vaultName={vaultName}
          saving={save.isPending}
          readOnly={readOnly}
          onChange={(form) =>
            setPanel(panel.mode === "edit" ? { ...panel, form } : { mode: "new", form })
          }
          onSave={() => save.mutate(panel.form)}
          onClose={() => setPanel({ mode: "closed" })}
          onOpenWizard={() => setPanel({ mode: "wizard", form: panel.form })}
          menu={editing ? ruleMenu(editing) : []}
        />
      )}

      <ActionMenu
        anchor={null}
        position={ctx ? { left: ctx.left, top: ctx.top } : null}
        onClose={() => setCtx(null)}
        items={ctx ? ruleMenu(ctx.rule) : []}
      />

      <Menu anchorEl={sortAnchor} open={Boolean(sortAnchor)} onClose={() => setSortAnchor(null)}>
        {(Object.keys(sortLabel) as SortKey[]).map((k) => (
          <MenuItem
            key={k}
            selected={sort === k}
            onClick={() => {
              setSort(k);
              setSortAnchor(null);
            }}
          >
            {sortLabel[k]}
          </MenuItem>
        ))}
      </Menu>

      <ConfirmDialog
        open={confirmRemove !== null}
        title="Remove Port Forwarding rule"
        confirmLabel="Remove"
        danger
        busy={op.isPending}
        onCancel={() => setConfirmRemove(null)}
        onConfirm={() => confirmRemove && remove(confirmRemove)}
      >
        {confirmRemove && (
          <>
            <b>{ruleTitle(confirmRemove)}</b> will be removed
            {confirmRemove.runtime.state !== "stopped" ? " and stopped" : ""}. This cannot be
            undone.
          </>
        )}
      </ConfirmDialog>
    </Box>
  );
}
