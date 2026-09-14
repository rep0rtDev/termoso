package com.termoso.android.ui.snippets

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
        TransferSubject.Package -> "\"$label\" with its sub-packages and snippets"
    }
    val placement = "at the top level of the chosen vault"
    return if (move) {
        "$what is removed from this vault and placed $placement. Host targets and startup links to it stay behind."
    } else {
        "A copy of $what is placed $placement. Host targets are not copied."
    }
}

/** Snackbar after a successful copy or move. */
fun transferNotice(label: String, destination: VaultInfo, move: Boolean): String =
    "\"$label\" ${if (move) "moved" else "copied"} to ${vaultLabel(destination)}"
