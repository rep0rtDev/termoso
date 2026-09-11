import {
  Box,
  Chip,
  CircularProgress,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
import HistoryRoundedIcon from "@mui/icons-material/HistoryRounded";
import { EmptyState } from "@/components/EmptyState";
import { useHistory } from "@/ipc/hooks";
import { errorMessage } from "@/ipc/types";

function duration(secs: number | null): string {
  if (secs === null) return "open";
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m ${secs % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

export function HistoryPage() {
  const history = useHistory();
  return (
    <Box sx={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
      <Box sx={{ px: 2.5, py: 1.75, borderBottom: 1, borderColor: "divider" }}>
        <Typography variant="h5">History</Typography>
        <Typography variant="body2" color="text.secondary">
          Recent connections from this device. Stored encrypted, never uploaded.
        </Typography>
      </Box>
      <Box sx={{ flex: 1, overflowY: "auto", px: 2.5, pb: 3 }}>
        {history.isPending ? (
          <Box sx={{ display: "flex", justifyContent: "center", pt: 8 }}>
            <CircularProgress size={28} />
          </Box>
        ) : history.error ? (
          <EmptyState title="Could not load history" description={errorMessage(history.error)} />
        ) : history.data.length === 0 ? (
          <EmptyState
            icon={<HistoryRoundedIcon />}
            title="No connections yet"
            description="Once you open a terminal, it shows up here."
          />
        ) : (
          <Table size="small" sx={{ mt: 1 }}>
            <TableHead>
              <TableRow sx={{ "& th": { color: "text.secondary", fontWeight: 600 } }}>
                <TableCell>When</TableCell>
                <TableCell>Label</TableCell>
                <TableCell>Target</TableCell>
                <TableCell>Protocol</TableCell>
                <TableCell align="right">Duration</TableCell>
                <TableCell>Result</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {history.data.map((item) => (
                <TableRow key={item.id} hover>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary" noWrap>
                      {new Date(item.created_at).toLocaleString()}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontWeight: 600 }} noWrap>
                      {item.data.label}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontFamily: "monospace" }} noWrap>
                      {item.data.target}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Chip
                      size="small"
                      variant="outlined"
                      label={item.data.protocol.toUpperCase()}
                    />
                  </TableCell>
                  <TableCell align="right">{duration(item.data.duration_secs)}</TableCell>
                  <TableCell>
                    {item.data.error ? (
                      <Typography variant="body2" color="error" noWrap title={item.data.error}>
                        {item.data.error}
                      </Typography>
                    ) : (
                      <Typography variant="body2" color="text.secondary">
                        ok
                      </Typography>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </Box>
    </Box>
  );
}
