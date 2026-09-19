package com.termoso.android.ui.snippets

import com.termoso.android.R
import com.termoso.android.str
import com.termoso.android.ui.vault.vaultLabel
import com.termoso.core.VaultAccess
import com.termoso.core.VaultInfo

/** What is being carried across vaults; the wording of the dialog follows. */
enum class TransferSubject { Snippet, Package }

/**
 * Vaults a snippet or package from [sourceVaultId] can be copied or moved into:
 * open, writable and not the one it already lives in (Rust rejects that too).
 */
fun vaultDestinations(vaults: List<VaultInfo>, sourceVaultId: String): List<VaultInfo> =
    vaults.filter { it.id != sourceVaultId && !it.locked && it.access != VaultAccess.VIEW }

/** Body of the copy/move dialog: what lands in the target and what stays behind. */
fun transferExplanation(subject: TransferSubject, label: String, move: Boolean): String {
    val what = when (subject) {
        TransferSubject.Snippet -> "\"$label\""
        TransferSubject.Package -> str(R.string.with_its_sub_packages_and_snippets, label)
    }
    val placement = str(R.string.at_the_top_level_of_the_chosen_vault)
    return if (move) {
        str(R.string.is_removed_from_this_vault_and_placed_host, what, placement)
    } else {
        str(R.string.a_copy_of_is_placed_host_targets_are, what, placement)
    }
}

/** Snackbar after a successful copy or move. */
fun transferNotice(label: String, destination: VaultInfo, move: Boolean): String =
    if (move) str(R.string.moved_to, label, vaultLabel(destination)) else str(R.string.copied_to, label, vaultLabel(destination))
