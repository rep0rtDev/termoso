import { Alert, Box, Card, CardContent, Grid, Stack, Typography } from "@mui/material";
import { tr } from "@/i18n";
import { formatNumber } from "@/components/format";
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
                {typeof value === "number" ? formatNumber(value) : value}
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
        title={tr("Admin")}
        subtitle={
          info.data
            ? tr("{name} · Termoso {version}", { name: info.data.name, version: info.data.version })
            : tr("Server overview")
        }
      />
      <Grid container spacing={2} sx={{ mb: 2.5 }}>
        <Stat icon={<PeopleAltOutlinedIcon />} label={tr("Users")} value={s.users} />
        <Stat
          icon={<TrendingUpRoundedIcon />}
          label={tr("Active in the last 30 days")}
          value={s.active_users_30d}
        />
        <Stat
          icon={<DevicesOutlinedIcon />}
          label={tr("Active sessions")}
          value={s.active_sessions}
        />
        <Stat icon={<GroupsOutlinedIcon />} label={tr("Teams")} value={s.teams} />
        <Stat icon={<LockOutlinedIcon />} label={tr("Vaults")} value={s.vaults} />
        <Stat icon={<DataObjectRoundedIcon />} label={tr("Encrypted records")} value={s.entities} />
        <Stat
          icon={<StorageOutlinedIcon />}
          label={tr("Session log storage")}
          value={formatBytes(s.log_storage_bytes)}
        />
      </Grid>
      {f && (
        <Section
          title={tr("Enabled features")}
          description={tr("Determined by the server configuration (environment variables).")}
          disablePadding
        >
          <Box sx={{ px: 2.5 }}>
            <Feature on={f.email} label={tr("Outgoing email (SMTP)")} />
            <Feature on={f.session_logs} label={tr("Session logs (S3)")} />
            <Feature on={f.webauthn} label={tr("WebAuthn / passkeys")} />
            <Feature on={f.teams} label={tr("Team creation")} />
            <Feature
              on={(info.data?.sso_providers.length ?? 0) > 0}
              label={tr("SSO ({v1} providers)", { v1: info.data?.sso_providers.length ?? 0 })}
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
            {on ? tr("Enabled") : tr("Off")}
          </Typography>
        </Stack>
      }
    />
  );
}
