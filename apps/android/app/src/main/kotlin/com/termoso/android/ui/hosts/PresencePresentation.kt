package com.termoso.android.ui.hosts

import com.termoso.core.PresenceEntryCard
import com.termoso.core.PresenceSessionCard
import com.termoso.core.TeamPresenceCard
import java.time.Instant
import java.time.OffsetDateTime
import java.time.format.DateTimeParseException

/** One teammate's device on one host: the sessions it has open there. */
data class HostViewer(
    val userId: String,
    val email: String,
    val displayName: String?,
    val deviceId: String,
    val deviceName: String,
    val platform: String,
    /** Distinct protocols, in the order they were opened. */
    val protocols: List<String>,
    /** Earliest `since` (RFC 3339) across the device's sessions on this host. */
    val since: String,
    /** This is the signed-in account (any of its devices). */
    val me: Boolean,
    /** Profile picture tag, when the person has one. */
    val avatar: String? = null,
) {
    /** Display name when set, else the email. */
    val name: String get() = displayName?.takeIf { it.isNotBlank() } ?: email
}

/** Who is on which host, keyed by host id. Devices are listed oldest connection first. */
fun viewersByHost(presence: TeamPresenceCard?): Map<String, List<HostViewer>> {
    if (presence == null || !presence.enabled) return emptyMap()
    val out = HashMap<String, MutableList<HostViewer>>()
    for (entry in presence.entries) {
        for ((hostId, sessions) in entry.sessions.groupBy { it.hostId }) {
            out.getOrPut(hostId) { mutableListOf() } += viewer(entry, sessions)
        }
    }
    return out.mapValues { (_, list) -> list.sortedBy { it.since } }
}

private fun viewer(e: PresenceEntryCard, sessions: List<PresenceSessionCard>): HostViewer {
    val sorted = sessions.sortedBy { it.since }
    return HostViewer(
        userId = e.userId,
        email = e.email,
        displayName = e.displayName,
        deviceId = e.deviceId,
        deviceName = e.deviceName,
        platform = e.platform,
        protocols = sorted.map { it.protocol }.distinct(),
        since = sorted.firstOrNull()?.since ?: e.seenAt,
        me = e.me,
        avatar = e.avatar,
    )
}

/** Distinct people (not devices) among the viewers, in first-seen order. */
fun distinctPeople(viewers: List<HostViewer>): List<HostViewer> = viewers.distinctBy { it.userId }

/** `just now`, `2 min`, `1 h 05 min`, `3 d 2 h` — how long a connection has been open. */
fun connectedFor(sinceIso: String, now: Instant = Instant.now()): String {
    val at = try {
        OffsetDateTime.parse(sinceIso).toInstant()
    } catch (_: DateTimeParseException) {
        return ""
    }
    val s = (now.epochSecond - at.epochSecond).coerceAtLeast(0)
    if (s < 60) return "just now"
    val m = s / 60
    if (m < 60) return "$m min"
    val h = m / 60
    if (h < 24) return "$h h ${(m % 60).toString().padStart(2, '0')} min"
    val d = h / 24
    return "$d d ${h % 24} h"
}

fun protocolLabel(p: String): String = when (p) {
    "ssh" -> "SSH"
    "mosh" -> "Mosh"
    "telnet" -> "Telnet"
    "sftp" -> "SFTP"
    "forward" -> "Port forwarding"
    "serial" -> "Serial"
    else -> p.uppercase()
}

fun platformLabel(p: String): String = when (p) {
    "windows" -> "Windows"
    "linux" -> "Linux"
    "macos" -> "macOS"
    "android" -> "Android"
    "ios" -> "iOS"
    "web" -> "Web"
    "cli" -> "CLI"
    else -> p
}

/** `Alice`, `Alice and Bob`, `Alice, Bob and 2 others` — for the card badge. */
fun viewersSummary(viewers: List<HostViewer>): String {
    val names = distinctPeople(viewers).map { if (it.me) "You" else it.name }
    return when (names.size) {
        0 -> ""
        1 -> names[0]
        2 -> "${names[0]} and ${names[1]}"
        else -> {
            val rest = names.size - 2
            "${names[0]}, ${names[1]} and $rest other${if (rest == 1) "" else "s"}"
        }
    }
}

/** Initial for an avatar tile. */
fun initial(name: String): String = name.trim().firstOrNull()?.uppercaseChar()?.toString() ?: "?"
