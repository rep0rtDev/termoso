import { useState } from "react";
import {
  Box,
  Button,
  Checkbox,
  Chip,
  CircularProgress,
  Divider,
  FormControlLabel,
  IconButton,
  InputAdornment,
  MenuItem,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import VisibilityRoundedIcon from "@mui/icons-material/VisibilityRounded";
import VisibilityOffRoundedIcon from "@mui/icons-material/VisibilityOffRounded";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import {
  useDeleteHost,
  useGroups,
  useHostForm,
  useIdentities,
  useSaveHost,
  useSshKeys,
  useTags,
} from "@/ipc/hooks";
import { emptyHostForm, errorMessage, type HostForm, type Uuid } from "@/ipc/types";
import { openTerminal } from "@/terminal/store";
import { openSftpForHost } from "@/sftp/store";

export const PANEL_WIDTH = 380;

interface Props {
  vaultId: Uuid;
  hostId: Uuid | null;
  initialGroupId: Uuid | null;
  onClose: () => void;
  onOpenSftp: () => void;
}

const panelSx = {
  width: PANEL_WIDTH,
  flexShrink: 0,
  display: "flex",
  flexDirection: "column",
  bgcolor: "background.paper",
} as const;

/** Loads the form for an existing host (or starts blank) and hands it to the editor. */
export function HostEditPanel({ vaultId, hostId, initialGroupId, onClose, onOpenSftp }: Props) {
  const loaded = useHostForm(hostId);
  if (hostId !== null && loaded.data === undefined) {
    return (
      <Box component="aside" sx={panelSx}>
        <Box sx={{ display: "flex", alignItems: "center", px: 2, py: 1.25, gap: 1 }}>
          <Typography variant="h6" sx={{ flex: 1 }} noWrap>
            Edit host
          </Typography>
          <IconButton size="small" onClick={onClose} aria-label="Close">
            <CloseRoundedIcon fontSize="small" />
          </IconButton>
        </Box>
        <Divider />
        {loaded.error ? (
          <Typography color="error" sx={{ p: 2 }}>
            {errorMessage(loaded.error)}
          </Typography>
        ) : (
          <Box sx={{ display: "flex", justifyContent: "center", pt: 6 }}>
            <CircularProgress size={24} />
          </Box>
        )}
      </Box>
    );
  }
  return (
    <HostEditor
      vaultId={vaultId}
      hostId={hostId}
      initial={loaded.data ?? emptyHostForm(vaultId, initialGroupId)}
      onClose={onClose}
      onOpenSftp={onOpenSftp}
    />
  );
}

function HostEditor({
  vaultId,
  hostId,
  initial,
  onClose,
  onOpenSftp,
}: {
  vaultId: Uuid;
  hostId: Uuid | null;
  initial: HostForm;
  onClose: () => void;
  onOpenSftp: () => void;
}) {
  const snackbar = useSnackbar();
  const groups = useGroups(vaultId);
  const tags = useTags(vaultId);
  const identities = useIdentities(vaultId);
  const sshKeys = useSshKeys(vaultId);
  const save = useSaveHost();
  const del = useDeleteHost();

  const [form, setForm] = useState<HostForm>(initial);
  const [showPassword, setShowPassword] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [touched, setTouched] = useState(false);

  const set = <K extends keyof HostForm>(k: K, v: HostForm[K]) => {
    setTouched(true);
    setForm((f) => ({ ...f, [k]: v }));
  };

  const usingIdentity = form.identityId !== null;
  const canSave = form.address.trim().length > 0 && !save.isPending;

  const onSave = () => {
    save.mutate(form, {
      onSuccess: (card) => {
        snackbar.notify(hostId ? "Host saved" : `Host “${card.label}” added`);
        onClose();
      },
      onError: (e) => snackbar.error(errorMessage(e)),
    });
  };

  const onDelete = () => {
    if (!hostId) return;
    del.mutate(
      { id: hostId, vaultId },
      {
        onSuccess: () => {
          snackbar.notify("Host deleted");
          setConfirmDelete(false);
          onClose();
        },
        onError: (e) => snackbar.error(errorMessage(e)),
      },
    );
  };

  return (
    <Box component="aside" sx={panelSx}>
      <Box sx={{ display: "flex", alignItems: "center", px: 2, py: 1.25, gap: 1 }}>
        <Typography variant="h6" sx={{ flex: 1 }} noWrap>
          {hostId ? "Edit host" : "New host"}
        </Typography>
        {hostId && (
          <Tooltip title={touched ? "Save before connecting" : "Connect"}>
            <span>
              <IconButton
                size="small"
                color="primary"
                disabled={touched}
                onClick={() => openTerminal({ kind: "host", host_id: hostId })}
                aria-label="Connect"
              >
                <PlayArrowRoundedIcon fontSize="small" />
              </IconButton>
            </span>
          </Tooltip>
        )}
        {hostId && (
          <Tooltip title={touched ? "Save before opening SFTP" : "Open SFTP"}>
            <span>
              <IconButton
                size="small"
                disabled={touched}
                onClick={() => {
                  openSftpForHost(hostId, form.label || form.address);
                  onOpenSftp();
                }}
                aria-label="Open SFTP"
              >
                <FolderCopyRoundedIcon fontSize="small" />
              </IconButton>
            </span>
          </Tooltip>
        )}
        {hostId && (
          <Tooltip title="Delete host">
            <IconButton size="small" color="error" onClick={() => setConfirmDelete(true)}>
              <DeleteOutlineRoundedIcon fontSize="small" />
            </IconButton>
          </Tooltip>
        )}
        <IconButton size="small" onClick={onClose} aria-label="Close">
          <CloseRoundedIcon fontSize="small" />
        </IconButton>
      </Box>
      <Divider />

      <Box
        component="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (canSave) onSave();
        }}
        sx={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          px: 2,
          py: 2,
          display: "flex",
          flexDirection: "column",
          gap: 1.75,
        }}
      >
        <TextField
          label="Address"
          required
          autoFocus={!hostId}
          value={form.address}
          onChange={(e) => set("address", e.target.value)}
          placeholder="host or IP"
          slotProps={{ input: { sx: { fontFamily: "monospace" } } }}
          error={touched && form.address.trim().length === 0}
        />
        <TextField
          label="Label"
          value={form.label}
          onChange={(e) => set("label", e.target.value)}
          placeholder={form.address || "Defaults to the address"}
        />
        <TextField
          select
          label="Group"
          value={form.groupId ?? ""}
          onChange={(e) => set("groupId", e.target.value === "" ? null : e.target.value)}
        >
          <MenuItem value="">
            <em>None</em>
          </MenuItem>
          {(groups.data ?? []).map((g) => (
            <MenuItem key={g.id} value={g.id}>
              {g.label}
            </MenuItem>
          ))}
        </TextField>

        <Section title="SSH" />
        <Box sx={{ display: "flex", gap: 1.5 }}>
          <TextField
            label="Port"
            type="number"
            value={form.port ?? ""}
            onChange={(e) =>
              set(
                "port",
                e.target.value === "" ? null : Math.max(1, Math.min(65535, Number(e.target.value))),
              )
            }
            placeholder="22"
            sx={{ width: 120 }}
            slotProps={{ htmlInput: { min: 1, max: 65535 } }}
          />
          <TextField
            select
            label="Credentials"
            value={form.identityId ?? "inline"}
            onChange={(e) => {
              const v = e.target.value;
              set("identityId", v === "inline" ? null : v);
            }}
            sx={{ flex: 1 }}
          >
            <MenuItem value="inline">Set on this host</MenuItem>
            {(identities.data ?? []).map((i) => (
              <MenuItem key={i.id} value={i.id}>
                {i.data.label}
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ ml: 1 }}
                >
                  {i.data.username}
                </Typography>
              </MenuItem>
            ))}
          </TextField>
        </Box>

        {!usingIdentity && (
          <>
            <TextField
              label="Username"
              value={form.username}
              onChange={(e) => set("username", e.target.value)}
              autoComplete="off"
            />
            <TextField
              label="Password"
              type={showPassword ? "text" : "password"}
              value={form.password ?? ""}
              onChange={(e) => set("password", e.target.value)}
              autoComplete="new-password"
              placeholder={form.hasPassword && form.password === null ? "•••••••• (stored)" : ""}
              helperText={
                form.hasPassword && form.password === null
                  ? "Leave empty to keep the stored password; clear it to remove."
                  : undefined
              }
              slotProps={{
                input: {
                  endAdornment: (
                    <InputAdornment position="end">
                      {form.hasPassword && form.password === null && (
                        <Button size="small" color="inherit" onClick={() => set("password", "")}>
                          Clear
                        </Button>
                      )}
                      <IconButton
                        size="small"
                        onClick={() => setShowPassword((v) => !v)}
                        aria-label="Toggle password visibility"
                      >
                        {showPassword ? (
                          <VisibilityOffRoundedIcon fontSize="small" />
                        ) : (
                          <VisibilityRoundedIcon fontSize="small" />
                        )}
                      </IconButton>
                    </InputAdornment>
                  ),
                },
              }}
            />
            <TextField
              select
              label="SSH key"
              value={form.sshKeyId ?? ""}
              onChange={(e) => set("sshKeyId", e.target.value === "" ? null : e.target.value)}
            >
              <MenuItem value="">
                <em>None (password / agent)</em>
              </MenuItem>
              {(sshKeys.data ?? []).map((k) => (
                <MenuItem key={k.id} value={k.id}>
                  {k.data.label}
                  <Typography
                    component="span"
                    variant="caption"
                    color="text.secondary"
                    sx={{ ml: 1 }}
                  >
                    {k.data.key_type}
                  </Typography>
                </MenuItem>
              ))}
            </TextField>
          </>
        )}

        <FormControlLabel
          control={
            <Switch
              checked={form.agentForwarding}
              onChange={(e) => set("agentForwarding", e.target.checked)}
            />
          }
          label="Agent forwarding"
        />

        <Section title="Tags" />
        <Box sx={{ display: "flex", flexWrap: "wrap", gap: 0.75 }}>
          {(tags.data ?? []).length === 0 && (
            <Typography variant="caption" color="text.secondary">
              No tags yet.
            </Typography>
          )}
          {(tags.data ?? []).map((t) => {
            const on = form.tagIds.includes(t.id);
            return (
              <Chip
                key={t.id}
                label={t.label}
                size="small"
                color={on ? "primary" : "default"}
                variant={on ? "filled" : "outlined"}
                onClick={() =>
                  set("tagIds", on ? form.tagIds.filter((x) => x !== t.id) : [...form.tagIds, t.id])
                }
                icon={<Checkbox size="small" checked={on} sx={{ p: 0, ml: 0.5 }} tabIndex={-1} />}
              />
            );
          })}
        </Box>

        <Section title="Notes" />
        <TextField
          multiline
          minRows={3}
          value={form.notes}
          onChange={(e) => set("notes", e.target.value)}
          placeholder="Anything worth remembering about this host"
        />
      </Box>

      <Divider />
      <Box sx={{ display: "flex", gap: 1, px: 2, py: 1.5, justifyContent: "flex-end" }}>
        <Button color="inherit" onClick={onClose} disabled={save.isPending}>
          Cancel
        </Button>
        <Button variant="contained" onClick={onSave} disabled={!canSave}>
          {save.isPending ? "Saving…" : "Save"}
        </Button>
      </Box>

      <ConfirmDialog
        open={confirmDelete}
        title="Delete host?"
        danger
        confirmLabel="Delete"
        busy={del.isPending}
        onCancel={() => setConfirmDelete(false)}
        onConfirm={onDelete}
      >
        “{form.label || form.address}” and its inline credentials will be removed from this device.
      </ConfirmDialog>
    </Box>
  );
}

function Section({ title }: { title: string }) {
  return (
    <Typography variant="overline" color="text.secondary" sx={{ mt: 0.5, mb: -0.75, fontSize: 10 }}>
      {title}
    </Typography>
  );
}
