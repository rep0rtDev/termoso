import {
  CLOUD_SYNC_MAX_INTERVAL,
  CLOUD_SYNC_MIN_INTERVAL,
  type CloudProvider,
  type CloudSyncConfig,
  type CloudSyncGroup,
  type CloudSyncSecret,
  type Uuid,
} from "@/ipc/types";
import { CLOUD_PROVIDERS, cloudErrorMessage, emptyDraft, type Draft } from "./cloud";

/** Refresh periods offered in the editor; `0` = manual only. */
export const SYNC_INTERVALS: { minutes: number; label: string }[] = [
  { minutes: 0, label: "Manually only" },
  { minutes: 15, label: "Every 15 minutes" },
  { minutes: 30, label: "Every 30 minutes" },
  { minutes: 60, label: "Every hour" },
  { minutes: 6 * 60, label: "Every 6 hours" },
  { minutes: 24 * 60, label: "Every day" },
  { minutes: 7 * 24 * 60, label: "Every week" },
];

export function intervalLabel(minutes: number): string {
  const known = SYNC_INTERVALS.find((i) => i.minutes === minutes);
  if (known) return known.label;
  if (minutes % (24 * 60) === 0) return `Every ${minutes / (24 * 60)} days`;
  if (minutes % 60 === 0) return `Every ${minutes / 60} hours`;
  return `Every ${minutes} minutes`;
}

/** Clamp a typed interval into what Rust accepts (`0` stays manual). */
export function clampInterval(minutes: number): number {
  if (!Number.isFinite(minutes) || minutes <= 0) return 0;
  return Math.min(CLOUD_SYNC_MAX_INTERVAL, Math.max(CLOUD_SYNC_MIN_INTERVAL, Math.round(minutes)));
}

export const providerName = (p: CloudProvider) =>
  CLOUD_PROVIDERS.find((x) => x.id === p)?.name ?? p;

export const SYNC_PRIVACY_NOTE =
  "The key or token is encrypted on this device with your vault key, used only to list machines at the provider, and never synced, shown again or logged.";

/** Editor state: the shared credential draft plus the schedule fields. */
export interface SyncDraft {
  provider: CloudProvider;
  creds: Draft;
  username: string;
  port: string;
  tagIds: Uuid[];
  removeMissing: boolean;
  intervalMinutes: number;
  enabled: boolean;
}

export function emptySyncDraft(provider: CloudProvider): SyncDraft {
  return {
    provider,
    creds: emptyDraft(),
    username: "",
    port: "",
    tagIds: [],
    removeMissing: true,
    intervalMinutes: 60,
    enabled: true,
  };
}

/** Fill the editor from a stored config; secret fields start empty. */
export function syncDraftFromConfig(c: CloudSyncConfig): SyncDraft {
  const d = emptySyncDraft(c.provider);
  d.creds.aws = {
    region: c.region ?? d.creds.aws.region,
    accessKeyId: c.accessKeyId ?? "",
    secretAccessKey: "",
    service: c.service ?? "ec2",
    addressType: c.addressType ?? "public",
  };
  d.creds.azure = { tenantId: c.tenantId ?? "", clientId: c.clientId ?? "", clientSecret: "" };
  d.username = c.username;
  d.port = c.port === null ? "" : String(c.port);
  d.tagIds = [...c.tagIds];
  d.removeMissing = c.removeMissing;
  d.intervalMinutes = c.intervalMinutes;
  d.enabled = c.enabled;
  return d;
}

/**
 * The non-secret half of the draft. `null` when a required identifier is
 * missing or the port is out of range.
 */
export function toSyncConfig(d: SyncDraft): CloudSyncConfig | null {
  const t = (s: string) => s.trim();
  const port = d.port.trim() === "" ? null : Number(d.port);
  if (port !== null && (!Number.isInteger(port) || port < 1 || port > 65535)) return null;
  const base = {
    username: t(d.username),
    port,
    tagIds: d.tagIds,
    removeMissing: d.removeMissing,
    intervalMinutes: clampInterval(d.intervalMinutes),
    enabled: d.enabled,
  };
  switch (d.provider) {
    case "aws":
      if (!t(d.creds.aws.region) || !t(d.creds.aws.accessKeyId)) return null;
      return {
        provider: "aws",
        region: t(d.creds.aws.region),
        accessKeyId: t(d.creds.aws.accessKeyId),
        service: d.creds.aws.service,
        addressType: d.creds.aws.addressType,
        ...base,
      };
    case "digital_ocean":
      return { provider: "digital_ocean", ...base };
    case "azure":
      if (!t(d.creds.azure.tenantId) || !t(d.creds.azure.clientId)) return null;
      return {
        provider: "azure",
        tenantId: t(d.creds.azure.tenantId),
        clientId: t(d.creds.azure.clientId),
        ...base,
      };
  }
}

