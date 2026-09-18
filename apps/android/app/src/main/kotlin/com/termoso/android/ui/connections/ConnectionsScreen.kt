package com.termoso.android.ui.connections

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
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
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Groups
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.OpenInNew
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material.icons.filled.PowerSettingsNew
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
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
import androidx.compose.ui.graphics.vector.ImageVector
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
import com.termoso.android.ui.components.closeHostLabel
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.terminal.JoinLiveDialog
import com.termoso.android.ui.terminal.quickTargetText
import com.termoso.android.ui.terminal.siblingsOf
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
    onSftpHost: (String) -> Unit = {},
    onEditHost: (String) -> Unit = {},
    onAddHostFrom: (String) -> Unit = {},
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
    var topMenu by remember { mutableStateOf(false) }
    var confirmCloseAll by remember { mutableStateOf(false) }
    val open = sessions.size + sftp.size

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

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Connections") },
                actions = {
                    if (open > 0) {
                        Box {
                            IconButton(onClick = { topMenu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = "More") }
                            DropdownMenu(expanded = topMenu, onDismissRequest = { topMenu = false }) {
                                DropdownMenuItem(
                                    text = { Text("Close all sessions ($open)", color = MaterialTheme.colorScheme.error) },
                                    leadingIcon = { Icon(Icons.Filled.PowerSettingsNew, null, tint = MaterialTheme.colorScheme.error) },
                                    onClick = { topMenu = false; confirmCloseAll = true },
                                )
                            }
                        }
                    }
                },
            )
        },
    ) { padding ->
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
                placeholder = { Text("user@host:port, telnet://host or join link") },
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
                SectionLabel(if (open == 1) "Active session" else "$open active sessions")
                SectionCard {
                    sessions.forEachIndexed { i, s ->
                        if (i > 0) RowDivider()
                        ActiveSessionRow(
                            session = s,
                            siblings = siblingsOf(s, sessions),
                            sftpForHost = s.hostId?.let { id -> sftp.filter { it.hostId == id } } ?: emptyList(),
                            onOpen = { shell.sessions.setActive(s.id); onOpenTerminal() },
                            onDuplicate = { scope.launch { if (shell.duplicateSession(s.id) != null) onOpenTerminal() } },
                            onReconnect = { scope.launch { shell.sessions.reconnect(s.id) } },
                            onSftp = {
                                val hostId = s.hostId
                                val quick = s.quick
                                when {
                                    hostId != null -> onSftpHost(hostId)
                                    quick != null -> scope.launch { shell.openSftpQuick(quick)?.let { onOpenSftp(it.id) } }
                                }
                            },
                            onEditHost = { s.hostId?.let(onEditHost) },
                            onAddHost = { s.quick?.let { onAddHostFrom(quickTargetText(it)) } },
                            onClose = { scope.launch { shell.sessions.close(s.id) } },
                            onCloseHost = { ids, sftpIds ->
                                scope.launch {
                                    shell.sessions.closeMany(ids)
                                    shell.sftp.closeMany(sftpIds)
                                }
                            },
                        )
                    }
                    sftp.forEachIndexed { i, c ->
                        if (i > 0 || sessions.isNotEmpty()) RowDivider()
                        SftpRow(
                            conn = c,
                            terminalsForHost = c.hostId?.let { id -> sessions.filter { it.hostId == id } } ?: emptyList(),
                            sftpSiblings = c.hostId?.let { id -> sftp.filter { it.hostId == id && it.id != c.id } } ?: emptyList(),
                            onOpen = { onOpenSftp(c.id) },
                            onTerminal = {
                                val hostId = c.hostId
                                val quick = c.quick
                                scope.launch {
                                    val opened = when {
                                        hostId != null -> shell.connectHost(hostId)
                                        quick != null -> shell.connectQuick(quick)
                                        else -> null
                                    }
                                    if (opened != null) onOpenTerminal()
                                }
                            },
                            onEditHost = { c.hostId?.let(onEditHost) },
                            onAddHost = { c.quick?.let { onAddHostFrom(quickTargetText(it)) } },
                            onClose = { scope.launch { shell.sftp.close(c.id) } },
                            onCloseHost = { ids, sftpIds ->
                                scope.launch {
                                    shell.sessions.closeMany(ids)
                                    shell.sftp.closeMany(sftpIds)
                                }
                            },
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
                    title = "Local terminal",
                    subtitle = "A shell on this device",
                    leading = { IconTile(Icons.Filled.PhoneAndroid) },
                    modifier = Modifier.clickable {
                        scope.launch { if (shell.connectLocal() != null) onOpenTerminal() }
                    },
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
                    subtitle = "Type user@host or telnet://host above and press Go",
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
    if (confirmCloseAll) {
        ConfirmDialog(
            title = "Close all sessions?",
            text = "$open open ${if (open == 1) "connection" else "connections"} — every terminal and SFTP session is disconnected.",
            confirm = "Close all",
            onConfirm = {
                confirmCloseAll = false
                scope.launch {
                    shell.sessions.closeAll()
                    shell.sftp.closeMany(sftp.map { it.id })
                }
            },
            onDismiss = { confirmCloseAll = false },
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ActiveSessionRow(
    session: TerminalSession,
    siblings: List<TerminalSession>,
    sftpForHost: List<SftpConnection>,
    onOpen: () -> Unit,
    onDuplicate: () -> Unit,
    onReconnect: () -> Unit,
    onSftp: () -> Unit,
    onEditHost: () -> Unit,
    onAddHost: () -> Unit,
    onClose: () -> Unit,
    onCloseHost: (List<String>, List<String>) -> Unit,
) {
    val state by session.state.collectAsStateWithLifecycle()
    val detected by session.detectedOs.collectAsStateWithLifecycle()
    val title by session.title.collectAsStateWithLifecycle()
    var menu by remember { mutableStateOf(false) }
    val subtitle = when (val s = state) {
        is SessionState.Connecting -> s.detail
        is SessionState.Connected -> title ?: session.target
        is SessionState.Closed -> "Closed" + (s.reason?.let { " · $it" } ?: "")
        is SessionState.Failed -> s.message
    }
    val remote = !session.isView && session.local == null
    val ssh = remote && (session.quick?.protocol?.equals("ssh", true) ?: true)
    val hostTotal = siblings.size + 1 + sftpForHost.size
    ListRow(
        title = session.label,
        subtitle = subtitle,
        leading = {
            if (session.isView) IconTile(Icons.Filled.Groups) else HostAvatar(detected ?: session.savedOsName)
        },
        trailing = {
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = "Session actions") }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    @Composable
                    fun item(icon: ImageVector, label: String, destructive: Boolean = false, action: () -> Unit) {
                        MenuItem(icon, label, destructive) { menu = false; action() }
                    }
                    item(Icons.Filled.OpenInNew, "Open", action = onOpen)
                    if (!session.isView) item(Icons.Filled.ContentCopy, "Duplicate", action = onDuplicate)
                    if (session.reconnectable) item(Icons.Filled.Refresh, "Reconnect", action = onReconnect)
                    if (ssh) item(Icons.Filled.FolderOpen, "Open SFTP", action = onSftp)
                    if (session.hostId != null) item(Icons.Filled.Edit, "Edit host", action = onEditHost)
                    else if (session.quick != null) item(Icons.Filled.Add, "Add to hosts", action = onAddHost)
                    item(Icons.Filled.Close, "Close session", destructive = true, action = onClose)
                    if (hostTotal > 1) {
                        item(Icons.Filled.PowerSettingsNew, closeHostLabel(hostTotal), destructive = true) {
                            onCloseHost(siblings.map { it.id } + session.id, sftpForHost.map { it.id })
                        }
                    }
                }
            }
            IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = "Close session") }
        },
        modifier = Modifier.combinedClickable(onClick = onOpen, onLongClick = { menu = true }, onLongClickLabel = "Session actions"),
        titleColor = if (state is SessionState.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
    )
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SftpRow(
    conn: SftpConnection,
    terminalsForHost: List<TerminalSession>,
    sftpSiblings: List<SftpConnection>,
    onOpen: () -> Unit,
    onTerminal: () -> Unit,
    onEditHost: () -> Unit,
    onAddHost: () -> Unit,
    onClose: () -> Unit,
    onCloseHost: (List<String>, List<String>) -> Unit,
) {
    val state by conn.state.collectAsStateWithLifecycle()
    val transfers by conn.transfers.collectAsStateWithLifecycle()
    var menu by remember { mutableStateOf(false) }
    val active = transfers.count { it.status is TransferStatus.Running || it.status is TransferStatus.Queued }
    val subtitle = when (val s = state) {
        is SessionState.Connecting -> s.detail
        is SessionState.Connected -> "SFTP · ${conn.target}" + if (active > 0) " · $active transferring" else ""
        is SessionState.Closed -> "Closed" + (s.reason?.let { " · $it" } ?: "")
        is SessionState.Failed -> s.message
    }
    val hostTotal = terminalsForHost.size + sftpSiblings.size + 1
    ListRow(
        title = conn.label,
        subtitle = subtitle,
        leading = { IconTile(Icons.Filled.FolderOpen) },
        trailing = {
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = "Connection actions") }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    @Composable
                    fun item(icon: ImageVector, label: String, destructive: Boolean = false, action: () -> Unit) {
                        MenuItem(icon, label, destructive) { menu = false; action() }
                    }
                    item(Icons.Filled.OpenInNew, "Open", action = onOpen)
                    if (conn.hostId != null || conn.quick != null) item(Icons.Filled.Terminal, "Open terminal", action = onTerminal)
                    if (conn.hostId != null) item(Icons.Filled.Edit, "Edit host", action = onEditHost)
                    else if (conn.quick != null) item(Icons.Filled.Add, "Add to hosts", action = onAddHost)
                    item(Icons.Filled.Close, "Close connection", destructive = true, action = onClose)
                    if (hostTotal > 1) {
                        item(Icons.Filled.PowerSettingsNew, closeHostLabel(hostTotal), destructive = true) {
                            onCloseHost(terminalsForHost.map { it.id }, sftpSiblings.map { it.id } + conn.id)
                        }
                    }
                }
            }
            IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = "Close connection") }
        },
        modifier = Modifier.combinedClickable(onClick = onOpen, onLongClick = { menu = true }, onLongClickLabel = "Connection actions"),
        titleColor = if (state is SessionState.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
    )
}

@Composable
private fun MenuItem(icon: ImageVector, label: String, destructive: Boolean, onClick: () -> Unit) {
    val tint = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
    DropdownMenuItem(
        text = { Text(label, color = tint) },
        leadingIcon = { Icon(icon, null, tint = tint) },
        onClick = onClick,
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
