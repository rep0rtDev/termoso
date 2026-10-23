package com.termoso.android.ui.snippets

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.PendingPrompt
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.terminal.PromptDialog
import com.termoso.core.HostItem
import com.termoso.core.PromptRequest
import com.termoso.core.QuietHostResult
import com.termoso.core.QuietRun
import com.termoso.core.QuietRunListener
import com.termoso.core.SnippetItem
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Rust quiet-run callbacks republished as flows for the UI. */
private class QuietBridge : QuietRunListener {
    private val _results = MutableStateFlow<Map<String, QuietHostResult>>(emptyMap())
    val results: StateFlow<Map<String, QuietHostResult>> = _results.asStateFlow()

    private val _done = MutableStateFlow(false)
    val done: StateFlow<Boolean> = _done.asStateFlow()

    private val _prompt = MutableStateFlow<PendingPrompt?>(null)
    val prompt: StateFlow<PendingPrompt?> = _prompt.asStateFlow()

    override fun onResult(result: QuietHostResult) {
        _results.update { it + (result.hostId to result) }
    }

    override fun onDone() {
        _done.value = true
        _prompt.value = null
    }

    override fun onPrompt(promptId: ULong, request: PromptRequest) {
        _prompt.value = PendingPrompt(promptId, request)
    }

    fun promptAnswered(id: ULong) {
        _prompt.update { if (it?.id == id) null else it }
    }
}

/**
 * Run a snippet on several hosts without opening terminals: pick hosts, fill
 * `{{variables}}`, then one `exec` channel per host and a table of
 * stdout / stderr / exit code. The desktop «Run quietly…» on a phone.
 */
