package com.termoso.android.ui.terminal

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.view.HapticFeedbackConstants
import android.view.inputmethod.InputMethodManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Groups
import androidx.compose.material.icons.filled.KeyboardDoubleArrowDown
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material.icons.filled.TouchApp
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SmallFloatingActionButton
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.core.content.getSystemService
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.LiveEvent
import com.termoso.android.data.SessionEvent
import com.termoso.android.data.TerminalSession
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.HostAvatar
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.snippets.SnippetPickerSheet
import com.termoso.core.LiveEndReason
import com.termoso.core.SessionState
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * Terminal tab host: session chips on top, the active terminal in the middle,
 * the key panel above the soft keyboard. Sessions keep running when the user
 * navigates away; this screen only shows them.
 */
@Composable
fun TerminalScreen(
    shell: ShellViewModel,
    onBack: () -> Unit,
    onNewSession: () -> Unit,
    onOpenSnippets: () -> Unit,
    /** Files shared into the app, offered to the active terminal once; `null` when none. */
    pendingShare: List<Uri>? = null,
    onShareConsumed: () -> Unit = {},
) {
    val sessions by shell.sessions.sessions.collectAsStateWithLifecycle()
    val activeId by shell.sessions.activeId.collectAsStateWithLifecycle()
    val settings by shell.repo.settings.collectAsStateWithLifecycle()
    val active = sessions.firstOrNull { it.id == activeId } ?: sessions.lastOrNull()
    val scope = rememberCoroutineScope()
    val snackbar = remember { SnackbarHostState() }
    val view = LocalView.current
    val context = LocalContext.current

    // The foreground-service notification is invisible until the user grants
    // POST_NOTIFICATIONS on Android 13+; ask once, the first time a terminal is shown.
    val askNotifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    var askedNotifications by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(active != null) {
        if (active == null || askedNotifications || Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return@LaunchedEffect
        askedNotifications = true
        val granted = ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED
        if (!granted) askNotifications.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    DisposableEffect(settings.keepScreenOn, active) {
        view.keepScreenOn = settings.keepScreenOn && active != null
        onDispose { view.keepScreenOn = false }
    }

    LaunchedEffect(sessions.isEmpty()) {
        if (sessions.isEmpty()) onBack()
    }

    var liveSheet by remember { mutableStateOf(false) }
    // The sheet is per session: switching chips or losing the session closes it.
    LaunchedEffect(active?.id) { liveSheet = false }

    Scaffold(
        contentWindowInsets = WindowInsets(0),
        snackbarHost = { SnackbarHost(snackbar, Modifier.navigationBarsPadding().imePadding()) },
        containerColor = MaterialTheme.colorScheme.surface,
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).statusBarsPadding().imePadding()) {
            SessionChips(
                sessions = sessions,
                active = active,
                onBack = onBack,
                onSelect = { shell.sessions.setActive(it) },
                onClose = { id -> scope.launch { shell.sessions.close(id) } },
                onLive = { liveSheet = true },
                onNew = onNewSession,
            )
            if (active == null) {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    EmptyState(
                        title = "No sessions",
                        hint = "Connect to a host to open a terminal.",
                        icon = Icons.Filled.Terminal,
                        action = { Button(onClick = onNewSession) { Text("New connection") } },
                    )
                }
            } else {
                ActiveSession(
                    session = active,
                    shell = shell,
                    snackbar = snackbar,
                    fontSize = settings.terminalFontSize.toInt(),
                    fontFamily = settings.terminalFontFamily,
                    cursorBlink = settings.cursorBlink,
                    cursorStyle = settings.cursorStyle,
                    haptics = settings.hapticFeedback,
                    bell = settings.terminalBell,
                    onOpenSnippets = onOpenSnippets,
                    pendingShare = pendingShare,
                    onShareConsumed = onShareConsumed,
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                )
            }
        }
    }
    if (liveSheet && active != null) {
        LiveSheet(session = active, shell = shell, onClose = { liveSheet = false })
    }
}

