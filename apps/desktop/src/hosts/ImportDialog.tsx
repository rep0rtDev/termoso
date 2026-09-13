import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Collapse,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  IconButton,
  MenuItem,
  Select,
  Tab,
  Tabs,
  Tooltip,
  Typography,
} from "@mui/material";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import CableRoundedIcon from "@mui/icons-material/CableRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import ShieldOutlinedIcon from "@mui/icons-material/ShieldOutlined";
import SwapHorizRoundedIcon from "@mui/icons-material/SwapHorizRounded";
import TableChartOutlinedIcon from "@mui/icons-material/TableChartOutlined";
import FolderOpenRoundedIcon from "@mui/icons-material/FolderOpenRounded";
import DescriptionOutlinedIcon from "@mui/icons-material/DescriptionOutlined";
import WarningAmberRoundedIcon from "@mui/icons-material/WarningAmberRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import CheckCircleOutlineRoundedIcon from "@mui/icons-material/CheckCircleOutlineRounded";
import ArrowBackRoundedIcon from "@mui/icons-material/ArrowBackRounded";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import DownloadRoundedIcon from "@mui/icons-material/DownloadRounded";
import { open as openFile, save as saveFile } from "@tauri-apps/plugin-dialog";
import { useQueryClient } from "@tanstack/react-query";
import { useSnackbar } from "@/components/Snackbar";
import { CheckTile, IconTile, Loading, Mono, type TileTone } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { useAppInfo, useVaults } from "@/ipc/hooks";
import {
  errorMessage,
  type ImportApplyReport,
  type ImportPreview,
  type ImportSelection,
  type ImportSource,
  type ImportedHost,
  type LocalVault,
  type Uuid,
} from "@/ipc/types";
import { vaultHint, vaultIcon } from "@/app/vault";

type Section = keyof ImportSelection;

type Step =
  | { kind: "source"; busy: string | null }
  | { kind: "preview"; preview: ImportPreview }
  | { kind: "done"; report: ImportApplyReport; vaultName: string };

const SECTIONS: { key: Section; label: string }[] = [
  { key: "hosts", label: "Hosts" },
  { key: "keys", label: "Keys" },
  { key: "knownHosts", label: "Known hosts" },
  { key: "pfRules", label: "Forwarding" },
];

const SOURCE_LABEL: Record<ImportSource, string> = {
  ssh_config: "OpenSSH",
  putty: "PuTTY",
  csv: "Termius / CSV",
};

const all = (n: number) => Array.from({ length: n }, (_, i) => i);

const allOf = (p: ImportPreview): ImportSelection => ({
  hosts: all(p.hosts.length),
  keys: all(p.keys.length),
  knownHosts: all(p.knownHosts.length),
  pfRules: all(p.pfRules.length),
});

const count = (p: ImportPreview, s: Section) => p[s].length;

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

function skippedSummary(r: ImportApplyReport) {
  const parts = [
    r.skippedHosts > 0 && plural(r.skippedHosts, "host"),
    r.skippedKeys > 0 && plural(r.skippedKeys, "key"),
  ].filter((p): p is string => typeof p === "string");
  if (parts.length === 0) return "Everything selected was new.";
  const verb = r.skippedHosts + r.skippedKeys === 1 ? "was" : "were";
  return `${parts.join(" and ")} already existed and ${verb} reused.`;
}

/**
 * Hosts → New host → Import. Parsing happens in Rust and only non-secret
 * metadata comes back (`hasPassword`, key fingerprints); the vault is written
 * once the user confirms a selection and a destination vault.
 */
export function ImportDialog({
  open,
  vaultId,
  onClose,
  onImported,
}: {
  open: boolean;
  vaultId: Uuid;
  onClose: () => void;
  onImported: () => void;
}) {
  const [tall, setTall] = useState(false);
  const onStep = useCallback((kind: Step["kind"]) => setTall(kind === "preview"), []);
  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth="md"
      fullWidth
      slotProps={{
        paper: { sx: { height: tall ? "min(720px, calc(100vh - 64px))" : undefined } },
      }}
    >
      {open && <Body vaultId={vaultId} onClose={onClose} onImported={onImported} onStep={onStep} />}
    </Dialog>
  );
}

