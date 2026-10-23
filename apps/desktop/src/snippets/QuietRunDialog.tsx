import { useEffect, useRef, useState } from "react";
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Collapse,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tooltip,
  Typography,
} from "@mui/material";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import { useSnackbar } from "@/components/Snackbar";
import * as ipc from "@/ipc/commands";
import {
  errorMessage,
  type GroupNode,
  type HostCard,
  type QuietHostResult,
  type SnippetCard,
  type Uuid,
} from "@/ipc/types";
import { copyToClipboard } from "@/lib/clipboard";
import { tr, trn } from "@/i18n";
import { TargetsDialog } from "./TargetsDialog";
import { VariablesDialog } from "./SnippetsPage";

type Step =
  | { kind: "pick" }
  | { kind: "vars"; hostIds: Uuid[] }
  | { kind: "run"; hostIds: Uuid[]; vars: Record<string, string> };

/** Run a snippet on several hosts without opening terminals: one `exec`
 *  channel per host, stdout / stderr / exit code collected into a table. */
export function QuietRunDialog({
  snippet,
  hosts,
  groups,
  onClose,
}: {
  snippet: SnippetCard;
  hosts: readonly HostCard[];
  groups: readonly GroupNode[];
  onClose: () => void;
}) {
  const [step, setStep] = useState<Step>({ kind: "pick" });
  const next = (hostIds: Uuid[]) => {
    if (snippet.variables.length > 0) setStep({ kind: "vars", hostIds });
    else setStep({ kind: "run", hostIds, vars: {} });
  };
  if (step.kind === "pick") {
    return (
      <TargetsDialog
        hosts={hosts}
        groups={groups}
        initial={snippet.targetHostIds}
        busy={false}
        onCancel={onClose}
        onConfirm={next}
      />
    );
  }
  if (step.kind === "vars") {
    return (
      <VariablesDialog
        snippet={snippet}
        description={trn(
          step.hostIds.length,
          "Runs quietly on {count} host.",
          "Runs quietly on {count} hosts.",
        )}
        onCancel={onClose}
        onConfirm={(vars) => setStep({ kind: "run", hostIds: step.hostIds, vars })}
      />
    );
  }
  return (
    <QuietResults
      snippet={snippet}
      hosts={hosts}
      hostIds={step.hostIds}
      vars={step.vars}
      onClose={onClose}
    />
  );
}

