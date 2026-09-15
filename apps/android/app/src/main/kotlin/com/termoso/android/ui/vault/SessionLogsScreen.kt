package com.termoso.android.ui.vault

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Videocam
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.sftp.formatSize
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.SessionLogCard
import kotlinx.coroutines.launch
import java.text.DateFormat
import java.util.Date

/** Terminal recordings of the selected vault, newest first. */
@Composable
fun SessionLogsScreen(shell: ShellViewModel, onBack: () -> Unit, onOpen: (String) -> Unit) {
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val vaults by shell.vaults.collectAsStateWithLifecycle()
    val selectedId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vault = vaults.firstOrNull { it.id == selectedId }
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<SessionLogCard>>(emptyList()) }
    var confirm by remember { mutableStateOf<SessionLogCard?>(null) }
    LaunchedEffect(revision, vault) {
        val v = vault ?: return@LaunchedEffect
        runCatching { shell.repo.read { sessionLogs() } }
            .onSuccess { items = it.filter { l -> l.vaultId == v.id } }
            .onFailure { shell.notify(it.userMessage()) }
    }

    SubScreen(if (vault != null) "Recordings · ${vault.name}" else "Recordings", onBack) { padding ->
        if (items.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = "No recordings",
                    hint = "Turn on Settings → Terminal → Record sessions, or enable session logging on a team vault. " +
                        "Only what the remote side prints is kept — never what you type.",
                    icon = Icons.Filled.Videocam,
                )
            }
            return@SubScreen
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(16.dp)) {
            item {
                SectionCard {
                    items.forEachIndexed { i, l ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = l.label.ifBlank { l.target },
                            subtitle = logSubtitle(l),
                            leading = { IconTile(Icons.Filled.Videocam) },
                            modifier = Modifier.clickable { onOpen(l.id) },
                        ) {
                            if (l.mine) {
                                IconButton(onClick = { confirm = l }) {
                                    Icon(Icons.Filled.Delete, contentDescription = "Delete", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    confirm?.let { l ->
        ConfirmDialog(
            title = "Delete recording?",
            text = "Removes it from this device and from every device it was synced to.",
            confirm = "Delete",
            onConfirm = {
                confirm = null
                scope.launch {
                    runCatching { shell.repo.write { deleteSessionLog(l.id) } }
                        .onFailure { shell.notify(it.userMessage()) }
                }
            },
            onDismiss = { confirm = null },
        )
    }
}

fun logSubtitle(l: SessionLogCard): String {
    val time = DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(l.startedAt))
    val duration = l.endedAt?.let { end -> formatDuration((end - l.startedAt) / 1000) }
    val who = when {
        !l.mine -> l.author ?: "teammate"
        else -> null
    }
    val state = if (l.completed) null else "recording…"
    return listOfNotNull(l.protocol.uppercase(), time, duration, formatSize(l.bytes), who, state).joinToString(" · ")
}

fun formatDuration(secs: Long): String = when {
    secs < 60 -> "${secs}s"
    secs < 3600 -> "${secs / 60}m"
    else -> "${secs / 3600}h ${(secs % 3600) / 60}m"
}

/** One recording as plain text: escape sequences stripped, remote output only. */
@Composable
fun SessionLogScreen(shell: ShellViewModel, logId: String, onBack: () -> Unit) {
    val clipboard = LocalClipboardManager.current
    var card by remember { mutableStateOf<SessionLogCard?>(null) }
    var text by remember { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(logId) {
        runCatching {
            shell.repo.read {
                card = sessionLogs().firstOrNull { it.id == logId }
                sessionLogText(logId)
            }
        }
            .onSuccess { text = stripAnsi(it) }
            .onFailure { error = it.userMessage() }
    }

    SubScreen(
        card?.let { it.label.ifBlank { it.target } } ?: "Recording",
        onBack,
        actions = {
            text?.let { t ->
                IconButton(onClick = { clipboard.setText(AnnotatedString(t)); shell.notify("Copied") }) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = "Copy")
                }
            }
        },
    ) { padding ->
        when {
            error != null -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(title = "Could not open recording", hint = error ?: "", icon = Icons.Filled.Videocam)
            }
            text == null -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            else -> SelectionContainer {
                Text(
                    text ?: "",
                    fontFamily = FontFamily.Monospace,
                    fontSize = 12.sp,
                    lineHeight = 16.sp,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(padding)
                        .verticalScroll(rememberScrollState())
                        .horizontalScroll(rememberScrollState())
                        .padding(12.dp),
                )
            }
        }
    }
}

private val CSI = Regex("\u001B\\[[0-?]*[ -/]*[@-~]")
private val OSC = Regex("\u001B\\][^\u0007\u001B]*(\u0007|\u001B\\\\)")
private val ESC_OTHER = Regex("\u001B[0-?@-Z\\\\-_]|\u001B[ -/][@-~]")
private val ERASE_TO_EOL = Regex("\u001B\\[0?K")
private const val ERASE_MARK = '\u000B'

/**
 * Drop terminal control sequences (CSI, OSC, two-byte escapes, CR, BEL) so a
 * raw recording reads as plain text. Lines that were redrawn with `\r` are
 * overwritten from column 0 the way a terminal would show them, honouring
 * erase-to-end-of-line so progress bars collapse to their final state.
 */
fun stripAnsi(raw: String): String {
    val plain = raw
        .replace(OSC, "")
        .replace(ERASE_TO_EOL, ERASE_MARK.toString())
        .replace(CSI, "")
        .replace(ESC_OTHER, "")
        .replace("\u0007", "")
    return plain.split('\n').joinToString("\n") { line ->
        val out = StringBuilder()
        for (seg in line.split('\r')) {
            val parts = seg.split(ERASE_MARK)
            val head = parts[0]
            if (head.length >= out.length) {
                out.setLength(0)
                out.append(head)
            } else {
                out.replace(0, head.length, head)
            }
            if (parts.size > 1) {
                out.setLength(head.length)
                for (i in 1 until parts.size) out.append(parts[i])
            }
        }
        out.toString().trimEnd()
    }
}
