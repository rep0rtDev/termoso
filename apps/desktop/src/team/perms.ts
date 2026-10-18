import { useEffect } from "react";
import { useTeams, useVaults } from "@/ipc/hooks";
import type { LocalVault, Team, Uuid } from "@/ipc/types";
import { toast } from "@/components/Snackbar";
import { tr } from "@/i18n";
import { can } from "./roles";

/**
 * Team permissions on the client. They are an attribute of a *team*, so they
 * only ever restrict actions on objects that live in that team's vaults —
 * personal and local vaults are never touched. A few permissions are not
 * tied to a vault (local terminal, AI, SSH ID, bridges, serial): those follow
 * the same strictest-team-wins rule as policies.
 *
 * Marked `enforced_by: client` in the server catalog: this is UX for managed
 * teams, not a security boundary — the server never sees vault contents.
 */
let vaults: LocalVault[] = [];
let teams: Team[] = [];

export function syncPerms(v: LocalVault[] | undefined, t: Team[] | undefined) {
  if (v) vaults = v;
  if (t) teams = t;
}

/** Keeps the module-level snapshot fresh; mount once near the app root. */
export function usePermsSync() {
  const v = useVaults();
  const t = useTeams(true);
  useEffect(() => syncPerms(v.data, t.data), [v.data, t.data]);
}

export function vaultTeam(vaultId: Uuid | null | undefined): Team | null {
  if (!vaultId) return null;
  const vault = vaults.find((x) => x.id === vaultId);
  if (!vault?.team_id) return null;
  return teams.find((t) => t.id === vault.team_id) ?? null;
}

/** Allowed for an object in `vaultId`; always true outside team vaults. */
export function canInVault(vaultId: Uuid | null | undefined, permission: string): boolean {
  const team = vaultTeam(vaultId);
  return team ? can(team, permission) : true;
}

/** Permissions without a vault (local terminal, serial, AI, SSH ID, bridges):
 *  every team the user is in must grant it. */
export function canAnywhere(permission: string): boolean {
  return teams.every((t) => can(t, permission));
}

/** Tells the user which team permission is missing. */
export function denyPerm(permission: string, vaultId?: Uuid | null) {
  const team = vaultTeam(vaultId);
  toast(
    team
      ? tr("Your role in {team} does not include “{permission}”.", {
          team: team.name,
          permission,
        })
      : tr("Your team role does not include “{permission}”.", { permission }),
    "warning",
  );
}

/** `true` when allowed; otherwise toasts and returns `false`. */
export function requirePerm(vaultId: Uuid | null | undefined, permission: string): boolean {
  if (canInVault(vaultId, permission)) return true;
  denyPerm(permission, vaultId);
  return false;
}

export function requireAnywhere(permission: string): boolean {
  if (canAnywhere(permission)) return true;
  denyPerm(permission);
  return false;
}
