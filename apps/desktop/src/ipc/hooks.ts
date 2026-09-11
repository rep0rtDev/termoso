import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as ipc from "./commands";
import type { HostForm, IdentityData, Settings, SshKeyData, Uuid } from "./types";

export const keys = {
  app: ["app"] as const,
  settings: ["settings"] as const,
  vaults: ["vaults"] as const,
  defaultVault: ["vaults", "default"] as const,
  hosts: (vaultId: Uuid | null) => ["hosts", vaultId] as const,
  groups: (vaultId: Uuid | null) => ["groups", vaultId] as const,
  tags: (vaultId: Uuid | null) => ["tags", vaultId] as const,
  hostForm: (id: Uuid) => ["hostForm", id] as const,
  identities: (vaultId: Uuid | null) => ["identities", vaultId] as const,
  sshKeys: (vaultId: Uuid | null) => ["sshKeys", vaultId] as const,
  history: ["history"] as const,
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

export const useIdentities = (vaultId: Uuid | null) =>
  useQuery({
    queryKey: keys.identities(vaultId),
    queryFn: async () =>
      (await ipc.entitiesList<IdentityData>("identity", vaultId)).filter((e) => e.data.is_visible),
  });

export const useSshKeys = (vaultId: Uuid | null) =>
  useQuery({
    queryKey: keys.sshKeys(vaultId),
    queryFn: () => ipc.entitiesList<SshKeyData>("ssh_key", vaultId),
  });

export const useHistory = () =>
  useQuery({ queryKey: keys.history, queryFn: () => ipc.historyConnections(50) });

function useInvalidateVault() {
  const qc = useQueryClient();
  return (vaultId: Uuid) =>
    Promise.all(
      (["hosts", "groups", "tags", "identities", "hostForm"] as const).map((k) =>
        qc.invalidateQueries({ queryKey: [k] }),
      ),
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

export function useSaveGroup() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: ipc.groupSave,
    onSuccess: (g) => invalidate(g.vaultId),
  });
}

export function useDeleteGroup() {
  const invalidate = useInvalidateVault();
  return useMutation({
    mutationFn: ({ id }: { id: Uuid; vaultId: Uuid }) => ipc.groupDelete(id),
    onSuccess: (_r, v) => invalidate(v.vaultId),
  });
}
