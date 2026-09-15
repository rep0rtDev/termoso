package com.termoso.android.ui.hosts

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.theme.Emerald
import com.termoso.core.TeamPresenceCard
import com.termoso.core.VaultInfo
import java.time.Instant
import kotlinx.coroutines.delay

/**
 * Presence for the vault on screen: `null` for personal / local vaults and
 * while the snapshot has not arrived. Follows the team while composed.
 */
@Composable
fun rememberVaultPresence(shell: ShellViewModel, vault: VaultInfo?): TeamPresenceCard? {
    val teamId = vault?.teamId
    DisposableEffect(teamId) {
        if (teamId == null) return@DisposableEffect onDispose {}
        shell.presence.watch(teamId)
        onDispose { shell.presence.unwatch(teamId) }
    }
    val byTeam by shell.presence.byTeam.collectAsStateWithLifecycle()
    return teamId?.let { byTeam[it] }
}

/** A clock that ticks once a minute so "connected for" labels stay current. */
@Composable
fun rememberMinuteNow(): Instant {
    var now by remember { mutableStateOf(Instant.now()) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(30_000)
            now = Instant.now()
        }
    }
    return now
}

/** Overlapping initials of the people on a host, `+N` when there are more than [max]. */
@Composable
fun PresenceStack(viewers: List<HostViewer>, modifier: Modifier = Modifier, max: Int = 3) {
    val people = distinctPeople(viewers)
    if (people.isEmpty()) return
    val shown = people.take(max)
    val rest = people.size - shown.size
    Row(
        modifier
            .padding(start = 8.dp)
            .semantics { contentDescription = "Connected now: ${viewersSummary(viewers)}" },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        shown.forEachIndexed { i, v ->
            PresenceAvatar(v, size = 22, modifier = Modifier.offset(x = (-6 * i).dp))
        }
        if (rest > 0) {
            Text(
                "+$rest",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.offset(x = (-6 * (shown.size - 1)).dp).padding(start = 3.dp),
            )
        }
    }
}

@Composable
private fun PresenceAvatar(v: HostViewer, size: Int, modifier: Modifier = Modifier) {
    Box(
        modifier
            .size(size.dp)
            .clip(CircleShape)
            .background(MaterialTheme.colorScheme.surface)
            .padding(1.5.dp)
            .clip(CircleShape)
            .background(if (v.me) Emerald else MaterialTheme.colorScheme.tertiary),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            initial(v.name),
            color = Color.White,
            style = MaterialTheme.typography.labelSmall,
            fontWeight = FontWeight.Bold,
        )
    }
}

/**
 * "Connected now" section for a host in a team vault: one row per teammate
 * device, with the protocols it has open and for how long.
 */
@Composable
fun ConnectedNowSection(viewers: List<HostViewer>) {
    if (viewers.isEmpty()) return
    val now = rememberMinuteNow()
    SectionLabel("Connected now")
    SectionCard {
        viewers.forEachIndexed { i, v ->
            if (i > 0) RowDivider()
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                PresenceAvatar(v, size = 36)
                Spacer(Modifier.width(14.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        if (v.me) "${v.name} (you)" else v.name,
                        style = MaterialTheme.typography.bodyLarge,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        Icon(
                            if (v.platform == "android" || v.platform == "ios") Icons.Filled.PhoneAndroid else Icons.Filled.Computer,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(14.dp),
                        )
                        Text(
                            "${v.deviceName} · ${platformLabel(v.platform)}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                Spacer(Modifier.width(12.dp))
                Column(horizontalAlignment = Alignment.End) {
                    Text(
                        v.protocols.joinToString(" · ") { protocolLabel(it) },
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Text(
                        connectedFor(v.since, now),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}
