import { Alert, Card, CardContent, Grid, Stack, Typography } from "@mui/material";
import DevicesRoundedIcon from "@mui/icons-material/DevicesRounded";
import GroupsRoundedIcon from "@mui/icons-material/GroupsRounded";
import LockRoundedIcon from "@mui/icons-material/LockRounded";
import PeopleAltRoundedIcon from "@mui/icons-material/PeopleAltRounded";
import StorageRoundedIcon from "@mui/icons-material/StorageRounded";
import DataObjectRoundedIcon from "@mui/icons-material/DataObjectRounded";
import TrendingUpRoundedIcon from "@mui/icons-material/TrendingUpRounded";
import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { errorMessage } from "@/api/client";
import { adminApi } from "@/api/endpoints";
import { queryKeys, useServerInfo } from "@/api/hooks";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { Section } from "@/components/Section";
import { formatBytes } from "@/components/format";

function Stat({ icon, label, value }: { icon: ReactNode; label: string; value: string | number }) {
  return (
    <Grid size={{ xs: 12, sm: 6, md: 4, lg: 3 }}>
      <Card sx={{ height: "100%" }}>
        <CardContent>
          <Stack direction="row" spacing={2} sx={{ alignItems: "center" }}>
            <Stack
              sx={{
                width: 44,
                height: 44,
                borderRadius: 2,
                bgcolor: "primary.main",
                color: "primary.contrastText",
                alignItems: "center",
                justifyContent: "center",
                opacity: 0.9,
              }}
            >
              {icon}
            </Stack>
            <Stack>
              <Typography variant="h5" sx={{ fontWeight: 700, lineHeight: 1.1 }}>
                {typeof value === "number" ? value.toLocaleString() : value}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                {label}
              </Typography>
            </Stack>
          </Stack>
        </CardContent>
      </Card>
    </Grid>
  );
}

export function AdminOverviewPage() {
  const stats = useQuery({
    queryKey: queryKeys.adminStats,
    queryFn: adminApi.stats,
    refetchInterval: 30_000,
  });
  const info = useServerInfo();
  if (stats.isPending) return <Loading />;
  if (stats.isError) return <Alert severity="error">{errorMessage(stats.error)}</Alert>;
  const s = stats.data;
  const f = info.data?.features;
  return (
    <>
      <PageHeader
        title="Admin"
        subtitle={
          info.data ? `${info.data.name} · Termoso ${info.data.version}` : "Server overview"
        }
      />
      <Grid container spacing={2} sx={{ mb: 2.5 }}>
        <Stat icon={<PeopleAltRoundedIcon />} label="Users" value={s.users} />
        <Stat
          icon={<TrendingUpRoundedIcon />}
          label="Active in the last 30 days"
          value={s.active_users_30d}
        />
        <Stat icon={<DevicesRoundedIcon />} label="Active sessions" value={s.active_sessions} />
        <Stat icon={<GroupsRoundedIcon />} label="Teams" value={s.teams} />
        <Stat icon={<LockRoundedIcon />} label="Vaults" value={s.vaults} />
        <Stat icon={<DataObjectRoundedIcon />} label="Encrypted records" value={s.entities} />
        <Stat
          icon={<StorageRoundedIcon />}
          label="Session log storage"
          value={formatBytes(s.log_storage_bytes)}
        />
      </Grid>
      {f && (
        <Section
          title="Enabled features"
          description="Determined by the server configuration (environment variables)."
        >
          <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: "wrap" }}>
            <Feature on={f.email} label="Outgoing email (SMTP)" />
            <Feature on={f.session_logs} label="Session logs (S3)" />
            <Feature on={f.webauthn} label="WebAuthn / passkeys" />
            <Feature on={f.teams} label="Team creation" />
            <Feature
              on={(info.data?.sso_providers.length ?? 0) > 0}
              label={`SSO (${info.data?.sso_providers.length ?? 0} providers)`}
            />
          </Stack>
        </Section>
      )}
    </>
  );
}

function Feature({ on, label }: { on: boolean; label: string }) {
  return (
    <Typography
      variant="body2"
      sx={{
        px: 1.5,
        py: 0.5,
        borderRadius: 2,
        border: 1,
        borderColor: on ? "success.main" : "divider",
        color: on ? "success.main" : "text.disabled",
      }}
    >
      {on ? "On" : "Off"} · {label}
    </Typography>
  );
}
