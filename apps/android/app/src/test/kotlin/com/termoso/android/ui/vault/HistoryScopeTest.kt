package com.termoso.android.ui.vault

import com.termoso.core.HistoryItem
import com.termoso.core.VaultAccess
import com.termoso.core.VaultInfo
import com.termoso.core.VaultKind
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HistoryScopeTest {
    private fun vault(id: String, kind: VaultKind) =
        VaultInfo(id = id, kind = kind, name = id, teamId = null, access = VaultAccess.MANAGE, locked = false)

    private fun item(vaultId: String?, hostId: String? = vaultId?.let { "host-$it" }) =
        HistoryItem(
            id = "h",
            vaultId = vaultId,
            hostId = hostId,
            label = "prod",
            target = "root@10.0.0.1:22",
            protocol = "ssh",
            startedAt = 0,
            durationSecs = null,
            error = null,
        )

    @Test
    fun savedHostsFollowTheirVault() {
        val local = vault("local", VaultKind.LOCAL)
        val team = vault("team", VaultKind.TEAM)

        assertTrue(item("team").belongsTo(team))
        assertFalse(item("team").belongsTo(local))
        assertFalse(item("local").belongsTo(team))
    }

    @Test
    fun vaultlessEntriesLiveUnderTheLocalVault() {
        val local = vault("local", VaultKind.LOCAL)
        val personal = vault("personal", VaultKind.PERSONAL)
        val quickConnect = item(vaultId = null, hostId = null)
        val deletedHost = item(vaultId = null, hostId = "gone")

        assertTrue(quickConnect.belongsTo(local))
        assertTrue(deletedHost.belongsTo(local))
        assertFalse(quickConnect.belongsTo(personal))
    }
}
