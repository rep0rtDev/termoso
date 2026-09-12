// Multi-target snippet execution: fan a snippet out to open terminals and
// configured hosts, open sessions for hosts that are not connected yet, and
// track the outcome per target (Termius "Targets for execution").

import * as ipc from "@/ipc/commands";
import { errorMessage, type HostCard, type SnippetCard, type Uuid } from "@/ipc/types";
import { createStore, useStore } from "@/lib/store";
import {
  closePane,
  onCommandEvent,
  openTerminal,
  terminalStore,
  type Pane,
} from "@/terminal/store";

export type TargetState = "pending" | "connecting" | "running" | "done" | "failed";

export interface RunTarget {
  key: string;
  label: string;
  subtitle: string;
  hostId: Uuid | null;
  paneId: Uuid | null;
  /** The run opened this terminal itself (and closes it when the snippet asks). */
  opened: boolean;
  state: TargetState;
  /** Exit code of the last command when shell integration reported one. */
  exit: number | null;
  message: string | null;
}

export interface SnippetRun {
  id: string;
  snippetId: Uuid;
  label: string;
  startedAt: number;
  finishedAt: number | null;
  targets: RunTarget[];
}

interface RunsState {
  /** Newest first. */
  runs: SnippetRun[];
}

const MAX_RUNS = 30;
/** Shell integration usually announces itself within this after connect. */
const INTEGRATION_WAIT_MS = 2500;
/** Wait this long for the first command mark before giving up on exit codes. */
const FIRST_MARK_MS = 5000;
/** Idle time at the prompt after which a script is considered finished. */
const QUIET_MS = 1500;

export const runsStore = createStore<RunsState>({ runs: [] });
export const useRuns = <S>(selector: (s: RunsState) => S) => useStore(runsStore, selector);
export const useLastRun = (snippetId: Uuid | null) =>
  useRuns((s) => (snippetId ? s.runs.find((r) => r.snippetId === snippetId) : undefined));

export interface RunRequest {
  snippet: SnippetCard;
  vars: Record<string, string>;
  /** Open terminals to type into. */
  sessionIds: Uuid[];
  /** Configured hosts; a connected terminal is reused, otherwise one is opened. */
  hostIds: Uuid[];
  /** Host cards for labels. */
  hosts: readonly HostCard[];
  /** Type without the final newline (sessions only). */
  paste?: boolean;
}

export const isSettled = (t: RunTarget) => t.state === "done" || t.state === "failed";

export function summarize(run: SnippetRun): string {
  const ok = run.targets.filter((t) => t.state === "done").length;
  const failed = run.targets.filter((t) => t.state === "failed").length;
  const total = run.targets.length;
  if (run.finishedAt === null) return `Running on ${total} ${total === 1 ? "target" : "targets"}…`;
  if (failed === 0) return `Done on ${ok} of ${total} ${total === 1 ? "target" : "targets"}`;
  return `${ok} succeeded, ${failed} failed`;
}

/** Call `cb` once every target of the run has settled. */
export function watchRun(runId: string, cb: (run: SnippetRun) => void): () => void {
  const check = () => {
    const run = runsStore.get().runs.find((r) => r.id === runId);
    if (!run) {
      off();
      return;
    }
    if (run.finishedAt !== null) {
      off();
      cb(run);
    }
  };
  const off = runsStore.subscribe(check);
  check();
  return off;
}

function patchTarget(runId: string, key: string, patch: Partial<RunTarget>) {
  runsStore.set((s) => ({
    runs: s.runs.map((r) =>
      r.id !== runId
        ? r
        : { ...r, targets: r.targets.map((t) => (t.key === key ? { ...t, ...patch } : t)) },
    ),
  }));
}

function finishRun(runId: string) {
  runsStore.set((s) => ({
    runs: s.runs.map((r) => (r.id === runId ? { ...r, finishedAt: Date.now() } : r)),
  }));
}

function hostTarget(
  host: HostCard | undefined,
  hostId: Uuid,
): Pick<RunTarget, "label" | "subtitle"> {
  if (!host) return { label: "Removed host", subtitle: hostId };
  const port = host.port === (host.protocol === "telnet" ? 23 : 22) ? "" : `:${host.port}`;
  return {
    label: host.label,
    subtitle: host.username ? `${host.username}@${host.address}${port}` : `${host.address}${port}`,
  };
}

function buildTargets(req: RunRequest): RunTarget[] {
  const panes = terminalStore.get().panes;
  const targets: RunTarget[] = [];
  const usedPanes = new Set<Uuid>();
  const coveredHosts = new Set<Uuid>();

  for (const sid of new Set(req.sessionIds)) {
    const pane = panes[sid];
    if (!pane) continue;
    usedPanes.add(sid);
    if (pane.hostId) coveredHosts.add(pane.hostId);
    targets.push({
      key: `s:${sid}`,
      label: pane.title,
      subtitle: pane.subtitle,
      hostId: pane.hostId,
      paneId: sid,
      opened: false,
      state: pane.status === "connected" ? "pending" : "failed",
      exit: null,
      message: pane.status === "connected" ? null : "Terminal is not connected",
    });
  }

  for (const hid of new Set(req.hostIds)) {
    if (coveredHosts.has(hid)) continue;
    coveredHosts.add(hid);
    const base = hostTarget(
      req.hosts.find((h) => h.id === hid),
      hid,
    );
    const live = Object.values(panes).find(
      (p) => p.hostId === hid && p.status === "connected" && !usedPanes.has(p.id),
    );
    if (live) {
      usedPanes.add(live.id);
      targets.push({
        ...base,
        key: `h:${hid}`,
        hostId: hid,
        paneId: live.id,
        opened: false,
        state: "pending",
        exit: null,
        message: null,
      });
      continue;
    }
    const paneId = openTerminal({ kind: "host", host_id: hid }, { background: true });
    targets.push({
      ...base,
      key: `h:${hid}`,
      hostId: hid,
      paneId,
      opened: paneId !== null,
      state: paneId ? "connecting" : "failed",
      exit: null,
      message: paneId ? null : "Could not open a terminal",
    });
  }
  return targets;
}