function Body({
  vaultId,
  onClose,
  onImported,
  onStep,
}: {
  vaultId: Uuid;
  onClose: () => void;
  onImported: () => void;
  onStep: (kind: Step["kind"]) => void;
}) {
  const snackbar = useSnackbar();
  const qc = useQueryClient();
  const vaults = useVaults();
  const info = useAppInfo();
  const [step, setStep] = useState<Step>({ kind: "source", busy: null });
  const [target, setTarget] = useState<Uuid>(vaultId);
  const [selection, setSelection] = useState<ImportSelection>({
    hosts: [],
    keys: [],
    knownHosts: [],
    pfRules: [],
  });
  const [section, setSection] = useState<Section>("hosts");
  const [showWarnings, setShowWarnings] = useState(false);
  const [applying, setApplying] = useState(false);

  const stepKind = step.kind;
  useEffect(() => onStep(stepKind), [stepKind, onStep]);

  const previewId = step.kind === "preview" ? step.preview.id : null;
  // A preview left behind (dialog closed, other source picked) still holds
  // secrets in Rust memory — drop it.
  useEffect(() => {
    if (!previewId) return;
    return () => {
      void ipc.importDiscard(previewId).catch(() => undefined);
    };
  }, [previewId]);

  const load = async (label: string, run: () => Promise<ImportPreview>) => {
    setStep({ kind: "source", busy: label });
    try {
      const preview = await run();
      setSelection(allOf(preview));
      setSection(
        preview.hosts.length > 0
          ? "hosts"
          : (SECTIONS.find((s) => count(preview, s.key) > 0)?.key ?? "hosts"),
      );
      setShowWarnings(false);
      setStep({ kind: "preview", preview });
    } catch (e) {
      snackbar.error(errorMessage(e));
      setStep({ kind: "source", busy: null });
    }
  };

  const pickFile = async (title: string, filters?: { name: string; extensions: string[] }[]) => {
    const picked = await openFile({ multiple: false, directory: false, title, filters });
    return typeof picked === "string" ? picked : null;
  };

  const scanSshDefault = () => load("Scanning ~/.ssh…", () => ipc.importScanSsh(null));
  const scanSshDir = async () => {
    const picked = await openFile({ directory: true, multiple: false, title: "OpenSSH directory" });
    if (typeof picked === "string") void load("Scanning…", () => ipc.importScanSsh(picked));
  };
  const parseSshConfig = async () => {
    const path = await pickFile("OpenSSH config");
    if (path) void load("Parsing…", () => ipc.importParseFile("ssh_config", path));
  };
  const scanPuttyRegistry = () =>
    load("Reading PuTTY registry…", () => ipc.importScanPuttyRegistry());
  const parsePuttyReg = async () => {
    const path = await pickFile("PuTTY registry export", [
      { name: "Registry export", extensions: ["reg"] },
    ]);
    if (path) void load("Parsing…", () => ipc.importParseFile("putty", path));
  };
  const parseCsv = async () => {
    const path = await pickFile("Termius CSV export", [
      { name: "CSV", extensions: ["csv", "txt", "tsv"] },
    ]);
    if (path) void load("Parsing…", () => ipc.importParseFile("csv", path));
  };
  const saveTemplate = async () => {
    const path = await saveFile({
      title: "Save CSV template",
      defaultPath: "termoso-hosts.csv",
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    try {
      await ipc.importCsvTemplateSave(path);
      snackbar.notify("Template saved");
    } catch (e) {
      snackbar.error(errorMessage(e));
    }
  };

  const apply = async () => {
    if (step.kind !== "preview") return;
    const vault = (vaults.data ?? []).find((v) => v.id === target);
    setApplying(true);
    try {
      const report = await ipc.importApply(target, step.preview.id, selection);
      await Promise.all([
        qc.invalidateQueries({ queryKey: ["hosts"] }),
        qc.invalidateQueries({ queryKey: ["hostForm"] }),
        qc.invalidateQueries({ queryKey: ["groups"] }),
        qc.invalidateQueries({ queryKey: ["tags"] }),
        qc.invalidateQueries({ queryKey: ["sshKeys"] }),
        qc.invalidateQueries({ queryKey: ["identities"] }),
        qc.invalidateQueries({ queryKey: ["proxies"] }),
        qc.invalidateQueries({ queryKey: ["hostChains"] }),
        qc.invalidateQueries({ queryKey: ["pfRules"] }),
        qc.invalidateQueries({ queryKey: ["knownHosts"] }),
      ]);
      onImported();
      setStep({ kind: "done", report, vaultName: vault?.name ?? "vault" });
    } catch (e) {
      snackbar.error(errorMessage(e));
    } finally {
      setApplying(false);
    }
  };

  const toggle = (s: Section, i: number) =>
    setSelection((sel) => {
      const set = new Set(sel[s]);
      if (set.has(i)) set.delete(i);
      else set.add(i);
      return { ...sel, [s]: [...set].sort((a, b) => a - b) };
    });
  const setAll = (s: Section, on: boolean) =>
    setSelection((sel) => ({
      ...sel,
      [s]: on && step.kind === "preview" ? all(count(step.preview, s)) : [],
    }));

  const totalSelected =
    selection.hosts.length +
    selection.keys.length +
    selection.knownHosts.length +
    selection.pfRules.length;

  const targetVault = (vaults.data ?? []).find((v) => v.id === target);
  const canApply =
    totalSelected > 0 && targetVault !== undefined && targetVault.unlocked && !applying;

  if (step.kind === "source") {
    return (
      <>
        <DialogTitle>Import hosts</DialogTitle>
        <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}>
          <Typography variant="body2" color="text.secondary">
            Pick where your hosts live today. Files are parsed locally and nothing is saved until
            you review the preview and confirm.
          </Typography>
          {step.busy ? (
            <Box sx={{ flex: 1, display: "grid", placeItems: "center" }}>
              <Box sx={{ textAlign: "center" }}>
                <Loading pt={0} />
                <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                  {step.busy}
                </Typography>
              </Box>
            </Box>
          ) : (
            <Box sx={{ display: "flex", flexDirection: "column", gap: 1.25 }}>
              <SourceCard
                icon={<TerminalRoundedIcon />}
                tone="accent"
                title="OpenSSH"
                description="~/.ssh/config hosts, ProxyJump chains, port forwards, referenced keys and known_hosts."
                actions={
                  <>
                    <Button
                      size="small"
                      variant="contained"
                      startIcon={<FolderOpenRoundedIcon />}
                      onClick={() => void scanSshDefault()}
                    >
                      Scan ~/.ssh
                    </Button>
                    <Button size="small" color="inherit" onClick={() => void scanSshDir()}>
                      Other folder…
                    </Button>
                    <Button size="small" color="inherit" onClick={() => void parseSshConfig()}>
                      Config file…
                    </Button>
                  </>
                }
              />
              <SourceCard
                icon={<CableRoundedIcon />}
                tone="info"
                title="PuTTY"
                description={
                  info.data?.platform === "windows"
                    ? "Saved sessions from the Windows registry or a .reg export: SSH/Telnet, proxy, tunnels, key files."
                    : "A .reg export of HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions (run `reg export` on the Windows machine)."
                }
                actions={
                  <>
                    {info.data?.platform === "windows" && (
                      <Button
                        size="small"
                        variant="contained"
                        onClick={() => void scanPuttyRegistry()}
                      >
                        Read registry
                      </Button>
                    )}
                    <Button
                      size="small"
                      variant={info.data?.platform === "windows" ? "text" : "contained"}
                      color={info.data?.platform === "windows" ? "inherit" : "primary"}
                      startIcon={<DescriptionOutlinedIcon />}
                      onClick={() => void parsePuttyReg()}
                    >
                      .reg export…
                    </Button>
                  </>
                }
              />
              <SourceCard
                icon={<TableChartOutlinedIcon />}
                tone="purple"
                title="Termius / CSV"
                description="Termius CSV export (Groups, Label, Tags, Hostname/IP, Protocol, Port, Username, Password) or any sheet with the same columns."
                actions={
                  <>
                    <Button
                      size="small"
                      variant="contained"
                      startIcon={<DescriptionOutlinedIcon />}
                      onClick={() => void parseCsv()}
                    >
                      CSV file…
                    </Button>
                    <Button
                      size="small"
                      color="inherit"
                      startIcon={<DownloadRoundedIcon />}
                      onClick={() => void saveTemplate()}
                    >
                      Template
                    </Button>
                  </>
                }
              />
            </Box>
          )}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button color="inherit" onClick={onClose}>
            Cancel
          </Button>
        </DialogActions>
      </>
    );
  }

  if (step.kind === "done") {
    const r = step.report;
    const rows: [string, number][] = [
      ["Hosts", r.hosts],
      ["Groups", r.groups],
      ["Tags", r.tags],
      ["Keys", r.keys],
      ["Proxies", r.proxies],
      ["Host chains", r.hostChains],
      ["Forwarding rules", r.pfRules],
      ["Known hosts", r.knownHosts],
    ];
    return (
      <>
        <DialogTitle>Import complete</DialogTitle>
        <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
          <Box sx={{ display: "flex", alignItems: "center", gap: 1.5 }}>
            <IconTile tone="accent" size={48}>
              <CheckCircleOutlineRoundedIcon />
            </IconTile>
            <Box>
              <Typography variant="subtitle1">
                {r.hosts === 1 ? "1 host" : `${r.hosts} hosts`} added to {step.vaultName}
              </Typography>
              <Typography variant="body2" color="text.secondary">
                {skippedSummary(r)}
              </Typography>
            </Box>
          </Box>
          <Box
            sx={{
              display: "grid",
              gridTemplateColumns: "repeat(4, 1fr)",
              gap: 1,
            }}
          >
            {rows.map(([label, n]) => (
              <Box
                key={label}
                sx={{
                  bgcolor: "surface.high",
                  borderRadius: 2,
                  px: 1.5,
                  py: 1.25,
                  opacity: n === 0 ? 0.55 : 1,
                }}
              >
                <Typography variant="h6" sx={{ lineHeight: 1.2 }}>
                  {n}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  {label}
                </Typography>
              </Box>
            ))}
          </Box>
          {r.warnings.length > 0 && <WarningList warnings={r.warnings} title="Notes" open />}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button
            color="inherit"
            onClick={() => setStep({ kind: "source", busy: null })}
            sx={{ mr: "auto" }}
          >
            Import more
          </Button>
          <Button variant="contained" onClick={onClose}>
            Done
          </Button>
        </DialogActions>
      </>
    );
  }

  const { preview } = step;
  const empty = SECTIONS.every((s) => count(preview, s.key) === 0);
  const selectedIn = selection[section].length;
  const totalIn = count(preview, section);

  return (
    <>
      <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1, pr: 2 }}>
        <IconButton
          size="small"
          onClick={() => setStep({ kind: "source", busy: null })}
          aria-label="Back to sources"
        >
          <ArrowBackRoundedIcon fontSize="small" />
        </IconButton>
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography component="div" variant="h6" noWrap>
            Import from {SOURCE_LABEL[preview.source]}
          </Typography>
          <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
            {preview.origin}
          </Typography>
        </Box>
        {preview.warnings.length > 0 && (
          <Chip
            size="small"
            color="warning"
            variant="outlined"
            icon={<WarningAmberRoundedIcon />}
            label={`${preview.warnings.length} warning${preview.warnings.length === 1 ? "" : "s"}`}
            onClick={() => setShowWarnings((v) => !v)}
          />
        )}
      </DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5, pt: 0 }}>
        <Collapse in={showWarnings}>
          <WarningList warnings={preview.warnings} open />
        </Collapse>
        {empty ? (
          <Alert severity="info" variant="outlined">
            Nothing importable was found in this source.
          </Alert>
        ) : (
          <>
            <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
              <Tabs
                value={section}
                onChange={(_, v: Section) => setSection(v)}
                sx={{ flex: 1, minHeight: 36 }}
              >
                {SECTIONS.map((s) => (
                  <Tab
                    key={s.key}
                    value={s.key}
                    disabled={count(preview, s.key) === 0}
                    label={
                      <Box sx={{ display: "flex", alignItems: "center", gap: 0.75 }}>
                        {s.label}
                        <Chip
                          size="small"
                          label={`${selection[s.key].length}/${count(preview, s.key)}`}
                          sx={{ height: 18, fontSize: 11 }}
                        />
                      </Box>
                    }
                    sx={{ minHeight: 36, py: 0.5 }}
                  />
                ))}
              </Tabs>
              <Button
                size="small"
                color="inherit"
                onClick={() => setAll(section, selectedIn < totalIn)}
              >
                {selectedIn < totalIn ? "Select all" : "Select none"}
              </Button>
            </Box>
            <Box
              sx={{
                flex: 1,
                minHeight: 0,
                overflowY: "auto",
                display: "flex",
                flexDirection: "column",
                gap: 0.75,
                pr: 0.5,
              }}
            >
              {section === "hosts" &&
                preview.hosts.map((h, i) => (
                  <HostRow
                    key={i}
                    host={h}
                    checked={selection.hosts.includes(i)}
                    onToggle={() => toggle("hosts", i)}
                  />
                ))}
              {section === "keys" &&
                preview.keys.map((k, i) => (
                  <Row
                    key={i}
                    tile={<KeyRoundedIcon />}
                    tone="warning"
                    title={k.name}
                    subtitle={
                      <>
                        {k.keyType}
                        {k.bits > 0 ? ` ${k.bits}` : ""} · <Mono secondary>{k.fingerprint}</Mono>
                      </>
                    }
                    meta={
                      <>
                        <Chip size="small" label={k.path} sx={{ maxWidth: 360 }} />
                        {k.encrypted && (
                          <Chip
                            size="small"
                            icon={<LockOutlinedIcon />}
                            label="Passphrase asked on connect"
                          />
                        )}
                      </>
                    }
                    checked={selection.keys.includes(i)}
                    onToggle={() => toggle("keys", i)}
                  />
                ))}
              {section === "knownHosts" &&
                preview.knownHosts.map((k, i) => (
                  <Row
                    key={i}
                    tile={<ShieldOutlinedIcon />}
                    tone="info"
                    title={k.hostname}
                    subtitle={
                      <>
                        {k.keyType} · <Mono secondary>{k.fingerprint}</Mono>
                      </>
                    }
                    checked={selection.knownHosts.includes(i)}
                    onToggle={() => toggle("knownHosts", i)}
                  />
                ))}
              {section === "pfRules" &&
                preview.pfRules.map((r, i) => (
                  <Row
                    key={i}
                    tile={<SwapHorizRoundedIcon />}
                    tone="purple"
                    title={`${r.kind[0]?.toUpperCase() ?? ""}${r.kind.slice(1)} · ${r.hostLabel}`}
                    subtitle={
                      <Mono secondary>
                        {r.kind === "dynamic"
                          ? `${r.boundAddress || "127.0.0.1"}:${r.localPort} → SOCKS`
                          : r.kind === "remote"
                            ? `remote ${r.boundAddress || "*"}:${r.localPort} → ${r.remoteHost}:${r.remotePort}`
                            : `${r.boundAddress || "127.0.0.1"}:${r.localPort} → ${r.remoteHost}:${r.remotePort}`}
                      </Mono>
                    }
                    checked={selection.pfRules.includes(i)}
                    onToggle={() => toggle("pfRules", i)}
                  />
                ))}
            </Box>
          </>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2, gap: 1.5 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1, mr: "auto", minWidth: 0 }}>
          <Typography variant="body2" color="text.secondary" sx={{ whiteSpace: "nowrap" }}>
            Import into
          </Typography>
          <VaultSelect vaults={vaults.data ?? []} value={target} onChange={setTarget} />
        </Box>
        <Button color="inherit" onClick={onClose} disabled={applying}>
          Cancel
        </Button>
        <Button variant="contained" onClick={() => void apply()} disabled={!canApply}>
          {applying
            ? "Importing…"
            : totalSelected === 0
              ? "Import"
              : `Import ${totalSelected} item${totalSelected === 1 ? "" : "s"}`}
        </Button>
      </DialogActions>
    </>
  );
}

