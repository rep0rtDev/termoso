import { useMemo, useState, type ReactNode } from "react";
import {
  Box,
  Button,
  Checkbox,
  FormControlLabel,
  Link,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import ArrowBackRoundedIcon from "@mui/icons-material/ArrowBackRounded";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import LaptopMacRoundedIcon from "@mui/icons-material/LaptopMacRounded";
import DnsRoundedIcon from "@mui/icons-material/DnsRounded";
import StorageRoundedIcon from "@mui/icons-material/StorageRounded";
import PublicRoundedIcon from "@mui/icons-material/PublicRounded";
import ArrowForwardRoundedIcon from "@mui/icons-material/ArrowForwardRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import {
  ActionMenu,
  EntityCard,
  Field,
  IconTile,
  SearchField,
  SidePanel,
  ToolIconButton,
  type MenuAction,
} from "@/components/ui";
import { HostAvatar } from "@/hosts/HostAvatar";
import { hostSubtitle } from "@/hosts/HostGrid";
import { requestCreate } from "@/app/navigation";
import {
  hostProtocols,
  type HostCard,
  type PfKind,
  type PfRuleCard,
  type PfRuleForm,
} from "@/ipc/types";
import { formatSize } from "@/sftp/format";
import { sizes } from "@/theme/theme";
import { KIND_LETTER, KIND_NAME, KIND_ORDER, formProblem, parsePort } from "./model";

/* ------------------------------------------------------------- pieces */

export function RuleTile({
  kind,
  active,
  size = sizes.tile,
}: {
  kind: PfKind;
  active?: boolean;
  size?: number;
}) {
  return (
    <IconTile size={size} color={active ? "primary.main" : undefined}>
      <Typography sx={{ fontWeight: 700, fontSize: size * 0.45, lineHeight: 1 }}>
        {KIND_LETTER[kind]}
      </Typography>
    </IconTile>
  );
}

const KIND_BLURB: Record<PfKind, string> = {
  local:
    "Local port forwarding exposes a port of a remote server as a port on this device: connections to the local port travel through the intermediate host to the destination.",
  remote:
    "Remote port forwarding opens a port on the remote host and forwards connections made to it back through this device to the destination.",
  dynamic:
    "Dynamic port forwarding turns Termoso into a SOCKS proxy server. SOCKS proxy server is a protocol to request any connection via a remote host.",
};

function Node({ icon, label, lit }: { icon: ReactNode; label: string; lit?: boolean }) {
  return (
    <Box
      sx={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 0.5,
        minWidth: 64,
      }}
    >
      <IconTile size={40} tone={lit ? "accent" : "neutral"}>
        {icon}
      </IconTile>
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ textAlign: "center", lineHeight: 1.2 }}
      >
        {label}
      </Typography>
    </Box>
  );
}

function Arrow() {
  return <ArrowForwardRoundedIcon sx={{ color: "text.disabled", mt: 1.25, fontSize: 18 }} />;
}

/** Who talks to whom for the given type; the highlighted node is where the port opens. */
export function KindDiagram({ kind }: { kind: PfKind }) {
  const device = <LaptopMacRoundedIcon fontSize="small" />;
  const server = <DnsRoundedIcon fontSize="small" />;
  const dest = <StorageRoundedIcon fontSize="small" />;
  const anywhere = <PublicRoundedIcon fontSize="small" />;
  return (
    <Box
      sx={{
        display: "flex",
        justifyContent: "center",
        alignItems: "flex-start",
        gap: 0.5,
        py: 1.5,
        px: 1,
        borderRadius: 2,
        bgcolor: "surface.high",
      }}
    >
      {kind === "local" && (
        <>
          <Node icon={device} label="This device" lit />
          <Arrow />
          <Node icon={server} label="Intermediate host" />
          <Arrow />
          <Node icon={dest} label="Destination" />
        </>
      )}
      {kind === "remote" && (
        <>
          <Node icon={server} label="Remote host" lit />
          <Arrow />
          <Node icon={device} label="This device" />
          <Arrow />
          <Node icon={dest} label="Destination" />
        </>
      )}
      {kind === "dynamic" && (
        <>
          <Node icon={device} label="SOCKS proxy" lit />
          <Arrow />
          <Node icon={server} label="Intermediate host" />
          <Arrow />
          <Node icon={anywhere} label="Any host" />
        </>
      )}
    </Box>
  );
}

