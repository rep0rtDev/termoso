package com.termoso.android.ui.vault

import com.termoso.core.HistoryItem
import com.termoso.core.VaultAccess
import com.termoso.core.VaultInfo
import com.termoso.core.VaultKind
import org.junit.Assert.assertEquals
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

    @Test
    fun recentShowsOnlyTheSelectedVault() {
        val local = vault("local", VaultKind.LOCAL)
        val personal = vault("personal", VaultKind.PERSONAL)
        val team = vault("team", VaultKind.TEAM)
        val history = listOf(item("personal"), item(null, null), item("team"), item("personal"), item("local"))

        assertEquals(listOf("host-personal", "host-personal"), history.recentIn(personal, 10).map { it.hostId })
        assertEquals(listOf("host-team"), history.recentIn(team, 10).map { it.hostId })
        assertEquals(listOf(null, "host-local"), history.recentIn(local, 10).map { it.hostId })
        assertEquals(1, history.recentIn(personal, 1).size)
        assertTrue(history.recentIn(null, 10).isEmpty())
    }

    @Test
    fun aVaultWaitingForItsKeyHasNoRecent() {
        val team = vault("team", VaultKind.TEAM).copy(locked = true)
        val history = listOf(item("team"), item("team"))

        assertTrue(history.recentIn(team, 10).isEmpty())
        assertEquals(2, history.recentIn(team.copy(locked = false), 10).size)
    }
}