function SourceCard({
  icon,
  tone,
  title,
  description,
  actions,
}: {
  icon: ReactNode;
  tone: TileTone;
  title: string;
  description: string;
  actions: ReactNode;
}) {
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 1.5,
        p: 1.5,
        borderRadius: 2,
        bgcolor: "surface.high",
      }}
    >
      <IconTile tone={tone} size={44}>
        {icon}
      </IconTile>
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="subtitle2">{title}</Typography>
        <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
          {description}
        </Typography>
      </Box>
      <Box
        sx={{ display: "flex", gap: 0.5, flexShrink: 0, flexWrap: "wrap", justifyContent: "end" }}
      >
        {actions}
      </Box>
    </Box>
  );
}

export function Row({
  tile,
  tone,
  title,
  subtitle,
  meta,
  warnings,
  checked,
  onToggle,
}: {
  tile: ReactNode;
  tone?: TileTone;
  title: ReactNode;
  subtitle?: ReactNode;
  meta?: ReactNode;
  warnings?: string[];
  checked: boolean;
  onToggle: () => void;
}) {
  return (
    <Box
      role="checkbox"
      aria-checked={checked}
      tabIndex={0}
      onClick={onToggle}
      onKeyDown={(e) => {
        if (e.key === " " || e.key === "Enter") {
          e.preventDefault();
          onToggle();
        }
      }}
      sx={{
        display: "flex",
        alignItems: "center",
        gap: 1.5,
        px: 1.25,
        py: 0.75,
        minHeight: 52,
        borderRadius: 2,
        bgcolor: checked ? "surface.high" : "transparent",
        cursor: "default",
        outline: "1px solid transparent",
        outlineOffset: -1,
        opacity: checked ? 1 : 0.7,
        transition: "background-color 100ms, opacity 100ms",
        "&:hover": { bgcolor: "surface.highest", opacity: 1 },
        "&:focus-visible": { outlineColor: "primary.main" },
      }}
    >
      <CheckTile
        checked={checked}
        hoverHint
        tile={tone ? <IconTile tone={tone}>{tile}</IconTile> : tile}
      />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body1" noWrap sx={{ fontWeight: 500, lineHeight: 1.35 }}>
          {title}
        </Typography>
        {subtitle && (
          <Typography
            variant="caption"
            color="text.secondary"
            noWrap
            component="div"
            sx={{ lineHeight: 1.35, mt: 0.25 }}
          >
            {subtitle}
          </Typography>
        )}
        {meta && (
          <Box sx={{ display: "flex", alignItems: "center", gap: 0.5, mt: 0.75, flexWrap: "wrap" }}>
            {meta}
          </Box>
        )}
      </Box>
      {warnings && warnings.length > 0 && (
        <Tooltip
          title={
            <Box component="ul" sx={{ m: 0, pl: 2 }}>
              {warnings.map((w, i) => (
                <li key={i}>{w}</li>
              ))}
            </Box>
          }
        >
          <WarningAmberRoundedIcon fontSize="small" color="warning" />
        </Tooltip>
      )}
    </Box>
  );
}

