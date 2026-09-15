import {
  emptyHostForm,
  type HostCard,
  type HostForm,
  type LocalDevice,
  type Uuid,
} from "@/ipc/types";

export type LanAddressMode = "hostname" | "ip";

/** `nas.local.` → `nas.local`; empty when the advertisement carried no hostname. */
export function lanHostname(d: LocalDevice): string {
  return d.hostname.replace(/\.$/, "");
}

/** What the new host will connect to under the chosen mode, or `null` if nothing usable. */
export function lanAddress(d: LocalDevice, mode: LanAddressMode): string | null {
  const name = lanHostname(d);
  const ip = d.addresses[0] ?? null;
  if (mode === "hostname") return name || ip;
  return ip ?? (name || null);
}

/** Human label: the advertised instance name, else the bare hostname, else the address. */
export function lanLabel(d: LocalDevice): string {
  const name = d.name.trim();
  if (name) return name;
  const host = lanHostname(d);
  if (host) return host.replace(/\.local$/i, "");
  return d.addresses[0] ?? "Local device";
}

const norm = (s: string) => s.trim().toLowerCase().replace(/\.$/, "");

/**
 * Hosts in the vault that already point at this device (by `.local` name or
 * by any advertised address). Discovery never creates duplicates silently.
 */
export function lanExisting(d: LocalDevice, hosts: readonly HostCard[]): HostCard | null {
  const targets = new Set<string>();
  const name = norm(lanHostname(d));
  if (name) targets.add(name);
  for (const a of d.addresses) targets.add(norm(a));
  return hosts.find((h) => targets.has(norm(h.address))) ?? null;
}

/** Second line under a discovered device: hostname/addresses, port and services. */
export function lanSubtitle(d: LocalDevice, mode: LanAddressMode): string {
  const parts: string[] = [];
  const name = lanHostname(d);
  const addr = lanAddress(d, mode);
  if (mode === "hostname" && name) {
    parts.push(name);
    if (d.addresses.length) parts.push(d.addresses.slice(0, 2).join(", "));
  } else if (addr) {
    parts.push(addr);
    if (name && name !== addr) parts.push(name);
  }
  if (d.port !== 22) parts.push(`port ${d.port}`);
  parts.push(d.services.map((s) => s.toUpperCase()).join(" + "));
  return parts.join(" · ");
}

export function lanHostForm(
  d: LocalDevice,
  vaultId: Uuid,
  groupId: Uuid | null,
  mode: LanAddressMode,
  username: string,
): HostForm | null {
  const address = lanAddress(d, mode);
  if (!address) return null;
  return {
    ...emptyHostForm(vaultId, groupId),
    label: lanLabel(d),
    address,
    port: d.port === 22 ? null : d.port,
    username: username.trim(),
  };
}

/** Primary button text: `Add 3 hosts`, `Add host`, or with a skipped-duplicates note. */
export function addLabel(fresh: number, skipped: number): string {
  const base = fresh === 0 ? "Add hosts" : fresh === 1 ? "Add host" : `Add ${fresh} hosts`;
  return skipped > 0 ? `${base} (${skipped} already added)` : base;
}
