import { useState } from "react";
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Checkbox,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  MenuItem,
  Select,
  Switch,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import CloudSyncOutlinedIcon from "@mui/icons-material/CloudSyncOutlined";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useSnackbar } from "@/components/Snackbar";
import { Field, IconTile } from "@/components/ui";
import { useForgetCloudSync, useRunCloudSync, useSaveCloudSync, useTags } from "@/ipc/hooks";
import { errorMessage, type CloudProvider, type CloudSyncGroup, type GroupNode } from "@/ipc/types";
import { CloudCredentialFields } from "./CloudCredentialFields";
import { CLOUD_PROVIDERS } from "./cloud";
import {
  SYNC_INTERVALS,
  SYNC_PRIVACY_NOTE,
  canSaveSync,
  emptySyncDraft,
  identityChanged,
  reportLine,
  syncDraftFromConfig,
  syncSummary,
  toSyncConfig,
  type SyncDraft,
} from "./cloudSync";
import { TagChip } from "./TagChip";

interface Props {
  open: boolean;
  group: GroupNode;
  /** Current settings, or `null` when the group is not synced yet. */
  existing: CloudSyncGroup | null;
  readOnly: boolean;
  onClose: () => void;
}

/**
 * Group details → Cloud sync. The group mirrors one provider account: hosts
 * are created, updated and (optionally) removed to match the machines there,
 * on a schedule and on demand. Credentials go to Rust once and are stored
 * encrypted; they are never read back into the webview.
 */
export function CloudSyncDialog({ open, group, existing, readOnly, onClose }: Props) {
  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      {open && <Body group={group} existing={existing} readOnly={readOnly} onClose={onClose} />}
    </Dialog>
  );
}

