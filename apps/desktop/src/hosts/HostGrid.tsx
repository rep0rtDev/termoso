import { Box, Chip, IconButton, Tooltip } from "@mui/material";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import PlayArrowRoundedIcon from "@mui/icons-material/PlayArrowRounded";
import type { MouseEvent } from "react";
import type { GroupNode, HostCard } from "@/ipc/types";
import { CardGrid, EntityCard, IconTile, SectionTitle } from "@/components/ui";
import { HostAvatar } from "./HostAvatar";

export interface HostCollectionProps {
  groups: GroupNode[];
  hosts: HostCard[];
  selectedId: string | null;
  showPath: boolean;
  onOpenGroup: (id: string) => void;
  onEditGroup: (g: GroupNode) => void;
  onOpenHost: (h: HostCard) => void;
  onConnectHost: (h: HostCard) => void;
  onHostContext: (h: HostCard, e: MouseEvent<HTMLElement>) => void;
}

export function hostSubtitle(h: HostCard) {
  const port = h.protocol === "telnet" || h.port !== 22 ? `:${h.port}` : "";
  return `${h.username ? `${h.username}@` : ""}${h.address}${port}`;
}

export function HostGrid(p: HostCollectionProps) {
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
                tile={
                  <IconTile>
                    <FolderRoundedIcon />
                  </IconTile>
                }
                title={g.label}
                subtitle={`${g.hostCount} host${g.hostCount === 1 ? "" : "s"}`}
                onClick={() => p.onOpenGroup(g.id)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  p.onEditGroup(g);
                }}
                actions={
                  <IconButton
                    aria-label="Group options"
                    onClick={(e) => {
                      e.stopPropagation();
                      p.onEditGroup(g);
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
            {p.hosts.map((h) => (
              <EntityCard
                key={h.id}
                tile={<HostAvatar host={h} />}
                title={h.label}
                subtitle={
                  p.showPath && h.groupPath.length > 0
                    ? `${hostSubtitle(h)} · ${h.groupPath.join(" / ")}`
                    : hostSubtitle(h)
                }
                selected={h.id === p.selectedId}
                onClick={() => p.onOpenHost(h)}
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
            ))}
          </CardGrid>
        </Box>
      )}
    </Box>
  );
}
