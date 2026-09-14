import { describe, expect, it } from "vitest";
import type { HostCard } from "@/ipc/types";
import {
  hostLink,
  looksLikeTarget,
  parseKnownHostName,
  parseLink,
  parseQuickConnect,
  protocolLink,
  quickFromHistory,
  quickLabel,
} from "./links";

const ID = "3f2b7c9e-1d4a-4e8b-9c6f-0a1b2c3d4e5f";

describe("parseKnownHostName", () => {
  it("splits [host]:port entries", () => {
    expect(parseKnownHostName("[localhost]:2299")).toEqual({ address: "localhost", port: 2299 });
    expect(parseKnownHostName("[::1]:22")).toEqual({ address: "::1", port: 22 });
  });
  it("keeps plain and hashed names", () => {
    expect(parseKnownHostName("lab-alpine")).toEqual({ address: "lab-alpine", port: null });
    expect(parseKnownHostName("|1|abc=|def=")).toEqual({ address: "|1|abc=|def=", port: null });
    expect(parseKnownHostName("[host]:99999")).toEqual({ address: "[host]:99999", port: null });
  });
});

describe("parseQuickConnect", () => {
  it("parses user@host:port", () => {
    expect(parseQuickConnect("root@example.com:2222")).toEqual({
      kind: "quick",
      address: "example.com",
      username: "root",
      port: 2222,
      protocol: "ssh",
    });
  });

  it("defaults to no user and no port", () => {
    expect(parseQuickConnect("10.0.0.5")).toMatchObject({
      address: "10.0.0.5",
      username: null,
      port: null,
      protocol: "ssh",
    });
  });

  it("parses ssh:// URLs and ignores path/query", () => {
    expect(parseQuickConnect("ssh://deploy@bastion.internal:2200/some/path?x=1")).toMatchObject({
      address: "bastion.internal",
      username: "deploy",
      port: 2200,
      protocol: "ssh",
    });
    expect(parseQuickConnect("SSH://HOST")).toMatchObject({ address: "HOST", protocol: "ssh" });
  });

  it("parses telnet:// and telnet:host:port, dropping the user", () => {
    expect(parseQuickConnect("telnet://router.lan:2323")).toEqual({
      kind: "quick",
      address: "router.lan",
      username: null,
      port: 2323,
      protocol: "telnet",
    });
    expect(parseQuickConnect("telnet:switch01:23")).toMatchObject({
      address: "switch01",
      port: 23,
      protocol: "telnet",
    });
    expect(parseQuickConnect("telnet://admin@router.lan")).toMatchObject({
      address: "router.lan",
      username: null,
      protocol: "telnet",
    });
  });

  it("handles bracketed IPv6 with and without a port", () => {
    expect(parseQuickConnect("ssh://ops@[2001:db8::1]:22")).toMatchObject({
      address: "2001:db8::1",
      username: "ops",
      port: 22,
    });
    expect(parseQuickConnect("[fe80::1]")).toMatchObject({ address: "fe80::1", port: null });
  });

  it("never keeps a password from user:pass@host", () => {
    const t = parseQuickConnect("ssh://alice:hunter2@example.com");
    expect(t).toMatchObject({ address: "example.com", username: "alice" });
    expect(JSON.stringify(t)).not.toContain("hunter2");
  });

  it("rejects invalid ports and empty input", () => {
    expect(parseQuickConnect("host:0")).toBeNull();
    expect(parseQuickConnect("host:70000")).toBeNull();
    expect(parseQuickConnect("host:abc")).toBeNull();
    expect(parseQuickConnect("   ")).toBeNull();
    expect(parseQuickConnect("ssh://")).toBeNull();
    expect(parseQuickConnect("ssh://user@")).toBeNull();
  });

  it("rejects things that are not addresses", () => {
    expect(parseQuickConnect("has space.com")).toBeNull();
    expect(parseQuickConnect("a@b@c")).toBeNull();
  });
});

describe("looksLikeTarget", () => {
  it("accepts addresses, user@host and URLs", () => {
    expect(looksLikeTarget("example.com")).toBe(true);
    expect(looksLikeTarget("root@10.0.0.1")).toBe(true);
    expect(looksLikeTarget("ssh://x")).toBe(true);
    expect(looksLikeTarget("telnet:x:23")).toBe(true);
    expect(looksLikeTarget("10.0.0.1:22")).toBe(true);
  });
  it("rejects plain search words", () => {
    expect(looksLikeTarget("prod")).toBe(false);
    expect(looksLikeTarget("web server")).toBe(false);
    expect(looksLikeTarget("")).toBe(false);
  });
});

