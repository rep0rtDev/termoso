import type { ReactNode } from "react";
import {
  Box,
  Button,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import PersonRoundedIcon from "@mui/icons-material/PersonRounded";
import TuneRoundedIcon from "@mui/icons-material/TuneRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import ArticleRoundedIcon from "@mui/icons-material/ArticleRounded";
import SystemUpdateAltRoundedIcon from "@mui/icons-material/SystemUpdateAltRounded";
import InfoOutlinedIcon from "@mui/icons-material/InfoOutlined";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { AccountPage } from "@/account/AccountPage";
import { setSettingsPage, useNav, type SettingsPage as PageId } from "@/app/navigation";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import { Loading, Mono, SectionCard, SettingRow } from "@/components/ui";
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

const PAGES: { id: PageId; label: string; icon: ReactNode }[] = [
  { id: "account", label: "Account & sync", icon: <PersonRoundedIcon /> },
  { id: "general", label: "General", icon: <TuneRoundedIcon /> },
  { id: "terminal", label: "Terminal", icon: <TerminalRoundedIcon /> },
  { id: "logs", label: "Session logs", icon: <ArticleRoundedIcon /> },
  { id: "updates", label: "Updates", icon: <SystemUpdateAltRoundedIcon /> },
  { id: "about", label: "About", icon: <InfoOutlinedIcon /> },
];

export function SettingsPage() {
  const page = useNav((s) => s.settingsPage);
  return (
    <Box sx={{ flex: 1, minHeight: 0, display: "flex" }}>
      <Box
        sx={{
          width: 200,
          flexShrink: 0,
          borderRight: 1,
          borderColor: "border.light",
          py: 1,
          px: 1,
          overflowY: "auto",
        }}
      >
        <List dense disablePadding>
          {PAGES.map((p) => (
            <ListItemButton
              key={p.id}
              selected={page === p.id}
              onClick={() => setSettingsPage(p.id)}
              sx={{ "& .MuiListItemIcon-root": { minWidth: 32, "& svg": { fontSize: 18 } } }}
            >
              <ListItemIcon>{p.icon}</ListItemIcon>
              <ListItemText primary={p.label} />
            </ListItemButton>
          ))}
        </List>
      </Box>
      <Page>{page === "account" ? <AccountPage /> : <PreferencesPage page={page} />}</Page>
    </Box>
  );
}

function PreferencesPage({ page }: { page: Exclude<PageId, "account"> }) {
  const snackbar = useSnackbar();
  const settings = useSettings();
  const save = useSaveSettings();

  const update = (patch: Partial<Settings>) => {
    if (!settings.data) return;
    save.mutate(
      { ...settings.data, ...patch },
      { onError: (e) => snackbar.error(errorMessage(e)) },
    );
  };

  if (settings.isPending) return <Loading />;
  if (settings.error)
    return <EmptyState title="Settings unavailable" description={errorMessage(settings.error)} />;
  const s = settings.data;

  return (
    <PageBody>
      <Box sx={{ maxWidth: 760, display: "flex", flexDirection: "column", gap: 1.5 }}>
        {page === "general" && <General s={s} update={update} />}
        {page === "terminal" && <Terminal s={s} update={update} />}
        {page === "logs" && <Logs s={s} update={update} />}
        {page === "updates" && <UpdatesCard settings={s} onChange={update} />}
        {page === "about" && <About />}
      </Box>
    </PageBody>
  );
}

interface SectionProps {
  s: Settings;
  update: (patch: Partial<Settings>) => void;
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return <Switch checked={checked} onChange={(e) => onChange(e.target.checked)} />;
}

function NumberInput({
  value,
  onChange,
  min,
  max,
  step,
  width = 120,
}: {
  value: number;
  onChange: (n: number) => void;
  min: number;
  max: number;
  step?: number;
  width?: number;
}) {
  return (
    <TextField
      type="number"
      value={value}
      onChange={(e) => onChange(Math.max(min, Number(e.target.value)))}
      slotProps={{ htmlInput: { min, max, step } }}
      sx={{ width }}
    />
  );
}

function General({ s, update }: SectionProps) {
  return (
    <>
      <SectionCard title="Appearance">
        <SettingRow
          label="Theme"
          last
          control={
            <TextField
              select
              value={s.theme}
              onChange={(e) => update({ theme: e.target.value as ThemeMode })}
              sx={{ width: 200 }}
            >
              <MenuItem value="dark">Dark</MenuItem>
              <MenuItem value="light">Light</MenuItem>
              <MenuItem value="system">Follow system</MenuItem>
            </TextField>
          }
        />
      </SectionCard>

      <SectionCard title="Connections">
        <SettingRow
          label="Keep-alive interval"
          hint="Seconds between SSH keep-alive packets. 0 turns it off."
          control={
            <NumberInput
              value={s.keepAliveSeconds}
              onChange={(n) => update({ keepAliveSeconds: n })}
              min={0}
              max={3600}
            />
          }
        />
        <SettingRow
          label="Start forwarding rules on launch"
          hint="Rules marked auto-start are brought up when the app opens."
          last
          control={
            <Toggle
              checked={s.autostartForwarding}
              onChange={(v) => update({ autostartForwarding: v })}
            />
          }
        />
      </SectionCard>

      <SectionCard title="Sync">
        <SettingRow
          label="On conflict"
          hint="Which copy wins when the same item changed on two devices."
          control={
            <TextField
              select
              value={s.syncConflict}
              onChange={(e) => update({ syncConflict: e.target.value as SyncConflict })}
              sx={{ width: 200 }}
            >
              <MenuItem value="newest_wins">Newest change wins</MenuItem>
              <MenuItem value="local_wins">This device wins</MenuItem>
              <MenuItem value="server_wins">Server wins</MenuItem>
            </TextField>
          }
        />
        <SettingRow
          label="Background sync interval"
          hint="Seconds. 0 syncs only when something changes or the server signals an update."
          last
          control={
            <NumberInput
              value={s.syncIntervalSeconds}
              onChange={(n) => update({ syncIntervalSeconds: n === 0 ? 0 : Math.max(30, n) })}
              min={0}
              max={86_400}
              step={30}
            />
          }
        />
      </SectionCard>
    </>
  );
}

function Terminal({ s, update }: SectionProps) {
  return (
    <>
      <SectionCard title="Text">
        <SettingRow
          label="Font family"
          control={
            <TextField
              value={s.terminalFontFamily}
              onChange={(e) => update({ terminalFontFamily: e.target.value })}
              sx={{ width: 280 }}
            />
          }
        />
        <SettingRow
          label="Font size"
          control={
            <NumberInput
              value={s.terminalFontSize}
              onChange={(n) => update({ terminalFontSize: n })}
              min={8}
              max={40}
              width={90}
            />
          }
        />
        <SettingRow
          label="Scrollback lines"
          last
          control={
            <NumberInput
              value={s.scrollback}
              onChange={(n) => update({ scrollback: n })}
              min={100}
              max={1_000_000}
              step={100}
            />
          }
        />
      </SectionCard>

      <SectionCard title="Cursor">
        <SettingRow
          label="Style"
          control={
            <TextField
              select
              value={s.cursorStyle}
              onChange={(e) => update({ cursorStyle: e.target.value as CursorStyle })}
              sx={{ width: 160 }}
            >
              <MenuItem value="bar">Bar</MenuItem>
              <MenuItem value="block">Block</MenuItem>
              <MenuItem value="underline">Underline</MenuItem>
            </TextField>
          }
        />
        <SettingRow
          label="Blinking"
          last
          control={<Toggle checked={s.cursorBlink} onChange={(v) => update({ cursorBlink: v })} />}
        />
      </SectionCard>

      <SectionCard title="Behaviour">
        <SettingRow
          label="Audible bell"
          control={
            <Toggle checked={s.terminalBell} onChange={(v) => update({ terminalBell: v })} />
          }
        />
        <SettingRow
          label="Copy on select"
          control={
            <Toggle checked={s.copyOnSelect} onChange={(v) => update({ copyOnSelect: v })} />
          }
        />
        <SettingRow
          label="Paste on right click"
          control={
            <Toggle
              checked={s.pasteOnRightClick}
              onChange={(v) => update({ pasteOnRightClick: v })}
            />
          }
        />
        <SettingRow
          label="Confirm before pasting multiple lines"
          control={
            <Toggle
              checked={s.confirmPasteMultiline}
              onChange={(v) => update({ confirmPasteMultiline: v })}
            />
          }
        />
        <SettingRow
          label="Confirm before closing a connected tab"
          control={
            <Toggle checked={s.confirmCloseTab} onChange={(v) => update({ confirmCloseTab: v })} />
          }
        />
        <SettingRow
          label="Command autocomplete"
          hint="Suggests commands from your history as you type."
          last
          control={
            <Toggle checked={s.autocomplete} onChange={(v) => update({ autocomplete: v })} />
          }
        />
      </SectionCard>
    </>
  );
}

function Logs({ s, update }: SectionProps) {
  return (
    <SectionCard title="Session recording">
      <SettingRow
        label="Record terminal sessions"
        hint="Recordings are encrypted with your master key and stay on this device."
        control={
          <Toggle checked={s.recordSessions} onChange={(v) => update({ recordSessions: v })} />
        }
      />
      <SettingRow
        label="Keep recordings for"
        hint="Days. 0 keeps them forever."
        control={
          <NumberInput
            value={s.logRetentionDays}
            onChange={(n) => update({ logRetentionDays: n })}
            min={0}
            max={3650}
          />
        }
      />
      <SettingRow
        label="Upload to the account server"
        hint="Encrypted with your vault key before leaving the device; requires being signed in."
        last
        control={<Toggle checked={s.uploadLogs} onChange={(v) => update({ uploadLogs: v })} />}
      />
    </SectionCard>
  );
}

function About() {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const info = useAppInfo();
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
  const d = info.data;

  return (
    <>
      <SectionCard title="Termoso">
        <SettingRow label="Version" control={<Value>{d ? d.version : "…"}</Value>} />
        <SettingRow label="Platform" control={<Value>{d?.platform ?? "…"}</Value>} />
        <SettingRow
          label="Profile"
          last
          control={
            <Value>
              <Mono>{d?.profileDir ?? "…"}</Mono>
            </Value>
          }
        />
      </SectionCard>

      <SectionCard title="Security">
        <SettingRow
          label="Master key"
          hint={
            d?.masterKeySource === "file"
              ? "No OS keychain was available; the key is kept in an owner-only file in the profile."
              : "Encrypts the local database and every stored secret."
          }
          last
          control={
            d?.masterKeySource === "file" ? (
              <Button variant="tonal" disabled={migrate.isPending} onClick={() => migrate.mutate()}>
                Move to OS keychain
              </Button>
            ) : (
              <Value>{d ? "OS keychain" : "…"}</Value>
            )
          }
        />
      </SectionCard>

      <SectionCard title="Privacy">
        <Typography variant="body2" color="text.secondary">
          Termoso sends nothing anywhere unless you sign in to a server you chose or ask it to check
          for updates. No analytics, no crash reports, no background pings.
        </Typography>
      </SectionCard>
    </>
  );
}

function Value({ children }: { children: ReactNode }) {
  return (
    <Typography
      variant="body2"
      color="text.secondary"
      sx={{ maxWidth: 360, textAlign: "right", wordBreak: "break-all" }}
    >
      {children}
    </Typography>
  );
}
