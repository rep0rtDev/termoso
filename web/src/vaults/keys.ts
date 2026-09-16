import { vaultsApi } from "@/api/endpoints";
import type { PendingVaultKey, Vault, VaultMember } from "@/api/types";
import { requireUnlocked } from "@/auth/unlock";
import { generateVaultKey, loadCrypto, openVaultKey, sealVaultKey } from "@/crypto";

/** Opens the caller's copy of a vault key; throws when the key is still pending. */
export async function openMyVaultKey(vault: Vault): Promise<string> {
  if (!vault.sealed_key)
    throw new Error("Your copy of this vault key is still pending; ask a vault manager.");
  const privateKey = await requireUnlocked();
  await loadCrypto();
  return openVaultKey(privateKey, vault.sealed_key);
}

/** Creates a fresh vault key and seals it to each recipient's public key. */
export async function newSealedVaultKey(recipients: { user_id: string; public_key: string }[]) {
  await loadCrypto();
  const vaultKey = generateVaultKey();
  return recipients.map((r) => ({
    user_id: r.user_id,
    sealed_key: sealVaultKey(r.public_key, vaultKey),
  }));
}

/** Seals the vault key for a member who joined without one (e.g. via invite). */
export async function grantPendingKey(vault: Vault, pending: PendingVaultKey): Promise<void> {
  const vaultKey = await openMyVaultKey(vault);
  await vaultsApi.upsertMember(
    vault.id,
    pending.user_id,
    pending.role,
    sealVaultKey(pending.public_key, vaultKey),
  );
}

/** Adds or re-roles a member, sealing the current vault key to them. */
export async function upsertMemberWithKey(
  vault: Vault,
  member: { user_id: string; public_key: string },
  role: VaultMember["role"],
): Promise<void> {
  const vaultKey = await openMyVaultKey(vault);
  await vaultsApi.upsertMember(
    vault.id,
    member.user_id,
    role,
    sealVaultKey(member.public_key, vaultKey),
  );
}

/** Vaults an API bridge can be given: the caller can write to them and holds the current key. */
export function bridgeEligible(vaults: Vault[]): Vault[] {
  return vaults.filter((v) => v.my_role !== "viewer" && !!v.sealed_key);
}

/**
 * Seals the caller's copy of each vault key to a bridge public key. Runs entirely
 * in this tab: the server receives sealed boxes only.
 */
export async function sealVaultKeysFor(
  bridgePublicKey: string,
  vaults: Vault[],
): Promise<{ vault_id: string; sealed_key: string }[]> {
  await requireUnlocked();
  const out = [];
  for (const v of vaults) {
    const vaultKey = await openMyVaultKey(v);
    out.push({ vault_id: v.id, sealed_key: sealVaultKey(bridgePublicKey, vaultKey) });
  }
  return out;
}

/**
 * Rotates the vault key: generates a new one locally and seals it to every
 * member who currently holds a key (pending members stay pending). Entities
 * still encrypted under the old version are re-encrypted by clients as they sync.
 */
export async function rotateVaultKey(vault: Vault, members: VaultMember[]): Promise<number> {
  await requireUnlocked();
  const sealed = await newSealedVaultKey(members.filter((m) => !m.pending));
  const r = await vaultsApi.rotateKey(vault.id, vault.key_version, sealed);
  return r.key_version;
}
