import { useState } from "react";
import {
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  TextField,
  Typography,
} from "@mui/material";
import { Field } from "@/components/ui";
import { useSnackbar } from "@/components/Snackbar";
import { useHosts, useIdentities, useSaveHostChain, useSaveProxy } from "@/ipc/hooks";
import { errorMessage, type HostChainData, type ProxyData, type Uuid } from "@/ipc/types";
import { monoFontFamily } from "@/theme/theme";
import { tr } from "@/i18n";

/** Create a SOCKS/HTTP proxy entity and hand its id back. */
export function ProxyDialog({
  open,
  vaultId,
  onClose,
  onCreated,
}: {
  open: boolean;
  vaultId: Uuid;
  onClose: () => void;
  onCreated: (id: Uuid) => void;
}) {
  const snackbar = useSnackbar();
  const identities = useIdentities(vaultId);
  const save = useSaveProxy();
  const [data, setData] = useState<ProxyData>({
    kind: "socks5",
    host: "",
    port: 1080,
    identity_id: null,
  });
  const valid = data.host.trim().length > 0 && data.port >= 1 && data.port <= 65535;

  const submit = () => {
    save.mutate(
      { vaultId, id: null, data: { ...data, host: data.host.trim() } },
      {
        onSuccess: (e) => {
          onCreated(e.id);
          onClose();
        },
        onError: (err) => snackbar.error(errorMessage(err)),
      },
    );
  };

  return (
    <Dialog open={open} onClose={onClose} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("New proxy")}</DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}>
        <Box sx={{ display: "flex", gap: 1.5 }}>
          <Field label={tr("Type")} sx={{ width: 130, flexShrink: 0 }}>
            <TextField
              select
              value={data.kind}
              onChange={(e) => setData({ ...data, kind: e.target.value as ProxyData["kind"] })}
            >
              <MenuItem value="socks5">SOCKS5</MenuItem>
              <MenuItem value="socks4">SOCKS4</MenuItem>
              <MenuItem value="http">HTTP</MenuItem>
            </TextField>
          </Field>
          <Field label={tr("Host")} sx={{ flex: 1 }}>
            <TextField
              autoFocus
              value={data.host}
              onChange={(e) => setData({ ...data, host: e.target.value })}
              placeholder="proxy.example.com"
              slotProps={{ input: { sx: { fontFamily: monoFontFamily } } }}
            />
          </Field>
          <Field label={tr("Port")} sx={{ width: 96, flexShrink: 0 }}>
            <TextField
              type="number"
              value={data.port}
              onChange={(e) => setData({ ...data, port: Number(e.target.value) })}
              slotProps={{ htmlInput: { min: 1, max: 65535 } }}
            />
          </Field>
        </Box>
        <Field
          label={tr("Credentials")}
          hint={tr("Optional identity used to authenticate with the proxy.")}
        >
          <TextField
            select
            value={data.identity_id ?? ""}
            onChange={(e) =>
              setData({ ...data, identity_id: e.target.value === "" ? null : e.target.value })
            }
          >
            <MenuItem value="">
              <em>{tr("None")}</em>
            </MenuItem>
            {(identities.data ?? []).map((i) => (
              <MenuItem key={i.id} value={i.id}>
                {i.label}
              </MenuItem>
            ))}
          </TextField>
        </Field>
      </DialogContent>
      <DialogActions>
        <Button color="inherit" onClick={onClose}>
          {tr("Cancel")}
        </Button>
        <Button variant="contained" disabled={!valid || save.isPending} onClick={submit}>
          {tr("Create")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

/** Create a host chain (ordered jump hosts) and hand its id back. */
export function ChainDialog({
  open,
  vaultId,
  excludeHostId,
  onClose,
  onCreated,
}: {
  open: boolean;
  vaultId: Uuid;
  excludeHostId: Uuid | null;
  onClose: () => void;
  onCreated: (id: Uuid) => void;
}) {
  const snackbar = useSnackbar();
  const hosts = useHosts(vaultId);
  const save = useSaveHostChain();
  const [label, setLabel] = useState("");
  const [ids, setIds] = useState<Uuid[]>([]);
  const candidates = (hosts.data ?? []).filter((h) => h.id !== excludeHostId);

  const toggle = (id: Uuid) =>
    setIds((cur) => (cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id]));

  const submit = () => {
    const data: HostChainData = {
      label: label.trim() || (candidates.find((h) => h.id === ids[0])?.label ?? tr("Chain")),
      host_ids: ids,
    };
    save.mutate(
      { vaultId, id: null, data },
      {
        onSuccess: (e) => {
          onCreated(e.id);
          onClose();
        },
        onError: (err) => snackbar.error(errorMessage(err)),
      },
    );
  };

  return (
    <Dialog open={open} onClose={onClose} maxWidth="xs" fullWidth>
      <DialogTitle>{tr("New host chain")}</DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}>
        <Field label={tr("Label")}>
          <TextField
            autoFocus
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder={tr("Bastion → internal")}
          />
        </Field>
        <Field
          label={tr("Jump hosts")}
          hint={tr("Connections go through the selected hosts in the order they are ticked.")}
        >
          {candidates.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              {tr("Add another host first — a chain needs at least one intermediate host.")}
            </Typography>
          ) : (
            <List
              dense
              disablePadding
              sx={{
                maxHeight: 240,
                overflowY: "auto",
                bgcolor: "surface.base",
                borderRadius: 2,
                p: 0.5,
              }}
            >
              {candidates.map((h) => {
                const idx = ids.indexOf(h.id);
                return (
                  <ListItemButton key={h.id} dense onClick={() => toggle(h.id)}>
                    <ListItemIcon sx={{ minWidth: 32 }}>
                      <Checkbox size="small" edge="start" checked={idx >= 0} tabIndex={-1} />
                    </ListItemIcon>
                    <ListItemText
                      primary={h.label}
                      secondary={h.address}
                      slotProps={{
                        primary: { noWrap: true },
                        secondary: { noWrap: true, sx: { fontFamily: monoFontFamily } },
                      }}
                    />
                    {idx >= 0 && (
                      <Typography variant="caption" color="primary" sx={{ fontWeight: 600 }}>
                        {idx + 1}
                      </Typography>
                    )}
                  </ListItemButton>
                );
              })}
            </List>
          )}
        </Field>
      </DialogContent>
      <DialogActions>
        <Button color="inherit" onClick={onClose}>
          {tr("Cancel")}
        </Button>
        <Button variant="contained" disabled={ids.length === 0 || save.isPending} onClick={submit}>
          {tr("Create")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
