import { describe, expect, it } from "vitest";
import type { CertificateCard, IdentityCard, KeyCard } from "@/ipc/types";
import {
  certificateState,
  certificateName,
  certificateSummary,
  droppedKind,
  filterIdentities,
  filterKeys,
  identitySubtitle,
  keyTypeLabel,
  labelFromPath,
} from "./model";

const cert = (over: Partial<CertificateCard> = {}): CertificateCard => ({
  id: "c1",
  certType: "ssh-ed25519-cert-v01@openssh.com",
  kind: "user",
  keyId: "deploy@ci",
  serial: 7,
  principals: ["root", "deploy"],
  validAfter: "2026-01-01T00:00:00Z",
  validBefore: "2026-12-31T00:00:00Z",
  fingerprint: "SHA256:key",
  caFingerprint: "SHA256:ca",
  caKeyType: "ssh-ed25519",
  validNow: true,
  ...over,
});

const key = (over: Partial<KeyCard> = {}): KeyCard => ({
  id: "k1",
  vaultId: "v",
  label: "laptop",
  keyType: "ssh-ed25519",
  bits: 256,
  fingerprint: "SHA256:abc",
  publicKey: "ssh-ed25519 AAAA",
  comment: "me@laptop",
  encrypted: false,
  hasPassphrase: false,
  unreadable: false,
  agentBacked: false,
  usedBy: 0,
  certificate: null,
  certificateUnreadable: false,
  securityKey: null,
  updatedAt: "2026-01-01T00:00:00Z",
  dirty: false,
  ...over,
});

const identity = (over: Partial<IdentityCard> = {}): IdentityCard => ({
  id: "i1",
  vaultId: "v",
  label: "prod",
  username: "root",
  hasPassword: false,
  sshKeyId: null,
  sshKeyLabel: null,
  sshCertificateId: null,
  hasCertificate: false,
  sshId: false,
  sshIdKeyType: null,
  updatedAt: "2026-01-01T00:00:00Z",
  ...over,
});

describe("keyTypeLabel", () => {
  it("prints the algorithm like Termius", () => {
    expect(keyTypeLabel(key())).toBe("Type ED25519");
    expect(keyTypeLabel(key({ keyType: "ssh-rsa", bits: 4096 }))).toBe("Type RSA 4096");
    expect(keyTypeLabel(key({ keyType: "ecdsa-sha2-nistp384", bits: 384 }))).toBe("Type ECDSA 384");
    expect(keyTypeLabel(key({ unreadable: true }))).toBe("Type unknown");
    expect(keyTypeLabel(key({ keyType: "", bits: 0 }))).toBe("Type unknown");
  });
});

describe("certificateState", () => {
  const now = new Date("2026-06-01T00:00:00Z");
  it("classifies validity windows", () => {
    expect(certificateState(cert(), false, now)).toBe("valid");
    expect(certificateState(cert({ validBefore: "2026-05-01T00:00:00Z" }), false, now)).toBe(
      "expired",
    );
    expect(certificateState(cert({ validAfter: "2026-07-01T00:00:00Z" }), false, now)).toBe(
      "not_yet",
    );
    expect(certificateState(cert({ validAfter: null, validBefore: null }), false, now)).toBe(
      "valid",
    );
    expect(certificateState(null, false, now)).toBeNull();
    expect(certificateState(null, true, now)).toBe("unreadable");
  });

  it("summarises kind, principals and expiry", () => {
    const s = certificateSummary(cert(), now);
    expect(s).toContain("user certificate");
    expect(s).toContain("root, deploy");
    expect(s).toContain("until");
    expect(certificateSummary(cert({ principals: [], kind: "host" }), now)).toContain(
      "host certificate · any principal",
    );
    expect(certificateSummary(cert({ validBefore: "2026-05-01T00:00:00Z" }), now)).toContain(
      "expired",
    );
    expect(certificateSummary(cert({ validBefore: null }), now)).toContain("no expiry");
  });

  it("names a certificate by key ID, then principals, then the CA", () => {
    expect(certificateName(cert())).toBe("deploy@ci");
    expect(certificateName(cert({ keyId: " " }))).toBe("root, deploy");
    expect(certificateName(cert({ keyId: "", principals: [] }))).toBe("signed by SHA256:ca");
  });
});

describe("dropped files", () => {
  it("tells certificates, public and private keys apart by name", () => {
    expect(droppedKind("/home/u/.ssh/id_ed25519-cert.pub")).toBe("certificate");
    expect(droppedKind("C:\\Users\\u\\id_rsa.pub")).toBe("public");
    expect(droppedKind("/home/u/.ssh/id_ed25519")).toBe("private");
    expect(droppedKind("/home/u/server.ppk")).toBe("private");
  });

  it("derives a label from the file name", () => {
    expect(labelFromPath("/home/u/keys/server.ppk")).toBe("server");
    expect(labelFromPath("C:\\keys\\id_ed25519")).toBe("id_ed25519");
    expect(labelFromPath("/x/prod.pem")).toBe("prod");
  });
});

describe("search", () => {
  const keys = [
    key(),
    key({
      id: "k2",
      label: "ci",
      comment: "",
      fingerprint: "SHA256:def",
      certificate: cert({ principals: ["builder"] }),
    }),
    key({
      id: "k3",
      label: "keepass",
      comment: "me@vault",
      fingerprint: "SHA256:ghi",
      agentBacked: true,
    }),
  ];
  it("matches keys by label, comment, fingerprint and certificate", () => {
    expect(filterKeys(keys, "")).toHaveLength(3);
    expect(filterKeys(keys, "me@vault").map((k) => k.id)).toEqual(["k3"]);
    expect(filterKeys(keys, "agent").map((k) => k.id)).toEqual(["k3"]);
    expect(filterKeys(keys, "LAPTOP").map((k) => k.id)).toEqual(["k1"]);
    expect(filterKeys(keys, "sha256:abc").map((k) => k.id)).toEqual(["k1"]);
    expect(filterKeys(keys, "builder").map((k) => k.id)).toEqual(["k2"]);
    expect(filterKeys(keys, "deploy@ci").map((k) => k.id)).toEqual(["k2"]);
    expect(filterKeys(keys, "nothing")).toHaveLength(0);
  });

  it("matches identities by label, username and key", () => {
    const ids = [
      identity(),
      identity({ id: "i2", label: "dev", username: "me", sshKeyLabel: "laptop" }),
    ];
    expect(filterIdentities(ids, "root").map((i) => i.id)).toEqual(["i1"]);
    expect(filterIdentities(ids, "laptop").map((i) => i.id)).toEqual(["i2"]);
  });
});

describe("identitySubtitle", () => {
  it("lists the auth methods", () => {
    expect(identitySubtitle(identity({ username: "" }))).toBe("No auth method");
    expect(identitySubtitle(identity({ hasPassword: true }))).toBe("root · Password");
    expect(identitySubtitle(identity({ sshKeyLabel: "laptop" }))).toBe("root · Key · laptop");
    expect(
      identitySubtitle(identity({ sshKeyLabel: "ci", hasCertificate: true, hasPassword: true })),
    ).toBe("root · Certificate · ci + Password");
  });
});
