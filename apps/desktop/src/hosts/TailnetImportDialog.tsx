import { useCallback, useEffect, useRef, useState } from "react";
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import HubOutlinedIcon from "@mui/icons-material/HubOutlined";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import ComputerRoundedIcon from "@mui/icons-material/ComputerRounded";
import { useSnackbar } from "@/components/Snackbar";
import { Field, IconTile } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { useGroups, useHosts, useSaveHost, useVaults } from "@/ipc/hooks";
import { errorMessage, type TailnetDevice, type TailnetScan, type Uuid } from "@/ipc/types";
import { GroupSelect } from "./CloudImportDialog";
import { Row, VaultSelect } from "./ImportDialog";
import { addLabel } from "./lan";
import {
  PROVIDER_NAMES,
  tailnetAddress,
  tailnetExisting,
  tailnetHostForm,
  tailnetKey,
  tailnetSubtitle,
  type TailnetAddressMode,
} from "./tailnet";
import { tr, trn } from "@/i18n";

interface Props {
  open: boolean;
  vaultId: Uuid;
  onClose: () => void;
  onImported: () => void;
}

/**
 * New host → “Import from Tailscale / NetBird / ZeroTier”. Asks the locally
 * installed overlay-network client for the machines it knows and saves the
 * picked ones as hosts. Nothing leaves the machine and nothing is stored
 * until the user clicks Add.
 */
export function TailnetImportDialog({ open, vaultId, onClose, onImported }: Props) {
  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      {open && <Body vaultId={vaultId} onClose={onClose} onImported={onImported} />}
    </Dialog>
  );
}

type Scan =
  | { kind: "scanning"; result: TailnetScan | null }
  | { kind: "done"; result: TailnetScan | null; error: string | null };

