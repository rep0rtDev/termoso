import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as ipc from "./commands";
import type { GroupForm, HostChainData, HostForm, ProxyData, Settings, Uuid } from "./types";

export const keys = {
  app: ["app"] as const,
  settings: ["settings"] as const,
  vaults: ["vaults"] as const,
  defaultVault: ["vaults", "default"] as const,
  hosts: (vaultId: Uuid | null) => ["hosts", vaultId] as const,
  groups: (vaultId: Uuid | null) => ["groups", vaultId] as const,
  tags: (vaultId: Uuid | null) => ["tags", vaultId] as const,
  hostForm: (id: Uuid) => ["hostForm", id] as const,
  groupForm: (id: Uuid) => ["groupForm", id] as const,
  inherited: (groupId: Uuid | null) => ["inherited", groupId] as const,
  identities: (vaultId: Uuid | null) => ["identities", vaultId] as const,
  sshKeys: (vaultId: Uuid | null) => ["sshKeys", vaultId] as const,
  proxies: (vaultId: Uuid | null) => ["proxies", vaultId] as const,
  hostChains: (vaultId: Uuid | null) => ["hostChains", vaultId] as const,
  history: ["history"] as const,
  commandHistory: ["history", "commands"] as const,
  pfRules: (vaultId: Uuid | null) => ["pfRules", vaultId] as const,
  snippets: (vaultId: Uuid | null) => ["snippets", vaultId] as const,
  packages: (vaultId: Uuid | null) => ["packages", vaultId] as const,
  knownHosts: ["knownHosts"] as const,
  logs: ["logs"] as const,
  logBody: (id: Uuid) => ["logs", id, "body"] as const,
  bookmarks: (id: Uuid) => ["logs", id, "bookmarks"] as const,
  account: ["account"] as const,
  devices: ["account", "devices"] as const,
  vaultMembers: (id: Uuid) => ["account", "vault-members", id] as const,
  teams: ["account", "teams"] as const,
  sshid: ["account", "sshid"] as const,
  teamMembers: (id: Uuid) => ["account", "teams", id, "members"] as const,
  teamInvites: (id: Uuid) => ["account", "teams", id, "invites"] as const,
  teamPendingKeys: (id: Uuid) => ["account", "teams", id, "pending-keys"] as const,
  presence: (teamId: Uuid) => ["presence", teamId] as const,
  profile: ["account", "profile"] as const,
  serialPorts: ["serialPorts"] as const,
};

export const useAppInfo = () => useQuery({ queryKey: keys.app, queryFn: ipc.appInfo });

export const useSettings = () =>
  useQuery({ queryKey: keys.settings, queryFn: ipc.settingsGet, staleTime: Infinity });

export function useSaveSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (s: Settings) => ipc.settingsSet(s),
    onSuccess: (s) => qc.setQueryData(keys.settings, s),
  });
}

export const useVaults = () => useQuery({ queryKey: keys.vaults, queryFn: ipc.vaultsList });
export const useDefaultVault = () =>
  useQuery({ queryKey: keys.defaultVault, queryFn: ipc.vaultDefault });

export const useHosts = (vaultId: Uuid | null) =>
  useQuery({ queryKey: keys.hosts(vaultId), queryFn: () => ipc.hostsList(vaultId) });
export const useGroups = (vaultId: Uuid | null) =>
  useQuery({ queryKey: keys.groups(vaultId), queryFn: () => ipc.groupsList(vaultId) });
export const useTags = (vaultId: Uuid | null) =>
  useQuery({ queryKey: keys.tags(vaultId), queryFn: () => ipc.tagsList(vaultId) });

export const useHostForm = (id: Uuid | null) =>
  useQuery({
    queryKey: keys.hostForm(id ?? ""),
    queryFn: () => ipc.hostForm(id ?? ""),
    enabled: id !== null,
  });

