import type { TeamRole, VaultRole } from "@/ipc/types";
import { msg } from "@/i18n";

export const vaultRoleLabel: Record<VaultRole, string> = {
  viewer: msg("can view"),
  editor: msg("can edit"),
  manager: msg("can manage"),
};

export const vaultRoleHint: Record<VaultRole, string> = {
  viewer: msg("Connects and reads hosts, keys and snippets"),
  editor: msg("Also adds, changes and removes items"),
  manager: msg("Also decides who has access and rotates the key"),
};

export const VAULT_ROLES: VaultRole[] = ["viewer", "editor", "manager"];

export const teamRoleLabel: Record<TeamRole, string> = {
  member: msg("Member"),
  admin: msg("Admin"),
  owner: msg("Owner"),
};

export const teamRoleHint: Record<TeamRole, string> = {
  member: msg("Uses the vaults they were given access to"),
  admin: msg("Also invites, removes members and creates vaults"),
  owner: msg("Everything, including deleting the team"),
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