function QuietResults({
  snippet,
  hosts,
  hostIds,
  vars,
  onClose,
}: {
  snippet: SnippetCard;
  hosts: readonly HostCard[];
  hostIds: Uuid[];
  vars: Record<string, string>;
  onClose: () => void;
}) {
  const snackbar = useSnackbar();
  const [results, setResults] = useState<Map<Uuid, QuietHostResult>>(new Map());
  const [running, setRunning] = useState(true);
  const [failure, setFailure] = useState<string | null>(null);
  const [open, setOpen] = useState<Set<Uuid>>(new Set());
  const started = useRef(false);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void ipc
      .onSnippetQuietProgress((p) => {
        if (cancelled) return;
        setResults((m) => new Map(m).set(p.result.hostId, p.result));
      })
      .then((u) => {
        unlisten = u;
      });
    ipc
      .snippetRunQuiet(snippet.id, hostIds, vars)
      .then((run) => {
        if (cancelled) return;
        setResults(new Map(run.results.map((r: QuietHostResult) => [r.hostId, r])));
      })
      .catch((e: unknown) => {
        if (!cancelled) setFailure(errorMessage(e));
      })
      .finally(() => {
        if (!cancelled) setRunning(false);
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [snippet.id, hostIds, vars]);

  const label = (id: Uuid) => {
    const l = results.get(id)?.label;
    return l !== undefined && l !== "" ? l : (hosts.find((h) => h.id === id)?.label ?? id);
  };
  const done = results.size;
  const ok = [...results.values()].filter((r) => r.error === null && r.exitCode === 0).length;
  const failed = done - ok;

  const summaryText = () =>
    hostIds
      .map((id) => {
        const r = results.get(id);
        if (!r) return `## ${label(id)}\n(pending)`;
        const head = r.error
          ? `## ${label(id)} — ${tr("error")}: ${r.error}`
          : `## ${label(id)} (${r.target}) — exit ${r.exitCode ?? "?"}, ${r.ms} ms`;
        const body = [
          r.stdout.trimEnd(),
          r.stderr.trimEnd() ? `[stderr]\n${r.stderr.trimEnd()}` : "",
        ]
          .filter(Boolean)
          .join("\n");
        return body ? `${head}\n${body}` : head;
      })
      .join("\n\n");

  const copyAll = () =>
    copyToClipboard(`$ ${snippet.script}\n\n${summaryText()}`).then(
      () => snackbar.notify(tr("Summary copied")),
      (e: unknown) => snackbar.error(errorMessage(e)),
    );

  const toggle = (id: Uuid) =>
    setOpen((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  return (
    <Dialog open onClose={running ? undefined : onClose} maxWidth="md" fullWidth>
      <DialogTitle sx={{ display: "flex", alignItems: "center", gap: 1 }}>
        <Box sx={{ flex: 1, minWidth: 0 }}>
          {tr("Quiet run: {name}", { name: snippet.label })}
          <Typography variant="caption" color="text.secondary" sx={{ display: "block" }} noWrap>
            {running
              ? trn(
                  hostIds.length,
                  "{done} of {count} host finished",
                  "{done} of {count} hosts finished",
                  {
                    done,
                  },
                )
              : trn(
                  hostIds.length,
                  "{count} host · {ok} ok · {failed} failed",
                  "{count} hosts · {ok} ok · {failed} failed",
                  {
                    ok,
                    failed,
                  },
                )}
          </Typography>
        </Box>
        {running && <CircularProgress size={18} />}
        <Tooltip title={tr("Copy summary")}>
          <IconButton size="small" onClick={() => void copyAll()} aria-label={tr("Copy summary")}>
            <ContentCopyRoundedIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </DialogTitle>
      <DialogContent sx={{ p: 0 }}>
        {failure && (
          <Typography color="error" variant="body2" sx={{ px: 3, py: 1 }}>
            {failure}
          </Typography>
        )}
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell sx={{ width: 32 }} />
              <TableCell>{tr("Host")}</TableCell>
              <TableCell>{tr("Result")}</TableCell>
              <TableCell align="right" sx={{ width: 90 }}>
                {tr("Time")}
              </TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {hostIds.map((id) => {
              const r = results.get(id);
              const expanded = open.has(id);
              const hasOutput = !!r && (r.stdout.length > 0 || r.stderr.length > 0);
              return (
                <HostRows
                  key={id}
                  label={label(id)}
                  result={r}
                  expanded={expanded}
                  hasOutput={hasOutput}
                  onToggle={() => toggle(id)}
                />
              );
            })}
          </TableBody>
        </Table>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose} disabled={running}>
          {tr("Close")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function HostRows({
  label,
  result: r,
  expanded,
  hasOutput,
  onToggle,
}: {
  label: string;
  result: QuietHostResult | undefined;
  expanded: boolean;
  hasOutput: boolean;
  onToggle: () => void;
}) {
  const status = !r ? (
    <Chip size="small" label={tr("Pending")} />
  ) : r.error ? (
    <Tooltip title={r.error}>
      <Chip size="small" color="error" label={tr("Connection failed")} />
    </Tooltip>
  ) : r.exitCode === 0 ? (
    <Chip size="small" color="success" label={tr("OK")} />
  ) : (
    <Chip size="small" color="warning" label={tr("Exit {code}", { code: r.exitCode ?? "?" })} />
  );
  return (
    <>
      <TableRow
        hover
        onClick={hasOutput ? onToggle : undefined}
        sx={{ cursor: hasOutput ? "pointer" : "default" }}
      >
        <TableCell padding="none" sx={{ pl: 1 }}>
          {hasOutput && (
            <ExpandMoreRoundedIcon
              fontSize="small"
              sx={{
                transform: expanded ? "rotate(180deg)" : "none",
                transition: "transform .15s",
                opacity: 0.7,
              }}
            />
          )}
        </TableCell>
        <TableCell>
          <Typography variant="body2" noWrap>
            {label}
          </Typography>
          {r?.target && (
            <Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block" }}>
              {r.target}
            </Typography>
          )}
        </TableCell>
        <TableCell>
          <Box sx={{ display: "flex", alignItems: "center", gap: 1, minWidth: 0 }}>
            {status}
            {r && !expanded && (
              <Typography variant="caption" color="text.secondary" noWrap sx={{ minWidth: 0 }}>
                {(r.stdout || r.stderr).split("\n")[0]}
              </Typography>
            )}
          </Box>
        </TableCell>
        <TableCell align="right">
          <Typography variant="caption" color="text.secondary">
            {r ? `${r.ms} ms` : ""}
          </Typography>
        </TableCell>
      </TableRow>
      {hasOutput && (
        <TableRow>
          <TableCell colSpan={4} sx={{ p: 0, borderBottom: expanded ? undefined : 0 }}>
            <Collapse in={expanded} unmountOnExit>
              <Box sx={{ px: 2, py: 1, bgcolor: "surface.high", maxHeight: 320, overflow: "auto" }}>
                {r?.stdout && (
                  <Typography
                    component="pre"
                    sx={{ m: 0, whiteSpace: "pre-wrap", fontSize: 12, fontFamily: "monospace" }}
                  >
                    {r.stdout}
                  </Typography>
                )}
                {r?.stderr && (
                  <Typography
                    component="pre"
                    sx={{
                      m: 0,
                      mt: r.stdout ? 1 : 0,
                      whiteSpace: "pre-wrap",
                      fontSize: 12,
                      color: "error.main",
                    }}
                  >
                    {r.stderr}
                  </Typography>
                )}
              </Box>
            </Collapse>
          </TableCell>
        </TableRow>
      )}
    </>
  );
}
