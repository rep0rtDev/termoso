import { useState, type ReactNode } from "react";
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
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import FingerprintRoundedIcon from "@mui/icons-material/FingerprintRounded";
import TuneRoundedIcon from "@mui/icons-material/TuneRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import KeyboardRoundedIcon from "@mui/icons-material/KeyboardRounded";
import FolderCopyRoundedIcon from "@mui/icons-material/FolderCopyRounded";
import ArticleRoundedIcon from "@mui/icons-material/ArticleRounded";
import SystemUpdateAltRoundedIcon from "@mui/icons-material/SystemUpdateAltRounded";
import InfoOutlinedIcon from "@mui/icons-material/InfoOutlined";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AccountPage } from "@/account/AccountPage";
import { setSettingsPage, useNav, type SettingsPage as PageId } from "@/app/navigation";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody } from "@/components/PageHeader";
import { useSnackbar } from "@/components/Snackbar";
import { Loading, Mono, SectionCard, SettingRow } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { keys, useAppInfo, useSaveSettings, useSettings } from "@/ipc/hooks";
import { UpdatesCard } from "@/update/UpdatesCard";
import { FontPicker, FontPreview } from "./FontPicker";
import { KeyboardPage } from "./KeyboardPage";
import { SftpPage } from "./SftpPage";
import { TeamPage } from "@/team/TeamPage";
import { VaultsPage } from "@/team/VaultsPage";
import { SshIdPage } from "@/sshid/SshIdPage";
import { ThemeGallery } from "./ThemeGallery";
import {
  errorMessage,
  TERM_TYPES,
  type CursorStyle,
  type RestoreCommands,
  type Settings,
  type SyncConflict,
  type TermType,
  type ThemeMode,
} from "@/ipc/types";

const PAGES: { id: PageId; label: string; icon: ReactNode }[] = [
  { id: "account", label: "Account & sync", icon: <PersonRoundedIcon /> },
  { id: "team", label: "Team", icon: <GroupsRoundedIcon /> },
  { id: "vaults", label: "Vaults", icon: <LockRoundedIcon /> },
  { id: "sshid", label: "SSH ID", icon: <FingerprintRoundedIcon /> },
  { id: "general", label: "General", icon: <TuneRoundedIcon /> },
  { id: "terminal", label: "Terminal", icon: <TerminalRoundedIcon /> },
  { id: "keyboard", label: "Keyboard", icon: <KeyboardRoundedIcon /> },
  { id: "sftp", label: "SFTP", icon: <FolderCopyRoundedIcon /> },
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
      {page === "team" ? (
        <TeamPage />
      ) : page === "vaults" ? (
        <VaultsPage />
      ) : page === "sshid" ? (
        <SshIdPage />
      ) : (
        <Page>{page === "account" ? <AccountPage /> : <PreferencesPage page={page} />}</Page>
      )}
    </Box>
  );
}

