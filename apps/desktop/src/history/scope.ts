import type { LocalVault, VaultConnection } from "@/ipc/types";

type Vault = Pick<LocalVault, "id" | "kind">;

/**
 * Whether a connection belongs to `vault`. Saved hosts belong to the vault
 * they live in today; quick connects, local shells and deleted hosts belong
 * to the local vault only. Nothing belongs to "no vault".
 */
export function inVault(item: Pick<VaultConnection, "vault_id">, vault: Vault | null): boolean {
  if (!vault) return false;
  return item.vault_id !== null ? item.vault_id === vault.id : vault.kind === "local";
}

/** The connections of `vault`, in the order given. */
export function scopedTo<T extends Pick<VaultConnection, "vault_id">>(
  items: readonly T[],
  vault: Vault | null,
): T[] {
  return items.filter((it) => inVault(it, vault));
}