function Body({ group, existing, readOnly, onClose }: Omit<Props, "open">) {
  const snackbar = useSnackbar();
  const tags = useTags(group.vaultId);
  const save = useSaveCloudSync();
  const forget = useForgetCloudSync();
  const run = useRunCloudSync();
  const [draft, setDraft] = useState<SyncDraft>(() =>
    existing ? syncDraftFromConfig(existing.config) : emptySyncDraft("aws"),
  );
  const [reveal, setReveal] = useState(false);
  const [confirmForget, setConfirmForget] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const busy = save.isPending || forget.isPending || run.isPending || readOnly;
  const check = canSaveSync(draft, existing);
  const config = toSyncConfig(draft);
  const needsSecret =
    !!config && !check.ok && (!existing?.hasSecret || identityChanged(existing.config, config));

  const set = <K extends keyof SyncDraft>(k: K, v: SyncDraft[K]) =>
    setDraft((d) => ({ ...d, [k]: v }));

  const onSave = (thenSync: boolean) => {
    if (!check.ok) return;
    setError(null);
    save.mutate(
      { groupId: group.id, config: check.config, secret: check.secret },
      {
        onSuccess: (g) => {
          // The secret has done its job; don't keep it in webview state.
          setDraft(syncDraftFromConfig(g.config));
          if (thenSync) {
            run.mutate(g.groupId, {
              onSuccess: (r) => {
                if (r.status.error) setError(r.status.error);
                else snackbar.notify(`“${group.label}” synced: ${reportLine(r) ?? "done"}`);
                onClose();
              },
              onError: (e) => setError(errorMessage(e)),
            });
          } else {
            snackbar.notify(existing ? "Cloud sync updated" : `“${group.label}” is now synced`);
            onClose();
          }
        },
        onError: (e) => setError(errorMessage(e)),
      },
    );
  };

  const onForget = () => {
    forget.mutate(
      { groupId: group.id },
      {
        onSuccess: () => {
          setConfirmForget(false);
          snackbar.notify("Cloud sync turned off; hosts were kept");
          onClose();
        },
        onError: (e) => {
          setConfirmForget(false);
          setError(errorMessage(e));
        },
      },
    );
  };

  const summary = existing ? syncSummary(existing) : null;
  const report = existing ? reportLine(existing) : null;

  return (
    <>
      <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
        <IconTile>
          <CloudSyncOutlinedIcon />
        </IconTile>
        <Box sx={{ minWidth: 0 }}>
          <Typography variant="h6" component="div" noWrap>
            Cloud sync · {group.label}
          </Typography>
          <Typography variant="body2" color="text.secondary" noWrap>
            {summary ? summary.text : "Keep this group's hosts in step with a cloud account"}
          </Typography>
        </Box>
      </DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 2, pt: 0 }}>
        {(error ??
          (existing?.status.error && !existing.running ? existing.status.error : null)) && (
          <Alert severity="error" variant="outlined" data-testid="cloud-sync-error">
            {error ?? existing?.status.error}
          </Alert>
        )}
        {existing && !existing.hasSecret && (
          <Alert severity="warning" variant="outlined">
            The credentials for this group are stored on another device. Enter them here to sync
            from this one too.
          </Alert>
        )}
        {report && !error && (
          <Typography variant="body2" color="text.secondary" data-testid="cloud-sync-report">
            Last result: {report}
          </Typography>
        )}

        <ToggleButtonGroup
          exclusive
          size="small"
          value={draft.provider}
          onChange={(_, v: CloudProvider | null) => {
            if (v) setDraft((d) => ({ ...d, provider: v }));
          }}
          disabled={busy}
          sx={{ alignSelf: "flex-start" }}
        >
          {CLOUD_PROVIDERS.map((p) => (
            <ToggleButton key={p.id} value={p.id} sx={{ px: 2 }}>
              {p.short}
            </ToggleButton>
          ))}
        </ToggleButtonGroup>

        <Box
          component="form"
          onSubmit={(e) => {
            e.preventDefault();
            onSave(false);
          }}
          sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}
        >
          <CloudCredentialFields
            provider={draft.provider}
            draft={draft.creds}
            setDraft={(u) =>
              setDraft((d) => ({ ...d, creds: typeof u === "function" ? u(d.creds) : u }))
            }
            reveal={reveal}
            onReveal={() => setReveal((v) => !v)}
            disabled={busy}
            storedSecretHint={
              existing?.hasSecret && existing.config.provider === draft.provider
                ? "Stored on this device — leave empty to keep"
                : undefined
            }
          />

          <Box sx={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 1.5 }}>
            <Field label="Username" hint="New hosts only; existing hosts keep theirs.">
              <TextField
                fullWidth
                size="small"
                value={draft.username}
                disabled={busy}
                onChange={(e) => set("username", e.target.value)}
                placeholder={
                  draft.provider === "aws"
                    ? "ec2-user"
                    : draft.provider === "azure"
                      ? "azureuser"
                      : "root"
                }
                autoComplete="off"
              />
            </Field>
            <Field label="Port">
              <TextField
                fullWidth
                size="small"
                value={draft.port}
                disabled={busy}
                onChange={(e) => set("port", e.target.value.replace(/[^\d]/g, ""))}
                placeholder="22"
                slotProps={{ input: { inputMode: "numeric" } }}
              />
            </Field>
            <Field label="Tags" sx={{ gridColumn: "1 / -1" }}>
              <Autocomplete
                multiple
                size="small"
                options={tags.data ?? []}
                getOptionLabel={(t) => t.label}
                isOptionEqualToValue={(a, b) => a.id === b.id}
                value={(tags.data ?? []).filter((t) => draft.tagIds.includes(t.id))}
                onChange={(_, v) =>
                  set(
                    "tagIds",
                    v.map((t) => t.id),
                  )
                }
                disabled={busy}
                renderValue={(value, getItemProps) =>
                  value.map((t, idx) => {
                    const { key, ...props } = getItemProps({ index: idx });
                    return <TagChip key={key} label={t.label} color={t.color} {...props} />;
                  })
                }
                renderInput={(params) => (
                  <TextField {...params} placeholder={draft.tagIds.length ? "" : "No tags"} />
                )}
              />
            </Field>
            <Field label="Refresh">
              <Select
                fullWidth
                size="small"
                value={draft.intervalMinutes}
                disabled={busy}
                onChange={(e) => set("intervalMinutes", e.target.value)}
              >
                {SYNC_INTERVALS.map((i) => (
                  <MenuItem key={i.minutes} value={i.minutes}>
                    {i.label}
                  </MenuItem>
                ))}
              </Select>
            </Field>
            <Box sx={{ display: "flex", flexDirection: "column", justifyContent: "flex-end" }}>
              <FormControlLabel
                sx={{ ml: -0.75 }}
                control={
                  <Switch
                    size="small"
                    checked={draft.enabled}
                    disabled={busy}
                    onChange={(e) => set("enabled", e.target.checked)}
                  />
                }
                label="Sync in the background"
              />
            </Box>
          </Box>
          <FormControlLabel
            sx={{ ml: -0.75 }}
            control={
              <Checkbox
                size="small"
                checked={draft.removeMissing}
                disabled={busy}
                onChange={(e) => set("removeMissing", e.target.checked)}
              />
            }
            label={
              <Box>
                <Typography variant="body2">
                  Remove hosts whose machine is gone from the provider
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  Only hosts this sync created; hosts you added by hand are never touched.
                </Typography>
              </Box>
            }
          />
          <button type="submit" hidden disabled={!check.ok || busy} />
        </Box>

        <Box
          sx={{
            display: "flex",
            gap: 1,
            alignItems: "flex-start",
            bgcolor: "surface.high",
            borderRadius: 2,
            px: 1.5,
            py: 1,
          }}
        >
          <LockOutlinedIcon fontSize="small" sx={{ color: "text.secondary", mt: "1px" }} />
          <Typography variant="caption" color="text.secondary">
            {SYNC_PRIVACY_NOTE}
          </Typography>
        </Box>
        {needsSecret && (
          <Typography variant="caption" color="warning.main" data-testid="cloud-sync-needs-secret">
            Enter the {draft.provider === "digital_ocean" ? "token" : "secret"} for this account to
            save.
          </Typography>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2, gap: 1 }}>
        {existing && (
          <Button
            color="error"
            variant="text"
            disabled={busy}
            onClick={() => setConfirmForget(true)}
            sx={{ mr: "auto" }}
          >
            Turn off
          </Button>
        )}
        <Button onClick={onClose} color="inherit" disabled={save.isPending || run.isPending}>
          Cancel
        </Button>
        <Button
          variant="outlined"
          disabled={!check.ok || busy}
          onClick={() => onSave(true)}
          startIcon={run.isPending ? <CircularProgress size={14} color="inherit" /> : undefined}
        >
          Save & sync now
        </Button>
        <Button variant="contained" disabled={!check.ok || busy} onClick={() => onSave(false)}>
          Save
        </Button>
      </DialogActions>

      <ConfirmDialog
        open={confirmForget}
        title="Turn off cloud sync?"
        confirmLabel="Turn off"
        danger
        busy={forget.isPending}
        onCancel={() => setConfirmForget(false)}
        onConfirm={onForget}
      >
        The stored credentials are erased from this device and the group stops refreshing. Hosts
        already in the group stay as they are.
      </ConfirmDialog>
    </>
  );
}
