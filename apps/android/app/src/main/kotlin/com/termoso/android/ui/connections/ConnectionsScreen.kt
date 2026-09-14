package com.termoso.android.ui.connections

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Groups
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.SftpConnection
import com.termoso.android.data.TerminalSession
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.HostAvatar
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.terminal.JoinLiveDialog
import com.termoso.core.HistoryItem
import com.termoso.core.MobileException
import com.termoso.core.SessionState
import com.termoso.core.TransferStatus
import com.termoso.core.isLiveLink
import com.termoso.core.parseTarget
import kotlinx.coroutines.launch
import java.text.DateFormat
import java.util.Date

/** Connections tab: quick connect, active terminals, ways to connect, recent sessions. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConnectionsScreen(
    shell: ShellViewModel,
    onAddHost: () -> Unit,
    onConnectHost: (String) -> Unit,
    onOpenTerminal: () -> Unit,
    onNewSftp: () -> Unit,
    onOpenSftp: (String) -> Unit,
) {
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val sessions by shell.sessions.sessions.collectAsStateWithLifecycle()
    val sftp by shell.sftp.connections.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var target by remember { mutableStateOf("") }
    var recent by remember { mutableStateOf<List<HistoryItem>>(emptyList()) }
    LaunchedEffect(revision) {
        recent = runCatching { shell.repo.read { history(10u) } }.getOrDefault(emptyList())
    }

    var joinDialog by remember { mutableStateOf(false) }

    fun join(link: String) {
        scope.launch {
            if (shell.joinLive(link) != null) {
                target = ""
                joinDialog = false
                onOpenTerminal()
            }
        }
    }

    fun connect() {
        if (isLiveLink(target.trim())) {
            join(target)
            return
        }
        val parsed = try {
            parseTarget(target)
        } catch (e: MobileException) {
            shell.notify(e.userMessage())
            return
        }
        scope.launch {
            if (shell.connectQuick(parsed) != null) {
                target = ""
                onOpenTerminal()
            }
        }
    }

    Scaffold(topBar = { TopAppBar(title = { Text("Connections") }) }) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            OutlinedTextField(
                value = target,
                onValueChange = { target = it },
                placeholder = { Text("user@host:port or join link") },
                label = { Text("Quick connect") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, capitalization = KeyboardCapitalization.None, imeAction = ImeAction.Go, autoCorrectEnabled = false),
                keyboardActions = KeyboardActions(onGo = { connect() }),
                trailingIcon = {
                    IconButton(onClick = ::connect, enabled = target.isNotBlank()) {
                        Icon(Icons.AutoMirrored.Filled.ArrowForward, contentDescription = "Connect")
                    }
                },
            )

            if (sessions.isNotEmpty() || sftp.isNotEmpty()) {
                SectionLabel("Active sessions")
                SectionCard {
                    sessions.forEachIndexed { i, s ->
                        if (i > 0) RowDivider()
                        ActiveSessionRow(
                            session = s,
                            onOpen = { shell.sessions.setActive(s.id); onOpenTerminal() },
                            onClose = { scope.launch { shell.sessions.close(s.id) } },
                        )
                    }
                    sftp.forEachIndexed { i, c ->
                        if (i > 0 || sessions.isNotEmpty()) RowDivider()
                        SftpRow(
                            conn = c,
                            onOpen = { onOpenSftp(c.id) },
                            onClose = { scope.launch { shell.sftp.close(c.id) } },
                        )
                    }
                }
            }

            SectionLabel("Ways to connect")
            SectionCard {
                ChevronRow(
                    title = "Add host",
                    subtitle = "Save a server with its credentials",
                    leading = { IconTile(Icons.Filled.Add) },
                    modifier = Modifier.clickable(onClick = onAddHost),
                )
                RowDivider()
                ChevronRow(
                    title = "SFTP",
                    subtitle = "Browse and transfer files on a host",
                    leading = { IconTile(Icons.Filled.FolderOpen) },
                    modifier = Modifier.clickable(onClick = onNewSftp),
                )
                RowDivider()
                ChevronRow(
                    title = "Join shared terminal",
                    subtitle = "Open a join link somebody sent you",
                    leading = { IconTile(Icons.Filled.Groups) },
                    modifier = Modifier.clickable { joinDialog = true },
                )
                RowDivider()
                ListRow(
                    title = "Quick connect",
                    subtitle = "Type user@host above and press Go",
                    leading = { IconTile(Icons.Filled.Terminal) },
                )
            }

            if (recent.isNotEmpty()) {
                SectionLabel("Recent")
                SectionCard {
                    recent.forEachIndexed { i, h ->
                        if (i > 0) RowDivider()
                        val hostId = h.hostId
                        ListRow(
                            title = h.label.ifBlank { h.target },
                            subtitle = historySubtitle(h),
                            leading = { IconTile(Icons.Filled.History) },
                            modifier = if (hostId != null) Modifier.clickable { onConnectHost(hostId) } else Modifier,
                            titleColor = if (h.error != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                        )
                    }
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (joinDialog) {
        JoinLiveDialog(onDismiss = { joinDialog = false }, onJoin = ::join)
    }
}

@Composable
private fun ActiveSessionRow(session: TerminalSession, onOpen: () -> Unit, onClose: () -> Unit) {
    val state by session.state.collectAsStateWithLifecycle()
    val detected by session.detectedOs.collectAsStateWithLifecycle()
    val title by session.title.collectAsStateWithLifecycle()
    val subtitle = when (val s = state) {
        is SessionState.Connecting -> s.detail
        is SessionState.Connected -> title ?: session.target
        is SessionState.Closed -> "Closed" + (s.reason?.let { " · $it" } ?: "")
        is SessionState.Failed -> s.message
    }
    ListRow(
        title = session.label,
        subtitle = subtitle,
        leading = {
            if (session.isView) IconTile(Icons.Filled.Groups) else HostAvatar(detected ?: session.savedOsName)
        },
        trailing = {
            IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = "Close session") }
        },
        modifier = Modifier.clickable(onClick = onOpen),
        titleColor = if (state is SessionState.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
    )
}

@Composable
private fun SftpRow(conn: SftpConnection, onOpen: () -> Unit, onClose: () -> Unit) {
    val state by conn.state.collectAsStateWithLifecycle()
    val transfers by conn.transfers.collectAsStateWithLifecycle()
    val active = transfers.count { it.status is TransferStatus.Running || it.status is TransferStatus.Queued }
    val subtitle = when (val s = state) {
        is SessionState.Connecting -> s.detail
        is SessionState.Connected -> "SFTP · ${conn.target}" + if (active > 0) " · $active transferring" else ""
        is SessionState.Closed -> "Closed" + (s.reason?.let { " · $it" } ?: "")
        is SessionState.Failed -> s.message
    }
    ListRow(
        title = conn.label,
        subtitle = subtitle,
        leading = { IconTile(Icons.Filled.FolderOpen) },
        trailing = {
            IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = "Close connection") }
        },
        modifier = Modifier.clickable(onClick = onOpen),
        titleColor = if (state is SessionState.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
    )
}

fun historySubtitle(h: HistoryItem): String {
    val time = DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(h.startedAt))
    val duration = h.durationSecs?.let { s ->
        when {
            s < 60u -> "${s}s"
            s < 3600u -> "${s / 60u}m"
            else -> "${s / 3600u}h ${(s % 3600u) / 60u}m"
        }
    }
    return listOfNotNull(h.target, time, duration, h.error).joinToString(" · ")
}