export function KindTabs({ value, onChange }: { value: PfKind; onChange: (k: PfKind) => void }) {
  return (
    <ToggleButtonGroup
      exclusive
      fullWidth
      value={value}
      onChange={(_e, v: PfKind | null) => v && onChange(v)}
    >
      {KIND_ORDER.map((k) => (
        <ToggleButton key={k} value={k}>
          {KIND_NAME[k]}
        </ToggleButton>
      ))}
    </ToggleButtonGroup>
  );
}

function PortField({
  label,
  value,
  onChange,
  required,
  autoFocus,
}: {
  label: string;
  value: number;
  onChange: (n: number) => void;
  required?: boolean;
  autoFocus?: boolean;
}) {
  return (
    <Field label={required ? `${label} *` : label}>
      <TextField
        fullWidth
        autoFocus={autoFocus}
        value={value > 0 ? String(value) : ""}
        onChange={(e) => onChange(parsePort(e.target.value))}
        placeholder="1–65535"
        slotProps={{ htmlInput: { inputMode: "numeric" } }}
      />
    </Field>
  );
}

/** Read-only host slot: shows the picked host; `Hosts` opens the picker, `Remove Host` clears. */
function HostField({
  label,
  host,
  onPick,
  onClear,
}: {
  label: string;
  host: HostCard | null;
  onPick: () => void;
  onClear: () => void;
}) {
  return (
    <Field label={`${label} *`}>
      <Box sx={{ display: "flex", gap: 1, alignItems: "center" }}>
        <TextField
          fullWidth
          value={host?.label ?? ""}
          placeholder="Select a host"
          onClick={onPick}
          slotProps={{ input: { readOnly: true, sx: { cursor: "pointer" } } }}
        />
        {host ? (
          <Button
            variant="text"
            size="small"
            color="inherit"
            onClick={onClear}
            sx={{ flexShrink: 0 }}
          >
            Remove Host
          </Button>
        ) : (
          <Button variant="text" size="small" onClick={onPick} sx={{ flexShrink: 0 }}>
            Hosts
          </Button>
        )}
      </Box>
    </Field>
  );
}

function BackLink({ onClick }: { onClick: () => void }) {
  return (
    <Button
      variant="text"
      size="small"
      startIcon={<ArrowBackRoundedIcon />}
      onClick={onClick}
      sx={{ alignSelf: "flex-start", ml: -1 }}
    >
      Back
    </Button>
  );
}

/* --------------------------------------------------------- host picker */

export function HostPicker({
  hosts,
  vaultName,
  selectedId,
  onPick,
  onBack,
  onClose,
}: {
  hosts: HostCard[];
  vaultName: string;
  selectedId: string | null;
  onPick: (h: HostCard) => void;
  onBack: () => void;
  onClose: () => void;
}) {
  const [q, setQ] = useState("");
  const list = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return hosts
      .filter((h) => hostProtocols(h).includes("ssh"))
      .filter(
        (h) =>
          !needle ||
          h.label.toLowerCase().includes(needle) ||
          h.address.toLowerCase().includes(needle) ||
          h.username.toLowerCase().includes(needle),
      )
      .sort((a, b) => a.label.localeCompare(b.label));
  }, [hosts, q]);
  return (
    <SidePanel title="Select Host" subtitle={vaultName} onBack={onBack} onClose={onClose}>
      <Box sx={{ display: "flex", gap: 1, alignItems: "center" }}>
        <Button
          variant="tonal"
          startIcon={<AddRoundedIcon />}
          onClick={() => requestCreate("host")}
          sx={{ flexShrink: 0 }}
        >
          New Host
        </Button>
        <SearchField value={q} onChange={setQ} placeholder="Search hosts" width="100%" autoFocus />
      </Box>
      <Typography variant="body2" color="text.secondary" sx={{ fontWeight: 600 }}>
        Hosts
      </Typography>
      {list.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          {q ? "No SSH hosts match the search." : "No SSH hosts in this vault yet."}
        </Typography>
      ) : (
        <Stack spacing={1}>
          {list.map((h) => (
            <EntityCard
              key={h.id}
              dense
              tile={<HostAvatar host={h} size={sizes.tileSmall} />}
              title={h.label}
              subtitle={`${h.address} · ${hostSubtitle(h)}`}
              selected={h.id === selectedId}
              onClick={() => onPick(h)}
            />
          ))}
        </Stack>
      )}
    </SidePanel>
  );
}

/* ------------------------------------------------------------- editor */

