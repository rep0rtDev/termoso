import {
  emptyHostForm,
  type HostCard,
  type HostForm,
  type TailnetDevice,
  type TailnetProvider,
  type Uuid,
} from "@/ipc/types";
import { tr } from "@/i18n";

export type TailnetAddressMode = "dns" | "ip";

export const PROVIDER_NAMES: Record<TailnetProvider, string> = {
  tailscale: "Tailscale",
  netbird: "NetBird",
  zerotier: "ZeroTier",
};

export function tailnetKey(d: TailnetDevice): string {
  return `${d.provider}|${d.dnsName ?? ""}|${d.addresses.join(",")}`;
}

/** What the new host will connect to under the chosen mode, or `null` if nothing usable. */
export function tailnetAddress(d: TailnetDevice, mode: TailnetAddressMode): string | null {
  const ip = d.addresses[0] ?? null;
  if (mode === "dns") return d.dnsName ?? ip;
  return ip ?? d.dnsName;
}

const norm = (s: string) => s.trim().toLowerCase().replace(/\.$/, "");

/** Hosts in the vault already pointing at this device (by DNS name, short name or any address). */
export function tailnetExisting(d: TailnetDevice, hosts: readonly HostCard[]): HostCard | null {
  const targets = new Set<string>();
  if (d.dnsName) {
    targets.add(norm(d.dnsName));
    targets.add(norm(d.dnsName.split(".")[0] ?? ""));
  }
  for (const a of d.addresses) targets.add(norm(a));
  targets.delete("");
  return hosts.find((h) => targets.has(norm(h.address))) ?? null;
}

/** Second line: address under the chosen mode, OS, tags, offline marker. */
export function tailnetSubtitle(d: TailnetDevice, mode: TailnetAddressMode): string {
  const parts: string[] = [];
  const addr = tailnetAddress(d, mode);
  if (addr) parts.push(addr);
  if (mode === "dns" && d.addresses[0] && d.addresses[0] !== addr) parts.push(d.addresses[0]);
  else if (mode === "ip" && d.dnsName && d.dnsName !== addr) parts.push(d.dnsName);
  if (d.os) parts.push(d.os);
  if (d.tags.length) parts.push(d.tags.join(", "));
  if (!d.online) parts.push(tr("offline"));
  return parts.join(" · ");
}

export function tailnetHostForm(
  d: TailnetDevice,
  vaultId: Uuid,
  groupId: Uuid | null,
  mode: TailnetAddressMode,
  username: string,
): HostForm | null {
  const address = tailnetAddress(d, mode);
  if (!address) return null;
  return {
    ...emptyHostForm(vaultId, groupId),
    label: d.name || address,
    address,
    username: username.trim(),
  };
}
