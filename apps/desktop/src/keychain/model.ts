import type { CertificateCard, IdentityCard, KeyCard } from "@/ipc/types";

/** Card subtitle as Termius prints it: `Type ED25519`, `Type RSA 4096`. */
/** FIDO2-backed keys (`sk-*`) are picked through the FIDO2 row, not Key. */
export const isHardwareKey = (k: Pick<KeyCard, "keyType">) => k.keyType.startsWith("sk-");

export function keyTypeLabel(k: Pick<KeyCard, "keyType" | "bits" | "unreadable">): string {
  if (k.unreadable || !k.keyType) return "Type unknown";
  const t = k.keyType.toLowerCase();
  const sk = t.startsWith("sk-");
  const base = t.includes("ed25519")
    ? "ED25519"
    : t.includes("rsa")
      ? "RSA"
      : t.includes("ecdsa")
        ? "ECDSA"
        : k.keyType.toUpperCase();
  const name = sk ? `${base}-SK` : base;
  return sk || base === "ED25519" || k.bits === 0 ? `Type ${name}` : `Type ${name} ${k.bits}`;
}

export type CertificateState = "valid" | "not_yet" | "expired" | "unreadable";

export function certificateState(
  c: CertificateCard | null,
  unreadable: boolean,
  now: Date = new Date(),
): CertificateState | null {
  if (unreadable) return "unreadable";
  if (!c) return null;
  if (c.validAfter && new Date(c.validAfter) > now) return "not_yet";
  if (c.validBefore && new Date(c.validBefore) <= now) return "expired";
  return "valid";
}

/** Short human name for a certificate: its key ID, else its principals, else the CA. */
export function certificateName(c: CertificateCard): string {
  if (c.keyId.trim()) return c.keyId;
  if (c.principals.length > 0) return c.principals.join(", ");
  return `signed by ${c.caFingerprint}`;
}

/** One-line human summary of a certificate for chips and hints. */
export function certificateSummary(c: CertificateCard, now: Date = new Date()): string {
  const who = c.principals.length > 0 ? c.principals.join(", ") : "any principal";
  const state = certificateState(c, false, now);
  const until = c.validBefore
    ? state === "expired"
      ? `expired ${new Date(c.validBefore).toLocaleDateString()}`
      : `until ${new Date(c.validBefore).toLocaleDateString()}`
    : "no expiry";
  const from =
    state === "not_yet" && c.validAfter
      ? `from ${new Date(c.validAfter).toLocaleDateString()}`
      : null;
  return [c.kind === "host" ? "host certificate" : "user certificate", who, from ?? until]
    .filter(Boolean)
    .join(" · ");
}

/** What a dropped/picked file is, judged by its name only (contents stay in Rust). */
export function droppedKind(path: string): "certificate" | "public" | "private" {
  const name = path.split(/[\\/]/).pop() ?? path;
  if (/-cert\.pub$/i.test(name)) return "certificate";
  if (/\.pub$/i.test(name)) return "public";
  return "private";
}

/** Default label for a key picked from disk: file name without extension. */
export function labelFromPath(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  return name.replace(/\.(ppk|pem|key)$/i, "");
}

const norm = (s: string) => s.trim().toLowerCase();

export function filterKeys(keys: KeyCard[], query: string): KeyCard[] {
  const q = norm(query);
  if (!q) return keys;
  return keys.filter(
    (k) =>
      norm(k.label).includes(q) ||
      norm(k.comment).includes(q) ||
      norm(k.fingerprint).includes(q) ||
      norm(k.keyType).includes(q) ||
      (k.certificate !== null &&
        (norm(k.certificate.keyId).includes(q) ||
          k.certificate.principals.some((p) => norm(p).includes(q)))),
  );
}

export function filterIdentities(ids: IdentityCard[], query: string): IdentityCard[] {
  const q = norm(query);
  if (!q) return ids;
  return ids.filter(
    (i) =>
      norm(i.label).includes(q) ||
      norm(i.username).includes(q) ||
      (i.sshKeyLabel !== null && norm(i.sshKeyLabel).includes(q)),
  );
}

/** Second line of an identity card. */
export function identitySubtitle(i: IdentityCard): string {
  const auth = [
    i.sshKeyLabel
      ? i.hasCertificate
        ? `Certificate · ${i.sshKeyLabel}`
        : `Key · ${i.sshKeyLabel}`
      : null,
    i.hasPassword ? "Password" : null,
  ].filter((s): s is string => s !== null);
  const via = auth.length > 0 ? auth.join(" + ") : "No auth method";
  return i.username ? `${i.username} · ${via}` : via;
}
