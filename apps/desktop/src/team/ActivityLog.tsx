import { useMemo, useState } from "react";
import { Box, Button, Chip, MenuItem, Stack, TextField, Tooltip, Typography } from "@mui/material";
import HistoryRoundedIcon from "@mui/icons-material/HistoryRounded";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import ChevronRightRoundedIcon from "@mui/icons-material/ChevronRightRounded";
import { useInfiniteQuery } from "@tanstack/react-query";
import { EmptyState } from "@/components/EmptyState";
import { Loading, SectionCard, ToolIconButton } from "@/components/ui";
import * as ipc from "@/ipc/commands";
import { useTeamMembers, useVaults } from "@/ipc/hooks";
import { errorMessage, type AuditEvent, type AuditFilter, type Team, type Uuid } from "@/ipc/types";
import { PersonAvatar, initialsOf } from "./PersonAvatar";
import { ACTIVITY_GROUPS, describeEvent, formatWhen, type ActivityContext } from "./activity";

const PAGE = 50;

function useActivity(teamId: Uuid, filter: Omit<AuditFilter, "before">, enabled = true) {
  return useInfiniteQuery({
    queryKey: ["teamAudit", teamId, filter],
    queryFn: ({ pageParam }) =>
      ipc.teamAudit(teamId, { ...filter, before: pageParam ?? undefined }),
    initialPageParam: null as number | null,
    getNextPageParam: (last) => last.next_before,
    enabled,
  });
}

function useActivityContext(teamId: Uuid): ActivityContext {
  const members = useTeamMembers(teamId);
  const vaults = useVaults();
  return useMemo(
    () => ({
      vaultNames: new Map((vaults.data ?? []).map((v) => [v.id, v.name])),
      people: new Map((members.data ?? []).map((m) => [m.user_id, m.display_name ?? m.email])),
    }),
    [vaults.data, members.data],
  );
}

function EventRow({ ev, ctx, dense }: { ev: AuditEvent; ctx: ActivityContext; dense?: boolean }) {
  const line = describeEvent(ev, ctx);
  const seed = ev.actor_email ?? ev.actor_id ?? "?";
  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "flex-start",
        gap: 1.5,
        px: dense ? 0 : 2,
        py: dense ? 0.75 : 1.25,
        minWidth: 0,
      }}
    >
      <PersonAvatar
        size={dense ? 28 : 32}
        seed={seed}
        kind={ev.actor_id ? "account" : "guest"}
        label={initialsOf(ev.actor_name, ev.actor_email ?? "?")}
      />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography variant="body2" sx={{ wordBreak: "break-word" }}>
          <Box component="span" sx={{ fontWeight: 600 }}>
            {line.actor}
          </Box>{" "}
          {line.text}
          {line.vault && (
            <>
              {" in "}
              <Box component="span" sx={{ fontWeight: 600 }}>
                {line.vault}
              </Box>
            </>
          )}
        </Typography>
        {line.meta && !dense && (
          <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
            {line.meta}
          </Typography>
        )}
      </Box>
      <Tooltip title={new Date(ev.created_at).toLocaleString()}>
        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ whiteSpace: "nowrap", pt: 0.25, flex: "0 0 auto" }}
        >
          {formatWhen(ev.created_at)}
        </Typography>
      </Tooltip>
    </Box>
  );
}

/** Compact "Activity" card on the team screen: the latest few events + View all. */
export function ActivityCard({ team, onOpen }: { team: Team; onOpen: () => void }) {
  const ctx = useActivityContext(team.id);
  const q = useActivity(team.id, { limit: 5 });
  const events = q.data?.pages[0]?.events ?? [];
  return (
    <SectionCard sx={{ p: 2.5, gap: 0.5 }}>
      <Box sx={{ display: "flex", alignItems: "center", gap: 1, mb: 0.5 }}>
        <Typography variant="subtitle1" sx={{ flex: 1, fontWeight: 600 }}>
          Activity
        </Typography>
        <Button
          size="small"
          endIcon={<ChevronRightRoundedIcon sx={{ fontSize: 16 }} />}
          onClick={onOpen}
          sx={{ minWidth: 0, px: 0.75 }}
        >
          View all
        </Button>
      </Box>
      {q.isPending ? (
        <Loading pt={1} />
      ) : q.error ? (
        <Typography variant="body2" color="error">
          {errorMessage(q.error)}
        </Typography>
      ) : events.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          Nothing has happened in this team yet.
        </Typography>
      ) : (
        events.map((ev) => <EventRow key={ev.id} ev={ev} ctx={ctx} dense />)
      )}
    </SectionCard>
  );
}