@Composable
private fun SessionChips(
    sessions: List<TerminalSession>,
    active: TerminalSession?,
    onBack: () -> Unit,
    onSelect: (String) -> Unit,
    onClose: (String) -> Unit,
    onLive: () -> Unit,
    onNew: () -> Unit,
) {
    val activeId = active?.id
    Row(
        Modifier.fillMaxWidth().height(52.dp).background(MaterialTheme.colorScheme.surfaceContainer),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") }
        val listState = rememberLazyListState()
        // Bring the active chip into view when the selection changes and again
        // whenever the row is re-measured to a new width (dialogs, IME), which
        // resets the scroll offset; manual scrolling in between is left alone.
        LaunchedEffect(activeId, sessions.size) {
            val index = sessions.indexOfFirst { it.id == activeId }
            if (index < 0) return@LaunchedEffect
            snapshotFlow {
                val info = listState.layoutInfo
                if (info.totalItemsCount > index) info.viewportEndOffset - info.viewportStartOffset else null
            }.filterNotNull().distinctUntilChanged().collect {
                listState.animateScrollToItem(index)
            }
        }
        LazyRow(
            Modifier.weight(1f),
            state = listState,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            items(sessions, key = { it.id }) { s -> SessionChip(s, s.id == activeId) { onSelect(s.id) } }
        }
        if (active != null) {
            val shared by active.share.collectAsStateWithLifecycle()
            val live = active.isView || shared != null
            IconButton(onClick = onLive) {
                Icon(
                    Icons.Filled.Groups,
                    contentDescription = if (active.isView) "Shared terminal" else "Terminal sharing",
                    tint = if (live) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                )
            }
            IconButton(onClick = { onClose(active.id) }) { Icon(Icons.Filled.Close, contentDescription = "Close session") }
        }
        IconButton(onClick = onNew) { Icon(Icons.Filled.Add, contentDescription = "New session") }
    }
}

@Composable
private fun SessionChip(session: TerminalSession, active: Boolean, onClick: () -> Unit) {
    val state by session.state.collectAsStateWithLifecycle()
    val title by session.title.collectAsStateWithLifecycle()
    val detected by session.detectedOs.collectAsStateWithLifecycle()
    val bg = if (active) MaterialTheme.colorScheme.surfaceContainerHighest else MaterialTheme.colorScheme.surfaceContainer
    Row(
        Modifier
            .clip(RoundedCornerShape(10.dp))
            .background(bg)
            // Not focusable: a focus grab (e.g. after a dialog closes) would
            // otherwise scroll the row back to the first chip.
            .focusProperties { canFocus = false }
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (session.isView) {
            Icon(Icons.Filled.Groups, contentDescription = null, modifier = Modifier.size(26.dp))
        } else {
            HostAvatar(detected ?: session.savedOsName, size = 26)
        }
        Column {
            Text(
                session.label,
                style = MaterialTheme.typography.labelLarge,
                fontWeight = if (active) FontWeight.SemiBold else FontWeight.Normal,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.width(120.dp),
            )
            val sub = when (val s = state) {
                is SessionState.Connecting -> s.detail
                is SessionState.Connected -> title ?: session.target
                is SessionState.Closed -> "Closed"
                is SessionState.Failed -> "Failed"
            }
            Text(
                sub,
                style = MaterialTheme.typography.labelSmall,
                color = when (state) {
                    is SessionState.Failed -> MaterialTheme.colorScheme.error
                    else -> MaterialTheme.colorScheme.onSurfaceVariant
                },
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.width(120.dp),
            )
        }
    }
}