function PreferencesPage({
  page,
}: {
  page: Exclude<PageId, "account" | "team" | "vaults" | "sshid">;
}) {
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
        {page === "keyboard" && <KeyboardPage s={s} update={update} />}
        {page === "sftp" && <SftpPage s={s} update={update} />}
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

function DeepLinksRow() {
  const snackbar = useSnackbar();
  const register = useMutation({
    mutationFn: ipc.deepLinksRegister,
    onSuccess: (schemes) =>
      snackbar.notify(
        schemes.length
          ? `Termoso now opens ${schemes.map((s) => `${s}://`).join(", ")} links`
          : "Registration ran, but the system reports no handler — check your desktop settings",
      ),
    onError: (e) => snackbar.error(errorMessage(e)),
  });
  return (
    <SettingRow
      label="Open ssh://, telnet:// and termoso:// links"
      hint="Installers register the handler already; use this for AppImage or portable builds. Links open a saved host or a quick connection — passwords in URLs are ignored."
      last
      control={
        <Button
          variant="outlined"
          size="small"
          onClick={() => register.mutate()}
          disabled={register.isPending}
        >
          Register as handler
        </Button>
      }
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
          control={
            <Toggle
              checked={s.autostartForwarding}
              onChange={(v) => update({ autostartForwarding: v })}
            />
          }
        />
        <SettingRow
          label="Detect OS on first connection"
          hint="Reads /etc/os-release once after connecting to pick the host's icon. Nothing leaves the SSH session."
          control={<Toggle checked={s.detectOs} onChange={(v) => update({ detectOs: v })} />}
        />
        <SettingRow
          label="Post-quantum key exchange"
          hint="Offers hybrid ML-KEM-768 + X25519 (mlkem768x25519-sha256) first; servers without it negotiate a classical exchange."
          control={
            <Toggle checked={s.postQuantumKex} onChange={(v) => update({ postQuantumKex: v })} />
          }
        />
        <DeepLinksRow />
      </SectionCard>

      <SectionCard title="SSH agent">
        <SettingRow
          label="Use the system SSH agent"
          hint="Offers keys held by ssh-agent (SSH_AUTH_SOCK), the Windows OpenSSH agent or Pageant after the host's own key. Private keys never leave the agent."
          last={!s.useSshAgent}
          control={<Toggle checked={s.useSshAgent} onChange={(v) => update({ useSshAgent: v })} />}
        />
        {s.useSshAgent && <AgentKeyList />}
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

function AgentKeyList() {
  const q = useQuery({ queryKey: ["agentKeys"], queryFn: ipc.agentKeys, staleTime: 10_000 });
  const a = q.data;
  return (
    <Box sx={{ py: 1 }}>
      <Box sx={{ display: "flex", alignItems: "center", gap: 2, mb: a?.keys.length ? 0.75 : 0 }}>
        <Typography variant="caption" color="text.secondary" sx={{ flex: 1 }}>
          {q.isPending
            ? "Looking for an agent…"
            : a?.available
              ? a.keys.length === 0
                ? "Agent reachable, no keys loaded (ssh-add to add one)."
                : `Agent reachable · ${a.keys.length} key${a.keys.length === 1 ? "" : "s"}`
              : `No agent: ${a?.error ?? errorMessage(q.error)}`}
        </Typography>
        <Button
          size="small"
          color="inherit"
          onClick={() => void q.refetch()}
          disabled={q.isFetching}
        >
          Refresh
        </Button>
      </Box>
      {a?.keys.map((k) => (
        <Box
          key={k.fingerprint}
          sx={{ display: "flex", gap: 1.5, alignItems: "baseline", minWidth: 0, py: 0.25 }}
        >
          <Mono secondary>{k.keyType}</Mono>
          <Mono
            sx={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
          >
            {k.fingerprint}
          </Mono>
          {k.comment && (
            <Typography variant="caption" color="text.secondary" noWrap sx={{ maxWidth: 200 }}>
              {k.comment}
            </Typography>
          )}
        </Box>
      ))}
    </Box>
  );
}

function Terminal({ s, update }: SectionProps) {
  return (
    <>
      <ThemeGallery value={s.terminalTheme} onChange={(id) => update({ terminalTheme: id })} />

      <SectionCard title="Text">
        <SettingRow
          label="Font family"
          hint="Bundled faces ship with the app; Nerd Font symbols are always available as a fallback."
          control={
            <FontPicker
              value={s.terminalFontFamily}
              onChange={(name) => update({ terminalFontFamily: name })}
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
          label="Line height"
          hint="Multiplier of the font's natural height."
          control={
            <NumberInput
              value={s.terminalLineHeight}
              onChange={(n) => update({ terminalLineHeight: Math.min(2, Math.max(0.8, n)) })}
              min={0.8}
              max={2}
              step={0.05}
              width={90}
            />
          }
        />
        <FontPreview
          family={s.terminalFontFamily}
          size={s.terminalFontSize}
          lineHeight={s.terminalLineHeight}
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

      <SectionCard title="Colors">
        <SettingRow
          label="Bright bold colors"
          hint="Draw bold text in the bright variant of its color, as classic terminals do."
          control={<Toggle checked={s.brightBold} onChange={(v) => update({ brightBold: v })} />}
        />
        <SettingRow
          label="Keyword highlighting"
          hint="Colors Error, Warning, OK, Info and Debug words plus IP and MAC addresses in the output. Applied locally while rendering; nothing is sent to the host."
          last
          control={
            <Toggle
              checked={s.keywordHighlight}
              onChange={(v) => update({ keywordHighlight: v })}
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

      <SectionCard title="Connection">
        <SettingRow
          label="Autoreconnect"
          hint="When an SSH or Telnet session drops, retry up to 6 times (30–60 s apart) while keeping the terminal contents. Sessions you close yourself are left alone."
          control={
            <Toggle checked={s.autoReconnect} onChange={(v) => update({ autoReconnect: v })} />
          }
        />
        <SettingRow
          label="Terminal type"
          hint="Sent as TERM to the remote side and to the local shell. Takes effect for new sessions."
          control={
            <TextField
              select
              value={s.termType}
              onChange={(e) => update({ termType: e.target.value as TermType })}
              sx={{ width: 200 }}
            >
              {TERM_TYPES.map((t) => (
                <MenuItem key={t} value={t}>
                  {t}
                </MenuItem>
              ))}
            </TextField>
          }
        />
        <LocalShellRow value={s.localShell} onChange={(v) => update({ localShell: v })} />
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
          label="Shell integration"
          hint="Marks prompts and commands in bash, zsh and fish (OSC 133) so history, exit codes and the working directory are tracked. Nothing is written to your dotfiles."
          control={
            <Toggle
              checked={s.shellIntegration}
              onChange={(v) => update({ shellIntegration: v })}
            />
          }
        />
        <SettingRow
          label="Restore running commands"
          hint="Workspaces and the previous session remember each pane's working directory and the command it was running. Reopening always returns to the directory; the command can be placed on the prompt for you to confirm, run right away, or dropped."
          control={
            <TextField
              select
              value={s.restoreCommands}
              onChange={(e) => update({ restoreCommands: e.target.value as RestoreCommands })}
              sx={{ width: 200 }}
            >
              <MenuItem value="type">Type, don't run</MenuItem>
              <MenuItem value="run">Run automatically</MenuItem>
              <MenuItem value="never">Directory only</MenuItem>
            </TextField>
          }
        />
        <SettingRow
          label="Command autocomplete"
          hint="Offline suggestions from ~500 commands, their options, paths, snippets and your encrypted history. Tab inserts, Esc dismisses. Can be paused per session from the terminal menu."
          last
          control={
            <Toggle checked={s.autocomplete} onChange={(v) => update({ autocomplete: v })} />
          }
        />
      </SectionCard>
    </>
  );
}

const CUSTOM_SHELL = "\u0000custom";

/** Shells found on this machine plus a free-form path with arguments. */
function LocalShellRow({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const shells = useQuery({
    queryKey: ["localShells"],
    queryFn: ipc.localShells,
    staleTime: 60_000,
  });
  const found = shells.data ?? [];
  const listed = value === "" || found.includes(value);
  const [custom, setCustom] = useState(!listed);
  const [draft, setDraft] = useState(listed ? "" : value);
  const selectValue = custom || !listed ? CUSTOM_SHELL : value;
  const commit = () => {
    const next = draft.trim();
    if (next !== value) onChange(next);
  };
  return (
    <SettingRow
      label="Local terminal shell"
      hint='Program started by Local Terminal, optionally with arguments. Default is your login shell (or PowerShell on Windows). Quote a path that contains spaces: "C:\Program Files\PowerShell\7\pwsh.exe" -NoLogo.'
      last
      control={
        <Box sx={{ display: "flex", flexDirection: "column", gap: 1, alignItems: "flex-end" }}>
          <TextField
            select
            value={selectValue}
            onChange={(e) => {
              if (e.target.value === CUSTOM_SHELL) {
                setCustom(true);
                setDraft(listed ? "" : value);
                return;
              }
              setCustom(false);
              onChange(e.target.value);
            }}
            sx={{ width: 280 }}
          >
            <MenuItem value="">Default shell</MenuItem>
            {found.map((sh) => (
              <MenuItem key={sh} value={sh} sx={{ fontFamily: "monospace", fontSize: 13 }}>
                {sh}
              </MenuItem>
            ))}
            <MenuItem value={CUSTOM_SHELL}>Custom command…</MenuItem>
          </TextField>
          {(custom || !listed) && (
            <TextField
              value={draft}
              placeholder="/usr/bin/fish --login"
              onChange={(e) => setDraft(e.target.value)}
              onBlur={commit}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  commit();
                }
              }}
              sx={{ width: 280 }}
              slotProps={{ htmlInput: { spellCheck: false } }}
            />
          )}
        </Box>
      }
    />
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
        hint="Encrypted with your vault key before leaving the device; requires being signed in. Team vaults with session recording turned on by a manager always share their recordings."
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
