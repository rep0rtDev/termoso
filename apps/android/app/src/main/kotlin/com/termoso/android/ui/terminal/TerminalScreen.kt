package com.termoso.android.ui.terminal

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.pm.PackageManager
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
import androidx.compose.material.icons.filled.KeyboardDoubleArrowDown
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import com.termoso.android.data.SessionEvent
import com.termoso.android.data.TerminalSession
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.HostAvatar
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.SessionState
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.launch

/**
 * Terminal tab host: session chips on top, the active terminal in the middle,
 * the key panel above the soft keyboard. Sessions keep running when the user
 * navigates away; this screen only shows them.
 */
@Composable
fun TerminalScreen(shell: ShellViewModel, onBack: () -> Unit, onNewSession: () -> Unit) {
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

    Scaffold(
        contentWindowInsets = WindowInsets(0),
        snackbarHost = { SnackbarHost(snackbar) },
        containerColor = MaterialTheme.colorScheme.surface,
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).statusBarsPadding().imePadding()) {
            SessionChips(
                sessions = sessions,
                activeId = active?.id,
                onBack = onBack,
                onSelect = { shell.sessions.setActive(it) },
                onClose = { id -> scope.launch { shell.sessions.close(id) } },
                onNew = onNewSession,
            )
            if (active == null) {
                EmptyState("No sessions", "Connect to a host to open a terminal.", Modifier.weight(1f))
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
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                )
            }
        }
    }
}

@Composable
private fun SessionChips(
    sessions: List<TerminalSession>,
    activeId: String?,
    onBack: () -> Unit,
    onSelect: (String) -> Unit,
    onClose: (String) -> Unit,
    onNew: () -> Unit,
) {
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
        if (activeId != null) {
            IconButton(onClick = { onClose(activeId) }) { Icon(Icons.Filled.Close, contentDescription = "Close session") }
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
        HostAvatar(detected ?: session.savedOsName, size = 26)
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
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val view = LocalView.current
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val controller = remember(session) { TerminalController(session) }
    val state by session.state.collectAsStateWithLifecycle()
    val prompt by session.prompt.collectAsStateWithLifecycle()

    var panelExpanded by rememberSaveable { mutableStateOf(false) }
    var imeShown by remember { mutableStateOf(false) }
    var hiddenInput by remember { mutableStateOf(false) }
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

    fun tap() {
        if (haptics) view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
    }

    fun paste() {
        val clip = context.getSystemService<ClipboardManager>()?.primaryClip
        val text = clip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
        if (text.isNullOrEmpty()) {
            scope.launch { snackbar.showSnackbar("Clipboard is empty") }
        } else {
            controller.paste(text)
        }
    }

    Box(modifier) {
        Column(Modifier.fillMaxSize()) {
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
                    }
                }
                StateOverlay(
                    state = state,
                    target = session.target,
                    onRetry = { scope.launch { shell.sessions.reconnect(session.id) } },
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
                onPaste = ::paste,
                onKeyPressed = ::tap,
            )
        }
    }

    prompt?.let { pending ->
        PromptDialog(pending) { answer -> scope.launch { session.answer(pending, answer) } }
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

@Composable
private fun StateOverlay(state: SessionState, target: String, onRetry: () -> Unit, onClose: () -> Unit) {
    when (val s = state) {
        is SessionState.Connected -> Unit
        is SessionState.Connecting -> OverlayCard {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
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
                Button(onClick = onRetry) { Text("Reconnect") }
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
                Button(onClick = onRetry) { Text("Retry") }
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

private fun copyToClipboard(context: android.content.Context, text: String) {
    context.getSystemService<ClipboardManager>()?.setPrimaryClip(ClipData.newPlainText("Termoso", text))
}
