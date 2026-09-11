import {
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
import { HostAvatar } from "./HostAvatar";
import type { HostCollectionProps } from "./HostGrid";

const rowSx = { cursor: "pointer", "& td": { py: 0.75, borderColor: "divider" } } as const;

export function HostList(p: HostCollectionProps) {
  return (
    <Table size="small" sx={{ mt: 1 }}>
      <TableHead>
        <TableRow
          sx={{ "& th": { color: "text.secondary", fontWeight: 600, borderColor: "divider" } }}
        >
          <TableCell sx={{ width: 44 }} />
          <TableCell>Label</TableCell>
          <TableCell>Address</TableCell>
          <TableCell>User</TableCell>
          <TableCell align="right">Port</TableCell>
          <TableCell>Tags</TableCell>
          <TableCell sx={{ width: 44 }} />
        </TableRow>
      </TableHead>
      <TableBody>
        {p.groups.map((g) => (
          <TableRow key={g.id} hover sx={rowSx} onClick={() => p.onOpenGroup(g.id)}>
            <TableCell>
              <FolderRoundedIcon sx={{ color: "secondary.main", display: "block" }} />
            </TableCell>
            <TableCell>
              <Typography variant="body2" sx={{ fontWeight: 600 }}>
                {g.label}
              </Typography>
            </TableCell>
            <TableCell colSpan={4}>
              <Typography variant="caption" color="text.secondary">
                {g.hostCount} host{g.hostCount === 1 ? "" : "s"}
              </Typography>
            </TableCell>
            <TableCell align="right">
              <IconButton
                size="small"
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
          >
            <TableCell>
              <HostAvatar host={h} size={28} />
            </TableCell>
            <TableCell>
              <Typography variant="body2" sx={{ fontWeight: 600 }} noWrap>
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
              <Typography variant="body2" sx={{ fontFamily: "monospace" }} noWrap>
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
              {h.tags.map((t) => (
                <Chip key={t} size="small" label={t} sx={{ height: 20, fontSize: 11, mr: 0.5 }} />
              ))}
            </TableCell>
            <TableCell align="right">
              <Chip
                size="small"
                variant="outlined"
                label={h.protocol.toUpperCase()}
                sx={{ height: 20, fontSize: 10 }}
              />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
