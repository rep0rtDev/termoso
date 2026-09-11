import { useState } from "react";
import { Chip, Stack, Tooltip, Typography } from "@mui/material";
import HistoryRoundedIcon from "@mui/icons-material/HistoryRounded";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import ErrorOutlineRoundedIcon from "@mui/icons-material/ErrorOutlineRounded";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageBody, PageHeader } from "@/components/PageHeader";
import { EntityCard, IconTile, Loading, Mono, SearchField } from "@/components/ui";
import { useHistory } from "@/ipc/hooks";
import { errorMessage } from "@/ipc/types";
import { sizes } from "@/theme/theme";

function duration(secs: number | null): string {
  if (secs === null) return "open";
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m ${secs % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

export function HistoryPage() {
  const history = useHistory();
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  const items = (history.data ?? []).filter(
    (i) => !q || i.data.label.toLowerCase().includes(q) || i.data.target.toLowerCase().includes(q),
  );

  return (
    <Page>
      <PageHeader
        actions={
          <Typography variant="body2" color="text.secondary">
            Recent connections from this device · stored encrypted, never uploaded
          </Typography>
        }
        trailing={<SearchField value={query} onChange={setQuery} placeholder="Search history" />}
      />
      <PageBody>
        {history.isPending ? (
          <Loading />
        ) : history.error ? (
          <EmptyState title="Could not load history" description={errorMessage(history.error)} />
        ) : history.data.length === 0 ? (
          <EmptyState
            icon={<HistoryRoundedIcon />}
            title="No connections yet"
            description="Once you open a terminal, it shows up here."
          />
        ) : (
          <Stack spacing={0.75}>
            {items.map((item) => (
              <EntityCard
                key={item.id}
                dense
                tile={
                  <IconTile size={sizes.tileSmall} tone={item.data.error ? "danger" : "neutral"}>
                    {item.data.error ? <ErrorOutlineRoundedIcon /> : <TerminalRoundedIcon />}
                  </IconTile>
                }
                title={item.data.label}
                subtitle={
                  <>
                    <Mono>{item.data.target}</Mono>
                    {" · "}
                    {new Date(item.created_at).toLocaleString()}
                    {" · "}
                    {duration(item.data.duration_secs)}
                  </>
                }
                trailing={
                  <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
                    {item.data.error && (
                      <Tooltip title={item.data.error}>
                        <Chip size="small" color="error" label="Failed" />
                      </Tooltip>
                    )}
                    <Chip
                      size="small"
                      variant="outlined"
                      label={item.data.protocol.toUpperCase()}
                    />
                  </Stack>
                }
              />
            ))}
            {items.length === 0 && (
              <Typography variant="body2" color="text.secondary" sx={{ px: 1, py: 2 }}>
                Nothing matches “{query}”.
              </Typography>
            )}
          </Stack>
        )}
      </PageBody>
    </Page>
  );
}
