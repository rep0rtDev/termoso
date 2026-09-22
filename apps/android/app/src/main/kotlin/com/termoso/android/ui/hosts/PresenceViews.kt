package com.termoso.android.ui.hosts

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.VaultRepository
import com.termoso.android.str
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.UserAvatar
import com.termoso.android.ui.shell.ShellViewModel
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
fun PresenceStack(
    repo: VaultRepository,
    viewers: List<HostViewer>,
    modifier: Modifier = Modifier,
    max: Int = 3,
    size: Int = 22,
) {
    val people = distinctPeople(viewers)
    if (people.isEmpty()) return
    val shown = people.take(max)
    val rest = people.size - shown.size
    val overlap = size * 3 / 11
    Row(
        modifier
            .padding(start = 8.dp)
            .semantics { contentDescription = str(R.string.connected_now, viewersSummary(viewers)) },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        shown.forEachIndexed { i, v ->
            PresenceAvatar(repo, v, size = size, modifier = Modifier.offset(x = (-overlap * i).dp))
        }
        if (rest > 0) {
            Text(
                "+$rest",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.offset(x = (-overlap * (shown.size - 1)).dp).padding(start = 3.dp),
            )
        }
    }
}

/**
 * Teammates online anywhere in a team vault, as a tappable avatar stack next to
 * a section label (the “who is around” row above Groups).
 */
@Composable
fun TeamOnlineStack(repo: VaultRepository, viewers: List<HostViewer>, onClick: () -> Unit, modifier: Modifier = Modifier) {
    if (viewers.isEmpty()) return
    PresenceStack(
        repo = repo,
        viewers = viewers,
        size = 28,
        modifier = modifier
            .clip(CircleShape)
            .clickable(onClick = onClick, onClickLabel = stringResource(R.string.connected_now_2))
            .padding(horizontal = 6.dp, vertical = 4.dp),
    )
}

/** Who is connected where in the vault: one row per teammate device and host. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TeamOnlineSheet(
    repo: VaultRepository,
    viewersByHost: Map<String, List<HostViewer>>,
    hostLabel: (String) -> String?,
    onClose: () -> Unit,
) {
    val now = rememberMinuteNow()
    ModalBottomSheet(onDismissRequest = onClose) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp)
                .padding(bottom = 24.dp),
        ) {
            Text(
                stringResource(R.string.connected_now_2),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(start = 4.dp, bottom = 4.dp),
            )
            viewersByHost.entries
                .sortedBy { (id, _) -> hostLabel(id)?.lowercase() ?: "\uFFFF" }
                .forEach { (hostId, viewers) ->
                    SectionLabel(hostLabel(hostId) ?: stringResource(R.string.unknown_host))
                    SectionCard {
                        viewers.forEachIndexed { i, v ->
                            if (i > 0) RowDivider()
                            ViewerRow(repo, v, now)
                        }
                    }
                }
        }
    }
}

@Composable
private fun PresenceAvatar(repo: VaultRepository, v: HostViewer, size: Int, modifier: Modifier = Modifier) {
    Box(
        modifier
            .size(size.dp)
            .clip(CircleShape)
            .background(MaterialTheme.colorScheme.surface)
            .padding(1.5.dp),
    ) {
        UserAvatar(
            repo = repo,
            userId = v.userId,
            tag = v.avatar,
            name = v.name,
            size = size - 3,
            shape = CircleShape,
            container = if (v.me) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.tertiary,
            textStyle = MaterialTheme.typography.labelSmall,
        )
    }
}

/**
 * "Connected now" section for a host in a team vault: one row per teammate
 * device, with the protocols it has open and for how long.
 */
@Composable
fun ConnectedNowSection(repo: VaultRepository, viewers: List<HostViewer>) {
    if (viewers.isEmpty()) return
    val now = rememberMinuteNow()
    SectionLabel(stringResource(R.string.connected_now_2))
    SectionCard {
        viewers.forEachIndexed { i, v ->
            if (i > 0) RowDivider()
            ViewerRow(repo, v, now)
        }
    }
}

@Composable
private fun ViewerRow(repo: VaultRepository, v: HostViewer, now: Instant) {
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PresenceAvatar(repo, v, size = 36)
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(
                if (v.me) stringResource(R.string.you_2, v.name) else v.name,
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
                    platformLabel(v.platform),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    "· ${v.deviceName}",
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