describe("quickFromHistory / quickLabel", () => {
  it("restores telnet protocol from history rows", () => {
    expect(quickFromHistory("router.lan:2323", "telnet")).toMatchObject({
      protocol: "telnet",
      username: null,
    });
    expect(quickFromHistory("root@host", "ssh")).toMatchObject({
      protocol: "ssh",
      username: "root",
    });
  });
  it("labels targets the way they were typed", () => {
    const label = (s: string) => {
      const t = parseQuickConnect(s);
      return t ? quickLabel(t) : null;
    };
    expect(label("root@host:2222")).toBe("root@host:2222");
    expect(label("telnet://router:23")).toBe("telnet router:23");
    expect(label("[2001:db8::1]")).toBe("[2001:db8::1]");
  });
});

const card = (over: Partial<HostCard>): HostCard => ({
  id: ID,
  vaultId: ID,
  label: "web",
  address: "example.com",
  port: 22,
  username: "root",
  protocol: "ssh",
  telnetPort: null,
  useMosh: false,
  groupId: null,
  groupPath: [],
  tags: [],
  icon: null,
  osName: null,
  ipVersion: "auto",
  notes: "",
  sortOrder: 0,
  updatedAt: "",
  lastConnected: null,
  cloudProvider: null,
  dirty: false,
  ...over,
});

describe("links", () => {
  it("builds termoso:// and protocol links without secrets", () => {
    expect(hostLink({ id: ID })).toBe(`termoso://host/${ID}`);
    expect(protocolLink(card({}))).toBe("ssh://root@example.com");
    expect(protocolLink(card({ port: 2222, username: "a b" }))).toBe(
      "ssh://a%20b@example.com:2222",
    );
    expect(protocolLink(card({ username: "", address: "2001:db8::1" }))).toBe(
      "ssh://[2001:db8::1]",
    );
    expect(protocolLink(card({ protocol: "telnet", port: 23, username: "ignored" }))).toBe(
      "telnet://example.com:23",
    );
    expect(protocolLink(card({}), "telnet")).toBeNull();
    expect(protocolLink(card({ telnetPort: 2323 }), "telnet")).toBe("telnet://example.com:2323");
    expect(protocolLink(card({ protocol: "telnet", port: 23 }), "ssh")).toBeNull();
  });

  it("parses incoming links", () => {
    expect(parseLink(`termoso://host/${ID}`)).toEqual({ kind: "host", hostId: ID });
    expect(parseLink(`termoso://host/${ID.toUpperCase()}/`)).toMatchObject({ kind: "host" });
    expect(parseLink("termoso://host/not-a-uuid")).toMatchObject({ kind: "unsupported" });
    expect(parseLink("ssh://root@example.com:2222")).toEqual({
      kind: "quick",
      target: parseQuickConnect("ssh://root@example.com:2222"),
    });
    expect(parseLink("telnet://router:23")).toMatchObject({ kind: "quick" });
    expect(parseLink("ssh://")).toMatchObject({ kind: "unsupported" });
    expect(parseLink("https://example.com")).toMatchObject({ kind: "unsupported" });
  });

  it("parses multiplayer join links", () => {
    const secret = "A".repeat(43);
    const link = `termoso://join/${ID}?s=https%3A%2F%2Fcloud.example.com#${secret}`;
    expect(parseLink(link)).toEqual({ kind: "live", link });
    expect(parseLink(`termoso://join/${ID}#${secret}`)).toEqual({
      kind: "live",
      link: `termoso://join/${ID}#${secret}`,
    });
    expect(parseLink(`termoso://join/${ID}`)).toMatchObject({ kind: "unsupported" });
    expect(parseLink(`termoso://join/${ID}#short`)).toMatchObject({ kind: "unsupported" });
    expect(parseLink(`termoso://join/nope#${secret}`)).toMatchObject({ kind: "unsupported" });
    const web = `https://cloud.example.com/join/${ID}#${secret}`;
    expect(parseLink(web)).toEqual({ kind: "live", link: web });
    expect(parseLink(`https://x.test:8443/termoso/join/${ID}/?s=x#${secret}`)).toMatchObject({
      kind: "live",
    });
    expect(parseLink(`https://cloud.example.com/join/${ID}`)).toMatchObject({
      kind: "unsupported",
    });
    expect(parseLink(`https://cloud.example.com/invite/${ID}#${secret}`)).toMatchObject({
      kind: "unsupported",
    });
  });
});
