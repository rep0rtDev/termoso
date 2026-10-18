import { useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  IconButton,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import AddRoundedIcon from "@mui/icons-material/AddRounded";
import DeleteOutlineRoundedIcon from "@mui/icons-material/DeleteOutlineRounded";
import DownloadRoundedIcon from "@mui/icons-material/DownloadRounded";
import HistoryRoundedIcon from "@mui/icons-material/HistoryRounded";
import SendRoundedIcon from "@mui/icons-material/SendRounded";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { errorMessage } from "@/api/client";
import { teamsApi } from "@/api/endpoints";
import { queryKeys } from "@/api/hooks";
import type { AuditEvent, Team, Webhook, WebhookKind } from "@/api/types";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Loading } from "@/components/Loading";
import { Section } from "@/components/Section";
import { useSnackbar } from "@/components/Snackbar";
import { UserCell } from "@/components/UserCell";
import { formatDateTime, formatRelative } from "@/components/format";
import { can } from "./perm";

export function AuditTab({ team }: { team: Team }) {
  return (
    <>
      {can(team, "audit.view") && <AuditSection team={team} />}
      {can(team, "webhooks.manage") && <WebhooksSection team={team} />}
    </>
  );
}

const KINDS: { value: WebhookKind; label: string; hint: string }[] = [
  {
    value: "generic",
    label: "Generic JSON (HMAC)",
    hint: "POST batches signed with X-Termoso-Signature",
  },
  { value: "splunk_hec", label: "Splunk HEC", hint: "secret = HEC token" },
  { value: "elastic", label: "Elasticsearch / OpenSearch", hint: "bulk NDJSON; secret = API key" },
  { value: "datadog", label: "Datadog Logs", hint: "secret = API key" },
  {
    value: "syslog_tls",
    label: "Syslog over TLS (CEF)",
    hint: "syslog+tls://host:6514; no secret",
  },
];

function AuditSection({ team }: { team: Team }) {
  const [action, setAction] = useState("");
  const [applied, setApplied] = useState("");
  const snack = useSnackbar();
  const q = useInfiniteQuery({
    queryKey: queryKeys.teamAudit(team.id, applied),
    queryFn: ({ pageParam }) =>
      teamsApi.audit(team.id, { before: pageParam, limit: 100, action: applied || undefined }),
    initialPageParam: undefined as number | undefined,
    getNextPageParam: (last) => last.next_before ?? undefined,
  });
  const [exporting, setExporting] = useState<"csv" | "jsonl" | null>(null);
  const exportAs = async (format: "csv" | "jsonl") => {
    setExporting(format);
    try {
      const blob = await teamsApi.auditExport(team.id, { format, action: applied || undefined });
      if (!blob) return;
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `termoso-audit-${team.name.replace(/\W+/g, "-")}.${format}`;
      a.click();
      URL.revokeObjectURL(a.href);
    } catch (e) {
      snack.error(errorMessage(e));
    } finally {
      setExporting(null);
    }
  };
  const events = q.data?.pages.flatMap((p) => p.events) ?? [];
  return (
    <Section
      title="Audit log"
      description="Who did what in this team. Filter by action (exact, or a family like “vault.”); export up to a million rows as CSV or JSON Lines."
      actions={
        can(team, "audit.export") && (
          <Stack direction="row" spacing={1}>
            <Button
              startIcon={<DownloadRoundedIcon />}
              disabled={exporting !== null}
              onClick={() => exportAs("csv")}
            >
              CSV
            </Button>
            <Button
              startIcon={<DownloadRoundedIcon />}
              disabled={exporting !== null}
              onClick={() => exportAs("jsonl")}
            >
              JSONL
            </Button>
          </Stack>
        )
      }
      disablePadding
    >
      <Box
        component="form"
        onSubmit={(e) => {
          e.preventDefault();
          setApplied(action.trim());
        }}
        sx={{ p: 2, display: "flex", gap: 1 }}
      >
        <TextField
          size="small"
          label="Action"
          placeholder="vault. / member.removed"
          value={action}
          onChange={(e) => setAction(e.target.value)}
          sx={{ width: 280 }}
        />
        <Button type="submit" variant="outlined">
          Filter
        </Button>
      </Box>
      {q.isPending ? (
        <Loading minHeight={120} />
      ) : q.isError ? (
        <Box sx={{ p: 3 }}>
          <Alert severity="error">{errorMessage(q.error)}</Alert>
        </Box>
      ) : events.length === 0 ? (
        <EmptyState title="No events" />
      ) : (
        <>
          <TableContainer>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>When</TableCell>
                  <TableCell>Actor</TableCell>
                  <TableCell>Action</TableCell>
                  <TableCell>Details</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {events.map((e) => (
                  <EventRow key={e.id} e={e} />
                ))}
              </TableBody>
            </Table>
          </TableContainer>
          {q.hasNextPage && (
            <Box sx={{ p: 2, textAlign: "center" }}>
              <Button disabled={q.isFetchingNextPage} onClick={() => q.fetchNextPage()}>
                Load older
              </Button>
            </Box>
          )}
        </>
      )}
    </Section>
  );
}

