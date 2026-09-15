import { describe, expect, it } from "vitest";
import type { CloudSyncConfig, CloudSyncGroup } from "@/ipc/types";
import {
  canSaveSync,
  clampInterval,
  emptySyncDraft,
  identityChanged,
  intervalLabel,
  relative,
  reportLine,
  syncDraftFromConfig,
  syncSummary,
  toSyncConfig,
  toSyncSecret,
} from "./cloudSync";

const G = "11111111-1111-1111-1111-111111111111";
const V = "22222222-2222-2222-2222-222222222222";

const awsConfig: CloudSyncConfig = {
  provider: "aws",
  region: "eu-central-1",
  accessKeyId: "AKIA",
  service: "ec2",
  addressType: "public",
  username: "ec2-user",
  port: null,
  tagIds: [],
  removeMissing: true,
  intervalMinutes: 60,
  enabled: true,
};

const group = (over: Partial<CloudSyncGroup> = {}): CloudSyncGroup => ({
  groupId: G,
  vaultId: V,
  label: "AWS eu",
  config: awsConfig,
  status: { instances: 0 },
  hasSecret: true,
  running: false,
  ...over,
});

describe("clampInterval / intervalLabel", () => {
  it("keeps 0 as manual and clamps everything else into Rust's bounds", () => {
    expect(clampInterval(0)).toBe(0);
    expect(clampInterval(-5)).toBe(0);
    expect(clampInterval(NaN)).toBe(0);
    expect(clampInterval(1)).toBe(5);
    expect(clampInterval(60.4)).toBe(60);
    expect(clampInterval(10 * 24 * 60)).toBe(7 * 24 * 60);
  });

  it("labels known and ad-hoc intervals", () => {
    expect(intervalLabel(0)).toBe("Manually only");
    expect(intervalLabel(60)).toBe("Every hour");
    expect(intervalLabel(2 * 24 * 60)).toBe("Every 2 days");
    expect(intervalLabel(180)).toBe("Every 3 hours");
    expect(intervalLabel(45)).toBe("Every 45 minutes");
  });
});

describe("toSyncConfig / toSyncSecret", () => {
  it("splits AWS input into the stored config and the one-shot secret", () => {
    const d = emptySyncDraft("aws");
    d.creds.aws = {
      region: " eu-west-1 ",
      accessKeyId: " AKIA ",
      secretAccessKey: " s3cr3t ",
      service: "lightsail",
      addressType: "private",
    };
    d.username = " ubuntu ";
    d.port = "2222";
    d.intervalMinutes = 3;
    expect(toSyncConfig(d)).toEqual({
      provider: "aws",
      region: "eu-west-1",
      accessKeyId: "AKIA",
      service: "lightsail",
      addressType: "private",
      username: "ubuntu",
      port: 2222,
      tagIds: [],
      removeMissing: true,
      intervalMinutes: 5,
      enabled: true,
    });
    expect(toSyncSecret(d)).toEqual({ secretAccessKey: "s3cr3t" });
  });

  it("never puts a secret into the config object", () => {
    const d = emptySyncDraft("azure");
    d.creds.azure = { tenantId: "t", clientId: "c", clientSecret: "shh" };
    const c = toSyncConfig(d);
    expect(JSON.stringify(c)).not.toContain("shh");
    expect(toSyncSecret(d)).toEqual({ clientSecret: "shh" });
    const dop = emptySyncDraft("digital_ocean");
    dop.creds.digitalOcean.token = "dop_v1_x";
    expect(JSON.stringify(toSyncConfig(dop))).not.toContain("dop_v1_x");
    expect(toSyncSecret(dop)).toEqual({ token: "dop_v1_x" });
  });

  it("rejects incomplete identifiers and bad ports", () => {
    const d = emptySyncDraft("aws");
    expect(toSyncConfig(d)).toBeNull();
    d.creds.aws.region = "eu-central-1";
    d.creds.aws.accessKeyId = "AKIA";
    expect(toSyncConfig(d)).not.toBeNull();
    d.port = "70000";
    expect(toSyncConfig(d)).toBeNull();
    d.port = "abc";
    expect(toSyncConfig(d)).toBeNull();
    const az = emptySyncDraft("azure");
    az.creds.azure = { tenantId: "t", clientId: "", clientSecret: "x" };
    expect(toSyncConfig(az)).toBeNull();
  });

  it("round-trips a stored config into an editor draft with empty secrets", () => {
    const d = syncDraftFromConfig({ ...awsConfig, port: 2200, tagIds: [V] });
    expect(d.creds.aws.region).toBe("eu-central-1");
    expect(d.creds.aws.accessKeyId).toBe("AKIA");
    expect(d.creds.aws.secretAccessKey).toBe("");
    expect(d.port).toBe("2200");
    expect(d.tagIds).toEqual([V]);
    expect(toSyncConfig(d)).toEqual({ ...awsConfig, port: 2200, tagIds: [V] });
    expect(toSyncSecret(d)).toBeNull();
  });
});

