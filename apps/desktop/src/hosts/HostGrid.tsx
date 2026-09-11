import { Box, Checkbox, Chip, IconButton, Tooltip } from "@mui/material";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import KeyRoundedIcon from "@mui/icons-material/KeyRounded";
import type { MouseEvent, ReactNode } from "react";
import type { GroupNode, HostCard } from "@/ipc/types";
import { CardGrid, EntityCard, IconTile, SectionTitle } from "@/components/ui";
import { HostAvatar } from "./HostAvatar";

export interface HostCollectionProps {
  groups: GroupNode[];
  hosts: HostCard[];
  /** Host open in the editor. */
  selectedId: string | null;
  /** Multi-selection (checkboxes / ⌘-click). */
  checked: ReadonlySet<string>;
  showPath: boolean;
  onOpenGroup: (id: string) => void;
  onEditGroup: (g: GroupNode) => void;
  onGroupContext: (g: GroupNode, e: MouseEvent<HTMLElement>) => void;
  /** Plain click: modifier keys turn it into a selection toggle / range. */
  onOpenHost: (h: HostCard, e: MouseEvent<HTMLElement>) => void;
  onToggleHost: (h: HostCard) => void;
  onConnectHost: (h: HostCard) => void;
  onHostContext: (h: HostCard, e: MouseEvent<HTMLElement>) => void;
}

export function hostSubtitle(h: HostCard) {
  const port = h.protocol === "telnet" || h.port !== 22 ? `:${h.port}` : "";
  return `${h.username ? `${h.username}@` : ""}${h.address}${port}`;
}

export function groupSubtitle(g: GroupNode) {
  const parts = [];
  if (g.groupCount > 0) parts.push(`${g.groupCount} group${g.groupCount === 1 ? "" : "s"}`);
  parts.push(`${g.hostCount} host${g.hostCount === 1 ? "" : "s"}`);
  return parts.join(", ");
}

/** Card tile that turns into a checkbox on hover or when checked. */
export function SelectableTile({
  tile,
  checked,
  selecting,
  onToggle,
  size,
}: {
  tile: ReactNode;
  checked: boolean;
  /** A selection is in progress: always show the checkbox. */
  selecting: boolean;
  onToggle: () => void;
  size: number;
}) {
  return (
    <Box
      className={checked || selecting ? "tile-selecting" : undefined}
      sx={{
        position: "relative",
        width: size,
        height: size,
        flexShrink: 0,
        "& .tile-check": { position: "absolute", inset: 0, opacity: 0, m: 0, p: 0 },
        "& .tile-icon": { transition: "opacity 100ms" },
        "&.tile-selecting .tile-check, .entity-card:hover & .tile-check, tr:hover & .tile-check": {
          opacity: 1,
        },
        "&.tile-selecting .tile-icon, .entity-card:hover & .tile-icon, tr:hover & .tile-icon": {
          opacity: 0,
        },
      }}
    >
      <Box className="tile-icon">{tile}</Box>
      <Checkbox
        className="tile-check"
        checked={checked}
        size="small"
        aria-label="Select"
        onClick={(e) => e.stopPropagation()}
        onDoubleClick={(e) => e.stopPropagation()}
        onChange={onToggle}
        sx={{
          width: size,
          height: size,
          borderRadius: size >= 40 ? 2 : 1.5,
          bgcolor: "surface.highest",
          "& .MuiSvgIcon-root": { fontSize: Math.round(size * 0.5) },
        }}
      />
    </Box>
  );
}

export function GroupTile({ g, size }: { g: GroupNode; size?: number }) {
  return (
    <IconTile size={size} sx={{ position: "relative" }}>
      <FolderRoundedIcon />
      {g.hasConfig && (
        <Tooltip title="Hosts inherit credentials from this group">
          <KeyRoundedIcon
            sx={{
              position: "absolute",
              right: -3,
              bottom: -3,
              fontSize: "12px !important",
              color: "primary.main",
              bgcolor: "surface.high",
              borderRadius: "50%",
              p: "2px",
            }}
          />
        </Tooltip>
      )}
    </IconTile>
  );
}

export function HostGrid(p: HostCollectionProps) {
  const selecting = p.checked.size > 0;
  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 3 }}>
      {p.groups.length > 0 && (
        <Box>
          <SectionTitle>Groups</SectionTitle>
          <CardGrid min={240}>
            {p.groups.map((g) => (
              <EntityCard
                key={g.id}
                dense
                tile={<GroupTile g={g} />}
                title={g.label}
                subtitle={groupSubtitle(g)}
                onClick={() => p.onOpenGroup(g.id)}
                onContextMenu={(e) => p.onGroupContext(g, e)}
                actions={
                  <IconButton
                    aria-label="Group options"
                    onClick={(e) => {
                      e.stopPropagation();
                      p.onGroupContext(g, e);
                    }}
                  >
                    <MoreHorizRoundedIcon fontSize="small" />
                  </IconButton>
                }
              />
            ))}
          </CardGrid>
        </Box>
      )}
      {p.hosts.length > 0 && (
        <Box>
          {p.groups.length > 0 && <SectionTitle>Hosts</SectionTitle>}
          <CardGrid min={280}>
            {p.hosts.map((h) => {
              const isChecked = p.checked.has(h.id);
              return (
                <EntityCard
                  key={h.id}
                  className="entity-card"
                  tile={
                    <SelectableTile
                      tile={<HostAvatar host={h} />}
                      checked={isChecked}
                      selecting={selecting}
                      onToggle={() => p.onToggleHost(h)}
                      size={40}
                    />
                  }
                  title={h.label}
                  subtitle={
                    p.showPath && h.groupPath.length > 0
                      ? `${hostSubtitle(h)} · ${h.groupPath.join(" / ")}`
                      : hostSubtitle(h)
                  }
                  selected={isChecked || h.id === p.selectedId}
                  onClick={(e) => p.onOpenHost(h, e)}
                  onDoubleClick={() => p.onConnectHost(h)}
                  onContextMenu={(e) => p.onHostContext(h, e)}
                  meta={
                    h.tags.length > 0 ? (
                      <>
                        {h.tags.slice(0, 3).map((t) => (
                          <Chip key={t} size="small" label={t} />
                        ))}
                        {h.tags.length > 3 && <Chip size="small" label={`+${h.tags.length - 3}`} />}
                      </>
                    ) : undefined
                  }
                  actions={
                    <>
                      <Tooltip title="Connect">
                        <IconButton
                          aria-label={`Connect to ${h.label}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            p.onConnectHost(h);
                          }}
                        >
                          <PlayArrowRoundedIcon fontSize="small" />
                        </IconButton>
                      </Tooltip>
                      <IconButton
                        aria-label="Host options"
                        onClick={(e) => {
                          e.stopPropagation();
                          p.onHostContext(h, e);
                        }}
                      >
                        <MoreHorizRoundedIcon fontSize="small" />
                      </IconButton>
                    </>
                  }
                />
              );
            })}
          </CardGrid>
        </Box>
      )}
    </Box>
  );
}
