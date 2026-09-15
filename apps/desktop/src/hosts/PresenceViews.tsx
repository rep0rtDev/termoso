import { useEffect, useMemo, useState } from "react";
import { Box, Tooltip, Typography } from "@mui/material";
import SensorsRoundedIcon from "@mui/icons-material/SensorsRounded";
import { SectionCard } from "@/components/ui";
import { useAccount, useTeamPresence, useVaults } from "@/ipc/hooks";
import type { Uuid } from "@/ipc/types";
import { initialsOf, PersonAvatar } from "@/team/PersonAvatar";
import {
  connectedFor,
  distinctPeople,
  platformLabel,
  protocolLabel,
  viewerName,
  viewersByHost,
  viewersSummary,
  type HostViewer,
} from "./presence";

const EMPTY: ReadonlyMap<Uuid, HostViewer[]> = new Map();

/**
 * Who is connected to each host of `vaultId` right now, keyed by host id.
 * Empty for local / personal vaults and for teams with presence switched off.
 */
export function useVaultPresence(vaultId: Uuid | null): ReadonlyMap<Uuid, HostViewer[]> {
  const vaults = useVaults();
  const account = useAccount();
  const me = account.data?.account?.userId ?? null;
  const teamId = me === null ? null : (vaults.data?.find((v) => v.id === vaultId)?.team_id ?? null);
  const presence = useTeamPresence(teamId);
  return useMemo(
    () => (presence.data ? viewersByHost(presence.data, me) : EMPTY),
    [presence.data, me],
  );
}

/** Re-renders once a minute so "connected for" labels stay honest. */
function useMinuteTick() {
  const [, setTick] = useState(0);
  useEffect(() => {
    const t = window.setInterval(() => setTick((n) => n + 1), 30_000);
    return () => window.clearInterval(t);
  }, []);
}

const MAX_FACES = 3;

/** Overlapping initials tiles of the people on a host, with a `+N` overflow; for cards and rows. */
export function PresenceStack({ viewers, size = 20 }: { viewers: HostViewer[]; size?: number }) {
  const people = distinctPeople(viewers);
  if (people.length === 0) return null;
  const shown = people.slice(0, MAX_FACES);
  const more = people.length - shown.length;
  return (
    <Tooltip title={`Connected now: ${viewersSummary(viewers)}`} placement="top">
      <Box
        aria-label={`${people.length} connected`}
        sx={{ display: "flex", alignItems: "center", flexShrink: 0, pl: `${size * 0.3}px` }}
      >
        {shown.map((v) => (
          <Box
            key={v.userId}
            sx={{
              ml: `-${size * 0.3}px`,
              borderRadius: "6px",
              outline: "2px solid",
              outlineColor: "surface.high",
              display: "flex",
            }}
          >
            <PersonAvatar
              size={size}
              label={initialsOf(v.displayName, v.email)}
              seed={v.email}
              kind="account"
            />
          </Box>
        ))}
        {more > 0 && (
          <Typography
            variant="caption"
            sx={{ ml: 0.5, color: "text.secondary", fontWeight: 600, lineHeight: 1 }}
          >
            +{more}
          </Typography>
        )}
      </Box>
    </Tooltip>
  );
}

/** Host panel card: every teammate device on this host, with device, protocols and duration. */
export function ConnectedNowCard({ viewers }: { viewers: HostViewer[] }) {
  useMinuteTick();
  if (viewers.length === 0) return null;
  const people = distinctPeople(viewers).length;
  return (
    <SectionCard
      title={
        <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
          <SensorsRoundedIcon fontSize="small" color="primary" />
          Connected now
          <Typography variant="caption" color="text.secondary">
            {people} {people === 1 ? "person" : "people"}
            {viewers.length !== people && ` · ${viewers.length} devices`}
          </Typography>
        </Box>
      }
      sx={{ gap: 1 }}
    >
      {viewers.map((v) => (
        <Box
          key={`${v.userId}:${v.deviceId}`}
          sx={{ display: "flex", alignItems: "center", gap: 1.25, minHeight: 36 }}
        >
          <PersonAvatar
            size={28}
            label={initialsOf(v.displayName, v.email)}
            seed={v.email}
            kind="account"
          />
          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="body2" noWrap sx={{ fontWeight: 500 }}>
              {v.me ? "You" : viewerName(v)}
              {v.me && v.displayName && (
                <Typography component="span" variant="caption" color="text.secondary">
                  {" "}
                  · {v.displayName}
                </Typography>
              )}
            </Typography>
            <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
              {[v.deviceName, platformLabel(v.platform), v.protocols.map(protocolLabel).join(" + ")]
                .filter(Boolean)
                .join(" · ")}
            </Typography>
          </Box>
          <Typography variant="caption" color="text.secondary" sx={{ flexShrink: 0 }}>
            {connectedFor(v.since)}
          </Typography>
        </Box>
      ))}
    </SectionCard>
  );
}
