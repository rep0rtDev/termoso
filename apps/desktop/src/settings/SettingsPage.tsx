import type { ReactNode } from "react";
import {
  Box,
  Button,
  CircularProgress,
  Divider,
  FormControlLabel,
  MenuItem,
  Paper,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import { keys, useAppInfo, useSaveSettings, useSettings } from "@/ipc/hooks";
import { UpdatesCard } from "@/update/UpdatesCard";
import {
  errorMessage,
  type CursorStyle,
  type Settings,
  type SyncConflict,
  type ThemeMode,
} from "@/ipc/types";

export function SettingsPage() {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const settings = useSettings();
  const info = useAppInfo();
  const save = useSaveSettings();
  const migrate = useMutation({
    mutationFn: ipc.masterKeyMigrate,
    onSuccess: (src) => {
      void qc.invalidateQueries({ queryKey: keys.app });
      snackbar.notify(
        src === "keychain" ? "Master key moved to the OS keychain" : "No OS keychain available",
      );
    },
    onError: (e) => snackbar.error(errorMessage(e)),
  });

  const update = (patch: Partial<Settings>) => {
    if (!settings.data) return;
    save.mutate(
      { ...settings.data, ...patch },
      { onError: (e) => snackbar.error(errorMessage(e)) },
    );
  };

  if (settings.isPending) {
    return (
      <Box sx={{ display: "flex", justifyContent: "center", pt: 8 }}>
        <CircularProgress size={28} />
      </Box>
    );
  }
  if (settings.error) {
    return (
      <Typography color="error" sx={{ p: 3 }}>
        {errorMessage(settings.error)}
      </Typography>
    );
  }
  const s = settings.data;

  return (
    <Box sx={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
      <Box sx={{ px: 2.5, py: 1.75, borderBottom: 1, borderColor: "divider" }}>
        <Typography variant="h5">Settings</Typography>
        <Typography variant="body2" color="text.secondary">
          Preferences live in the encrypted local profile and never leave this device.
        </Typography>
      </Box>

      <Box sx={{ p: 2.5, display: "flex", flexDirection: "column", gap: 2.5, maxWidth: 720 }}>
        <Card title="Appearance">
          <TextField
            select
            label="Theme"
            value={s.theme}
            onChange={(e) => update({ theme: e.target.value as ThemeMode })}
            sx={{ width: 240 }}
          >
            <MenuItem value="dark">Dark</MenuItem>
            <MenuItem value="light">Light</MenuItem>
            <MenuItem value="system">Follow system</MenuItem>
          </TextField>
        </Card>

        <Card title="Terminal">
          <Box sx={{ display: "flex", gap: 2, flexWrap: "wrap" }}>
            <TextField
              label="Font family"
              value={s.terminalFontFamily}
              onChange={(e) => update({ terminalFontFamily: e.target.value })}
              sx={{ flex: 1, minWidth: 240 }}
            />
            <TextField
              label="Font size"
              type="number"
              value={s.terminalFontSize}
              onChange={(e) => update({ terminalFontSize: Number(e.target.value) })}
              slotProps={{ htmlInput: { min: 8, max: 40 } }}
              sx={{ width: 120 }}
            />
            <TextField
              label="Scrollback lines"
              type="number"
              value={s.scrollback}
              onChange={(e) => update({ scrollback: Number(e.target.value) })}
              slotProps={{ htmlInput: { min: 100, max: 1_000_000, step: 100 } }}
              sx={{ width: 160 }}
            />
          </Box>
          <TextField
            select
            label="Cursor"
            value={s.cursorStyle}
            onChange={(e) => update({ cursorStyle: e.target.value as CursorStyle })}
            sx={{ width: 240 }}
          >
            <MenuItem value="bar">Bar</MenuItem>
            <MenuItem value="block">Block</MenuItem>
            <MenuItem value="underline">Underline</MenuItem>
          </TextField>
          <Toggle
            label="Blinking cursor"
            checked={s.cursorBlink}
            onChange={(v) => update({ cursorBlink: v })}
          />
          <Toggle
            label="Audible bell"
            checked={s.terminalBell}
            onChange={(v) => update({ terminalBell: v })}
          />
          <Toggle
            label="Copy on select"
            checked={s.copyOnSelect}
            onChange={(v) => update({ copyOnSelect: v })}
          />
          <Toggle
            label="Paste on right click"
            checked={s.pasteOnRightClick}
            onChange={(v) => update({ pasteOnRightClick: v })}
          />
          <Toggle
            label="Confirm before pasting multiple lines"
            checked={s.confirmPasteMultiline}
            onChange={(v) => update({ confirmPasteMultiline: v })}
          />
          <Toggle
            label="Confirm before closing a connected tab"
            checked={s.confirmCloseTab}
            onChange={(v) => update({ confirmCloseTab: v })}
          />
          <Toggle
            label="Command autocomplete"
            checked={s.autocomplete}
            onChange={(v) => update({ autocomplete: v })}
          />
        </Card>

        <Card title="Connections">
          <TextField
            label="Keep-alive interval (seconds, 0 = off)"
            type="number"
            value={s.keepAliveSeconds}
            onChange={(e) => update({ keepAliveSeconds: Math.max(0, Number(e.target.value)) })}
            slotProps={{ htmlInput: { min: 0, max: 3600 } }}
            sx={{ width: 280 }}
          />
          <Toggle
            label="Start auto-start forwarding rules when the app launches"
            checked={s.autostartForwarding}
            onChange={(v) => update({ autostartForwarding: v })}
          />
        </Card>

        <Card title="Sessions">
          <Toggle
            label="Record terminal sessions (encrypted, on this device)"
            checked={s.recordSessions}
            onChange={(v) => update({ recordSessions: v })}
          />
          <TextField
            label="Keep recordings for (days, 0 = forever)"
            type="number"
            value={s.logRetentionDays}
            onChange={(e) => update({ logRetentionDays: Math.max(0, Number(e.target.value)) })}
            slotProps={{ htmlInput: { min: 0, max: 3650 } }}
            sx={{ width: 280 }}
          />
          <Toggle
            label="Upload recordings to the account server (encrypted with your vault key)"
            checked={s.uploadLogs}
            onChange={(v) => update({ uploadLogs: v })}
          />
        </Card>

        <Card title="Sync">
          <Box sx={{ display: "flex", gap: 2, flexWrap: "wrap" }}>
            <TextField
              select
              label="On conflict"
              value={s.syncConflict}
              onChange={(e) => update({ syncConflict: e.target.value as SyncConflict })}
              sx={{ width: 240 }}
            >
              <MenuItem value="newest_wins">Newest change wins</MenuItem>
              <MenuItem value="local_wins">This device wins</MenuItem>
              <MenuItem value="server_wins">Server wins</MenuItem>
            </TextField>
            <TextField
              label="Background sync every (seconds, 0 = only on changes)"
              type="number"
              value={s.syncIntervalSeconds}
              onChange={(e) => {
                const n = Math.max(0, Number(e.target.value));
                update({ syncIntervalSeconds: n === 0 ? 0 : Math.max(30, n) });
              }}
              slotProps={{ htmlInput: { min: 0, max: 86_400, step: 30 } }}
              sx={{ width: 280 }}
            />
          </Box>
          <Typography variant="body2" color="text.secondary">
            Changes are also pushed immediately when the server signals an update. Everything is
            end-to-end encrypted; the server only stores ciphertext.
          </Typography>
        </Card>

        <UpdatesCard settings={s} onChange={update} />

        <Card title="About">
          <Row k="Version" v={info.data ? `Termoso ${info.data.version}` : "…"} />
          <Row k="Platform" v={info.data?.platform ?? "…"} />
          <Row k="Profile" v={info.data?.profileDir ?? "…"} mono />
          <Row
            k="Master key"
            v={
              info.data?.masterKeySource === "keychain"
                ? "OS keychain"
                : info.data?.masterKeySource === "file"
                  ? "Owner-only file in profile (no keychain available)"
                  : "…"
            }
          />
          {info.data?.masterKeySource === "file" && (
            <Box>
              <Button
                variant="outlined"
                size="small"
                disabled={migrate.isPending}
                onClick={() => migrate.mutate()}
              >
                Move master key to OS keychain
              </Button>
            </Box>
          )}
          <Divider sx={{ my: 1 }} />
          <Typography variant="body2" color="text.secondary">
            Termoso sends nothing anywhere unless you sign in to a server you chose or ask it to
            check for updates. No analytics, no crash reports, no background pings.
          </Typography>
        </Card>
      </Box>
    </Box>
  );
}

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Paper variant="outlined" sx={{ p: 2.5, display: "flex", flexDirection: "column", gap: 1.5 }}>
      <Typography variant="h6">{title}</Typography>
      {children}
    </Paper>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <FormControlLabel
      control={<Switch checked={checked} onChange={(e) => onChange(e.target.checked)} />}
      label={label}
      sx={{ ml: 0 }}
    />
  );
}

function Row({ k, v, mono }: { k: string; v: string; mono?: boolean }) {
  return (
    <Box sx={{ display: "flex", gap: 2 }}>
      <Typography variant="body2" color="text.secondary" sx={{ width: 110, flexShrink: 0 }}>
        {k}
      </Typography>
      <Typography
        variant="body2"
        sx={{ fontFamily: mono ? "monospace" : undefined, wordBreak: "break-all" }}
      >
        {v}
      </Typography>
    </Box>
  );
}
