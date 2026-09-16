import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { authStore } from "@/auth/store";
import { accountApi, serverApi } from "./endpoints";

export const queryKeys = {
  serverInfo: ["server", "info"] as const,
  account: ["account"] as const,
  mfa: ["account", "mfa"] as const,
  devices: ["account", "devices"] as const,
  bridges: ["account", "bridges"] as const,
  securityEvents: ["account", "security-events"] as const,
  teams: ["teams"] as const,
  team: (id: string) => ["teams", id] as const,
  teamMembers: (id: string) => ["teams", id, "members"] as const,
  teamInvites: (id: string) => ["teams", id, "invites"] as const,
  teamPendingKeys: (id: string) => ["teams", id, "pending-keys"] as const,
  teamDigest: (id: string) => ["teams", id, "digest"] as const,
  vaults: ["vaults"] as const,
  vault: (id: string) => ["vaults", id] as const,
  vaultMembers: (id: string) => ["vaults", id, "members"] as const,
  vaultLogs: (id: string) => ["vaults", id, "logs"] as const,
  adminStats: ["admin", "stats"] as const,
  adminUsers: (q: string, offset: number, limit: number) =>
    ["admin", "users", q, offset, limit] as const,
  adminTeams: (q: string, offset: number, limit: number) =>
    ["admin", "teams", q, offset, limit] as const,
  adminSettings: ["admin", "settings"] as const,
};

export function useServerInfo() {
  return useQuery({
    queryKey: queryKeys.serverInfo,
    queryFn: serverApi.info,
    staleTime: 5 * 60_000,
  });
}

/** Fresh `/account` state; mirrors profile/key changes into the session store. */
export function useAccount() {
  const q = useQuery({ queryKey: queryKeys.account, queryFn: () => accountApi.get() });
  const data = q.data;
  useEffect(() => {
    if (!data) return;
    authStore.updateUser(data.user);
    authStore.updateKeys(data.keys);
  }, [data]);
  return q;
}