@Composable
fun QuietRunDialog(shell: ShellViewModel, snippet: SnippetItem, onDismiss: () -> Unit) {
    val scope = rememberCoroutineScope()
    var hosts by remember { mutableStateOf<List<HostItem>>(emptyList()) }
    var selected by remember { mutableStateOf(snippet.targetHostIds.toSet()) }
    var vars by remember { mutableStateOf(snippet.variables.associateWith { "" }) }
    var run by remember { mutableStateOf<QuietRun?>(null) }
    val bridge = remember { QuietBridge() }
    var error by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(snippet.id) {
        hosts = runCatching { shell.repo.read { hosts(snippet.vaultId).filter { it.protocol == "ssh" } } }
            .getOrDefault(emptyList())
            .sortedBy { it.id !in snippet.targetHostIds }
    }
    DisposableEffect(Unit) { onDispose { run?.cancel() } }

    val results by bridge.results.collectAsStateWithLifecycle()
    val done by bridge.done.collectAsStateWithLifecycle()
    val prompt by bridge.prompt.collectAsStateWithLifecycle()
    val allFilled = vars.values.all { it.isNotEmpty() }
    val clipboard = LocalClipboardManager.current
    val copiedMessage = stringResource(R.string.summary_copied)

    prompt?.let { p ->
        PromptDialog(p) { answer ->
            scope.launch {
                run?.answer(p.id, answer)
                bridge.promptAnswered(p.id)
            }
        }
    }

    val current = run
    if (current == null) {
        AlertDialog(
            onDismissRequest = onDismiss,
            title = { Text(stringResource(R.string.run_quietly_title, snippet.label), maxLines = 1, overflow = TextOverflow.Ellipsis) },
            text = {
                Column(
                    Modifier.heightIn(max = 440.dp).verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        stringResource(R.string.run_quietly_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    snippet.variables.forEach { name ->
                        FormField(vars[name] ?: "", { v -> vars = vars + (name to v) }, name)
                    }
                    if (hosts.isEmpty()) {
                        Text(stringResource(R.string.no_ssh_hosts_in_this_vault), color = MaterialTheme.colorScheme.error)
                    }
                    hosts.forEach { h ->
                        CheckRow(
                            title = h.label.ifBlank { h.address },
                            subtitle = h.address,
                            checked = h.id in selected,
                            onToggle = { selected = if (h.id in selected) selected - h.id else selected + h.id },
                        )
                    }
                    error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
                }
            },
            confirmButton = {
                TextButton(
                    enabled = allFilled && selected.isNotEmpty(),
                    onClick = {
                        scope.launch {
                            val ordered = hosts.map { it.id }.filter { it in selected }
                            runCatching { shell.repo.read { startQuietRun(snippet.id, ordered, vars, bridge) } }
                                .onSuccess { run = it }
                                .onFailure { error = it.message }
                        }
                    },
                ) { Text(stringResource(R.string.run)) }
            },
            dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
        )
        return
    }

    val order = hosts.map { it.id }.filter { it in selected }
    val ok = results.values.count { it.error == null && it.exitCode == 0u }
    val failed = results.size - ok
    AlertDialog(
        onDismissRequest = { if (done) onDismiss() },
        title = {
            Column {
                Text(stringResource(R.string.run_quietly_title, snippet.label), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    if (done) {
                        stringResource(R.string.quiet_summary, order.size, ok, failed)
                    } else {
                        stringResource(R.string.quiet_progress, results.size, order.size)
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        text = {
            Column(
                Modifier.heightIn(max = 480.dp).verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                order.forEach { id ->
                    val r = results[id]
                    val label = r?.label?.ifBlank { null } ?: hosts.firstOrNull { it.id == id }?.label ?: id
                    ResultRow(label = label, result = r)
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = results.isNotEmpty(),
                onClick = {
                    val text = buildString {
                        append("$ ").append(current.command()).append("\n\n")
                        order.forEach { id ->
                            val r = results[id]
                            val label = r?.label?.ifBlank { null } ?: hosts.firstOrNull { it.id == id }?.label ?: id
                            if (r == null) {
                                append("## $label\n(pending)\n\n")
                            } else {
                                val head = r.error?.let { "## $label — error: $it" }
                                    ?: "## $label (${r.target}) — exit ${r.exitCode ?: "?"}, ${r.ms} ms"
                                append(head).append('\n')
                                if (r.stdout.isNotBlank()) append(r.stdout.trimEnd()).append('\n')
                                if (r.stderr.isNotBlank()) append("[stderr]\n").append(r.stderr.trimEnd()).append('\n')
                                append('\n')
                            }
                        }
                    }
                    clipboard.setText(AnnotatedString(text))
                    shell.notify(copiedMessage)
                },
            ) { Text(stringResource(R.string.copy_summary)) }
        },
        dismissButton = {
            TextButton(onClick = { if (done) onDismiss() else current.cancel() }) {
                Text(stringResource(if (done) R.string.close else R.string.cancel))
            }
        },
    )
}

@Composable
private fun ResultRow(label: String, result: QuietHostResult?) {
    var open by remember { mutableStateOf(false) }
    val hasOutput = result != null && (result.stdout.isNotEmpty() || result.stderr.isNotEmpty())
    Column(Modifier.fillMaxWidth().clickable(enabled = hasOutput) { open = !open }) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(label, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                val sub = when {
                    result == null -> stringResource(R.string.pending)
                    result.error != null -> result.error ?: ""
                    else -> (result.stdout.ifEmpty { result.stderr }).lineSequence().firstOrNull().orEmpty()
                }
                Text(
                    sub,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            when {
                result == null -> CircularProgressIndicator(Modifier.width(18.dp).heightIn(max = 18.dp), strokeWidth = 2.dp)
                result.error != null -> Text(stringResource(R.string.connection_failed), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.labelMedium)
                result.exitCode == 0u -> Text(stringResource(R.string.ok), color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelMedium)
                else -> Text(stringResource(R.string.exit_code, result.exitCode?.toString() ?: "?"), color = MaterialTheme.colorScheme.tertiary, style = MaterialTheme.typography.labelMedium)
            }
        }
        AnimatedVisibility(visible = open && hasOutput) {
            Column(Modifier.padding(top = 4.dp)) {
                if (result!!.stdout.isNotEmpty()) {
                    Text(result.stdout.trimEnd(), fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                }
                if (result.stderr.isNotEmpty()) {
                    Text(result.stderr.trimEnd(), fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                }
            }
        }
    }
}