/** Full team activity log: filters by kind, member and vault; loads older pages on demand. */
export function ActivityLog({ team }: { team: Team }) {
  const ctx = useActivityContext(team.id);
  const members = useTeamMembers(team.id);
  const vaults = useVaults();
  const teamVaults = (vaults.data ?? []).filter((v) => v.kind === "team" && v.team_id === team.id);

  const [group, setGroup] = useState<string>("");
  const [actor, setActor] = useState<string>("");
  const [vault, setVault] = useState<string>("");
  const filter = useMemo<Omit<AuditFilter, "before">>(
    () => ({
      limit: PAGE,
      ...(group ? { action: group } : {}),
      ...(actor ? { actor } : {}),
      ...(vault ? { vault } : {}),
    }),
    [group, actor, vault],
  );
  const q = useActivity(team.id, filter);
  const events = q.data?.pages.flatMap((p) => p.events) ?? [];

  const select = (
    value: string,
    onChange: (v: string) => void,
    label: string,
    items: { value: string; label: string }[],
    width: number,
  ) => (
    <TextField
      select
      size="small"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      slotProps={{ htmlInput: { "aria-label": label } }}
      sx={{ width }}
    >
      {items.map((it) => (
        <MenuItem key={it.value} value={it.value}>
          {it.label}
        </MenuItem>
      ))}
    </TextField>
  );

  return (
    <SectionCard sx={{ p: 0, gap: 0, overflow: "hidden" }}>
      <Box
        sx={{
          display: "flex",
          alignItems: "center",
          gap: 1,
          px: 2,
          py: 1.5,
          flexWrap: "wrap",
          borderBottom: 1,
          borderColor: "border.light",
        }}
      >
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mr: 1 }}>
          Activity log
        </Typography>
        {select(group, setGroup, "Kind", [...ACTIVITY_GROUPS], 160)}
        {select(
          actor,
          setActor,
          "Member",
          [
            { value: "", label: "Everyone" },
            ...(members.data ?? []).map((m) => ({
              value: m.user_id,
              label: m.display_name ?? m.email,
            })),
          ],
          180,
        )}
        {select(
          vault,
          setVault,
          "Vault",
          [
            { value: "", label: "All vaults" },
            ...teamVaults.map((v) => ({ value: v.id, label: v.name })),
          ],
          160,
        )}
        <Box sx={{ flex: 1 }} />
        <ToolIconButton title="Refresh" onClick={() => void q.refetch()} disabled={q.isFetching}>
          <RefreshRoundedIcon fontSize="small" />
        </ToolIconButton>
      </Box>

      {q.isPending ? (
        <Loading pt={4} />
      ) : q.error ? (
        <Typography variant="body2" color="error" sx={{ p: 2 }}>
          {errorMessage(q.error)}
        </Typography>
      ) : events.length === 0 ? (
        <EmptyState
          compact
          icon={<HistoryRoundedIcon />}
          title="No activity"
          description={
            group || actor || vault
              ? "Nothing matches these filters."
              : "Team changes, vault access and shared data edits will show up here."
          }
        />
      ) : (
        <Stack divider={<Box sx={{ borderBottom: 1, borderColor: "border.light" }} />}>
          {events.map((ev) => (
            <EventRow key={ev.id} ev={ev} ctx={ctx} />
          ))}
        </Stack>
      )}

      {(q.hasNextPage || events.length > 0) && (
        <Box
          sx={{
            display: "flex",
            alignItems: "center",
            gap: 1,
            px: 2,
            py: 1,
            borderTop: 1,
            borderColor: "border.light",
          }}
        >
          <Chip
            size="small"
            variant="outlined"
            label={`${events.length} event${events.length === 1 ? "" : "s"}`}
          />
          <Box sx={{ flex: 1 }} />
          {q.hasNextPage && (
            <Button
              size="small"
              onClick={() => void q.fetchNextPage()}
              disabled={q.isFetchingNextPage}
            >
              {q.isFetchingNextPage ? "Loading…" : "Load older"}
            </Button>
          )}
        </Box>
      )}
    </SectionCard>
  );
}