export const useGroupForm = (id: Uuid | null) =>
  useQuery({
    queryKey: keys.groupForm(id ?? ""),
    queryFn: () => ipc.groupForm(id ?? ""),
    enabled: id !== null,
  });

export const useInherited = (groupId: Uuid | null) =>
  useQuery({
    queryKey: keys.inherited(groupId),
    queryFn: () => ipc.hostInherited(groupId),
  });

export const useIdentities = (vaultId: Uuid | null) =>
  useQuery({
    queryKey: keys.identities(vaultId),
    queryFn: () => ipc.identitiesList(vaultId),
  });

export const useSshKeys = (vaultId: Uuid | null) =>
  useQuery({ queryKey: keys.sshKeys(vaultId), queryFn: () => ipc.keysList(vaultId) });

export const useProxies = (vaultId: Uuid | null) =>
  useQuery({
    queryKey: keys.proxies(vaultId),
    queryFn: () => ipc.entitiesList<ProxyData>("proxy", vaultId),
  });
export const useHostChains = (vaultId: Uuid | null) =>
  useQuery({
    queryKey: keys.hostChains(vaultId),
    queryFn: () => ipc.entitiesList<HostChainData>("host_chain", vaultId),
  });

export function useSaveProxy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: { vaultId: Uuid; id: Uuid | null; data: ProxyData }) =>
      ipc.entitySave("proxy", a.vaultId, a.id, a.data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["proxies"] }),
  });
}

export function useSaveHostChain() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: { vaultId: Uuid; id: Uuid | null; data: HostChainData }) =>
      ipc.entitySave("host_chain", a.vaultId, a.id, a.data),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["hostChains"] }),
  });
}

export function useCreateTag() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: { vaultId: Uuid; label: string }) =>
      ipc.entitySave("tag", a.vaultId, null, { label: a.label }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["tags"] }),
  });
}

/** Tag edits change host cards too (labels are denormalised there). */
function useInvalidateTags() {
  const qc = useQueryClient();
  return () =>
    Promise.all(
      (["tags", "hosts", "hostForm"] as const).map((k) => qc.invalidateQueries({ queryKey: [k] })),
    );
}

export function useUpdateTag() {
  const invalidate = useInvalidateTags();
  return useMutation({
    mutationFn: (a: { id: Uuid; label: string; color: string | null }) =>
      ipc.tagUpdate(a.id, a.label, a.color),
    onSuccess: () => invalidate(),
  });
}

export function useDeleteTag() {
  const invalidate = useInvalidateTags();
  return useMutation({
    mutationFn: (id: Uuid) => ipc.tagDelete(id),
    onSuccess: () => invalidate(),
  });
}

export function useMergeTags() {
  const invalidate = useInvalidateTags();
  return useMutation({
    mutationFn: (a: { sources: Uuid[]; target: Uuid }) => ipc.tagsMerge(a.sources, a.target),
    onSuccess: () => invalidate(),
  });
}

/** Serial devices present right now; refetched on demand, never in the background. */
export const useSerialPorts = (enabled: boolean) =>
  useQuery({
    queryKey: keys.serialPorts,
    queryFn: ipc.serialPorts,
    enabled,
    staleTime: 10_000,
  });

export function useDeleteEntity(kind: "proxies" | "hostChains") {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Uuid) => ipc.entityDelete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: [kind] }),
  });
}

export const useHistory = () =>
  useQuery({ queryKey: keys.history, queryFn: () => ipc.historyConnections(50) });
export const useCommandHistory = () =>
  useQuery({ queryKey: keys.commandHistory, queryFn: () => ipc.historyCommands(1000) });
export function useDeleteHistoryItem() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: Uuid) => ipc.historyDelete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.history }),
  });
}
export function useClearCommandHistory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => ipc.historyClearCommands(),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.history }),
  });
}

export const usePfRules = (vaultId: Uuid | null) =>
  useQuery({ queryKey: keys.pfRules(vaultId), queryFn: () => ipc.pfRules(vaultId) });