@Composable
private fun ActiveSession(
    session: TerminalSession,
    shell: ShellViewModel,
    snackbar: SnackbarHostState,
    fontSize: Int,
    fontFamily: String,
    cursorBlink: Boolean,
    cursorStyle: String,
    haptics: Boolean,
    bell: Boolean,
    onOpenSnippets: () -> Unit,
    pendingShare: List<Uri>?,
    onShareConsumed: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val view = LocalView.current
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val controller = remember(session) { TerminalController(session) }
    val state by session.state.collectAsStateWithLifecycle()
    val prompt by session.prompt.collectAsStateWithLifecycle()
    val canWrite by session.canWrite.collectAsStateWithLifecycle()

    var panelExpanded by rememberSaveable { mutableStateOf(false) }
    var imeShown by remember { mutableStateOf(false) }
    var hiddenInput by remember { mutableStateOf(false) }
    var snippetPicker by remember { mutableStateOf(false) }
    var panelSheet by remember { mutableStateOf(false) }
    var dropping by remember { mutableStateOf<DropProgress?>(null) }
    var confirmDrop by remember { mutableStateOf<List<Uri>?>(null) }
    var menuAt by remember { mutableStateOf<Pair<CellPoint, Offset>?>(null) }
    var zoomDelta by rememberSaveable { mutableStateOf(0) }
    var scrolled by remember { mutableStateOf(false) }

    LaunchedEffect(session) {
        session.events.collect { ev ->
            when (ev) {
                SessionEvent.Bell -> if (bell) {
                    view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
                }
                is SessionEvent.Clipboard -> {
                    val result = snackbar.showSnackbar(
                        message = "The remote wants to put text on your clipboard",
                        actionLabel = "Copy",
                        duration = SnackbarDuration.Long,
                    )
                    if (result == SnackbarResult.ActionPerformed) copyToClipboard(context, ev.text)
                }
            }
        }
    }
    LaunchedEffect(session) {
        session.liveEvents.collect { ev ->
            val text = when (ev) {
                is LiveEvent.Control ->
                    if (ev.canWrite) "The host let you type" else "The host took back control; view only"
                is LiveEvent.Ended -> when {
                    session.isView && ev.reason == LiveEndReason.STOPPED -> "The host stopped sharing"
                    session.isView -> "Shared terminal ended: ${ev.message}"
                    ev.reason == LiveEndReason.STOPPED -> null
                    else -> "Sharing ended: ${ev.message}"
                }
            } ?: return@collect
            snackbar.showSnackbar(text)
        }
    }

    fun tap() {
        if (haptics) view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
    }

    fun dropFiles(uris: List<Uri>) {
        if (uris.isEmpty() || dropping != null) return
        FileDrop.blocker(session)?.let { why ->
            scope.launch { snackbar.showSnackbar(why) }
            return
        }
        scope.launch {
            dropping = DropProgress("", 0, uris.size, 0, null)
            runCatching { FileDrop.send(context, session, uris) { dropping = it } }
                .onSuccess { paths ->
                    snackbar.showSnackbar(if (paths.size == 1) "Uploaded to ${paths.single()}" else "Uploaded ${paths.size} files to /tmp")
                }
                .onFailure { snackbar.showSnackbar("Upload failed: ${it.userMessage()}") }
            dropping = null
        }
    }

    // Shared files arrive while another screen may be up; offer them once this terminal is showing.
    LaunchedEffect(pendingShare) {
        val uris = pendingShare ?: return@LaunchedEffect
        onShareConsumed()
        session.state.first { it !is SessionState.Connecting }
        FileDrop.blocker(session)?.let { why ->
            snackbar.showSnackbar(why)
            return@LaunchedEffect
        }
        confirmDrop = uris
    }

    val pickFiles = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        dropFiles(uris)
    }

    fun paste() {
        val clip = context.getSystemService<ClipboardManager>()?.primaryClip
        val item = clip?.takeIf { it.itemCount > 0 }?.getItemAt(0)
        val text = item?.coerceToText(context)?.toString()
        val uri = item?.uri?.takeIf { it.scheme == "content" }
        when {
            !text.isNullOrEmpty() && (uri == null || text != uri.toString()) -> controller.paste(text)
            // An image or file on the clipboard (gallery "copy", keyboard sticker): upload it instead.
            uri != null -> dropFiles(listOf(uri))
            else -> scope.launch { snackbar.showSnackbar("Clipboard is empty") }
        }
    }

    Box(modifier) {
        Column(Modifier.fillMaxSize()) {
            if (session.isView) ViewBanner(canWrite)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                TerminalView(
                    session = session,
                    controller = controller,
                    fontSizeSp = (fontSize + zoomDelta).coerceIn(6, 40),
                    fontFamily = fontFamily,
                    cursorBlink = cursorBlink,
                    cursorStyle = cursorStyle,
                    modifier = Modifier.fillMaxSize(),
                    onFrame = { scrolled = it.frame.displayOffset > 0u },
                    onTap = { imeShown = true },
                    onLongPress = { cell, offset -> menuAt = cell to offset },
                    onZoom = { d -> zoomDelta = (zoomDelta + d).coerceIn(6 - fontSize, 40 - fontSize) },
                )
                if (scrolled) {
                    SmallFloatingActionButton(
                        onClick = { controller.scrollToBottom() },
                        modifier = Modifier.align(Alignment.BottomEnd).padding(12.dp),
                    ) {
                        Icon(Icons.Filled.KeyboardDoubleArrowDown, contentDescription = "Scroll to bottom")
                    }
                }
                menuAt?.let { (cell, offset) ->
                    val at = with(density) { DpOffset(offset.x.toDp(), offset.y.toDp()) }
                    DropdownMenu(expanded = true, onDismissRequest = { menuAt = null }, offset = at) {
                        DropdownMenuItem(text = { Text("Paste") }, onClick = { menuAt = null; paste() })
                        DropdownMenuItem(
                            text = { Text("Copy line") },
                            onClick = {
                                menuAt = null
                                val line = CellGrid(session.rust.frame()).let { g ->
                                    if (cell.row in 0 until g.rows) g.lineText(cell.row) else ""
                                }
                                copyToClipboard(context, line)
                                scope.launch { snackbar.showSnackbar("Line copied") }
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("Copy screen") },
                            onClick = {
                                menuAt = null
                                copyToClipboard(context, controller.visibleText())
                                scope.launch { snackbar.showSnackbar("Screen copied") }
                            },
                        )
                        if (FileDrop.blocker(session) == null) {
                            DropdownMenuItem(
                                text = { Text("Send file…") },
                                onClick = { menuAt = null; pickFiles.launch(arrayOf("*/*")) },
                            )
                        }
                        DropdownMenuItem(text = { Text("History & themes") }, onClick = { menuAt = null; panelSheet = true })
                    }
                }
                StateOverlay(
                    state = state,
                    target = session.target,
                    onRetry = if (session.reconnectable) {
                        { scope.launch { shell.sessions.reconnect(session.id) } }
                    } else {
                        null
                    },
                    onClose = { scope.launch { shell.sessions.close(session.id) } },
                )
            }
            KeyPanel(
                controller = controller,
                expanded = panelExpanded,
                imeShown = imeShown,
                onToggleExpanded = { panelExpanded = !panelExpanded },
                onToggleIme = {
                    val imm = context.getSystemService<InputMethodManager>()
                    val input = controller.inputView
                    if (imeShown) {
                        imm?.hideSoftInputFromWindow(view.windowToken, 0)
                        imeShown = false
                    } else if (input != null) {
                        input.requestFocus()
                        imm?.showSoftInput(input, 0)
                        imeShown = true
                    }
                },
                onHiddenInput = { hiddenInput = true },
                onSnippets = { snippetPicker = true },
                onPanel = { panelSheet = true },
                onPaste = ::paste,
                onKeyPressed = ::tap,
            )
        }
    }

    prompt?.let { pending ->
        PromptDialog(pending) { answer -> scope.launch { session.answer(pending, answer) } }
    }
    if (snippetPicker) {
        SnippetPickerSheet(
            shell = shell,
            sessionId = session.id,
            onOpenSnippets = onOpenSnippets,
            onClose = { snippetPicker = false },
        )
    }
    if (panelSheet) {
        TerminalPanelSheet(shell = shell, session = session, controller = controller, onClose = { panelSheet = false })
    }
    confirmDrop?.let { uris ->
        AlertDialog(
            onDismissRequest = { confirmDrop = null },
            title = { Text(if (uris.size == 1) "Send file to ${session.label}?" else "Send ${uris.size} files to ${session.label}?") },
            text = { Text("Copied to /tmp on the remote over this session's SSH connection; the path is typed at the prompt.") },
            confirmButton = { TextButton(onClick = { confirmDrop = null; dropFiles(uris) }) { Text("Send") } },
            dismissButton = { TextButton(onClick = { confirmDrop = null }) { Text("Cancel") } },
        )
    }
    dropping?.let { p ->
        AlertDialog(
            onDismissRequest = {},
            title = { Text(if (p.count > 1) "Sending ${p.index + 1} of ${p.count}" else "Sending file") },
            text = {
                Column {
                    Text(p.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Spacer(Modifier.height(12.dp))
                    p.fraction?.let { LinearProgressIndicator(progress = { it }, modifier = Modifier.fillMaxWidth()) }
                        ?: LinearProgressIndicator(Modifier.fillMaxWidth())
                }
            },
            confirmButton = {},
        )
    }
    if (hiddenInput) {
        HiddenInputDialog(
            onSend = { text, enter ->
                hiddenInput = false
                controller.sendRaw(text, enter)
            },
            onDismiss = { hiddenInput = false },
        )
    }
}