describe("canSaveSync", () => {
  it("needs a secret for a brand new sync", () => {
    const d = syncDraftFromConfig(awsConfig);
    expect(canSaveSync(d, null)).toEqual({ ok: false });
    d.creds.aws.secretAccessKey = "s";
    expect(canSaveSync(d, null)).toEqual({
      ok: true,
      config: awsConfig,
      secret: { secretAccessKey: "s" },
    });
  });

  it("keeps the stored secret when only schedule fields change", () => {
    const d = syncDraftFromConfig(awsConfig);
    d.intervalMinutes = 0;
    d.enabled = false;
    const r = canSaveSync(d, group());
    expect(r.ok && r.secret).toBeNull();
    expect(r.ok && r.config.intervalMinutes).toBe(0);
  });

  it("demands a new secret when the account identity changes or none is stored", () => {
    const d = syncDraftFromConfig(awsConfig);
    d.creds.aws.accessKeyId = "AKIA-OTHER";
    expect(canSaveSync(d, group())).toEqual({ ok: false });
    expect(canSaveSync(syncDraftFromConfig(awsConfig), group({ hasSecret: false }))).toEqual({
      ok: false,
    });
    const az = syncDraftFromConfig(awsConfig);
    az.provider = "azure";
    az.creds.azure = { tenantId: "t", clientId: "c", clientSecret: "" };
    expect(canSaveSync(az, group())).toEqual({ ok: false });
  });

  it("identityChanged ignores schedule and address fields", () => {
    expect(identityChanged(awsConfig, { ...awsConfig, region: "us-east-1", port: 22 })).toBe(false);
    expect(identityChanged(awsConfig, { ...awsConfig, accessKeyId: "X" })).toBe(true);
    expect(identityChanged(awsConfig, { ...awsConfig, provider: "digital_ocean" })).toBe(true);
  });
});

describe("syncSummary / reportLine / relative", () => {
  const now = Date.parse("2026-09-14T12:00:00Z");

  it("describes relative times coarsely in both directions", () => {
    expect(relative(undefined, now)).toBe("never");
    expect(relative("garbage", now)).toBe("never");
    expect(relative("2026-09-14T11:59:50Z", now)).toBe("just now");
    expect(relative("2026-09-14T11:40:00Z", now)).toBe("20 min ago");
    expect(relative("2026-09-14T13:00:00Z", now)).toBe("in 1 h");
    expect(relative("2026-09-11T12:00:00Z", now)).toBe("3 d ago");
  });

  it("prefers running, then missing secret, then error, then cadence", () => {
    expect(syncSummary(group({ running: true }), now).tone).toBe("running");
    expect(syncSummary(group({ hasSecret: false }), now)).toEqual({
      tone: "paused",
      text: "Amazon AWS · credentials not on this device",
    });
    const failed = syncSummary(
      group({
        status: {
          instances: 0,
          lastRun: "2026-09-14T11:30:00Z",
          errorKind: "cloud_invalid_credentials",
          error: "403",
        },
      }),
      now,
    );
    expect(failed.tone).toBe("error");
    expect(failed.text).toContain("failed 30 min ago");
    expect(failed.text).toContain("not able to validate");
    expect(failed.text).not.toContain("403");

    const ok = syncSummary(
      group({
        status: { instances: 3, lastSuccess: "2026-09-14T11:00:00Z" },
        nextRun: "2026-09-14T12:00:20Z",
      }),
      now,
    );
    expect(ok).toEqual({ tone: "ok", text: "Amazon AWS · synced 1 h ago · next in moments" });
    expect(syncSummary(group(), now)).toEqual({
      tone: "idle",
      text: "Amazon AWS · not synced yet · every hour",
    });
    expect(syncSummary(group({ config: { ...awsConfig, enabled: false } }), now).text).toMatch(
      /paused$/,
    );
    expect(syncSummary(group({ config: { ...awsConfig, intervalMinutes: 0 } }), now).text).toMatch(
      /manual$/,
    );
  });

  it("summarises the last report", () => {
    expect(reportLine(group())).toBeNull();
    expect(
      reportLine(
        group({
          status: {
            instances: 4,
            report: { created: 1, updated: 2, unchanged: 1, removed: 0, skipped: 0, warnings: [] },
          },
        }),
      ),
    ).toBe("4 machines · 1 added, 2 updated");
    expect(
      reportLine(
        group({
          status: {
            instances: 1,
            report: { created: 0, updated: 0, unchanged: 1, removed: 0, skipped: 0, warnings: [] },
          },
        }),
      ),
    ).toBe("1 machine · up to date");
  });
});
