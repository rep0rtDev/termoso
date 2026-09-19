package com.termoso.android.ui.hosts

import com.termoso.android.ResourceTest
import com.termoso.core.PresenceEntryCard
import com.termoso.core.PresenceSessionCard
import com.termoso.core.TeamPresenceCard
import java.time.Instant
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PresencePresentationTest : ResourceTest() {
    private fun session(host: String, protocol: String, since: String) =
        PresenceSessionCard(vaultId = "v", hostId = host, protocol = protocol, since = since)

    private fun entry(
        user: String,
        device: String,
        sessions: List<PresenceSessionCard>,
        name: String? = null,
        platform: String = "linux",
        me: Boolean = false,
    ) = PresenceEntryCard(
        userId = user,
        email = "$user@example.com",
        displayName = name,
        deviceId = device,
        deviceName = "dev-$device",
        platform = platform,
        sessions = sessions,
        seenAt = "2026-01-01T10:00:00Z",
        me = me,
        avatar = null,
    )

    @Test
    fun groupsDevicesPerHostOldestFirstWithDistinctProtocols() {
        val presence = TeamPresenceCard(
            enabled = true,
            entries = listOf(
                entry(
                    "alice", "a1",
                    listOf(
                        session("h1", "sftp", "2026-01-01T10:05:00Z"),
                        session("h1", "ssh", "2026-01-01T10:01:00Z"),
                        session("h1", "ssh", "2026-01-01T10:02:00Z"),
                        session("h2", "forward", "2026-01-01T10:03:00Z"),
                    ),
                    name = "Alice",
                ),
                entry("bob", "b1", listOf(session("h1", "mosh", "2026-01-01T09:59:00Z")), platform = "android", me = true),
            ),
        )

        val byHost = viewersByHost(presence)
        assertEquals(setOf("h1", "h2"), byHost.keys)

        val h1 = byHost.getValue("h1")
        assertEquals(listOf("bob", "alice"), h1.map { it.userId })
        assertEquals(listOf("ssh", "sftp"), h1[1].protocols)
        assertEquals("2026-01-01T10:01:00Z", h1[1].since)
        assertEquals("Alice", h1[1].name)
        assertEquals("bob@example.com", h1[0].name)
        assertTrue(h1[0].me)

        assertEquals(listOf("forward"), byHost.getValue("h2").single().protocols)
    }

    @Test
    fun disabledOrMissingPresenceShowsNobody() {
        val entries = listOf(entry("alice", "a1", listOf(session("h1", "ssh", "2026-01-01T10:00:00Z"))))
        assertTrue(viewersByHost(TeamPresenceCard(enabled = false, entries = entries)).isEmpty())
        assertTrue(viewersByHost(null).isEmpty())
    }

    @Test
    fun summaryCountsPeopleNotDevices() {
        fun viewer(user: String, device: String, me: Boolean = false) = HostViewer(
            userId = user, email = "$user@x", displayName = user.replaceFirstChar { it.uppercase() },
            deviceId = device, deviceName = device, platform = "linux",
            protocols = listOf("ssh"), since = "2026-01-01T10:00:00Z", me = me,
        )
        assertEquals("", viewersSummary(emptyList()))
        assertEquals("You", viewersSummary(listOf(viewer("me", "d1", me = true), viewer("me", "d2", me = true))))
        assertEquals("Alice and Bob", viewersSummary(listOf(viewer("alice", "a"), viewer("bob", "b"))))
        assertEquals(
            "Alice, Bob and 1 other",
            viewersSummary(listOf(viewer("alice", "a"), viewer("bob", "b"), viewer("carol", "c"), viewer("bob", "b2"))),
        )
        assertEquals(
            "Alice, Bob and 2 others",
            viewersSummary(listOf(viewer("alice", "a"), viewer("bob", "b"), viewer("carol", "c"), viewer("dan", "d"))),
        )
    }

    @Test
    fun connectedForBuckets() {
        val now = Instant.parse("2026-01-02T12:00:00Z")
        assertEquals("just now", connectedFor("2026-01-02T11:59:30Z", now))
        assertEquals("5 min", connectedFor("2026-01-02T11:55:00Z", now))
        assertEquals("1 h 05 min", connectedFor("2026-01-02T10:55:00Z", now))
        assertEquals("1 d 2 h", connectedFor("2026-01-01T10:00:00Z", now))
        assertEquals("just now", connectedFor("2026-01-02T12:10:00Z", now))
        assertEquals("", connectedFor("garbage", now))
    }

    @Test
    fun labels() {
        assertEquals("Port forwarding", protocolLabel("forward"))
        assertEquals("FOO", protocolLabel("foo"))
        assertEquals("macOS", platformLabel("macos"))
        assertEquals("beos", platformLabel("beos"))
    }
}