/** Viewer strip: read-only until the host grants control; Rust drops input either way. */
@Composable
private fun ViewBanner(canWrite: Boolean) {
    val bg = if (canWrite) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh
    val fg = if (canWrite) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier.fillMaxWidth().background(bg).padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(Icons.Filled.Visibility, contentDescription = null, tint = fg, modifier = Modifier.size(16.dp))
        Text(
            if (canWrite) "Shared terminal · you can type" else "Shared terminal · view only",
            style = MaterialTheme.typography.labelMedium,
            color = fg,
        )
    }
}

@Composable
private fun StateOverlay(state: SessionState, target: String, onRetry: (() -> Unit)?, onClose: () -> Unit) {
    when (val s = state) {
        is SessionState.Connected -> Unit
        is SessionState.Connecting -> OverlayCard {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                if (s.detail.contains("Touch your security key")) {
                    Icon(Icons.Filled.TouchApp, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(24.dp))
                } else {
                    CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                }
                Column {
                    Text(target, style = MaterialTheme.typography.titleSmall)
                    Text(s.detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
        is SessionState.Closed -> OverlayCard {
            Text("Session closed", style = MaterialTheme.typography.titleSmall)
            s.reason?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = onClose) { Text("Close") }
                if (onRetry != null) Button(onClick = onRetry) { Text("Reconnect") }
            }
        }
        is SessionState.Failed -> OverlayCard {
            Text(failureTitle(s.kind), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.error)
            if (!s.message.equals(s.kind, ignoreCase = true)) {
                Text(s.message, style = MaterialTheme.typography.bodySmall)
            }
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = onClose) { Text("Close") }
                if (onRetry != null) Button(onClick = onRetry) { Text("Retry") }
            }
        }
    }
}

private fun failureTitle(kind: String): String = when (kind) {
    "auth_failed" -> "Authentication failed"
    "host_key_rejected" -> "Host key rejected"
    "io", "network" -> "Could not connect"
    "ssh" -> "SSH error"
    "cancelled" -> "Cancelled"
    "not_found" -> "Not found"
    else -> "Connection failed"
}

@Composable
private fun OverlayCard(content: @Composable () -> Unit) {
    Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surfaceContainerHigh,
            tonalElevation = 3.dp,
        ) {
            Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) { content() }
        }
    }
}

internal fun copyToClipboard(context: android.content.Context, text: String) {
    context.getSystemService<ClipboardManager>()?.setPrimaryClip(ClipData.newPlainText("Termoso", text))
}