function RuntimeLine({ rule }: { rule: PfRuleCard }) {
  const rt = rule.runtime;
  if (rt.state === "stopped") {
    return rt.lastError ? (
      <Typography variant="caption" color="error.main">
        Last error: {rt.lastError}
      </Typography>
    ) : null;
  }
  if (rt.state === "starting") {
    return (
      <Typography variant="caption" color="text.secondary">
        Starting…
      </Typography>
    );
  }
  if (rt.state === "reconnecting") {
    return (
      <Typography variant="caption" color="warning.main">
        Reconnecting (attempt {rt.attempt}){rt.lastError ? ` — ${rt.lastError}` : ""}
      </Typography>
    );
  }
  return (
    <Stack spacing={0}>
      {rt.bound && (
        <Typography variant="caption" color="text.secondary" noWrap>
          Listening on {rt.bound}
        </Typography>
      )}
      <Typography variant="caption" color="text.secondary" noWrap>
        {rt.active} active · {rt.connections} total · ↓ {formatSize(rt.bytesIn)} ↑{" "}
        {formatSize(rt.bytesOut)}
      </Typography>
    </Stack>
  );
}

export function RuleEditor({
  form,
  rule,
  hosts,
  vaultName,
  saving,
  onChange,
  onSave,
  onClose,
  onOpenWizard,
  menu,
}: {
  form: PfRuleForm;
  /** Stored card when editing (runtime line, header menu); null for a new rule. */
  rule: PfRuleCard | null;
  hosts: HostCard[];
  vaultName: string;
  saving: boolean;
  onChange: (f: PfRuleForm) => void;
  onSave: () => void;
  onClose: () => void;
  onOpenWizard: () => void;
  menu: MenuAction[];
}) {
  const [picking, setPicking] = useState(false);
  const [menuAnchor, setMenuAnchor] = useState<HTMLElement | null>(null);
  const host = hosts.find((h) => h.id === form.hostId) ?? null;
  const problem = formProblem(form);
  const set = (patch: Partial<PfRuleForm>) => onChange({ ...form, ...patch });

  if (picking) {
    return (
      <HostPicker
        hosts={hosts}
        vaultName={vaultName}
        selectedId={form.hostId || null}
        onPick={(h) => {
          set({ hostId: h.id });
          setPicking(false);
        }}
        onBack={() => setPicking(false)}
        onClose={onClose}
      />
    );
  }

  const hostField = (
    <HostField
      label={form.kind === "remote" ? "Remote host" : "Intermediate host"}
      host={host}
      onPick={() => setPicking(true)}
      onClear={() => set({ hostId: "" })}
    />
  );

  return (
    <SidePanel
      title={rule ? "Edit Port Forwarding" : "New Port Forwarding"}
      subtitle={vaultName}
      onClose={onClose}
      actions={
        rule && (
          <ToolIconButton title="More" onClick={(e) => setMenuAnchor(e.currentTarget)}>
            <MoreHorizRoundedIcon fontSize="small" />
          </ToolIconButton>
        )
      }
      footer={
        <Tooltip title={problem ?? ""} disableHoverListener={!problem}>
          <span style={{ flex: 1, display: "flex" }}>
            <Button
              variant="tonal"
              size="large"
              fullWidth
              disabled={Boolean(problem) || saving}
              onClick={onSave}
              sx={{ height: 40, borderRadius: 2.5 }}
            >
              {saving ? "Saving…" : rule ? "Save" : "Create"}
            </Button>
          </span>
        </Tooltip>
      }
    >
      <Box
        component="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (!problem && !saving) onSave();
        }}
        sx={{ display: "flex", flexDirection: "column", gap: 2 }}
      >
        {!rule && <KindTabs value={form.kind} onChange={(kind) => set({ kind })} />}
        <KindDiagram kind={form.kind} />
        {rule && <RuntimeLine rule={rule} />}

        <Box sx={{ display: "flex", gap: 1.5, alignItems: "flex-end" }}>
          <RuleTile
            kind={form.kind}
            active={rule?.runtime.state === "running"}
            size={sizes.tileSmall}
          />
          <Field label="Label" sx={{ flex: 1 }}>
            <TextField
              fullWidth
              value={form.label}
              onChange={(e) => set({ label: e.target.value })}
              placeholder="Label"
            />
          </Field>
        </Box>

        {form.kind === "remote" ? (
          <>
            {hostField}
            <PortField
              label="Remote port number"
              required
              value={form.remotePort}
              onChange={(remotePort) => set({ remotePort })}
            />
            <Field label="Bind address">
              <TextField
                fullWidth
                value={form.boundAddress}
                onChange={(e) => set({ boundAddress: e.target.value })}
                placeholder="127.0.0.1"
              />
            </Field>
            <Field label="Destination address *">
              <TextField
                fullWidth
                value={form.remoteHost}
                onChange={(e) => set({ remoteHost: e.target.value })}
                placeholder="127.0.0.1"
              />
            </Field>
            <PortField
              label="Destination port number"
              required
              value={form.localPort}
              onChange={(localPort) => set({ localPort })}
            />
          </>
        ) : (
          <>
            <PortField
              label="Local port number"
              required
              value={form.localPort}
              onChange={(localPort) => set({ localPort })}
            />
            <Field label="Bind address">
              <TextField
                fullWidth
                value={form.boundAddress}
                onChange={(e) => set({ boundAddress: e.target.value })}
                placeholder="127.0.0.1"
              />
            </Field>
            {hostField}
            {form.kind === "local" && (
              <>
                <Field label="Destination address *">
                  <TextField
                    fullWidth
                    value={form.remoteHost}
                    onChange={(e) => set({ remoteHost: e.target.value })}
                    placeholder="localhost"
                  />
                </Field>
                <PortField
                  label="Destination port number"
                  required
                  value={form.remotePort}
                  onChange={(remotePort) => set({ remotePort })}
                />
              </>
            )}
          </>
        )}

        <FormControlLabel
          control={
            <Checkbox
              checked={form.autoStart}
              onChange={(e) => set({ autoStart: e.target.checked })}
            />
          }
          label="Start automatically when Termoso launches"
        />

        {!rule && (
          <Typography variant="body2" color="text.secondary">
            Need help configuring the port forwarding rule?{" "}
            <Link component="button" type="button" onClick={onOpenWizard}>
              Open Port Forwarding Wizard
            </Link>
          </Typography>
        )}
      </Box>
      <ActionMenu anchor={menuAnchor} onClose={() => setMenuAnchor(null)} items={menu} />
    </SidePanel>
  );
}

