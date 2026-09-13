import { describe, expect, it } from "vitest";
import type { CloudPreview } from "@/ipc/types";
import { cloudErrorMessage, emptyDraft, importable, toConfig } from "./cloud";

describe("toConfig", () => {
  it("maps the AWS draft to the snake_case IPC shape and trims non-secret fields", () => {
    const d = emptyDraft();
    d.aws = {
      region: " eu-west-1 ",
      accessKeyId: " AKIA ",
      secretAccessKey: "s3cr3t",
      service: "lightsail",
      addressType: "private",
    };
    expect(toConfig("aws", d)).toEqual({
      provider: "aws",
      region: "eu-west-1",
      access_key_id: "AKIA",
      secret_access_key: "s3cr3t",
      service: "lightsail",
      address_type: "private",
    });
  });

  it("refuses incomplete input before anything is sent over IPC", () => {
    const d = emptyDraft();
    expect(toConfig("aws", d)).toBeNull();
    expect(toConfig("digital_ocean", d)).toBeNull();
    expect(toConfig("azure", d)).toBeNull();
    d.digitalOcean.token = "   ";
    expect(toConfig("digital_ocean", d)).toBeNull();
    d.azure = { tenantId: "t", clientId: "c", clientSecret: "" };
    expect(toConfig("azure", d)).toBeNull();
  });

  it("maps DigitalOcean and Azure drafts", () => {
    const d = emptyDraft();
    d.digitalOcean.token = "dop_v1_x";
    d.azure = { tenantId: "t", clientId: "c", clientSecret: "sec" };
    expect(toConfig("digital_ocean", d)).toEqual({ provider: "digital_ocean", token: "dop_v1_x" });
    expect(toConfig("azure", d)).toEqual({
      provider: "azure",
      tenant_id: "t",
      client_id: "c",
      client_secret: "sec",
    });
  });
});

describe("cloudErrorMessage", () => {
  it("uses provider wording for typed cloud errors", () => {
    expect(
      cloudErrorMessage(
        { kind: "cloud_invalid_credentials", message: "AuthFailure" },
        "Amazon AWS",
      ),
    ).toBe("Amazon AWS was not able to validate the provided access credentials.");
    expect(cloudErrorMessage({ kind: "rate_limited", message: "429" }, "DigitalOcean")).toMatch(
      /rate limiting/,
    );
    expect(cloudErrorMessage({ kind: "cloud_unavailable", message: "dns" }, "Azure")).toMatch(
      /could not be reached/,
    );
  });

  it("falls back to the plain message for other errors", () => {
    expect(cloudErrorMessage({ kind: "invalid", message: "region is required" }, "AWS")).toBe(
      "region is required",
    );
    expect(cloudErrorMessage(new Error("boom"), "AWS")).toBe("boom");
  });
});

describe("importable", () => {
  it("preselects every machine that has an address", () => {
    const inst = (action: "new" | "update" | "no_address", address: string | null) => ({
      instanceId: "i",
      label: "l",
      address,
      state: null,
      region: null,
      size: null,
      os: null,
      osName: null,
      action,
      hostId: null,
    });
    const preview: CloudPreview = {
      id: "00000000-0000-0000-0000-000000000000",
      provider: "aws",
      providerName: "Amazon AWS",
      service: "ec2",
      addressType: "public",
      instances: [inst("new", "1.2.3.4"), inst("no_address", null), inst("update", "5.6.7.8")],
    };
    expect(importable(preview)).toEqual([0, 2]);
  });
});
