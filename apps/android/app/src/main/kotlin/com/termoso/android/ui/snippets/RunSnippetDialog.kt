package com.termoso.android.ui.snippets

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.TerminalSession
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.HostItem
import com.termoso.core.SessionState
import com.termoso.core.SnippetItem
import kotlinx.coroutines.launch

/** What a run dialog did, so the caller can move to the terminal when something is now running there. */
data class SnippetLaunch(val sessions: Int, val connected: Int)

/**
 * Fill in `{{variables}}`, pick the open terminals (and target hosts to
 * connect first) and run — or paste without the trailing newline. Expansion
 * and typing happen in Rust; this only collects choices.
 */
@Composable
fun RunSnippetDialog(
    shell: ShellViewModel,
    snippet: SnippetItem,
    preselect: List<String>,
    onDismiss: () -> Unit,
    onLaunched: (SnippetLaunch) -> Unit,
) {
    val scope = rememberCoroutineScope()
    val open by shell.sessions.sessions.collectAsStateWithLifecycle()
    val live = open.filter { it.state.value !is SessionState.Closed && it.state.value !is SessionState.Failed }
    var hosts by remember { mutableStateOf<List<HostItem>>(emptyList()) }
    var vars by remember { mutableStateOf(snippet.variables.associateWith { "" }) }
    var selected by remember {
        mutableStateOf(
            preselect.toSet().ifEmpty {
                live.filter { it.hostId != null && it.hostId in snippet.targetHostIds }.map { it.id }.toSet()
                    .ifEmpty { setOfNotNull(shell.sessions.active?.id) }
            },
        )
    }
    var connect by remember { mutableStateOf(emptySet<String>()) }
    var paste by remember { mutableStateOf(false) }
    var preview by remember { mutableStateOf<String?>(null) }
    var working by remember { mutableStateOf(false) }

    LaunchedEffect(snippet.id) {
        hosts = runCatching { shell.repo.read { snippet.targetHostIds.mapNotNull { id -> runCatching { host(id) }.getOrNull() } } }
            .getOrDefault(emptyList())
        if (live.none { it.hostId in snippet.targetHostIds } && preselect.isEmpty()) {
            connect = hosts.filter { h -> live.none { it.hostId == h.id } }.map { it.id }.toSet()
        }
    }
    val allFilled = vars.values.all { it.isNotEmpty() }
    LaunchedEffect(vars, paste, allFilled) {
        preview = if (allFilled) {
            runCatching { shell.repo.read { previewSnippet(snippet.id, vars, paste) } }.getOrNull()
        } else {
            null
        }
    }

    val canRun = allFilled && (selected.isNotEmpty() || connect.isNotEmpty()) && !working

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(snippet.label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        text = {
            Column(
                Modifier.heightIn(max = 440.dp).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                snippet.variables.forEach { name ->
                    FormField(vars[name] ?: "", { v -> vars = vars + (name to v) }, name)
                }
                preview?.let {
                    Text(
                        it.trimEnd('\n'),
                        style = MaterialTheme.typography.bodySmall,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 6,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (live.isNotEmpty()) {
                    Text("Open terminals", style = MaterialTheme.typography.labelLarge)
                    live.forEach { s ->
                        CheckRow(
                            title = s.label,
                            subtitle = s.target,
                            checked = s.id in selected,
                            onToggle = { selected = if (s.id in selected) selected - s.id else selected + s.id },
                        )
                    }
                }
                val connectable = hosts.filter { h -> live.none { it.hostId == h.id } }
                if (connectable.isNotEmpty()) {
                    Text("Connect and run", style = MaterialTheme.typography.labelLarge)
                    connectable.forEach { h ->
                        CheckRow(
                            title = h.label.ifBlank { h.address },
                            subtitle = h.address,
                            checked = h.id in connect,
                            onToggle = { connect = if (h.id in connect) connect - h.id else connect + h.id },
                        )
                    }
                }
                if (live.isEmpty() && connectable.isEmpty()) {
                    Text(
                        "No open terminals. Connect to a host first, or add target hosts to the snippet.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("Paste only", style = MaterialTheme.typography.bodyLarge)
                        Text(
                            "Type the script without pressing Enter",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Switch(checked = paste, onCheckedChange = { paste = it })
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = canRun,
                onClick = {
                    working = true
                    scope.launch {
                        val targets: List<TerminalSession> = live.filter { it.id in selected }
                        var ran = 0
                        if (targets.isNotEmpty() && shell.runSnippet(snippet.id, targets, vars, paste) != null) ran = targets.size
                        var connected = 0
                        connect.forEach { hostId ->
                            if (shell.connectAndRunSnippet(hostId, snippet.id, vars, paste) != null) connected++
                        }
                        working = false
                        onLaunched(SnippetLaunch(ran, connected))
                    }
                },
            ) {
                if (working) {
                    CircularProgressIndicator(Modifier.width(18.dp).heightIn(max = 18.dp), strokeWidth = 2.dp)
                } else {
                    Text(if (paste) "Paste" else "Run")
                }
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
private fun CheckRow(title: String, subtitle: String, checked: Boolean, onToggle: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onToggle).padding(vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = checked, onCheckedChange = { onToggle() })
        Spacer(Modifier.width(4.dp))
        Column {
            Text(title, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}