/* ------------------------------------------------------------- wizard */

type Step = "type" | "listen" | "host" | "destination" | "label";

const STEPS: Record<PfKind, Step[]> = {
  local: ["type", "listen", "host", "destination", "label"],
  remote: ["type", "host", "listen", "destination", "label"],
  dynamic: ["type", "listen", "host", "label"],
};

const LISTEN_TEXT: Record<PfKind, { title: string; text: string; port: string }> = {
  local: {
    title: "Set the local port and binding address:",
    text: "This port will be open on the local (current) device, and traffic sent to it will be forwarded to the destination through the intermediate host.",
    port: "Local port number",
  },
  remote: {
    title: "Set the port and binding address:",
    text: "We will forward traffic from specified port and interface address of the selected host.",
    port: "Remote port number",
  },
  dynamic: {
    title: "Set the local port and binding address:",
    text: "This port will be open on the local (current) device, and it will receive the traffic.",
    port: "Local port number",
  },
};

const HOST_TEXT: Record<PfKind, { title: string; text: string }> = {
  local: {
    title: "Select the intermediate host:",
    text: "The intermediate host will receive the traffic and forward it to the destination host.",
  },
  remote: {
    title: "Select the remote host:",
    text: "Select a host where the port will be open. The traffic from this port will be forwarded to the destination host.",
  },
  dynamic: {
    title: "Select the intermediate host:",
    text: "The intermediate host will receive the traffic that will be forwarded to the local (current) host.",
  },
};

const DESTINATION_TEXT: Record<PfKind, string> = {
  local:
    "IP address or hostname and the port number of the remote host where the intermediate host will direct the traffic.",
  remote: "The destination address and port on this side where the traffic will be forwarded.",
  dynamic: "",
};