function HostRow({
  host,
  checked,
  onToggle,
}: {
  host: ImportedHost;
  checked: boolean;
  onToggle: () => void;
}) {
  const telnet = host.protocol === "telnet";
  const target = `${host.username ? `${host.username}@` : ""}${host.address}${
    host.port !== null ? `:${host.port}` : ""
  }`;
  const keyName = host.keyPath?.split(/[\\/]/).pop();
  return (
    <Row
      tile={telnet ? <CableRoundedIcon /> : <DnsRoundedIcon />}
      tone={telnet ? "info" : "accent"}
      title={host.label}
      subtitle={
        <>
          <Mono secondary>{target}</Mono>
          {telnet && " · Telnet"}
        </>
      }
      meta={
        <>
          {host.groupPath.length > 0 && (
            <Chip
              size="small"
              icon={<FolderOpenRoundedIcon />}
              label={host.groupPath.join(" / ")}
            />
          )}
          {host.tags.map((t) => (
            <Chip key={t} size="small" variant="outlined" label={t} />
          ))}
          {keyName && <Chip size="small" icon={<KeyRoundedIcon />} label={keyName} />}
          {host.hasPassword && <Chip size="small" icon={<LockOutlinedIcon />} label="Password" />}
          {host.jumpHosts.length > 0 && (
            <Chip size="small" label={`via ${host.jumpHosts.join(" → ")}`} />
          )}
          {host.proxy && (
            <Chip
              size="small"
              label={`${host.proxy.kind.toUpperCase()} ${host.proxy.host}:${host.proxy.port}`}
            />
          )}
          {host.agentForwarding && <Chip size="small" label="Agent forwarding" />}
        </>
      }
      warnings={host.warnings}
      checked={checked}
      onToggle={onToggle}
    />
  );
}