/** The secret typed in the editor, or `null` when the field was left empty. */
export function toSyncSecret(d: SyncDraft): CloudSyncSecret | null {
  const t = (s: string) => s.trim();
  switch (d.provider) {
    case "aws":
      return t(d.creds.aws.secretAccessKey)
        ? { secretAccessKey: t(d.creds.aws.secretAccessKey) }
        : null;
    case "digital_ocean":
      return t(d.creds.digitalOcean.token) ? { token: t(d.creds.digitalOcean.token) } : null;
    case "azure":
      return t(d.creds.azure.clientSecret) ? { clientSecret: t(d.creds.azure.clientSecret) } : null;
  }
}

/** Did the user change the account the stored secret belongs to? */
export function identityChanged(prev: CloudSyncConfig, next: CloudSyncConfig): boolean {
  return (
    prev.provider !== next.provider ||
    (prev.accessKeyId ?? "") !== (next.accessKeyId ?? "") ||
    (prev.tenantId ?? "") !== (next.tenantId ?? "") ||
    (prev.clientId ?? "") !== (next.clientId ?? "")
  );
}

/**
 * Whether Save can go ahead: a new sync needs the secret; an existing one
 * may keep it unless the provider account changed.
 */
export function canSaveSync(
  d: SyncDraft,
  existing: CloudSyncGroup | null,
): { ok: true; config: CloudSyncConfig; secret: CloudSyncSecret | null } | { ok: false } {
  const config = toSyncConfig(d);
  if (!config) return { ok: false };
  const secret = toSyncSecret(d);
  if (secret) return { ok: true, config, secret };
  if (!existing?.hasSecret) return { ok: false };
  if (identityChanged(existing.config, config)) return { ok: false };
  return { ok: true, config, secret: null };
}

export type SyncTone = "ok" | "error" | "paused" | "idle" | "running";

/** One-line status for cards and the group panel. */
export function syncSummary(g: CloudSyncGroup, now = Date.now()): { tone: SyncTone; text: string } {
  const name = providerName(g.config.provider);
  if (g.running) return { tone: "running", text: `Syncing with ${name}…` };
  if (!g.hasSecret) {
    return { tone: "paused", text: `${name} · credentials not on this device` };
  }
  if (g.status.error) {
    return {
      tone: "error",
      text: `${name} · failed ${relative(g.status.lastRun, now)}: ${cloudErrorMessage(
        { kind: g.status.errorKind ?? "", message: g.status.error },
        name,
      )}`,
    };
  }
  const last = g.status.lastSuccess
    ? `synced ${relative(g.status.lastSuccess, now)}`
    : "not synced yet";
  const cadence = !g.config.enabled
    ? "paused"
    : g.config.intervalMinutes === 0
      ? "manual"
      : g.nextRun
        ? `next ${relative(g.nextRun, now)}`
        : intervalLabel(g.config.intervalMinutes).toLowerCase();
  return {
    tone: !g.config.enabled ? "paused" : g.status.lastSuccess ? "ok" : "idle",
    text: `${name} · ${last} · ${cadence}`,
  };
}

/** `5 min ago` / `in 2 h` — both directions, coarse. */
export function relative(iso: string | undefined, now = Date.now()): string {
  if (!iso) return "never";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "never";
  const diff = Math.round((t - now) / 1000);
  const future = diff > 0;
  const s = Math.abs(diff);
  const unit =
    s < 45
      ? "moments"
      : s < 3600
        ? `${Math.max(1, Math.round(s / 60))} min`
        : s < 86400
          ? `${Math.round(s / 3600)} h`
          : `${Math.round(s / 86400)} d`;
  if (unit === "moments") return future ? "in moments" : "just now";
  return future ? `in ${unit}` : `${unit} ago`;
}

export function reportLine(g: CloudSyncGroup): string | null {
  const r = g.status.report;
  if (!r) return null;
  const parts = [
    r.created ? `${r.created} added` : null,
    r.updated ? `${r.updated} updated` : null,
    r.removed ? `${r.removed} removed` : null,
    r.skipped ? `${r.skipped} skipped` : null,
  ].filter((x): x is string => x !== null);
  const machines = g.status.instances === 1 ? "1 machine" : `${g.status.instances} machines`;
  return parts.length ? `${machines} · ${parts.join(", ")}` : `${machines} · up to date`;
}
