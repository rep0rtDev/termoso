import { useState, type SubmitEvent } from "react";
import { tr } from "@/i18n";
import {
  Alert,
  MenuItem,
  Box,
  Button,
  Grid,
  Stack,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "@/api/client";
import { adminApi } from "@/api/endpoints";
import { queryKeys, useServerInfo } from "@/api/hooks";
import type { ServerSettings } from "@/api/types";
import { useAuthState } from "@/auth/store";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { Section, SettingRow } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { formatBytes } from "@/components/format";

const MiB = 1024 * 1024;

export function AdminSettingsPage() {
  const settings = useQuery({ queryKey: queryKeys.adminSettings, queryFn: adminApi.settings });
  if (settings.isPending) return <Loading />;
  if (settings.isError) return <Alert severity="error">{errorMessage(settings.error)}</Alert>;
  return (
    <>
      <PageHeader
        title={tr("Server settings")}
        subtitle={tr(
          "Runtime policy for this server. Connection settings (database, SMTP, S3, SSO) live in the environment.",
        )}
      />
      <SettingsForm key={settings.dataUpdatedAt} initial={settings.data} />
      <TestEmailSection />
    </>
  );
}

function SettingsForm({ initial }: { initial: ServerSettings }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [s, setS] = useState<ServerSettings>(initial);
  const [domains, setDomains] = useState(initial.allowed_domains.join(", "));
  const teams = useQuery({
    queryKey: ["admin", "teams", "all"],
    queryFn: () => adminApi.teams({ offset: 0, limit: 200 }),
  });

  const save = useMutation({
    mutationFn: () =>
      adminApi.putSettings({
        ...s,
        allowed_domains: domains
          .split(/[,\s]+/)
          .map((d) => d.trim().toLowerCase())
          .filter((d) => d !== ""),
      }),
    onSuccess: async (saved) => {
      qc.setQueryData(queryKeys.adminSettings, saved);
      await qc.invalidateQueries({ queryKey: queryKeys.serverInfo });
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
            description={tr("Anyone who can reach this server can create an account.")}
            control={
              <Switch
                checked={s.registration_open}
                onChange={(e) => set("registration_open", e.target.checked)}
              />
            }
          />
          <SettingRow
            label={tr("Allowed email domains")}
            description={tr(
              "Comma-separated, e.g. example.com, corp.example.org. Empty allows any domain.",
            )}
            control={
              <TextField
                value={domains}
                onChange={(e) => setDomains(e.target.value)}
                placeholder={tr("Any domain")}
                fullWidth={false}
                sx={{ width: { xs: 180, sm: 280 } }}
              />
            }
          />
          <SettingRow
            label={tr("Sign-up via single sign-on")}
            description={tr(
              "Users verified by a configured SSO provider may create an account even when registration is closed. Allowed domains still apply.",
            )}
            control={
              <Switch
                checked={s.sso_registration}
                onChange={(e) => set("sso_registration", e.target.checked)}
              />
            }
          />
          <SettingRow
            label={tr("Require verified email")}
            description={tr("Accounts cannot be used until the address is confirmed.")}
            control={
              <Switch
                checked={s.require_email_verification}
                onChange={(e) => set("require_email_verification", e.target.checked)}
              />
            }
          />
          <SettingRow
            label={tr("New-device approval")}
            description={tr("Signing in from an unknown device requires a code sent by email.")}
            control={
              <Switch
                checked={s.new_device_email_approval}
                onChange={(e) => set("new_device_email_approval", e.target.checked)}
              />
            }
          />
          <SettingRow
            label={tr("Users can create teams")}
            description={tr("When off, only administrators create teams.")}
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
          "For a server that belongs to one company. Pick a home team and every account — new and existing, on their next sign-in — becomes its member with the default role. SSO-only refuses password sign-in and e-mail sign-up (the bootstrap admin keeps a password so you cannot lock yourself out). Clients hide team creation, sign-up and personal vaults accordingly.",
        )}
      >
        <SettingRow
          label={tr("Home team")}
          description={tr("All accounts automatically join this team.")}
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
          description={tr(
            "Only single sign-on (server providers or the home team's IdP). Password sign-in and e-mail sign-up return sso_required.",
          )}
          control={
            <Switch
              checked={s.sso_only ?? false}
              onChange={(e) => set("sso_only", e.target.checked)}
            />
          }
        />
        <SettingRow
          label={tr("Personal vaults")}
          description={tr(
            "Off hides the personal vault in every client; everything lives in team vaults.",
          )}
          divider={false}
          control={
            <Switch
              checked={s.personal_vaults ?? true}
              onChange={(e) => set("personal_vaults", e.target.checked)}
            />
          }
        />
      </Section>
      <Section title={tr("Sessions & limits")}>
        <Grid container spacing={2}>
          <Grid size={{ xs: 12, sm: 6, md: 3 }}>
            <TextField
              fullWidth
              type="number"
              label={tr("Session lifetime (days)")}
              value={s.session_ttl_days}
              onChange={(e) => set("session_ttl_days", num(e.target.value, s.session_ttl_days))}
              slotProps={{ htmlInput: { min: 1, max: 3650 } }}
            />
          </Grid>
          <Grid size={{ xs: 12, sm: 6, md: 3 }}>
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
              helperText={formatBytes(s.max_entity_bytes)}
            />
          </Grid>
          <Grid size={{ xs: 12, sm: 6, md: 3 }}>
            <TextField
              fullWidth
              type="number"
              label={tr("Max session log size (MiB)")}
              value={s.max_log_bytes / MiB}
              onChange={(e) =>
                set("max_log_bytes", Math.round(num(e.target.value, s.max_log_bytes / MiB) * MiB))
              }
              helperText={formatBytes(s.max_log_bytes)}
            />
          </Grid>
          <Grid size={{ xs: 12, sm: 6, md: 3 }}>
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
              helperText={formatBytes(s.log_quota_bytes)}
            />
          </Grid>
          <Grid size={{ xs: 12, sm: 6, md: 3 }}>
            <TextField
              fullWidth
              type="number"
              label={tr("Team activity log retention (days)")}
              value={s.audit_retention_days}
              onChange={(e) =>
                set("audit_retention_days", num(e.target.value, s.audit_retention_days))
              }
              slotProps={{ htmlInput: { min: 0, max: 3650 } }}
              helperText={s.audit_retention_days === 0 ? tr("Kept forever") : undefined}
            />
          </Grid>
        </Grid>
        <Stack direction="row" spacing={1} sx={{ mt: 2.5 }}>
          <Button type="submit" variant="contained" disabled={!dirty || save.isPending}>
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
        </Stack>
      </Section>
    </Box>
  );
}

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
