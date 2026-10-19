import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import {
  Alert,
  Box,
  Button,
  Chip,
  Divider,
  Grid,
  IconButton,
  Link,
  MenuItem,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type ReactNode, type SubmitEvent, useState } from "react";

import { adminApi } from "@/api/endpoints";
import { queryKeys, useServerInfo } from "@/api/hooks";
import type {
  AdminConfig,
  AdminConfigService,
  AdminSettingsMeta,
  ServerSettings,
} from "@/api/types";
import { useAuthState } from "@/auth/store";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { Section, SettingRow } from "@/components/Section";
import { formatBytes, formatDateTime } from "@/components/format";
import { useSnackbar } from "@/components/Snackbar";
import { tr, trn } from "@/i18n";
import { errorMessage } from "@/api/client";

const MiB = 1024 * 1024;
const DOMAIN_RE = /^(?=.{1,253}$)([a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$/;

export function AdminSettingsPage() {
  const settings = useQuery({ queryKey: queryKeys.adminSettings, queryFn: adminApi.settings });
  const config = useQuery({
    queryKey: queryKeys.adminConfig,
    queryFn: adminApi.config,
    refetchInterval: 30_000,
  });
  if (settings.isPending) return <Loading />;
  if (settings.isError) return <Alert severity="error">{errorMessage(settings.error)}</Alert>;
  return (
    <>
      <PageHeader
        title={tr("Server settings")}
        subtitle={tr(
          "Runtime policy stored in the database and editable here. Connection settings (database, Redis, S3, SMTP, SSO) come from the environment and are shown read-only below.",
        )}
      />
      <SettingsForm
        key={settings.dataUpdatedAt}
        initial={settings.data}
        meta={config.data?.settings}
        smtpEnabled={config.data ? config.data.smtp !== null : undefined}
      />
      <TestEmailSection />
      {config.isError ? (
        <Alert severity="error">{errorMessage(config.error)}</Alert>
      ) : config.data ? (
        <EnvironmentSections cfg={config.data} />
      ) : (
        <Loading />
      )}
    </>
  );
}

// ───────────────────────────── editable settings ────

function SettingsForm({
  initial,
  meta,
  smtpEnabled,
}: {
  initial: ServerSettings;
  meta: AdminSettingsMeta | undefined;
  smtpEnabled: boolean | undefined;
}) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [s, setS] = useState<ServerSettings>(initial);
  const [domains, setDomains] = useState(initial.allowed_domains.join(", "));
  const teams = useQuery({
    queryKey: ["admin", "teams", "all"],
    queryFn: () => adminApi.teams({ offset: 0, limit: 200 }),
  });
  const defaults = meta?.defaults;
  const limits = meta?.limits;

  const parsedDomains = domains
    .split(/[,\s]+/)
    .map((d) => d.trim().toLowerCase().replace(/^@/, ""))
    .filter((d) => d !== "");
  const badDomains = parsedDomains.filter((d) => !DOMAIN_RE.test(d));

  const save = useMutation({
    mutationFn: () => adminApi.putSettings({ ...s, allowed_domains: parsedDomains }),
    onSuccess: async (saved) => {
      qc.setQueryData(queryKeys.adminSettings, saved);
      await Promise.all([
        qc.invalidateQueries({ queryKey: queryKeys.serverInfo }),
        qc.invalidateQueries({ queryKey: queryKeys.adminConfig }),
      ]);
      snack.notify(tr("Settings saved"));
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  const set = <K extends keyof ServerSettings>(k: K, v: ServerSettings[K]) =>
    setS((prev) => ({ ...prev, [k]: v }));
  const num = (v: string, fallback: number) => {
    const n = Number(v);
    return Number.isFinite(n) && v.trim() !== "" ? n : fallback;
  };
  const dirty =
    JSON.stringify(s) !== JSON.stringify(initial) || domains !== initial.allowed_domains.join(", ");

  /** "Default: X · Reset" hint under a control when the value differs from the factory default. */
  const defaultHint = <K extends keyof ServerSettings>(
    k: K,
    render: (v: ServerSettings[K]) => string,
  ) => {
    if (!defaults) return null;
    const d = defaults[k];
    if (JSON.stringify(d) === JSON.stringify(s[k])) return null;
    return (
      <Typography variant="caption" color="text.secondary" component="div" sx={{ mt: 0.5 }}>
        {tr("Default: {v1}", { v1: render(d) })}
        {" · "}
        <Link component="button" type="button" variant="caption" onClick={() => set(k, d)}>
          {tr("Reset to default")}
        </Link>
      </Typography>
    );
  };
  const onOff = (v: boolean) => (v ? tr("on") : tr("off"));
  const range = (min: number | undefined, max: number | undefined, unit: string) =>
    min !== undefined && max !== undefined
      ? tr("{v1}–{v2} {v3}", { v1: min, v2: max, v3: unit })
      : undefined;

  return (
    <Box
      component="form"
      onSubmit={(e: SubmitEvent) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <Section title={tr("Registration & sign-in")} disablePadding>
        <Box sx={{ px: 2.5, py: 0.5 }}>
          <SettingRow
            label={tr("Open registration")}
            description={
              <>
                {tr(
                  "Anyone who can reach this server can create an account with e-mail and password. Off: accounts are created only through invitations, SSO sign-up (below) or by an administrator.",
                )}
                {defaultHint("registration_open", onOff)}
              </>
            }
            control={
              <Switch
                checked={s.registration_open}
                onChange={(e) => set("registration_open", e.target.checked)}
              />
            }
          />
          <SettingRow
            label={tr("Allowed email domains")}
            description={
              <>
                {tr(
                  "Sign-up and invitations accept only these domains (exact match, subdomains must be listed separately). Comma- or space-separated. Empty allows any domain.",
                )}
                {parsedDomains.length > 0 && (
                  <Typography
                    variant="caption"
                    color="text.secondary"
                    component="div"
                    sx={{ mt: 0.5 }}
                  >
                    {trn(parsedDomains.length, "{count} domain", "{count} domains")}
                  </Typography>
                )}
                {defaultHint("allowed_domains", (v) =>
                  v.length ? v.join(", ") : tr("any domain"),
                )}
              </>
            }
            control={
              <TextField
                value={domains}
                onChange={(e) => setDomains(e.target.value)}
                placeholder={tr("Any domain")}
                error={badDomains.length > 0}
                helperText={
                  badDomains.length > 0
                    ? tr("Not a domain: {v1}", { v1: badDomains.join(", ") })
                    : undefined
                }
                fullWidth={false}
                sx={{ width: { xs: 180, sm: 280 } }}
              />
            }
          />
          <SettingRow
            label={tr("Sign-up via single sign-on")}
            description={
              <>
                {tr(
                  "An account is created on first SSO sign-in even when registration is closed. Applies to the server-wide providers listed under Environment; team IdPs always create accounts for their own domains.",
                )}
                {defaultHint("sso_registration", onOff)}
              </>
            }
            control={
              <Switch
                checked={s.sso_registration}
                onChange={(e) => set("sso_registration", e.target.checked)}
              />
            }
          />
          <SettingRow
            label={tr("Require verified email")}
            description={
              <>
                {tr(
                  "The API refuses every call until the address is confirmed by the e-mailed link. Needs outgoing e-mail.",
                )}
                {defaultHint("require_email_verification", onOff)}
              </>
            }
            control={
              <Switch
                checked={s.require_email_verification}
                onChange={(e) => set("require_email_verification", e.target.checked)}
              />
            }
          />
          <SettingRow
            label={tr("New-device approval")}
            description={
              <>
                {tr(
                  "A sign-in from a device this account has never used must be confirmed with a one-time code sent by e-mail. Needs outgoing e-mail; without SMTP the check is skipped.",
                )}
                {smtpEnabled === false && s.new_device_email_approval && (
                  <Typography
                    variant="caption"
                    color="warning.main"
                    component="div"
                    sx={{ mt: 0.5 }}
                  >
                    {tr("SMTP is not configured — this setting currently has no effect.")}
                  </Typography>
                )}
                {defaultHint("new_device_email_approval", onOff)}
              </>
            }
            control={
              <Switch
                checked={s.new_device_email_approval}
                onChange={(e) => set("new_device_email_approval", e.target.checked)}
              />
            }
          />
          <SettingRow
            label={tr("Users can create teams")}
            description={
              <>
                {tr(
                  "Off: only server administrators create teams; existing teams are not affected.",
                )}
                {defaultHint("users_can_create_teams", onOff)}
              </>
            }
            divider={false}
            control={
              <Switch
                checked={s.users_can_create_teams}
                onChange={(e) => set("users_can_create_teams", e.target.checked)}
              />
            }
          />
        </Box>
      </Section>

      <Section
        title={tr("Corporate mode")}
        description={tr(
          "For a server that belongs to one organisation. Pick a home team and every account — new and existing, on their next sign-in — becomes its member with the team's default role.",
        )}
      >
        <SettingRow
          label={tr("Home team")}
          description={
            <>
              {tr(
                "All accounts automatically join this team. Clients mark it as the home team; leaving it is not possible.",
              )}
              {defaultHint("home_team_id", (v) => v ?? tr("none"))}
            </>
          }
          control={
            <TextField
              select
              size="small"
              value={s.home_team_id ?? ""}
              onChange={(e) => set("home_team_id", e.target.value === "" ? null : e.target.value)}
              sx={{ width: { xs: 180, sm: 280 } }}
            >
              <MenuItem value="">{tr("None")}</MenuItem>
              {(teams.data?.items ?? []).map((t) => (
                <MenuItem key={t.id} value={t.id}>
                  {t.name}
                </MenuItem>
              ))}
            </TextField>
          }
        />
        <SettingRow
          label={tr("SSO only")}
          description={
            <>
              {tr(
                "Only single sign-on (server-wide providers or the home team's IdP). Password sign-in and e-mail sign-up return sso_required. Bootstrap administrators from TERMOSO_ADMIN_EMAILS keep password sign-in so the server cannot lock you out. Requires at least one provider.",
              )}
              {defaultHint("sso_only", (v) => onOff(v ?? false))}
            </>
          }
          control={
            <Switch
              checked={s.sso_only ?? false}
              onChange={(e) => set("sso_only", e.target.checked)}
            />
          }
        />
        <SettingRow
          label={tr("Personal vaults")}
          description={
            <>
              {tr(
                "Off hides the personal vault in every client so everything lives in team vaults. Existing personal data is kept, not deleted, and reappears when turned back on.",
              )}
              {defaultHint("personal_vaults", (v) => onOff(v ?? true))}
            </>
          }
          divider={false}
          control={
            <Switch
              checked={s.personal_vaults ?? true}
              onChange={(e) => set("personal_vaults", e.target.checked)}
            />
          }
        />
      </Section>

      <Section
        title={tr("Sessions & limits")}
        description={tr(
          "Sizes are entered in MiB and stored in bytes; the exact stored value is shown under each field.",
        )}
      >
        <Grid container spacing={2}>
          <Grid size={{ xs: 12, sm: 6, md: 4 }}>
            <TextField
              fullWidth
              type="number"
              label={tr("Session lifetime (days)")}
              value={s.session_ttl_days}
              onChange={(e) => set("session_ttl_days", num(e.target.value, s.session_ttl_days))}
              slotProps={{
                htmlInput: {
                  min: limits?.session_ttl_days_min ?? 1,
                  max: limits?.session_ttl_days_max ?? 3650,
                },
              }}
              helperText={
                <>
                  {tr(
                    "Idle timeout: a session is extended on each use and expires this long after the last one. Applies to new sign-ins and, on their next request, to existing sessions. A team policy may shorten it; API bridges are not affected.",
                  )}
                  {limits && (
                    <>
                      {" "}
                      {range(limits.session_ttl_days_min, limits.session_ttl_days_max, tr("days"))}
                    </>
                  )}
                  {defaultHint("session_ttl_days", (v) => String(v))}
                </>
              }
            />
          </Grid>
          <Grid size={{ xs: 12, sm: 6, md: 4 }}>
            <TextField
              fullWidth
              type="number"
              label={tr("Max record size (MiB)")}
              value={s.max_entity_bytes / MiB}
              onChange={(e) =>
                set(
                  "max_entity_bytes",
                  Math.round(num(e.target.value, s.max_entity_bytes / MiB) * MiB),
                )
              }
              slotProps={{ htmlInput: { step: "any", min: 0.001, max: 4 } }}
              helperText={
                <>
                  {tr(
                    "Largest encrypted record (host, key, snippet…) the server accepts from a client.",
                  )}{" "}
                  {formatBytes(s.max_entity_bytes)}
                  {limits && (
                    <>
                      {" · "}
                      {tr("{v1}–{v2}", {
                        v1: formatBytes(limits.max_entity_bytes_min),
                        v2: formatBytes(limits.max_entity_bytes_max),
                      })}
                    </>
                  )}
                  {defaultHint("max_entity_bytes", formatBytes)}
                </>
              }
            />
          </Grid>
          <Grid size={{ xs: 12, sm: 6, md: 4 }}>
            <TextField
              fullWidth
              type="number"
              label={tr("Max session log size (MiB)")}
              value={s.max_log_bytes / MiB}
              onChange={(e) =>
                set("max_log_bytes", Math.round(num(e.target.value, s.max_log_bytes / MiB) * MiB))
              }
              slotProps={{ htmlInput: { step: "any", min: 0.001 } }}
              helperText={
                <>
                  {tr("Largest single session recording accepted for upload to S3.")}{" "}
                  {formatBytes(s.max_log_bytes)}
                  {limits && (
                    <>
                      {" · "}
                      {tr("min {v1}", { v1: formatBytes(limits.max_log_bytes_min) })}
                    </>
                  )}
                  {defaultHint("max_log_bytes", formatBytes)}
                </>
              }
            />
          </Grid>
          <Grid size={{ xs: 12, sm: 6, md: 4 }}>
            <TextField
              fullWidth
              type="number"
              label={tr("Log quota per user (MiB)")}
              value={s.log_quota_bytes / MiB}
              onChange={(e) =>
                set(
                  "log_quota_bytes",
                  Math.round(num(e.target.value, s.log_quota_bytes / MiB) * MiB),
                )
              }
              slotProps={{ htmlInput: { step: "any", min: 0 } }}
              helperText={
                <>
                  {tr(
                    "Total size of session recordings one account may keep in S3; new uploads are refused above it. 0 = unlimited.",
                  )}{" "}
                  {s.log_quota_bytes === 0 ? tr("Unlimited") : formatBytes(s.log_quota_bytes)}
                  {defaultHint("log_quota_bytes", (v) =>
                    v === 0 ? tr("unlimited") : formatBytes(v),
                  )}
                </>
              }
            />
          </Grid>
          <Grid size={{ xs: 12, sm: 6, md: 4 }}>
            <TextField
              fullWidth
              type="number"
              label={tr("Team activity log retention (days)")}
              value={s.audit_retention_days}
              onChange={(e) =>
                set("audit_retention_days", num(e.target.value, s.audit_retention_days))
              }
              slotProps={{ htmlInput: { min: 0, max: limits?.audit_retention_days_max ?? 3650 } }}
              helperText={
                <>
                  {tr(
                    "Server-wide default for team audit logs; a team policy may set its own value up to the cap from TERMOSO_AUDIT_RETENTION_DAYS_MAX. Older entries are deleted by a nightly job. 0 = keep forever.",
                  )}{" "}
                  {s.audit_retention_days === 0 ? tr("Kept forever") : null}
                  {limits && <> {tr("0–{v1} days", { v1: limits.audit_retention_days_max })}</>}
                  {defaultHint("audit_retention_days", (v) =>
                    v === 0 ? tr("forever") : String(v),
                  )}
                </>
              }
            />
          </Grid>
        </Grid>
        <Stack
          direction={{ xs: "column", sm: "row" }}
          spacing={1}
          sx={{ mt: 2.5, alignItems: { sm: "center" } }}
        >
          <Button
            type="submit"
            variant="contained"
            disabled={!dirty || save.isPending || badDomains.length > 0}
          >
            {tr("Save settings")}
          </Button>
          <Button
            color="inherit"
            disabled={!dirty || save.isPending}
            onClick={() => {
              setS(initial);
              setDomains(initial.allowed_domains.join(", "));
            }}
          >
            {tr("Reset")}
          </Button>
          <Box sx={{ flex: 1 }} />
          {meta && (
            <Typography variant="caption" color="text.secondary">
              {meta.updated_at
                ? meta.updated_by
                  ? tr("Last changed {v1} by {v2}", {
                      v1: formatDateTime(meta.updated_at),
                      v2: meta.updated_by,
                    })
                  : tr("Last changed {v1}", { v1: formatDateTime(meta.updated_at) })
                : tr("Never changed — factory defaults")}
            </Typography>
          )}
        </Stack>
      </Section>
    </Box>
  );
}

// ───────────────────────────── outgoing e-mail test ────

function TestEmailSection() {
  const info = useServerInfo();
  const snack = useSnackbar();
  const { session } = useAuthState();
  const [to, setTo] = useState(session?.user.email ?? "");
  const test = useMutation({
    mutationFn: () => adminApi.testEmail(to.trim()),
    onSuccess: () => snack.notify(tr("Test email sent to {v1}", { v1: to.trim() })),
    onError: (e) => snack.error(errorMessage(e)),
  });
  const enabled = info.data?.features.email ?? false;
  return (
    <Section
      title={tr("Outgoing email")}
      description={tr("SMTP is configured through TERMOSO_SMTP_* environment variables.")}
    >
      {!enabled ? (
        <Alert severity="info">
          {tr(
            "SMTP is not configured. Email verification, device approval codes, email MFA and invitation emails are disabled.",
          )}
        </Alert>
      ) : (
        <Box
          component="form"
          onSubmit={(e: SubmitEvent) => {
            e.preventDefault();
            test.mutate();
          }}
          sx={{ display: "flex", gap: 1, flexWrap: "wrap", alignItems: "center" }}
        >
          <TextField
            placeholder={tr("Send a test email to")}
            type="email"
            required
            value={to}
            onChange={(e) => setTo(e.target.value)}
            fullWidth={false}
            sx={{ width: { xs: "100%", sm: 320 } }}
          />
          <Button
            type="submit"
            variant="outlined"
            disabled={test.isPending || to.trim() === ""}
            sx={{ height: 36 }}
          >
            {tr("Send test")}
          </Button>
          <Typography variant="body2" color="text.secondary">
            {tr("Verifies the SMTP connection end-to-end.")}
          </Typography>
        </Box>
      )}
    </Section>
  );
}

// ───────────────────────────── environment (read-only) ────

function CopyButton({ value }: { value: string }) {
  const snack = useSnackbar();
  return (
    <Tooltip title={tr("Copy")}>
      <IconButton
        size="small"
        aria-label={tr("Copy")}
        onClick={() => {
          void navigator.clipboard.writeText(value).then(
            () => snack.notify(tr("Copied")),
            () => snack.error(tr("Could not copy")),
          );
        }}
        sx={{ ml: 0.5, p: 0.25 }}
      >
        <ContentCopyRoundedIcon sx={{ fontSize: 14 }} />
      </IconButton>
    </Tooltip>
  );
}

/** One `name · ENV_VAR` → value line; strings render monospace with a copy button. */
function KV({
  label,
  env,
  value,
  copy = true,
}: {
  label: ReactNode;
  env?: string;
  value: ReactNode;
  copy?: boolean;
}) {
  const text = typeof value === "string" ? value : null;
  return (
    <Box
      sx={{
        display: "flex",
        flexDirection: { xs: "column", sm: "row" },
        gap: { xs: 0.25, sm: 2 },
        py: 0.75,
        borderBottom: 1,
        borderColor: "divider",
        "&:last-of-type": { borderBottom: 0 },
      }}
    >
      <Box sx={{ flex: "0 0 auto", width: { sm: 260 }, minWidth: 0 }}>
        <Typography variant="body2">{label}</Typography>
        {env && (
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ fontFamily: "monospace", wordBreak: "break-all" }}
          >
            {env}
          </Typography>
        )}
      </Box>
      <Box sx={{ flex: 1, minWidth: 0, display: "flex", alignItems: "center" }}>
        {text !== null ? (
          <>
            <Typography
              variant="body2"
              sx={{ fontFamily: "monospace", wordBreak: "break-all" }}
              component="span"
            >
              {text === "" ? "—" : text}
            </Typography>
            {copy && text !== "" && <CopyButton value={text} />}
          </>
        ) : (
          value
        )}
      </Box>
    </Box>
  );
}

function Flag({ on }: { on: boolean }) {
  return (
    <Chip
      size="small"
      variant="outlined"
      color={on ? "success" : "default"}
      label={on ? tr("on") : tr("off")}
      sx={{ height: 20 }}
    />
  );
}

function Health({ svc, name }: { svc: AdminConfigService; name: string }) {
  const ok = svc.status === "ok";
  return (
    <Tooltip title={svc.error ?? ""} disableHoverListener={!svc.error}>
      <Chip
        size="small"
        color={ok ? "success" : "error"}
        variant={ok ? "outlined" : "filled"}
        label={
          ok
            ? svc.latency_ms !== null
              ? tr("{v1}: ok · {v2} ms", { v1: name, v2: svc.latency_ms })
              : tr("{v1}: ok", { v1: name })
            : tr("{v1}: error", { v1: name })
        }
      />
    </Tooltip>
  );
}

function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86_400);
  const h = Math.floor((secs % 86_400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return tr("{v1} d {v2} h", { v1: d, v2: h });
  if (h > 0) return tr("{v1} h {v2} min", { v1: h, v2: m });
  return tr("{v1} min", { v1: m });
}

function EnvironmentSections({ cfg }: { cfg: AdminConfig }) {
  const sv = cfg.server;
  const list = (v: string[]) => (v.length ? v.join(", ") : tr("none"));
  return (
    <>
      <Section
        id="environment"
        title={tr("Environment")}
        description={tr(
          "What this process is actually running with — environment variables plus built-in defaults, secrets removed. Probes run on every load and then every 30 s.",
        )}
        actions={
          <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: "wrap" }}>
            <Health svc={cfg.database} name="PostgreSQL" />
            <Health svc={cfg.redis} name="Redis" />
            {cfg.storage && (
              <Health
                svc={{
                  endpoint: "",
                  detail: null,
                  status: cfg.storage.status,
                  latency_ms: cfg.storage.latency_ms,
                  error: cfg.storage.error,
                }}
                name="S3"
              />
            )}
          </Stack>
        }
      >
        <Typography variant="overline" color="text.secondary">
          {tr("Process")}
        </Typography>
        <KV label={tr("Version")} value={sv.version} />
        <KV
          label={tr("Started")}
          value={`${formatDateTime(sv.started_at)} · ${tr("up {v1}", { v1: formatUptime(sv.uptime_secs) })}`}
          copy={false}
        />
        <KV label={tr("Server name")} env="TERMOSO_SERVER_NAME" value={sv.name} />
        <KV label={tr("API URL")} env="TERMOSO_PUBLIC_URL" value={sv.public_url} />
        <KV label={tr("Cabinet URL")} env="TERMOSO_WEB_URL" value={sv.web_url} />
        <KV label={tr("Listen address")} env="TERMOSO_BIND" value={sv.bind} />
        <KV
          label={tr("Trust X-Forwarded-For")}
          env="TERMOSO_TRUST_PROXY"
          value={<Flag on={sv.trust_proxy} />}
        />
        <KV
          label={tr("Extra CORS origins")}
          env="TERMOSO_CORS_ORIGINS"
          value={list(sv.cors_origins)}
        />
        <KV
          label={tr("Log")}
          env="TERMOSO_LOG / TERMOSO_LOG_FORMAT"
          value={`${sv.log} · ${sv.log_format}`}
        />
        <KV
          label={tr("Metrics (Prometheus)")}
          env="TERMOSO_METRICS__ENABLED / __BIND"
          value={sv.metrics_bind ?? tr("off")}
          copy={sv.metrics_bind !== null}
        />
        <KV
          label={tr("Data region")}
          env="TERMOSO_DATA_REGION"
          value={sv.data_region ?? tr("not set")}
        />

        <Typography variant="overline" color="text.secondary" sx={{ display: "block", mt: 2 }}>
          {tr("Policy from the environment")}
        </Typography>
        <KV
          label={tr("Minimum client version")}
          env="TERMOSO_MIN_CLIENT_VERSION"
          value={
            <>
              <Typography variant="body2" component="span" sx={{ fontFamily: "monospace" }}>
                {sv.min_client_version}
              </Typography>
              <Chip
                size="small"
                variant="outlined"
                sx={{ ml: 1, height: 20 }}
                label={sv.min_client_version_overridden ? tr("overridden") : tr("= server version")}
              />
            </>
          }
        />
        <KV
          label={tr("Bootstrap administrators")}
          env="TERMOSO_ADMIN_EMAILS"
          value={list(sv.admin_emails)}
        />
        <KV
          label={tr("Owner")}
          env="TERMOSO_OWNER_EMAIL"
          value={sv.owner_email ?? tr("first registered account")}
          copy={sv.owner_email !== null}
        />
        <KV
          label={tr("Start-over delay")}
          env="TERMOSO_START_OVER_DELAY_SECS"
          value={tr("{v1} s", { v1: sv.start_over_delay_secs })}
          copy={false}
        />
        <KV
          label={tr("Team deletion grace")}
          env="TERMOSO_TEAM_DELETION_GRACE_DAYS"
          value={trn(sv.team_deletion_grace_days, "{count} day", "{count} days")}
          copy={false}
        />
        <KV
          label={tr("Cap on team audit retention")}
          env="TERMOSO_AUDIT_RETENTION_DAYS_MAX"
          value={trn(sv.team_audit_retention_days_max, "{count} day", "{count} days")}
          copy={false}
        />
        <KV
          label={tr("Audit events to process log")}
          env="TERMOSO_AUDIT_SINK"
          value={<Flag on={sv.audit_sink} />}
        />
        <KV
          label={tr("Webhooks to private addresses")}
          env="TERMOSO_WEBHOOKS_ALLOW_PRIVATE"
          value={<Flag on={sv.webhooks_allow_private} />}
        />
        <KV
          label={tr("Custom CA for syslog-TLS")}
          env="TERMOSO_WEBHOOKS_SYSLOG_CA"
          value={<Flag on={sv.webhooks_syslog_ca} />}
        />

        <Typography variant="overline" color="text.secondary" sx={{ display: "block", mt: 2 }}>
          {tr("Features")}
        </Typography>
        <KV label={tr("API bridges")} env="TERMOSO_BRIDGES" value={<Flag on={sv.bridges} />} />
        <KV
          label={tr("Team IdP (per-team SSO)")}
          env="TERMOSO_SSO__TEAM_IDP"
          value={<Flag on={sv.team_idp} />}
        />
        <KV
          label={tr("Turnstile on sign-up")}
          env="TERMOSO_TURNSTILE__*"
          value={<Flag on={sv.turnstile} />}
        />
        <KV label={tr("Swagger UI")} env="TERMOSO_SWAGGER_UI" value={<Flag on={sv.swagger_ui} />} />
        <KV label={tr("Landing page")} env="TERMOSO_LANDING" value={<Flag on={sv.landing} />} />
        <KV label={tr("Docs")} env="TERMOSO_DOCS" value={<Flag on={sv.docs} />} />
      </Section>

      <Section title={tr("Backing services")}>
        <Typography variant="overline" color="text.secondary">
          PostgreSQL
        </Typography>
        <KV label={tr("Endpoint")} env="TERMOSO_DATABASE_URL" value={cfg.database.endpoint} />
        <KV
          label={tr("Pool")}
          env="TERMOSO_DATABASE_MAX_CONNECTIONS"
          value={cfg.database.detail ?? "—"}
          copy={false}
        />
        {cfg.database.error && (
          <Alert severity="error" sx={{ mt: 1 }}>
            {cfg.database.error}
          </Alert>
        )}

        <Typography variant="overline" color="text.secondary" sx={{ display: "block", mt: 2 }}>
          Redis
        </Typography>
        <KV label={tr("Endpoint")} env="TERMOSO_REDIS_URL" value={cfg.redis.endpoint} />
        <KV
          label={tr("Key prefix")}
          env="TERMOSO_REDIS_PREFIX"
          value={cfg.redis.detail ?? "—"}
          copy={false}
        />
        {cfg.redis.error && (
          <Alert severity="error" sx={{ mt: 1 }}>
            {cfg.redis.error}
          </Alert>
        )}

        <Typography variant="overline" color="text.secondary" sx={{ display: "block", mt: 2 }}>
          {tr("S3 storage (session recordings)")}
        </Typography>
        {cfg.storage ? (
          <>
            <KV label={tr("Bucket")} env="TERMOSO_S3__BUCKET" value={cfg.storage.bucket} />
            <KV
              label={tr("Region")}
              env="TERMOSO_S3__REGION"
              value={cfg.storage.region ?? tr("not set")}
            />
            <KV
              label={tr("Endpoint")}
              env="TERMOSO_S3__ENDPOINT"
              value={cfg.storage.endpoint ?? tr("AWS default")}
            />
            <KV
              label={tr("Public endpoint (presigned URLs)")}
              env="TERMOSO_S3__PUBLIC_ENDPOINT"
              value={cfg.storage.public_endpoint ?? tr("same as endpoint")}
            />
            <KV
              label={tr("Path-style addressing")}
              env="TERMOSO_S3__FORCE_PATH_STYLE"
              value={<Flag on={cfg.storage.force_path_style} />}
            />
            <KV
              label={tr("Presigned URL lifetime")}
              env="TERMOSO_S3__PRESIGN_SECS"
              value={tr("{v1} s", { v1: cfg.storage.presign_secs })}
              copy={false}
            />
            {cfg.storage.error && (
              <Alert severity="error" sx={{ mt: 1 }}>
                {cfg.storage.error}
              </Alert>
            )}
          </>
        ) : (
          <Typography variant="body2" color="text.secondary">
            {tr("Not configured — session recordings are disabled.")}
          </Typography>
        )}

        <Typography variant="overline" color="text.secondary" sx={{ display: "block", mt: 2 }}>
          SMTP
        </Typography>
        {cfg.smtp ? (
          <>
            <KV
              label={tr("Host")}
              env="TERMOSO_SMTP__HOST / __PORT"
              value={`${cfg.smtp.host}:${cfg.smtp.port}`}
            />
            <KV
              label={tr("Security")}
              env="TERMOSO_SMTP__SECURITY"
              value={cfg.smtp.security}
              copy={false}
            />
            <KV label={tr("From")} env="TERMOSO_SMTP__FROM" value={cfg.smtp.from} />
            <KV
              label={tr("Authentication")}
              env="TERMOSO_SMTP__USERNAME"
              value={<Flag on={cfg.smtp.auth} />}
            />
          </>
        ) : (
          <Typography variant="body2" color="text.secondary">
            {tr("Not configured.")}
          </Typography>
        )}
      </Section>

      <Section title={tr("Identity & assistants")}>
        <Typography variant="overline" color="text.secondary">
          {tr("Server-wide SSO providers")}
        </Typography>
        {cfg.sso.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            {tr("None. Configure TERMOSO_SSO__<id>__* or let teams bring their own IdP.")}
          </Typography>
        ) : (
          cfg.sso.map((p) => (
            <KV
              key={p.id}
              label={
                <>
                  {p.name}{" "}
                  <Chip
                    size="small"
                    variant="outlined"
                    label={p.kind.toUpperCase()}
                    sx={{ height: 18, ml: 0.5 }}
                  />
                </>
              }
              env={`TERMOSO_SSO__${p.id.toUpperCase()}__*`}
              value={
                (p.issuer ?? "—") +
                (p.allowed_domains.length ? ` · ${p.allowed_domains.join(", ")}` : "")
              }
            />
          ))
        )}

        <Typography variant="overline" color="text.secondary" sx={{ display: "block", mt: 2 }}>
          {tr("WebAuthn / passkeys")}
        </Typography>
        {cfg.webauthn ? (
          <>
            <KV
              label={tr("Relying party")}
              env="TERMOSO_WEBAUTHN__RP_ID"
              value={cfg.webauthn.rp_id}
            />
            <KV
              label={tr("Display name")}
              env="TERMOSO_WEBAUTHN__RP_NAME"
              value={cfg.webauthn.rp_name}
            />
            <KV
              label={tr("Origins")}
              env="TERMOSO_WEBAUTHN__ORIGINS"
              value={list(cfg.webauthn.origins)}
            />
          </>
        ) : (
          <Typography variant="body2" color="text.secondary">
            {tr("Off — passkeys and hardware keys cannot be enrolled.")}
          </Typography>
        )}

        <Divider sx={{ my: 2 }} />
        <Typography variant="overline" color="text.secondary">
          {tr("AI assistant")}
        </Typography>
        {cfg.ai ? (
          <>
            <KV
              label={tr("Provider / model")}
              env="TERMOSO_AI__PROVIDER / __MODEL"
              value={`${cfg.ai.provider} · ${cfg.ai.model}`}
            />
            <KV label={tr("Endpoint")} env="TERMOSO_AI__URL" value={cfg.ai.endpoint} />
            <KV
              label={tr("Limits")}
              env="TERMOSO_AI__DAILY_QUOTA / __MAX_PROMPT_CHARS / __TIMEOUT_SECS"
              value={tr("{v1} requests/day · {v2} chars · {v3} s", {
                v1: cfg.ai.daily_quota,
                v2: cfg.ai.max_prompt_chars,
                v3: cfg.ai.timeout_secs,
              })}
              copy={false}
            />
            <KV
              label={tr("Confidential (provider keeps no prompts)")}
              env="TERMOSO_AI__CONFIDENTIAL"
              value={<Flag on={cfg.ai.confidential} />}
            />
          </>
        ) : (
          <Typography variant="body2" color="text.secondary">
            {tr("Off.")}
          </Typography>
        )}
      </Section>
    </>
  );
}