export function WarningList({
  warnings,
  title = "Warnings",
  open,
}: {
  warnings: string[];
  title?: string;
  open?: boolean;
}) {
  const [expanded, setExpanded] = useState(open ?? false);
  const shown = useMemo(() => (expanded ? warnings : warnings.slice(0, 3)), [expanded, warnings]);
  if (warnings.length === 0) return null;
  return (
    <Box sx={{ bgcolor: "surface.high", borderRadius: 2, px: 1.5, py: 1 }}>
      <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
        <WarningAmberRoundedIcon fontSize="small" color="warning" />
        <Typography variant="subtitle2" sx={{ flex: 1 }}>
          {title} ({warnings.length})
        </Typography>
        {warnings.length > 3 && (
          <IconButton size="small" onClick={() => setExpanded((v) => !v)}>
            <ExpandMoreRoundedIcon
              fontSize="small"
              sx={{
                transform: expanded ? "rotate(180deg)" : "none",
                transition: "transform 120ms",
              }}
            />
          </IconButton>
        )}
      </Box>
      <Box component="ul" sx={{ m: 0, mt: 0.5, pl: 2.5, maxHeight: 160, overflowY: "auto" }}>
        {shown.map((w, i) => (
          <Typography key={i} component="li" variant="caption" color="text.secondary">
            {w}
          </Typography>
        ))}
      </Box>
    </Box>
  );
}

export function VaultSelect({
  vaults,
  value,
  onChange,
}: {
  vaults: LocalVault[];
  value: Uuid;
  onChange: (id: Uuid) => void;
}) {
  return (
    <FormControl size="small" sx={{ minWidth: 180 }}>
      <Select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        renderValue={(id) => {
          const v = vaults.find((x) => x.id === id);
          return (
            <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
              {v && vaultIcon(v)}
              <span>{v?.name ?? "Vault"}</span>
            </Box>
          );
        }}
      >
        {vaults.map((v) => (
          <MenuItem key={v.id} value={v.id} disabled={!v.unlocked || v.role === "viewer"}>
            <Box sx={{ display: "flex", alignItems: "center", gap: 1, minWidth: 0 }}>
              {vaultIcon(v)}
              <Box sx={{ minWidth: 0 }}>
                <Typography variant="body2" noWrap>
                  {v.name}
                </Typography>
                <Typography variant="caption" color="text.secondary" noWrap>
                  {vaultHint(v)}
                </Typography>
              </Box>
            </Box>
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
}
