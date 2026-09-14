package com.termoso.android.ui.snippets

import com.termoso.core.VaultAccess
import com.termoso.core.VaultInfo
import com.termoso.core.VaultKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class VaultTransferTest {
    private fun vault(
        id: String,
        kind: VaultKind = VaultKind.TEAM,
        access: VaultAccess = VaultAccess.EDIT,
        locked: Boolean = false,
    ) = VaultInfo(id = id, kind = kind, name = "Team $id", teamId = null, access = access, locked = locked)

    @Test
    fun destinationsExcludeSourceLockedAndViewOnlyVaults() {
        val local = vault("local", kind = VaultKind.LOCAL, access = VaultAccess.MANAGE)
        val personal = vault("personal", kind = VaultKind.PERSONAL, access = VaultAccess.MANAGE)
        val locked = vault("locked", locked = true)
        val viewOnly = vault("view", access = VaultAccess.VIEW)
        val ops = vault("ops")
        val all = listOf(local, personal, locked, viewOnly, ops)

        assertEquals(listOf(local, ops), vaultDestinations(all, personal.id))
        assertEquals(listOf(local, personal), vaultDestinations(all, ops.id))
        assertTrue(vaultDestinations(listOf(local), local.id).isEmpty())
    }

    @Test
    fun explanationsSpellOutWhatStaysBehind() {
        val copy = transferExplanation(TransferSubject.Snippet, "Disk", move = false)
        assertTrue(copy, copy.startsWith("A copy of \"Disk\""))
        assertTrue(copy, "Host targets are not copied" in copy)

        val move = transferExplanation(TransferSubject.Snippet, "Disk", move = true)
        assertTrue(move, "removed from this vault" in move)
        assertTrue(move, "startup links" in move)

        val pkg = transferExplanation(TransferSubject.Package, "Ops", move = true)
        assertTrue(pkg, "\"Ops\" with its sub-packages and snippets" in pkg)
    }

    @Test
    fun noticeNamesTheDestinationLikeTheVaultPicker() {
        assertEquals("\"Disk\" copied to Team ops", transferNotice("Disk", vault("ops"), move = false))
        assertEquals(
            "\"Disk\" moved to Personal vault",
            transferNotice("Disk", vault("p", kind = VaultKind.PERSONAL), move = true),
        )
    }
}