/** Start a run. Targets settle independently; watch `runsStore` for progress. */
export function startRun(req: RunRequest): SnippetRun {
  const run: SnippetRun = {
    id: crypto.randomUUID(),
    snippetId: req.snippet.id,
    label: req.snippet.label,
    startedAt: Date.now(),
    finishedAt: null,
    targets: buildTargets(req),
  };
  runsStore.set((s) => ({ runs: [run, ...s.runs].slice(0, MAX_RUNS) }));

  const expected = expectedCommands(req.snippet.script, req.vars);
  const jobs = run.targets
    .filter((t) => !isSettled(t) && t.paneId !== null)
    .map((t) => drive(run.id, t, req, expected));
  void Promise.allSettled(jobs).then(() => finishRun(run.id));
  return run;
}

async function drive(runId: string, target: RunTarget, req: RunRequest, expected: number) {
  const paneId = target.paneId;
  if (!paneId) return;
  const fail = (message: string) => patchTarget(runId, target.key, { state: "failed", message });
  try {
    if (target.state === "connecting") {
      await waitPane(paneId, (p) => p.status !== "connecting");
      const pane = terminalStore.get().panes[paneId];
      if (pane?.status !== "connected") {
        fail(pane?.message ?? "Connection failed");
        return;
      }
      await Promise.race([
        waitPane(paneId, (p) => p.integration || p.status !== "connected"),
        sleep(INTEGRATION_WAIT_MS),
      ]);
    }
    if (terminalStore.get().panes[paneId]?.status !== "connected") {
      fail("Terminal is not connected");
      return;
    }
    patchTarget(runId, target.key, { state: "running" });
    const outcome = req.paste
      ? null
      : watchCommands(paneId, expected, terminalStore.get().panes[paneId]?.integration ?? false);
    const res = await ipc.snippetRun(req.snippet.id, [paneId], req.vars, req.paste ?? false);
    if (outcome === null) {
      patchTarget(runId, target.key, { state: "done", message: "Pasted" });
      return;
    }
    const exit = await outcome;
    const failed = exit !== null && exit !== 0;
    patchTarget(runId, target.key, {
      state: failed ? "failed" : "done",
      exit,
      message: failed ? `Exit code ${exit}` : exit === null ? "Sent" : null,
    });
    if (res.closeAfterRun) void closePane(paneId);
  } catch (e) {
    fail(errorMessage(e));
  }
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

function waitPane(paneId: Uuid, pred: (p: Pane) => boolean): Promise<void> {
  return new Promise((resolve) => {
    const check = () => {
      const p = terminalStore.get().panes[paneId];
      if (!p || pred(p)) {
        off();
        resolve();
        return true;
      }
      return false;
    };
    const off = terminalStore.subscribe(() => void check());
    check();
  });
}

/**
 * Resolve with the exit code of the typed script once the shell is idle again.
 * Without shell integration there is nothing to observe and it resolves `null`.
 * The script is considered finished when `expected` commands completed, or
 * when the shell sits at the prompt for a moment (compound commands report
 * fewer completions than lines).
 */
function watchCommands(
  paneId: Uuid,
  expected: number,
  integration: boolean,
): Promise<number | null> {
  if (!integration) return Promise.resolve(null);
  return new Promise((resolve) => {
    let finished = 0;
    let running = false;
    let firstNonZero: number | null = null;
    let last: number | null = null;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const settle = () => {
      if (timer) clearTimeout(timer);
      offCommands();
      offStore();
      resolve(firstNonZero ?? last);
    };
    const arm = (ms: number) => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        if (!running) settle();
      }, ms);
    };
    const offCommands = onCommandEvent((id, ev) => {
      if (id !== paneId) return;
      if (ev.kind === "started") {
        running = true;
        if (timer) clearTimeout(timer);
        return;
      }
      running = false;
      finished += 1;
      last = ev.exit;
      if (ev.exit !== null && ev.exit !== 0 && firstNonZero === null) firstNonZero = ev.exit;
      if (finished >= expected) settle();
      else arm(QUIET_MS);
    });
    const offStore = terminalStore.subscribe(() => {
      if (terminalStore.get().panes[paneId]?.status !== "connected") settle();
    });
    arm(FIRST_MARK_MS);
  });
}

/** Upper bound on how many prompts the script will come back to. */
export function expectedCommands(script: string, vars: Record<string, string>): number {
  const expanded = script.replace(
    /\{\{\s*([^{}\n]+?)\s*\}\}/g,
    (m, name: string) => vars[name] ?? m,
  );
  const lines = expanded
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0 && !l.startsWith("#"));
  return Math.max(1, lines.length);
}
