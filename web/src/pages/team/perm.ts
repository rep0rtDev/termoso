import type { Team } from "@/api/types";

/** Does the caller hold `permission` in `team`? Owners carry `*`. Older servers
 *  (no `my_permissions`) fall back to the coarse role. */
export function can(team: Team, permission: string): boolean {
  const perms = team.my_permissions;
  if (!perms) return team.my_role === "owner" || team.my_role === "admin";
  return perms.includes("*") || perms.includes(permission);
}

export function describeAction(action: string): string {
  return action.replace(/[._]/g, " ");
}
