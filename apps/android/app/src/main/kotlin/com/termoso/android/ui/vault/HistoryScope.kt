package com.termoso.android.ui.vault

import com.termoso.core.HistoryItem
import com.termoso.core.VaultInfo
import com.termoso.core.VaultKind

/**
 * History is kept per device, but the Vaults tab shows it per vault: a
 * connection belongs to the vault its host lives in; quick connects, local
 * shells and connections to deleted hosts have no vault and are listed
 * under the local one.
 */
fun HistoryItem.belongsTo(vault: VaultInfo): Boolean =
    if (vaultId != null) vaultId == vault.id else vault.kind == VaultKind.LOCAL