function EventRow({ e }: { e: AuditEvent }) {
  const details =
    e.details && typeof e.details === "object" && Object.keys(e.details).length > 0
      ? JSON.stringify(e.details)
      : "";
  return (
    <TableRow hover>
      <TableCell sx={{ whiteSpace: "nowrap" }}>
        <Tooltip title={formatDateTime(e.created_at)}>
          <span>{formatRelative(e.created_at)}</span>
        </Tooltip>
      </TableCell>
      <TableCell>
        {e.actor_id ? (
          <UserCell
            userId={e.actor_id}
            email={e.actor_email ?? ""}
            displayName={e.actor_name}
            avatar={e.actor_avatar}
          />
        ) : (
          <Typography variant="body2" color="text.secondary">
            system
          </Typography>
        )}
      </TableCell>
      <TableCell>
        <Chip size="small" label={e.action} sx={{ fontFamily: "monospace" }} />
        {e.target_email && (
          <Typography variant="caption" color="text.secondary" sx={{ ml: 1 }}>
            → {e.target_email}
          </Typography>
        )}
      </TableCell>
      <TableCell sx={{ maxWidth: 420 }}>
        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ fontFamily: "monospace", wordBreak: "break-all" }}
        >
          {details}
        </Typography>
      </TableCell>
    </TableRow>
  );
}

