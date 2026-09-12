import type { TeamRole, VaultRole } from "@/ipc/types";

export const vaultRoleLabel: Record<VaultRole, string> = {
  viewer: "can view",
  editor: "can edit",
  manager: "can manage",
};

export const vaultRoleHint: Record<VaultRole, string> = {
  viewer: "Connects and reads hosts, keys and snippets",
  editor: "Also adds, changes and removes items",
  manager: "Also decides who has access and rotates the key",
};

export const VAULT_ROLES: VaultRole[] = ["viewer", "editor", "manager"];

export const teamRoleLabel: Record<TeamRole, string> = {
  member: "Member",
  admin: "Admin",
  owner: "Owner",
};

export const teamRoleHint: Record<TeamRole, string> = {
  member: "Uses the vaults they were given access to",
  admin: "Also invites, removes members and creates vaults",
  owner: "Everything, including deleting the team",
};

export const isTeamAdmin = (r: TeamRole) => r === "admin" || r === "owner";

/** Splits a pasted list of addresses (commas, semicolons, spaces, new lines). */
export function splitEmails(text: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of text.split(/[\s,;]+/)) {
    const e = raw.trim().toLowerCase();
    if (!e || seen.has(e)) continue;
    seen.add(e);
    out.push(e);
  }
  return out;
}

export const looksLikeEmail = (e: string) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(e);