function Body({ vaultId, onClose, onImported }: Omit<Props, "open">) {
  const snackbar = useSnackbar();
  const vaults = useVaults();
  const save = useSaveHost();
  const [target, setTarget] = useState<Uuid>(vaultId);
  const [groupId, setGroupId] = useState<Uuid | null>(null);
  const [username, setUsername] = useState("");
  const [mode, setMode] = useState<TailnetAddressMode>("dns");
  const [scan, setScan] = useState<Scan>({ kind: "scanning", result: null });
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [adding, setAdding] = useState(false);
  const groups = useGroups(target);
  const hosts = useHosts(target);
  const alive = useRef(true);
  const seeded = useRef(false);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const run = useCallback(() => {
    ipc
      .tailnetScan()
      .then((result) => {
        if (!alive.current) return;
        setScan({ kind: "done", result, error: null });
        const keys = result.devices.filter((d) => !d.thisDevice).map(tailnetKey);
        // First results start all peers selected; later scans keep the user's picks.
        setSelected((sel) => {
          if (seeded.current) return new Set(keys.filter((k) => sel.has(k)));
          seeded.current = true;
          return new Set(keys);
        });
      })
      .catch((e: unknown) => {
        if (!alive.current) return;
        setScan((s) => ({ kind: "done", result: s.result, error: errorMessage(e) }));
      });
  }, []);
  useEffect(run, [run]);
  const rescan = () => {
    setScan((s) => ({ kind: "scanning", result: s.result }));
    run();
  };

  const devices = scan.result?.devices ?? [];
  const providers = scan.result?.providers ?? [];
  const installed = providers.filter((p) => p.installed);
  const failed = installed.filter((p) => p.error);

  const vault = (vaults.data ?? []).find((v) => v.id === target);
  const vaultOk = !!vault && vault.unlocked && vault.role !== "viewer";
  const existing = new Map(
    devices.map((d) => [tailnetKey(d), tailnetExisting(d, hosts.data ?? [])] as const),
  );
  const toggle = (d: TailnetDevice) =>
    setSelected((s) => {
      const n = new Set(s);
      const k = tailnetKey(d);
      if (n.has(k)) n.delete(k);
      else n.add(k);
      return n;
    });
  const chosen = devices.filter((d) => selected.has(tailnetKey(d)) && tailnetAddress(d, mode));
  const fresh = chosen.filter((d) => !existing.get(tailnetKey(d)));

  const add = async () => {
    if (!vaultOk || fresh.length === 0) return;
    setAdding(true);
    let added = 0;
    try {
      for (const d of fresh) {
        const form = tailnetHostForm(d, target, groupId, mode, username);
        if (!form) continue;
        await save.mutateAsync(form);
        added += 1;
      }
      snackbar.notify(
        trn(
          added,
          "Added {count} host from the overlay network",
          "Added {count} hosts from the overlay network",
        ),
      );
      onImported();
      onClose();
    } catch (e) {
      snackbar.error(
        `${errorMessage(e)}${added ? ` — ${trn(added, "{count} host was added before the error", "{count} hosts were added before the error")}` : ""}`,
      );
    } finally {
      if (alive.current) setAdding(false);
    }
  };

  const scanning = scan.kind === "scanning";
  const busy = scanning || adding;
  const skipped = chosen.length - fresh.length;
  const providerLine = installed.length
    ? installed.map((p) => PROVIDER_NAMES[p.id]).join(", ")
    : tr("Tailscale, NetBird or ZeroTier installed on this computer");

  return (
    <>
      <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
        <IconTile>
          <HubOutlinedIcon />
        </IconTile>
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography variant="h6" component="div">
            {tr("Import from overlay network")}
          </Typography>
          <Typography variant="body2" color="text.secondary" noWrap>
            {providerLine}
          </Typography>
        </Box>
        <Button
          size="small"
          color="inherit"
          onClick={rescan}
          disabled={busy}
          startIcon={
            scanning ? (
              <CircularProgress size={12} color="inherit" />
            ) : (
              <RefreshRoundedIcon fontSize="small" />
            )
          }
        >
          {scanning ? tr("Asking…") : tr("Refresh")}
        </Button>
      </DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5, pt: 0 }}>
        {scan.kind === "done" && scan.error && (
          <Alert severity="error" variant="outlined" data-testid="tailnet-error">
            {scan.error}
          </Alert>
        )}
        {failed.map((p) => (
          <Alert key={p.id} severity="warning" variant="outlined">
            {PROVIDER_NAMES[p.id]}: {p.error}
          </Alert>
        ))}
        {scan.kind === "done" && !scan.error && installed.length === 0 && (
          <Alert severity="info" variant="outlined" data-testid="tailnet-none">
            {tr(
              "No Tailscale, NetBird or ZeroTier client found on this computer. Termoso reads the device list from the local client — install one and sign in, then refresh.",
            )}
          </Alert>
        )}
        {scan.kind === "done" && !scan.error && installed.length > 0 && devices.length === 0 && (
          <Alert severity="info" variant="outlined" data-testid="tailnet-empty">
            {tr(
              "The client is running but reports no machines. Check that it is signed in and connected.",
            )}
          </Alert>
        )}
        {devices.length > 0 && (
          <Box
            data-testid="tailnet-devices"
            sx={{ display: "flex", flexDirection: "column", gap: 0.25, mx: -1.25 }}
          >
            {devices.map((d) => {
              const k = tailnetKey(d);
              const dup = existing.get(k);
              const meta = dup
                ? tr("Already added as “{label}”", { label: dup.label })
                : d.thisDevice
                  ? tr("This device")
                  : PROVIDER_NAMES[d.provider];
              return (
                <Row
                  key={k}
                  tile={<ComputerRoundedIcon />}
                  tone={dup || !d.online ? "neutral" : undefined}
                  title={d.name}
                  subtitle={tailnetSubtitle(d, mode)}
                  meta={meta}
                  checked={selected.has(k) && !dup}
                  onToggle={() => {
                    if (!dup) toggle(d);
                  }}
                />
              );
            })}
          </Box>
        )}
        <Box sx={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 1.5, mt: 0.5 }}>
          <Field
            label={tr("Connect by")}
            hint={tr("MagicDNS / FQDN names keep working when the overlay address changes.")}
          >
            <ToggleButtonGroup
              exclusive
              fullWidth
              size="small"
              value={mode}
              onChange={(_, v: TailnetAddressMode | null) => {
                if (v) setMode(v);
              }}
              disabled={busy}
            >
              <ToggleButton value="dns">{tr("DNS name")}</ToggleButton>
              <ToggleButton value="ip">{tr("IP address")}</ToggleButton>
            </ToggleButtonGroup>
          </Field>
          <Field label={tr("Username")} hint={tr("Optional; asked on connect when empty.")}>
            <TextField
              fullWidth
              size="small"
              value={username}
              disabled={busy}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="off"
            />
          </Field>
          <Field label={tr("Group")} sx={{ gridColumn: "1 / -1" }}>
            <GroupSelect
              groups={groups.data ?? []}
              value={groupId}
              onChange={setGroupId}
              disabled={busy}
            />
          </Field>
        </Box>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2, gap: 1.5 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1, mr: "auto", minWidth: 0 }}>
          <Typography variant="body2" color="text.secondary" sx={{ whiteSpace: "nowrap" }}>
            {tr("Add to")}
          </Typography>
          <VaultSelect
            vaults={vaults.data ?? []}
            value={target}
            onChange={(id) => {
              setTarget(id);
              setGroupId(null);
            }}
          />
        </Box>
        <Button color="inherit" onClick={onClose} disabled={adding}>
          {tr("Cancel")}
        </Button>
        <Button
          variant="contained"
          onClick={() => void add()}
          disabled={busy || !vaultOk || fresh.length === 0}
          data-testid="tailnet-add"
        >
          {adding ? tr("Adding…") : addLabel(fresh.length, skipped)}
        </Button>
      </DialogActions>
    </>
  );
}
