package com.termoso.android.ui.shell

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Cable
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material.icons.outlined.Cable
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.SftpConnection
import com.termoso.android.data.TerminalSession
import com.termoso.android.plural
import com.termoso.android.ui.components.DistroIcons
import com.termoso.android.ui.components.distroVector
import com.termoso.core.SessionState

/** One thing that is live in the background: a host (by id) or a quick / local session. */
private data class LiveTarget(val key: String, val osName: String?)

private const val MAX_OS_TILES = 2
private const val MAX_REST = 9
private const val RING = 2

/** Tile side (dp) by how many tiles are stacked: one is as big as a nav icon, more shrink. */
private fun tileSize(tiles: Int) = when (tiles) {
    1 -> 24
    2 -> 20
    else -> 16
}

/** A lone tile needs no cut-out ring; the stack keeps a ring between tiles. */
private fun ringSize(tiles: Int) = if (tiles == 1) 0 else RING

/**
 * Bottom-bar icon for the Connections tab: the plain cable while nothing is
 * open, otherwise the OS tiles of the live terminals and file browsers (one per
 * host, at most [MAX_OS_TILES], then one `+N` tile for the rest) — a glance
 * shows what is running. A single tile is full icon size; the stack shrinks as
 * it grows so it stays within the tab.
 */
@Composable
fun ConnectionsTabIcon(sessions: List<TerminalSession>, sftp: List<SftpConnection>, selected: Boolean) {
    val targets = liveTargets(sessions, sftp)
    if (targets.isEmpty()) {
        Icon(
            if (selected) Icons.Filled.Cable else Icons.Outlined.Cable,
            contentDescription = stringResource(R.string.connections),
        )
        return
    }
    val shown = targets.take(MAX_OS_TILES)
    val rest = targets.size - shown.size
    val tiles = shown.size + if (rest > 0) 1 else 0
    val tile = tileSize(tiles)
    val corner = tile * 0.3f
    val ringDp = ringSize(tiles)
    val outer = tile + 2 * ringDp
    val step = outer / 2
    val summary = plural(R.plurals.n_active_connections, targets.size, targets.size)
    // The ring matches what the tile sits on, so overlaps read as cut-outs.
    val ring =
        if (selected) MaterialTheme.colorScheme.secondaryContainer
        else MaterialTheme.colorScheme.surfaceContainer
    // Fixed-width box, tiles at absolute x — the stack is exactly as wide as it looks.
    Box(
        Modifier
            .clearAndSetSemantics { contentDescription = summary }
            .width((outer + step * (tiles - 1)).dp)
            .height(outer.dp),
    ) {
        for (i in 0 until tiles) {
            Box(
                Modifier
                    .offset(x = (step * i).dp)
                    .size(outer.dp)
                    .clip(RoundedCornerShape((corner + ringDp).dp))
                    .background(ring)
                    .padding(ringDp.dp),
            ) {
                if (i < shown.size) MiniTile(shown[i].osName, tile, corner) else RestTile(rest, tile, corner)
            }
        }
    }
}

@Composable
private fun RestTile(rest: Int, tile: Int, corner: Float) {
    Box(
        Modifier
            .size(tile.dp)
            .clip(RoundedCornerShape(corner.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            "+${minOf(rest, MAX_REST)}",
            color = MaterialTheme.colorScheme.onSurface,
            fontSize = (tile / 2).sp,
            lineHeight = (tile / 2).sp,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            softWrap = false,
        )
    }
}

@Composable
private fun MiniTile(osName: String?, tile: Int, corner: Float) {
    val icon = remember(osName) { DistroIcons.forOs(osName) }
    Box(
        Modifier
            .size(tile.dp)
            .clip(RoundedCornerShape(corner.dp))
            .background(icon?.color ?: MaterialTheme.colorScheme.surfaceContainerHighest),
        contentAlignment = Alignment.Center,
    ) {
        if (icon != null) {
            Image(
                painter = rememberVectorPainter(distroVector(icon)),
                contentDescription = null,
                modifier = Modifier.size((tile * 0.6f).dp),
            )
        } else {
            Icon(
                Icons.Filled.Terminal,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.size((tile * 0.6f).dp),
            )
        }
    }
}

@Composable
private fun liveTargets(sessions: List<TerminalSession>, sftp: List<SftpConnection>): List<LiveTarget> {
    val out = LinkedHashMap<String, LiveTarget>()
    for (s in sessions) {
        val state by s.state.collectAsStateWithLifecycle()
        if (!state.isLive()) continue
        val detected by s.detectedOs.collectAsStateWithLifecycle()
        val key = s.hostId ?: "session:${s.id}"
        val os = detected ?: s.savedOsName
        // A later session may know the OS the first one did not yet.
        out[key] = LiveTarget(key, out[key]?.osName ?: os)
    }
    for (c in sftp) {
        val state by c.state.collectAsStateWithLifecycle()
        if (!state.isLive()) continue
        val key = c.hostId ?: "sftp:${c.id}"
        if (key !in out) out[key] = LiveTarget(key, c.osName)
    }
    return out.values.toList()
}

private fun SessionState.isLive() = this is SessionState.Connecting || this is SessionState.Connected
