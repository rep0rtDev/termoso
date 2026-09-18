package com.termoso.android.ui.terminal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Groups
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.PowerSettingsNew
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.TerminalSession
import com.termoso.android.ui.components.ActionRow
import com.termoso.android.ui.components.HostAvatar
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.closeHostLabel
import com.termoso.android.ui.components.stateLabel
import com.termoso.android.ui.components.transportLabel
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.QuickTarget
import com.termoso.core.SessionState
import kotlinx.coroutines.launch

/** `user@host[:port]` / `telnet://host[:port]` — what quick connect accepts back. */
fun quickTargetText(q: QuickTarget): String {
    val telnet = q.protocol.equals("telnet", true)
    val default = if (telnet) 23 else 22
    val port = if (q.port.toInt() == default) "" else ":${q.port}"
    return if (telnet) "telnet://${q.host}$port" else "${q.username}@${q.host}$port"
}

/** How many of [sessions] besides [session] point at the same saved host. */
fun siblingsOf(session: TerminalSession, sessions: List<TerminalSession>): List<TerminalSession> {
    val hostId = session.hostId ?: return emptyList()
    return sessions.filter { it.hostId == hostId && it.id != session.id }
}

/**
 * The `⋯` menu of a terminal tab: what else to open for this target (another
 * tab, SFTP, port forwarding), where its host lives (edit / add to hosts), the
 * panel and key customisation shortcuts, and the ways to close it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SessionActionsSheet(
    shell: ShellViewModel,
    session: TerminalSession,
    sessions: List<TerminalSession>,
    onLive: () -> Unit,
    onPanel: () -> Unit,
    onCustomizeKeys: () -> Unit,
    onNewSession: () -> Unit,
    onSftp: (String) -> Unit,
    onOpenSftp: (String) -> Unit,
    onForward: (String) -> Unit,
    onEditHost: (String) -> Unit,
    onAddHost: (String) -> Unit,
    onClose: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val state by session.state.collectAsStateWithLifecycle()
    val title by session.title.collectAsStateWithLifecycle()
    val detected by session.detectedOs.collectAsStateWithLifecycle()
    val shared by session.share.collectAsStateWithLifecycle()
    val hostId = session.hostId
    val quick = session.quick
    val siblings = siblingsOf(session, sessions)
    val ssh = !session.isView && session.local == null && (quick?.protocol?.equals("ssh", true) ?: (hostId != null))

    fun then(action: () -> Unit): () -> Unit = { onClose(); action() }

    ModalBottomSheet(onDismissRequest = onClose) {
        Column(
            Modifier
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp)
                .padding(bottom = 24.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                HostAvatar(detected ?: session.savedOsName)
                Spacer(Modifier.width(16.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        title ?: session.label,
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        session.target + " · " + transportLabel(session.transport) + " · " + stateLabel(state),
                        style = MaterialTheme.typography.bodySmall,
                        color = if (state is SessionState.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }

            SectionLabel("Session")
            SectionCard {
                if (!session.isView) {
                    ActionRow(
                        Icons.Filled.ContentCopy,
                        "Duplicate",
                        onClick = then { scope.launch { shell.duplicateSession(session.id) } },
                    )
                    RowDivider()
                }
                if (session.reconnectable) {
                    ActionRow(
                        Icons.Filled.Refresh,
                        "Reconnect",
                        onClick = then { scope.launch { shell.sessions.reconnect(session.id) } },
                    )
                    RowDivider()
                }
                if (ssh) {
                    ActionRow(
                        Icons.Filled.FolderOpen,
                        "Open SFTP",
                        onClick = then {
                            if (hostId != null) onSftp(hostId)
                            else if (quick != null) scope.launch { shell.openSftpQuick(quick)?.let { onOpenSftp(it.id) } }
                        },
                    )
                    RowDivider()
                    if (hostId != null) {
                        ActionRow(Icons.Filled.SwapHoriz, "Port forwarding…", onClick = then { onForward(hostId) })
                        RowDivider()
                    }
                }
                ActionRow(
                    Icons.Filled.Groups,
                    when {
                        session.isView -> "Shared terminal"
                        shared != null -> "Terminal sharing · live"
                        else -> "Terminal sharing"
                    },
                    onClick = then(onLive),
                )
                if (!session.isView && session.local == null) {
                    RowDivider()
                    ActionRow(
                        Icons.Filled.Link,
                        "Copy address",
                        onClick = then {
                            copyToClipboard(context, session.target)
                            shell.notify("Address copied")
                        },
                    )
                }
            }

            if (hostId != null || quick != null) {
                SectionLabel("Host")
                SectionCard {
                    if (hostId != null) {
                        ActionRow(Icons.Filled.Edit, "Edit host", onClick = then { onEditHost(hostId) })
                    } else if (quick != null) {
                        ActionRow(Icons.Filled.Add, "Add to hosts", onClick = then { onAddHost(quickTargetText(quick)) })
                    }
                }
            }

            SectionLabel("Terminal")
            SectionCard {
                ActionRow(Icons.Filled.History, "History & themes", onClick = then(onPanel))
                RowDivider()
                ActionRow(Icons.Filled.Keyboard, "Customize keys", onClick = then(onCustomizeKeys))
                RowDivider()
                ActionRow(Icons.Filled.Add, "New session", onClick = then(onNewSession))
            }

            SectionLabel("Close")
            SectionCard {
                ActionRow(
                    Icons.Filled.Close,
                    "Close session",
                    tint = MaterialTheme.colorScheme.error,
                    onClick = then { scope.launch { shell.sessions.close(session.id) } },
                )
                if (siblings.isNotEmpty()) {
                    RowDivider()
                    ActionRow(
                        Icons.Filled.PowerSettingsNew,
                        closeHostLabel(siblings.size + 1),
                        tint = MaterialTheme.colorScheme.error,
                        onClick = then { scope.launch { shell.sessions.closeMany(siblings.map { it.id } + session.id) } },
                    )
                }
                if (sessions.size > 1) {
                    RowDivider()
                    ActionRow(
                        Icons.Filled.PowerSettingsNew,
                        "Close all sessions (${sessions.size})",
                        tint = MaterialTheme.colorScheme.error,
                        onClick = then { scope.launch { shell.sessions.closeAll() } },
                    )
                }
            }
        }
    }
}
