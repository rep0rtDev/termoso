import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as ipc from "./commands";
import type { HostForm, Settings, Uuid } from "./types";

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
  pfRules: (vaultId: Uuid | null) => ["pfRules", vaultId] as const,
  snippets: (vaultId: Uuid | null) => ["snippets", vaultId] as const,
  packages: (vaultId: Uuid | null) => ["packages", vaultId] as const,
  knownHosts: ["knownHosts"] as const,
  logs: ["logs"] as const,
  logBody: (id: Uuid) => ["logs", id, "body"] as const,
  bookmarks: (id: Uuid) => ["logs", id, "bookmarks"] as const,
  account: ["account"] as const,
  devices: ["account", "devices"] as const,
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
    queryFn: () => ipc.identitiesList(vaultId),
  });

export const useSshKeys = (vaultId: Uuid | null) =>
  useQuery({ queryKey: keys.sshKeys(vaultId), queryFn: () => ipc.keysList(vaultId) });

export const useHistory = () =>
  useQuery({ queryKey: keys.history, queryFn: () => ipc.historyConnections(50) });

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
            "pfRules",
            "snippets",
            "packages",
            "knownHosts",
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
        case "signedOut":
        case "accountChanged":
          void qc.invalidateQueries({ queryKey: keys.app });
          void qc.invalidateQueries({ queryKey: keys.vaults });
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