function WebhooksSection({ team }: { team: Team }) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const q = useQuery({
    queryKey: queryKeys.teamWebhooks(team.id),
    queryFn: () => teamsApi.webhooks(team.id),
  });
  const [createOpen, setCreateOpen] = useState(false);
  const [deleting, setDeleting] = useState<Webhook | null>(null);
  const [history, setHistory] = useState<Webhook | null>(null);
  const invalidate = () => qc.invalidateQueries({ queryKey: queryKeys.teamWebhooks(team.id) });
  const toggle = useMutation({
    mutationFn: (w: Webhook) => teamsApi.updateWebhook(team.id, w.id, { enabled: !w.enabled }),
    onSuccess: invalidate,
    onError: (e) => snack.error(errorMessage(e)),
  });
  const test = useMutation({
    mutationFn: (w: Webhook) => teamsApi.testWebhook(team.id, w.id),
    onSuccess: async (r, w) => {
      await invalidate();
      const ok = r.deliveries.filter((d) => d.status === "ok").length;
      snack.notify(`${ok}/${r.deliveries.length} deliveries succeeded`);
      setHistory(w);
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const remove = useMutation({
    mutationFn: (w: Webhook) => teamsApi.deleteWebhook(team.id, w.id),
    onSuccess: async () => {
      setDeleting(null);
      await invalidate();
      snack.notify("Webhook deleted");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  if (q.isPending) return <Loading minHeight={120} />;
  if (q.isError) return <Alert severity="error">{errorMessage(q.error)}</Alert>;
  const hooks = q.data.webhooks;
  return (
    <Section
      title="Webhooks / SIEM"
      description="Every audit event is queued per webhook and delivered in batches with retries (30 s → 6 h). A hook is switched off after 50 consecutive failures and everyone who can manage webhooks is e-mailed. Up to 10 per team."
      actions={
        <Button
          startIcon={<AddRoundedIcon />}
          disabled={hooks.length >= 10}
          onClick={() => setCreateOpen(true)}
        >
          Add webhook
        </Button>
      }
      disablePadding={hooks.length > 0}
    >
      {hooks.length === 0 ? (
        <EmptyState title="No webhooks" description="Stream the audit log to your SIEM." />
      ) : (
        <TableContainer>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>Destination</TableCell>
                <TableCell>Kind</TableCell>
                <TableCell>Last delivery</TableCell>
                <TableCell>On</TableCell>
                <TableCell align="right" />
              </TableRow>
            </TableHead>
            <TableBody>
              {hooks.map((w) => (
                <TableRow key={w.id} hover>
                  <TableCell sx={{ maxWidth: 360 }}>
                    <Typography variant="body2" noWrap title={w.url}>
                      {w.url}
                    </Typography>
                    <Typography variant="caption" color="text.secondary">
                      {w.actions?.length ? w.actions.join(", ") : "all events"}
                    </Typography>
                  </TableCell>
                  <TableCell>{KINDS.find((k) => k.value === w.kind)?.label ?? w.kind}</TableCell>
                  <TableCell>
                    {w.last_delivered_at ? (
                      <Tooltip title={formatDateTime(w.last_delivered_at)}>
                        <span>{formatRelative(w.last_delivered_at)}</span>
                      </Tooltip>
                    ) : (
                      <Typography variant="caption" color="text.secondary">
                        never
                      </Typography>
                    )}
                    {w.failures > 0 && (
                      <Tooltip title={w.last_error ?? ""}>
                        <Chip
                          size="small"
                          color={w.enabled ? "warning" : "error"}
                          label={`${w.failures} failure${w.failures === 1 ? "" : "s"}`}
                          sx={{ ml: 1 }}
                        />
                      </Tooltip>
                    )}
                  </TableCell>
                  <TableCell>
                    <Switch
                      size="small"
                      checked={w.enabled}
                      disabled={toggle.isPending}
                      onChange={() => toggle.mutate(w)}
                      slotProps={{ input: { "aria-label": "Enabled" } }}
                    />
                  </TableCell>
                  <TableCell align="right" sx={{ whiteSpace: "nowrap" }}>
                    <Tooltip title="Send a test event and flush the queue">
                      <IconButton
                        size="small"
                        disabled={test.isPending}
                        onClick={() => test.mutate(w)}
                        aria-label="Test"
                      >
                        <SendRoundedIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                    <Tooltip title="Delivery history">
                      <IconButton size="small" onClick={() => setHistory(w)} aria-label="History">
                        <HistoryRoundedIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                    <Tooltip title="Delete">
                      <IconButton size="small" onClick={() => setDeleting(w)} aria-label="Delete">
                        <DeleteOutlineRoundedIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      )}
      <CreateWebhookDialog open={createOpen} team={team} onClose={() => setCreateOpen(false)} />
      <DeliveriesDialog team={team} hook={history} onClose={() => setHistory(null)} />
      <ConfirmDialog
        open={deleting !== null}
        title="Delete webhook?"
        confirmLabel="Delete"
        danger
        busy={remove.isPending}
        onCancel={() => setDeleting(null)}
        onConfirm={() => deleting && remove.mutate(deleting)}
      >
        Queued deliveries for it are dropped.
      </ConfirmDialog>
    </Section>
  );
}

function CreateWebhookDialog({
  open,
  team,
  onClose,
}: {
  open: boolean;
  team: Team;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const snack = useSnackbar();
  const [kind, setKind] = useState<WebhookKind>("generic");
  const [url, setUrl] = useState("");
  const [secret, setSecret] = useState("");
  const [actions, setActions] = useState("");
  const create = useMutation({
    mutationFn: () => {
      const list = actions
        .split(/[,\s]+/)
        .map((a) => a.trim())
        .filter(Boolean);
      return teamsApi.createWebhook(team.id, {
        kind,
        url: url.trim(),
        secret,
        actions: list.length ? list : null,
      });
    },
    onSuccess: async () => {
      onClose();
      setUrl("");
      setSecret("");
      setActions("");
      await qc.invalidateQueries({ queryKey: queryKeys.teamWebhooks(team.id) });
      snack.notify("Webhook added");
    },
    onError: (e) => snack.error(errorMessage(e)),
  });
  const k = KINDS.find((x) => x.value === kind);
  const syslog = kind === "syslog_tls";
  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      <DialogTitle>Add webhook</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 1 }}>
          <FormControl size="small" fullWidth>
            <InputLabel id="kind-label">Kind</InputLabel>
            <Select
              labelId="kind-label"
              label="Kind"
              value={kind}
              onChange={(e) => setKind(e.target.value)}
            >
              {KINDS.map((x) => (
                <MenuItem key={x.value} value={x.value}>
                  {x.label}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
          <Typography variant="caption" color="text.secondary">
            {k?.hint}
          </Typography>
          <TextField
            label={syslog ? "Destination (syslog+tls://host:port)" : "URL (https://…)"}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            fullWidth
            autoFocus
          />
          {!syslog && (
            <TextField
              label={kind === "generic" ? "Signing secret (8–512 chars)" : "Token / API key"}
              type="password"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              fullWidth
            />
          )}
          <TextField
            label="Only these actions (optional)"
            placeholder="member., vault.deleted, policy.updated"
            helperText="Comma-separated; a trailing dot matches a family. Empty = everything."
            value={actions}
            onChange={(e) => setActions(e.target.value)}
            fullWidth
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Cancel</Button>
        <Button
          variant="contained"
          disabled={!url.trim() || (!syslog && secret.length < 8) || create.isPending}
          onClick={() => create.mutate()}
        >
          Add
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function DeliveriesDialog({
  team,
  hook,
  onClose,
}: {
  team: Team;
  hook: Webhook | null;
  onClose: () => void;
}) {
  const q = useQuery({
    queryKey: queryKeys.teamWebhookDeliveries(team.id, hook?.id ?? ""),
    queryFn: () => teamsApi.webhookDeliveries(team.id, hook?.id ?? ""),
    enabled: hook !== null,
  });
  return (
    <Dialog open={hook !== null} onClose={onClose} maxWidth="md" fullWidth>
      <DialogTitle>Deliveries · {hook?.url}</DialogTitle>
      <DialogContent sx={{ p: 0 }}>
        {q.isPending ? (
          <Loading minHeight={120} />
        ) : q.isError ? (
          <Box sx={{ p: 3 }}>
            <Alert severity="error">{errorMessage(q.error)}</Alert>
          </Box>
        ) : q.data.deliveries.length === 0 ? (
          <EmptyState title="Nothing delivered yet" />
        ) : (
          <TableContainer>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>Event</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell>Attempt</TableCell>
                  <TableCell>Response</TableCell>
                  <TableCell>When</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {q.data.deliveries.map((d) => (
                  <TableRow key={d.id}>
                    <TableCell sx={{ fontFamily: "monospace" }}>{d.action}</TableCell>
                    <TableCell>
                      <Chip
                        size="small"
                        label={d.status}
                        color={
                          d.status === "ok" ? "success" : d.status === "dead" ? "error" : "default"
                        }
                      />
                    </TableCell>
                    <TableCell>{d.attempt}</TableCell>
                    <TableCell sx={{ maxWidth: 320 }}>
                      <Typography variant="caption" sx={{ fontFamily: "monospace" }} noWrap>
                        {d.response_status ?? "—"} {d.response_body ?? ""}
                      </Typography>
                    </TableCell>
                    <TableCell sx={{ whiteSpace: "nowrap" }}>
                      {formatRelative(d.delivered_at ?? d.next_at)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        )}
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Close</Button>
      </DialogActions>
    </Dialog>
  );
}
