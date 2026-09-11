import { useState } from "react";
import {
  Alert,
  Avatar,
  Box,
  Button,
  Chip,
  IconButton,
  List,
  ListItem,
  ListItemAvatar,
  ListItemText,
  Tooltip,
} from "@mui/material";
import ComputerRoundedIcon from "@mui/icons-material/ComputerRounded";
import LanguageRoundedIcon from "@mui/icons-material/LanguageRounded";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import PhoneAndroidRoundedIcon from "@mui/icons-material/PhoneAndroidRounded";
import PhoneIphoneRoundedIcon from "@mui/icons-material/PhoneIphoneRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "@/api/client";
import { accountApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { Device, Platform } from "@/api/types";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { PageHeader } from "@/components/PageHeader";
import { Section } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { formatDateTime, formatRelative } from "@/components/format";

function platformIcon(p: Platform) {
  switch (p) {
    case "android":
      return <PhoneAndroidRoundedIcon />;
    case "ios":
      return <PhoneIphoneRoundedIcon />;
    case "web":
      return <LanguageRoundedIcon />;
    case "cli":
      return <TerminalRoundedIcon />;
    default:
      return <ComputerRoundedIcon />;
  }
}

const platformLabel: Record<Platform, string> = {
  windows: "Windows",
  linux: "Linux",
  macos: "macOS",
  android: "Android",
  ios: "iOS",
  web: "Web",
  cli: "CLI",
};

export function DevicesPage() {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const devices = useQuery({ queryKey: queryKeys.devices, queryFn: accountApi.devices });
  const [target, setTarget] = useState<Device | null>(null);
  const [revokeOthers, setRevokeOthers] = useState(false);

  const revoke = useMutation({
    mutationFn: (ids: string[]) => Promise.all(ids.map((id) => accountApi.revokeDevice(id))),
    onSuccess: async (_r, ids) => {
      setTarget(null);
      setRevokeOthers(false);
      await qc.invalidateQueries({ queryKey: queryKeys.devices });
      snack.notify(ids.length === 1 ? "Device signed out" : `${ids.length} devices signed out`);
    },
    onError: (e) => snack.error(errorMessage(e)),
  });

  if (devices.isPending) return <Loading />;
  if (devices.isError) return <Alert severity="error">{errorMessage(devices.error)}</Alert>;

  const list = [...devices.data.devices].sort((a, b) => Number(b.current) - Number(a.current));
  const others = list.filter((d) => !d.current);

  return (
    <>
      <PageHeader
        title="Devices"
        subtitle="Every signed-in app and browser. Signing a device out revokes its session immediately."
        actions={
          others.length > 0 && (
            <Button
              variant="outlined"
              color="error"
              startIcon={<LogoutRoundedIcon />}
              onClick={() => setRevokeOthers(true)}
            >
              Sign out all other devices
            </Button>
          )
        }
      />
      <Section title={`${list.length} ${list.length === 1 ? "device" : "devices"}`} disablePadding>
        {list.length === 0 ? (
          <EmptyState title="No devices" />
        ) : (
          <List disablePadding>
            {list.map((d) => (
              <ListItem
                key={d.id}
                divider
                secondaryAction={
                  !d.current && (
                    <Tooltip title="Sign out this device">
                      <IconButton
                        edge="end"
                        onClick={() => setTarget(d)}
                        aria-label="Sign out device"
                      >
                        <LogoutRoundedIcon />
                      </IconButton>
                    </Tooltip>
                  )
                }
              >
                <ListItemAvatar>
                  <Avatar
                    sx={{
                      bgcolor: d.current ? "primary.main" : "action.selected",
                      color: d.current ? "primary.contrastText" : "text.primary",
                    }}
                  >
                    {platformIcon(d.platform)}
                  </Avatar>
                </ListItemAvatar>
                <ListItemText
                  primary={
                    <Box sx={{ display: "flex", alignItems: "center", gap: 1, flexWrap: "wrap" }}>
                      {d.name}
                      {d.current && <Chip size="small" color="primary" label="This device" />}
                    </Box>
                  }
                  secondary={`${platformLabel[d.platform]} · v${d.app_version} · ${d.last_ip ?? "unknown IP"} · Active ${formatRelative(d.last_seen_at)} · Added ${formatDateTime(d.created_at)}`}
                />
              </ListItem>
            ))}
          </List>
        )}
      </Section>

      <ConfirmDialog
        open={target !== null}
        title="Sign out device?"
        confirmLabel="Sign out"
        danger
        busy={revoke.isPending}
        onCancel={() => setTarget(null)}
        onConfirm={() => {
          if (target) revoke.mutate([target.id]);
        }}
      >
        “{target?.name}” will need to sign in again and unlock with the password.
      </ConfirmDialog>
      <ConfirmDialog
        open={revokeOthers}
        title="Sign out all other devices?"
        confirmLabel="Sign out all"
        danger
        busy={revoke.isPending}
        onCancel={() => setRevokeOthers(false)}
        onConfirm={() => revoke.mutate(others.map((d) => d.id))}
      >
        {others.length} {others.length === 1 ? "device" : "devices"} will be signed out. This
        browser stays signed in.
      </ConfirmDialog>
    </>
  );
}
