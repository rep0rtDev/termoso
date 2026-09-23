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

/**
 * The newest `max` entries that belong to `vault` (Connections tab "Recent").
 * Nothing when no vault is selected or the vault is still waiting for its
 * key: its hosts cannot be opened, so there is nothing to jump back into.
 */
fun List<HistoryItem>.recentIn(vault: VaultInfo?, max: Int): List<HistoryItem> =
    if (vault == null || vault.locked) emptyList() else filter { it.belongsTo(vault) }.take(max)