export const useSnippets = (vaultId: Uuid | null) =>
  useQuery({ queryKey: keys.snippets(vaultId), queryFn: () => ipc.snippetsList(vaultId) });
export const usePackages = (vaultId: Uuid | null) =>
  useQuery({ queryKey: keys.packages(vaultId), queryFn: () => ipc.snippetPackages(vaultId) });
export const useKnownHosts = () =>
  useQuery({ queryKey: keys.knownHosts, queryFn: ipc.knownHostsList });
export const useLogs = () => useQuery({ queryKey: keys.logs, queryFn: ipc.logsList });
export const useLogBody = (id: Uuid | null) =>
  useQuery({
    queryKey: keys.logBody(id ?? ""),
    queryFn: () => ipc.logRead(id ?? ""),
    enabled: id !== null,
    staleTime: Infinity,
  });
export const useBookmarks = (id: Uuid | null) =>
  useQuery({
    queryKey: keys.bookmarks(id ?? ""),
    queryFn: () => ipc.logBookmarks(id ?? ""),
    enabled: id !== null,
  });
export const useAccount = () =>
  useQuery({ queryKey: keys.account, queryFn: ipc.accountStatus, staleTime: 5_000 });
export const useDevices = (enabled: boolean) =>
  useQuery({ queryKey: keys.devices, queryFn: ipc.accountDevices, enabled });
/** Members of a team vault; `null` (local / personal vault) asks nothing. */
export const useVaultMembers = (vaultId: Uuid | null) =>
  useQuery({
    queryKey: keys.vaultMembers(vaultId ?? ""),
    queryFn: () => ipc.accountVaultMembers(vaultId ?? ""),
    enabled: vaultId !== null,
    staleTime: 60_000,
  });

export const useTeams = (enabled: boolean) =>
  useQuery({ queryKey: keys.teams, queryFn: ipc.teamsList, enabled, staleTime: 30_000 });
export const useTeamMembers = (teamId: Uuid | null) =>
  useQuery({
    queryKey: keys.teamMembers(teamId ?? ""),
    queryFn: () => ipc.teamMembers(teamId ?? ""),
    enabled: teamId !== null,
    staleTime: 30_000,
  });
export const useTeamInvites = (teamId: Uuid | null, enabled = true) =>
  useQuery({
    queryKey: keys.teamInvites(teamId ?? ""),
    queryFn: () => ipc.teamInvites(teamId ?? ""),
    enabled: teamId !== null && enabled,
    staleTime: 30_000,
  });
/**
 * Who is connected to the team's hosts right now. Realtime notices invalidate
 * it; the slow poll only covers a dropped WebSocket.
 */
export const useTeamPresence = (teamId: Uuid | null) =>
  useQuery({
    queryKey: keys.presence(teamId ?? ""),
    queryFn: () => ipc.teamPresence(teamId ?? ""),
    enabled: teamId !== null,
    staleTime: 15_000,
    refetchInterval: 60_000,
  });
export const useProfile = (enabled: boolean) =>
  useQuery({ queryKey: keys.profile, queryFn: ipc.accountProfile, enabled, staleTime: 60_000 });
export const useTeamPendingKeys = (teamId: Uuid | null, enabled = true) =>
  useQuery({
    queryKey: keys.teamPendingKeys(teamId ?? ""),
    queryFn: () => ipc.teamPendingKeys(teamId ?? ""),
    enabled: teamId !== null && enabled,
    staleTime: 30_000,
  });

/** Re-reads account, vaults, teams, members and invites after a team mutation. */
export function useInvalidateTeam() {
  const qc = useQueryClient();
  return () => {
    void qc.invalidateQueries({ queryKey: keys.account });
    void qc.invalidateQueries({ queryKey: keys.vaults });
  };
}

