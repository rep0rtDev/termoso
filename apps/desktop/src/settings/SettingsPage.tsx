import type { ReactNode } from "react";
import {
  Box,
  CircularProgress,
  Divider,
  FormControlLabel,
  MenuItem,
  Paper,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { useSnackbar } from "@/components/Snackbar";
import { useAppInfo, useSaveSettings, useSettings } from "@/ipc/hooks";
import { errorMessage, type Settings, type ThemeMode } from "@/ipc/types";

export function SettingsPage() {
  const snackbar = useSnackbar();
  const settings = useSettings();
  const info = useAppInfo();
  const save = useSaveSettings();

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
          <Toggle
            label="Blinking cursor"
            checked={s.cursorBlink}
            onChange={(v) => update({ cursorBlink: v })}
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
        </Card>

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
          <Divider sx={{ my: 1 }} />
          <Typography variant="body2" color="text.secondary">
            Termoso sends nothing anywhere unless you sign in to a server you chose. No analytics,
            no crash reports, no update pings.
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
