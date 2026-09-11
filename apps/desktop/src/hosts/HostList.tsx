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
import MoreHorizRoundedIcon from "@mui/icons-material/MoreHorizRounded";
import { monoFontFamily, sizes } from "@/theme/theme";
import { HostAvatar } from "./HostAvatar";
import { GroupTile, groupSubtitle, SelectableTile, type HostCollectionProps } from "./HostGrid";

const rowSx = {
  "& td": { py: 0.5 },
  "& .row-actions": { opacity: 0 },
  "&:hover .row-actions, &.Mui-selected .row-actions": { opacity: 1 },
} as const;

/** `2h ago`, `3d ago`, or a short date for anything older. */
export function relativeTime(iso: string | null, now = Date.now()) {
  if (!iso) return "—";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "—";
  const s = Math.max(0, Math.round((now - t) / 1000));
  if (s < 60) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  const d = Math.round(h / 24);
  if (d < 14) return `${d} d ago`;
  return new Date(t).toLocaleDateString();
}

export function HostList(p: HostCollectionProps) {
  const selecting = p.checked.size > 0;
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
          <TableCell sx={{ width: 130, whiteSpace: "nowrap" }}>Last connected</TableCell>
          <TableCell sx={{ width: 40 }} />
        </TableRow>
      </TableHead>
      <TableBody>
        {p.groups.map((g) => (
          <TableRow
            key={g.id}
            hover
            sx={rowSx}
            onClick={() => p.onOpenGroup(g.id)}
            onContextMenu={(e) => p.onGroupContext(g, e)}
          >
            <TableCell>
              <GroupTile g={g} size={sizes.tileSmall} />
            </TableCell>
            <TableCell>
              <Typography variant="body1" sx={{ fontWeight: 500 }}>
                {g.label}
              </Typography>
            </TableCell>
            <TableCell colSpan={5}>
              <Typography variant="body2" color="text.secondary">
                {groupSubtitle(g)}
              </Typography>
            </TableCell>
            <TableCell align="right">
              <IconButton
                className="row-actions"
                aria-label="Group options"
                onClick={(e) => {
                  e.stopPropagation();
                  p.onGroupContext(g, e);
                }}
              >
                <MoreHorizRoundedIcon fontSize="small" />
              </IconButton>
            </TableCell>
          </TableRow>
        ))}
        {p.hosts.map((h) => {
          const isChecked = p.checked.has(h.id);
          return (
            <TableRow
              key={h.id}
              hover
              selected={isChecked || h.id === p.selectedId}
              sx={rowSx}
              onClick={(e) => p.onOpenHost(h, e)}
              onDoubleClick={() => p.onConnectHost(h)}
              onContextMenu={(e) => p.onHostContext(h, e)}
            >
              <TableCell>
                <SelectableTile
                  tile={<HostAvatar host={h} size={sizes.tileSmall} />}
                  checked={isChecked}
                  selecting={selecting}
                  onToggle={() => p.onToggleHost(h)}
                  size={sizes.tileSmall}
                />
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
              <TableCell>
                <Typography variant="body2" color="text.secondary" noWrap>
                  {relativeTime(h.lastConnected)}
                </Typography>
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
          );
        })}
      </TableBody>
    </Table>
  );
}
