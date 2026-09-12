import { useState, type SubmitEvent } from "react";
import { Alert, Box, Button, Grid, Stack, Switch, TextField, Typography } from "@mui/material";
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
        title="Server settings"
        subtitle="Runtime policy for this server. Connection settings (database, SMTP, S3, SSO) live in the environment."
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
      snack.notify("Settings saved");
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
      <Section title="Registration & sign-in" disablePadding>
        <Box sx={{ px: 2.5, py: 0.5 }}>
          <SettingRow
            label="Open registration"
            description="Anyone who can reach this server can create an account."
            control={
              <Switch
                checked={s.registration_open}
                onChange={(e) => set("registration_open", e.target.checked)}
              />
            }
          />
          <SettingRow
            label="Allowed email domains"
            description="Comma-separated, e.g. example.com, corp.example.org. Empty allows any domain."
            control={
              <TextField
                value={domains}
                onChange={(e) => setDomains(e.target.value)}
                placeholder="Any domain"
                fullWidth={false}
                sx={{ width: { xs: 180, sm: 280 } }}
              />
            }
          />
          <SettingRow
            label="Require verified email"
            description="Accounts cannot be used until the address is confirmed."
            control={
              <Switch
                checked={s.require_email_verification}
                onChange={(e) => set("require_email_verification", e.target.checked)}
              />
            }
          />
          <SettingRow
            label="New-device approval"
            description="Signing in from an unknown device requires a code sent by email."
            control={
              <Switch
                checked={s.new_device_email_approval}
                onChange={(e) => set("new_device_email_approval", e.target.checked)}
              />
            }
          />
          <SettingRow
            label="Users can create teams"
            description="When off, only administrators create teams."
            control={
              <Switch
                checked={s.users_can_create_teams}
                onChange={(e) => set("users_can_create_teams", e.target.checked)}
              />
            }
          />
        </Box>
      </Section>
      <Section title="Sessions & limits">
        <Grid container spacing={2}>
          <Grid size={{ xs: 12, sm: 6, md: 3 }}>
            <TextField
              fullWidth
              type="number"
              label="Session lifetime (days)"
              value={s.session_ttl_days}
              onChange={(e) => set("session_ttl_days", num(e.target.value, s.session_ttl_days))}
              slotProps={{ htmlInput: { min: 1, max: 3650 } }}
            />
          </Grid>
          <Grid size={{ xs: 12, sm: 6, md: 3 }}>
            <TextField
              fullWidth
              type="number"
              label="Max record size (MiB)"
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
              label="Max session log size (MiB)"
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
              label="Log quota per user (MiB)"
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
        </Grid>
        <Stack direction="row" spacing={1} sx={{ mt: 2.5 }}>
          <Button type="submit" variant="contained" disabled={!dirty || save.isPending}>
            Save settings
          </Button>
          <Button
            color="inherit"
            disabled={!dirty || save.isPending}
            onClick={() => {
              setS(initial);
              setDomains(initial.allowed_domains.join(", "));
            }}
          >
            Reset
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
    onSuccess: () => snack.notify(`Test email sent to ${to.trim()}`),
    onError: (e) => snack.error(errorMessage(e)),
  });
  const enabled = info.data?.features.email ?? false;
  return (
    <Section
      title="Outgoing email"
      description="SMTP is configured through TERMOSO_SMTP_* environment variables."
    >
      {!enabled ? (
        <Alert severity="info">
          SMTP is not configured. Email verification, device approval codes, email MFA and
          invitation emails are disabled.
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
            placeholder="Send a test email to"
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
            Send test
          </Button>
          <Typography variant="body2" color="text.secondary">
            Verifies the SMTP connection end-to-end.
          </Typography>
        </Box>
      )}
    </Section>
  );
}
