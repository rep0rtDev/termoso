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
import LanOutlinedIcon from "@mui/icons-material/LanOutlined";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import ComputerRoundedIcon from "@mui/icons-material/ComputerRounded";
import { useSnackbar } from "@/components/Snackbar";
import { Field, IconTile } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { useGroups, useHosts, useSaveHost, useVaults } from "@/ipc/hooks";
import { errorMessage, type LocalDevice, type Uuid } from "@/ipc/types";
import { GroupSelect } from "./CloudImportDialog";
import { Row, VaultSelect } from "./ImportDialog";
import {
  addLabel,
  lanAddress,
  lanExisting,
  lanHostForm,
  lanLabel,
  lanSubtitle,
  type LanAddressMode,
} from "./lan";
import { tr, trx } from "@/i18n";

/** How long one browse listens for announcements; devices answer within a second or two. */
export const LAN_BROWSE_MS = 3000;

interface Props {
  open: boolean;
  vaultId: Uuid;
  onClose: () => void;
  onImported: () => void;
}

/**
 * New host → “Discover on LAN”. Lists machines advertising SSH/SFTP over
 * mDNS on the local network. Nothing is saved until the user picks devices
 * and clicks Add; the browse itself only listens and stores nothing.
 */
export function LanDiscoveryDialog({ open, vaultId, onClose, onImported }: Props) {
  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      {open && <Body vaultId={vaultId} onClose={onClose} onImported={onImported} />}
    </Dialog>
  );
}

type Scan =
  | { kind: "scanning"; devices: LocalDevice[] }
  | { kind: "done"; devices: LocalDevice[]; error: string | null };

function Body({ vaultId, onClose, onImported }: Omit<Props, "open">) {
  const snackbar = useSnackbar();
  const vaults = useVaults();
  const save = useSaveHost();
  const [target, setTarget] = useState<Uuid>(vaultId);
  const [groupId, setGroupId] = useState<Uuid | null>(null);
  const [username, setUsername] = useState("");
  const [mode, setMode] = useState<LanAddressMode>("hostname");
  const [scan, setScan] = useState<Scan>({ kind: "scanning", devices: [] });
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

  const key = (d: LocalDevice) => `${d.hostname}|${d.port}|${d.addresses.join(",")}`;

  const listen = useCallback(() => {
    ipc
      .mdnsBrowse(LAN_BROWSE_MS)
      .then((devices) => {
        if (!alive.current) return;
        setScan({ kind: "done", devices, error: null });
        // First results start all selected; later scans keep the user's picks.
        setSelected((sel) => {
          if (seeded.current) return new Set(devices.map(key).filter((k) => sel.has(k)));
          seeded.current = true;
          return new Set(devices.map(key));
        });
      })
      .catch((e: unknown) => {
        if (!alive.current) return;
        setScan((s) => ({ kind: "done", devices: s.devices, error: errorMessage(e) }));
      });
  }, []);
  // Initial state is already `scanning`; only the browse itself runs on mount.
  useEffect(listen, [listen]);
  const browse = () => {
    setScan((s) => ({ kind: "scanning", devices: s.devices }));
    listen();
  };

  const vault = (vaults.data ?? []).find((v) => v.id === target);
  const vaultOk = !!vault && vault.unlocked && vault.role !== "viewer";
  const existing = new Map(
    scan.devices.map((d) => [key(d), lanExisting(d, hosts.data ?? [])] as const),
  );
  const toggle = (d: LocalDevice) =>
    setSelected((s) => {
      const n = new Set(s);
      const k = key(d);
      if (n.has(k)) n.delete(k);
      else n.add(k);
      return n;
    });
  const chosen = scan.devices.filter((d) => selected.has(key(d)) && lanAddress(d, mode));
  const fresh = chosen.filter((d) => !existing.get(key(d)));

  const add = async () => {
    if (!vaultOk || fresh.length === 0) return;
    setAdding(true);
    let added = 0;
    try {
      for (const d of fresh) {
        const form = lanHostForm(d, target, groupId, mode, username);
        if (!form) continue;
        await save.mutateAsync(form);
        added += 1;
      }
      snackbar.notify(`Added ${added} host${added === 1 ? "" : "s"} from the local network`);
      onImported();
      onClose();
    } catch (e) {
      snackbar.error(
        `${errorMessage(e)}${added ? ` — ${added} host${added === 1 ? "" : "s"} were added before the error` : ""}`,
      );
    } finally {
      if (alive.current) setAdding(false);
    }
  };

  const scanning = scan.kind === "scanning";
  const busy = scanning || adding;
  const skipped = chosen.length - fresh.length;

  return (
    <>
      <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
        <IconTile>
          <LanOutlinedIcon />
        </IconTile>
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography variant="h6" component="div">
            {tr("Discover on local network")}
          </Typography>
          <Typography variant="body2" color="text.secondary" noWrap>
            {tr("Machines announcing SSH or SFTP over mDNS (Bonjour / Avahi)")}
          </Typography>
        </Box>
        <Button
          size="small"
          color="inherit"
          onClick={browse}
          disabled={busy}
          startIcon={
            scanning ? (
              <CircularProgress size={12} color="inherit" />
            ) : (
              <RefreshRoundedIcon fontSize="small" />
            )
          }
        >
          {scanning ? tr("Listening…") : tr("Scan again")}
        </Button>
      </DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5, pt: 0 }}>
        {scan.kind === "done" && scan.error && (
          <Alert severity="error" variant="outlined" data-testid="lan-error">
            {scan.error}
          </Alert>
        )}
        {scan.kind === "done" && !scan.error && scan.devices.length === 0 && (
          <Alert severity="info" variant="outlined" data-testid="lan-empty">
            {trx(
              "Nothing answered. Devices show up here when their SSH server is advertised over mDNS (macOS “Remote Login”, Avahi with an {service} service, many NAS boxes). Firewalls and VPNs often block multicast.",
              { service: <code>_ssh._tcp</code> },
            )}
          </Alert>
        )}
        {scan.devices.length > 0 && (
          <Box
            data-testid="lan-devices"
            sx={{ display: "flex", flexDirection: "column", gap: 0.25, mx: -1.25 }}
          >
            {scan.devices.map((d) => {
              const k = key(d);
              const dup = existing.get(k);
              return (
                <Row
                  key={k}
                  tile={<ComputerRoundedIcon />}
                  tone={dup ? "neutral" : undefined}
                  title={lanLabel(d)}
                  subtitle={lanSubtitle(d, mode)}
                  meta={dup ? `Already added as “${dup.label}”` : undefined}
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
            hint={tr("Names keep working when DHCP hands out a new address.")}
          >
            <ToggleButtonGroup
              exclusive
              fullWidth
              size="small"
              value={mode}
              onChange={(_, v: LanAddressMode | null) => {
                if (v) setMode(v);
              }}
              disabled={busy}
            >
              <ToggleButton value="hostname">{tr(".local name")}</ToggleButton>
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
          data-testid="lan-add"
        >
          {adding ? tr("Adding…") : addLabel(fresh.length, skipped)}
        </Button>
      </DialogActions>
    </>
  );
}