/** Invalidates queries when Rust reports sync / account changes. */
export function useSyncNotices() {
  const qc = useQueryClient();
  useEffect(() => {
    let active = true;
    const un = ipc.onSyncNotice((n) => {
      if (!active) return;
      void qc.invalidateQueries({ queryKey: keys.account });
      switch (n.kind) {
        case "entitiesChanged":
          for (const k of [
            "hosts",
            "groups",
            "tags",
            "identities",
            "sshKeys",
            "hostForm",
            "groupForm",
            "inherited",
            "pfRules",
            "snippets",
            "packages",
            "knownHosts",
            "proxies",
            "hostChains",
          ]) {
            void qc.invalidateQueries({ queryKey: [k] });
          }
          break;
        case "vaultsChanged":
          void qc.invalidateQueries({ queryKey: keys.vaults });
          break;
        case "historyChanged":
          void qc.invalidateQueries({ queryKey: keys.history });
          break;
        case "logsChanged":
          void qc.invalidateQueries({ queryKey: keys.logs });
          break;
        case "presenceChanged":
          void qc.invalidateQueries({ queryKey: keys.presence(n.teamId) });
          break;
        case "signedOut":
        case "accountChanged":
          void qc.invalidateQueries({ queryKey: keys.app });
          void qc.invalidateQueries({ queryKey: keys.vaults });
          void qc.invalidateQueries({ queryKey: keys.account });
          void qc.invalidateQueries({ queryKey: ["presence"] });
          break;
        case "status":
          break;
      }
    });
    return () => {
      active = false;
      void un.then((f) => f());
    };
  }, [qc]);
}

function useInvalidateVault() {
  const qc = useQueryClient();
  return (vaultId: Uuid) =>
    Promise.all(
      (
        ["hosts", "groups", "tags", "identities", "hostForm", "groupForm", "inherited"] as const
      ).map((k) => qc.invalidateQueries({ queryKey: [k] })),
    ).then(() => qc.invalidateQueries({ queryKey: keys.hosts(vaultId) }));
}

export function useSaveHost() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: (form: HostForm) => ipc.hostSave(form),
    onSuccess: (card) => invalidate(card.vaultId),
  });
}

export function useDeleteHost() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: ({ id }: { id: Uuid; vaultId: Uuid }) => ipc.hostDelete(id),
    onSuccess: (_r, v) => invalidate(v.vaultId),
  });
}

export function useDeleteHosts() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: ({ ids }: { ids: Uuid[]; vaultId: Uuid }) => ipc.hostsDelete(ids),
    onSuccess: (_r, v) => invalidate(v.vaultId),
  });
}

export function useDuplicateHost() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: (id: Uuid) => ipc.hostDuplicate(id),
    onSuccess: (card) => invalidate(card.vaultId),
  });
}

export function useMoveHosts() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: ({ ids, groupId }: { ids: Uuid[]; groupId: Uuid | null; vaultId: Uuid }) =>
      ipc.hostsMove(ids, groupId),
    onSuccess: (_r, v) => invalidate(v.vaultId),
  });
}

export function useCopyHostsToVault() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: ({
      ids,
      vaultId,
      move,
      withCredentials,
    }: {
      ids: Uuid[];
      vaultId: Uuid;
      move: boolean;
      withCredentials: boolean;
    }) => ipc.hostsCopyToVault(ids, vaultId, move, withCredentials),
    onSuccess: (_r, v) => invalidate(v.vaultId),
  });
}

export function useSaveGroup() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: ipc.groupSave,
    onSuccess: (g) => invalidate(g.vaultId),
  });
}

export function useSaveGroupForm() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: (form: GroupForm) => ipc.groupSaveForm(form),
    onSuccess: (g) => invalidate(g.vaultId),
  });
}

export function useDuplicateGroup() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: (id: Uuid) => ipc.groupDuplicate(id),
    onSuccess: (g) => invalidate(g.vaultId),
  });
}

export function useDeleteGroup() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: ({ id, recursive }: { id: Uuid; vaultId: Uuid; recursive?: boolean }) =>
      ipc.groupDelete(id, recursive ?? false),
    onSuccess: (_r, v) => invalidate(v.vaultId),
  });
}
