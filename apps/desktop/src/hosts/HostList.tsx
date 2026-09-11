import {
  Box,
  Chip,
  IconButton,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import { IconTile } from "@/components/ui";
import { monoFontFamily, sizes } from "@/theme/theme";
import { HostAvatar } from "./HostAvatar";
import type { HostCollectionProps } from "./HostGrid";

const rowSx = {
  "& td": { py: 0.5 },
  "& .row-actions": { opacity: 0 },
  "&:hover .row-actions, &.Mui-selected .row-actions": { opacity: 1 },
} as const;

export function HostList(p: HostCollectionProps) {
  return (
    <Table size="small">
      <TableHead>
        <TableRow>
          <TableCell sx={{ width: 48 }} />
          <TableCell>Name</TableCell>
          <TableCell>Address</TableCell>
          <TableCell>User</TableCell>
          <TableCell align="right">Port</TableCell>
          <TableCell>Tags</TableCell>
          <TableCell sx={{ width: 40 }} />
        </TableRow>
      </TableHead>
      <TableBody>
        {p.groups.map((g) => (
          <TableRow key={g.id} hover sx={rowSx} onClick={() => p.onOpenGroup(g.id)}>
            <TableCell>
              <IconTile size={sizes.tileSmall}>
                <FolderRoundedIcon />
              </IconTile>
            </TableCell>
            <TableCell>
              <Typography variant="body1" sx={{ fontWeight: 500 }}>
                {g.label}
              </Typography>
            </TableCell>
            <TableCell colSpan={4}>
              <Typography variant="body2" color="text.secondary">
                {g.hostCount} host{g.hostCount === 1 ? "" : "s"}
              </Typography>
            </TableCell>
            <TableCell align="right">
              <IconButton
                className="row-actions"
                aria-label="Group options"
                onClick={(e) => {
                  e.stopPropagation();
                  p.onEditGroup(g);
                }}
              >
                <MoreHorizRoundedIcon fontSize="small" />
              </IconButton>
            </TableCell>
          </TableRow>
        ))}
        {p.hosts.map((h) => (
          <TableRow
            key={h.id}
            hover
            selected={h.id === p.selectedId}
            sx={rowSx}
            onClick={() => p.onOpenHost(h)}
            onDoubleClick={() => p.onConnectHost(h)}
            onContextMenu={(e) => p.onHostContext(h, e)}
          >
            <TableCell>
              <HostAvatar host={h} size={sizes.tileSmall} />
            </TableCell>
            <TableCell>
              <Typography variant="body1" sx={{ fontWeight: 500 }} noWrap>
                {h.label}
              </Typography>
              {p.showPath && h.groupPath.length > 0 && (
                <Typography
                  variant="caption"
                  color="text.secondary"
                  noWrap
                  sx={{ display: "block" }}
                >
                  {h.groupPath.join(" / ")}
                </Typography>
              )}
            </TableCell>
            <TableCell>
              <Typography variant="body2" sx={{ fontFamily: monoFontFamily }} noWrap>
                {h.address}
              </Typography>
            </TableCell>
            <TableCell>
              <Typography variant="body2" color="text.secondary" noWrap>
                {h.username || "—"}
              </Typography>
            </TableCell>
            <TableCell align="right">
              <Typography variant="body2" color="text.secondary">
                {h.port}
              </Typography>
            </TableCell>
            <TableCell>
              <Box sx={{ display: "flex", gap: 0.5, flexWrap: "wrap" }}>
                {h.tags.map((t) => (
                  <Chip key={t} size="small" label={t} />
                ))}
              </Box>
            </TableCell>
            <TableCell align="right">
              <IconButton
                className="row-actions"
                aria-label="Host options"
                onClick={(e) => {
                  e.stopPropagation();
                  p.onHostContext(h, e);
                }}
              >
                <MoreHorizRoundedIcon fontSize="small" />
              </IconButton>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
