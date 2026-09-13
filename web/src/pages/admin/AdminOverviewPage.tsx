import { Alert, Box, Card, CardContent, Grid, Stack, Typography } from "@mui/material";
import DevicesOutlinedIcon from "@mui/icons-material/DevicesOutlined";
import GroupsOutlinedIcon from "@mui/icons-material/GroupsOutlined";
import LockOutlinedIcon from "@mui/icons-material/LockOutlined";
import PeopleAltOutlinedIcon from "@mui/icons-material/PeopleAltOutlined";
import StorageOutlinedIcon from "@mui/icons-material/StorageOutlined";
import DataObjectRoundedIcon from "@mui/icons-material/DataObjectRounded";
import TrendingUpRoundedIcon from "@mui/icons-material/TrendingUpRounded";
import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { errorMessage } from "@/api/client";
import { adminApi } from "@/api/endpoints";
import { queryKeys, useServerInfo } from "@/api/hooks";
import { IconTile } from "@/components/IconTile";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { Section, SettingRow } from "@/components/Section";
import { formatBytes } from "@/components/format";

function Stat({ icon, label, value }: { icon: ReactNode; label: string; value: string | number }) {
  return (
    <Grid size={{ xs: 12, sm: 6, md: 4, lg: 3 }}>
      <Card sx={{ height: "100%" }}>
        <CardContent sx={{ p: 2, "&:last-child": { pb: 2 } }}>
          <Stack direction="row" spacing={1.5} sx={{ alignItems: "center" }}>
            <IconTile>{icon}</IconTile>
            <Stack sx={{ minWidth: 0 }}>
              <Typography variant="h3" component="div" sx={{ lineHeight: 1.2 }}>
                {typeof value === "number" ? value.toLocaleString() : value}
              </Typography>
              <Typography variant="body2" color="text.secondary" noWrap>
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
        <Stat icon={<PeopleAltOutlinedIcon />} label="Users" value={s.users} />
        <Stat
          icon={<TrendingUpRoundedIcon />}
          label="Active in the last 30 days"
          value={s.active_users_30d}
        />
        <Stat icon={<DevicesOutlinedIcon />} label="Active sessions" value={s.active_sessions} />
        <Stat icon={<GroupsOutlinedIcon />} label="Teams" value={s.teams} />
        <Stat icon={<LockOutlinedIcon />} label="Vaults" value={s.vaults} />
        <Stat icon={<DataObjectRoundedIcon />} label="Encrypted records" value={s.entities} />
        <Stat
          icon={<StorageOutlinedIcon />}
          label="Session log storage"
          value={formatBytes(s.log_storage_bytes)}
        />
      </Grid>
      {f && (
        <Section
          title="Enabled features"
          description="Determined by the server configuration (environment variables)."
          disablePadding
        >
          <Box sx={{ px: 2.5 }}>
            <Feature on={f.email} label="Outgoing email (SMTP)" />
            <Feature on={f.session_logs} label="Session logs (S3)" />
            <Feature on={f.webauthn} label="WebAuthn / passkeys" />
            <Feature on={f.teams} label="Team creation" />
            <Feature
              on={(info.data?.sso_providers.length ?? 0) > 0}
              label={`SSO (${info.data?.sso_providers.length ?? 0} providers)`}
            />
          </Box>
        </Section>
      )}
    </>
  );
}

function Feature({ on, label }: { on: boolean; label: string }) {
  return (
    <SettingRow
      label={label}
      control={
        <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
          <Box
            sx={{
              width: 8,
              height: 8,
              borderRadius: "50%",
              bgcolor: on ? "success.main" : "text.disabled",
            }}
          />
          <Typography variant="body2" color={on ? "text.primary" : "text.secondary"}>
            {on ? "Enabled" : "Off"}
          </Typography>
        </Stack>
      }
    />
  );
}
