import { Box, Card, CardActionArea, Chip, IconButton, Typography } from "@mui/material";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import type { GroupNode, HostCard } from "@/ipc/types";
import { HostAvatar } from "./HostAvatar";

export interface HostCollectionProps {
  groups: GroupNode[];
  hosts: HostCard[];
  selectedId: string | null;
  showPath: boolean;
  onOpenGroup: (id: string) => void;
  onEditGroup: (g: GroupNode) => void;
  onOpenHost: (h: HostCard) => void;
}

const cardSx = {
  height: "100%",
  bgcolor: "background.paper",
  border: 1,
  borderColor: "divider",
  transition: "border-color 120ms, transform 120ms",
  "&:hover": { borderColor: "primary.main" },
} as const;

export function HostGrid(p: HostCollectionProps) {
  return (
    <Box
      sx={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
        gap: 1.5,
        pt: 1.5,
      }}
    >
      {p.groups.map((g) => (
        <Card key={g.id} variant="outlined" sx={cardSx}>
          <CardActionArea
            onClick={() => p.onOpenGroup(g.id)}
            onContextMenu={(e) => {
              e.preventDefault();
              p.onEditGroup(g);
            }}
            sx={{ p: 1.75, display: "flex", alignItems: "center", gap: 1.25, height: "100%" }}
          >
            <FolderRoundedIcon sx={{ color: "secondary.main", fontSize: 32 }} />
            <Box sx={{ minWidth: 0, flex: 1 }}>
              <Typography variant="subtitle2" noWrap>
                {g.label}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                {g.hostCount} host{g.hostCount === 1 ? "" : "s"}
              </Typography>
            </Box>
            <IconButton
              size="small"
              component="span"
              aria-label="Group options"
              onClick={(e) => {
                e.stopPropagation();
                p.onEditGroup(g);
              }}
            >
              <MoreHorizRoundedIcon fontSize="small" />
            </IconButton>
          </CardActionArea>
        </Card>
      ))}
      {p.hosts.map((h) => {
        const selected = h.id === p.selectedId;
        return (
          <Card
            key={h.id}
            variant="outlined"
            sx={{
              ...cardSx,
              ...(selected && { borderColor: "primary.main", bgcolor: "action.selected" }),
            }}
          >
            <CardActionArea
              onClick={() => p.onOpenHost(h)}
              sx={{
                p: 1.75,
                height: "100%",
                display: "flex",
                flexDirection: "column",
                alignItems: "stretch",
              }}
            >
              <Box sx={{ display: "flex", alignItems: "center", gap: 1.25 }}>
                <HostAvatar host={h} size={36} />
                <Box sx={{ minWidth: 0, flex: 1 }}>
                  <Typography variant="subtitle2" noWrap>
                    {h.label}
                  </Typography>
                  <Typography
                    variant="caption"
                    color="text.secondary"
                    noWrap
                    sx={{ display: "block" }}
                  >
                    {h.username ? `${h.username}@` : ""}
                    {h.address}
                    {h.protocol === "telnet" || h.port !== 22 ? `:${h.port}` : ""}
                  </Typography>
                </Box>
              </Box>
              {(p.showPath && h.groupPath.length > 0) || h.tags.length > 0 ? (
                <Box sx={{ display: "flex", flexWrap: "wrap", gap: 0.5, mt: 1.25 }}>
                  {p.showPath && h.groupPath.length > 0 && (
                    <Chip
                      size="small"
                      variant="outlined"
                      icon={<FolderRoundedIcon />}
                      label={h.groupPath.join(" / ")}
                      sx={{ height: 20, fontSize: 11, maxWidth: "100%" }}
                    />
                  )}
                  {h.tags.map((t) => (
                    <Chip key={t} size="small" label={t} sx={{ height: 20, fontSize: 11 }} />
                  ))}
                </Box>
              ) : null}
            </CardActionArea>
          </Card>
        );
      })}
    </Box>
  );
}