export function RuleWizard({
  form,
  hosts,
  vaultName,
  saving,
  onChange,
  onSave,
  onSkip,
  onClose,
}: {
  form: PfRuleForm;
  hosts: HostCard[];
  vaultName: string;
  saving: boolean;
  onChange: (f: PfRuleForm) => void;
  onSave: () => void;
  onSkip: () => void;
  onClose: () => void;
}) {
  const [index, setIndex] = useState(0);
  const [picking, setPicking] = useState(false);
  const steps = STEPS[form.kind];
  const step = steps[Math.min(index, steps.length - 1)];
  const set = (patch: Partial<PfRuleForm>) => onChange({ ...form, ...patch });
  const next = () => setIndex((i) => Math.min(i + 1, steps.length - 1));
  const back = () => setIndex((i) => Math.max(i - 1, 0));
  const host = hosts.find((h) => h.id === form.hostId) ?? null;

  if (picking) {
    return (
      <HostPicker
        hosts={hosts}
        vaultName={vaultName}
        selectedId={form.hostId || null}
        onPick={(h) => {
          set({ hostId: h.id });
          setPicking(false);
          next();
        }}
        onBack={() => setPicking(false)}
        onClose={onClose}
      />
    );
  }

  const listenPort = form.kind === "remote" ? form.remotePort : form.localPort;
  const setListenPort = (n: number) =>
    form.kind === "remote" ? set({ remotePort: n }) : set({ localPort: n });
  const destPort = form.kind === "remote" ? form.localPort : form.remotePort;
  const setDestPort = (n: number) =>
    form.kind === "remote" ? set({ localPort: n }) : set({ remotePort: n });
  const destOk = form.remoteHost.trim().length > 0 && destPort > 0;

  const heading = (t: string) => (
    <Typography variant="body2" sx={{ fontWeight: 600 }}>
      {t}
    </Typography>
  );
  const blurb = (t: string) => (
    <Typography variant="body2" color="text.secondary">
      {t}
    </Typography>
  );
  const primary = (label: string, disabled: boolean, onClick: () => void) => (
    <Button
      variant="contained"
      size="large"
      fullWidth
      disabled={disabled}
      onClick={onClick}
      sx={{ height: 40, borderRadius: 2.5 }}
    >
      {label}
    </Button>
  );

  return (
    <SidePanel
      title={step === "type" ? "New Port Forwarding" : `${KIND_NAME[form.kind]} Port Forwarding`}
      subtitle={vaultName}
      onClose={onClose}
    >
      <Box
        component="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (step === "listen" && listenPort > 0) next();
          else if (step === "destination" && destOk) next();
          else if (step === "label" && !saving) onSave();
        }}
        sx={{ display: "flex", flexDirection: "column", gap: 2 }}
      >
        {step !== "type" && <BackLink onClick={back} />}

        {step === "type" && (
          <>
            {heading("Select the port forwarding type:")}
            <KindTabs value={form.kind} onChange={(kind) => set({ kind })} />
            <KindDiagram kind={form.kind} />
            {blurb(KIND_BLURB[form.kind])}
            {primary("Continue", false, next)}
            <Button
              variant="tonal"
              size="large"
              fullWidth
              onClick={onSkip}
              sx={{ height: 40, borderRadius: 2.5 }}
            >
              Skip wizard
            </Button>
          </>
        )}

        {step === "listen" && (
          <>
            {heading(LISTEN_TEXT[form.kind].title)}
            <KindDiagram kind={form.kind} />
            {blurb(LISTEN_TEXT[form.kind].text)}
            <PortField
              label={LISTEN_TEXT[form.kind].port}
              required
              autoFocus
              value={listenPort}
              onChange={setListenPort}
            />
            <Field label="Bind address">
              <TextField
                fullWidth
                value={form.boundAddress}
                onChange={(e) => set({ boundAddress: e.target.value })}
                placeholder="127.0.0.1"
              />
            </Field>
            {primary("Continue", listenPort <= 0, next)}
          </>
        )}

        {step === "host" && (
          <>
            {heading(HOST_TEXT[form.kind].title)}
            <KindDiagram kind={form.kind} />
            {blurb(HOST_TEXT[form.kind].text)}
            {host && (
              <EntityCard
                dense
                tile={<HostAvatar host={host} size={sizes.tileSmall} />}
                title={host.label}
                subtitle={`${host.address} · ${hostSubtitle(host)}`}
                selected
                onClick={() => setPicking(true)}
              />
            )}
            {primary(host ? "Change host" : "Select a host", false, () => setPicking(true))}
            {host && primary("Continue", false, next)}
          </>
        )}

        {step === "destination" && (
          <>
            {heading("Select the destination host:")}
            <KindDiagram kind={form.kind} />
            {blurb(DESTINATION_TEXT[form.kind])}
            <Field label="Destination address *">
              <TextField
                fullWidth
                autoFocus
                value={form.remoteHost}
                onChange={(e) => set({ remoteHost: e.target.value })}
                placeholder={form.kind === "remote" ? "127.0.0.1" : "localhost"}
              />
            </Field>
            <PortField
              label="Destination port number"
              required
              value={destPort}
              onChange={setDestPort}
            />
            {primary("Continue", !destOk, next)}
          </>
        )}

        {step === "label" && (
          <>
            {heading("Select the label:")}
            <KindDiagram kind={form.kind} />
            <Field label="Label">
              <TextField
                fullWidth
                autoFocus
                value={form.label}
                onChange={(e) => set({ label: e.target.value })}
                placeholder="Label"
              />
            </Field>
            {primary(saving ? "Saving…" : "Done", saving, onSave)}
          </>
        )}
      </Box>
    </SidePanel>
  );
}
