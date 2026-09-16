package com.termoso.android.ui.terminal

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.TerminalSession
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.AiSuggestionCard
import kotlinx.coroutines.launch

/**
 * The sparkle key of the terminal panel: a short request → one shell command.
 * Before the account opts in the sheet only explains what leaves the phone.
 * The answer is pasted with [TerminalController.paste] (bracketed paste, no
 * Enter) so the user always reads and confirms it themselves.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AskAiSheet(
    shell: ShellViewModel,
    session: TerminalSession,
    connected: Boolean,
    onInsert: (String) -> Unit,
    onOpenAccount: () -> Unit,
    onClose: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val status by shell.ai.status.collectAsStateWithLifecycle()
    var loaded by remember { mutableStateOf(false) }
    var prompt by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var answer by remember { mutableStateOf<AiSuggestionCard?>(null) }
    var failure by remember { mutableStateOf<AiFailure?>(null) }
    val target = remember(session) { aiTargetFor(session) }

    LaunchedEffect(Unit) {
        shell.ai.refresh()
        loaded = true
    }

    fun ask() {
        val text = prompt.trim()
        if (text.isEmpty() || busy) return
        busy = true
        failure = null
        scope.launch {
            runCatching { shell.ai.ask(text, target) }
                .onSuccess { answer = it }
                .onFailure { failure = aiFailure(it) }
            busy = false
        }
    }

    ModalBottomSheet(onDismissRequest = onClose) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(Icons.Filled.AutoAwesome, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                Text("Ask AI for a command", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                status?.let { s ->
                    if (s.available && s.enabled) {
                        Text(
                            "${aiRemainingToday(s)} left today",
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            val s = status
            when {
                !shell.ai.signedIn -> EmptyState(
                    title = "Sign in to use AI suggestions",
                    hint = "Suggestions come from your Termoso server, so they need an account.",
                    action = { Button(onClick = { onClose(); onOpenAccount() }) { Text("Account") } },
                )
                s == null && !loaded -> Spacer(Modifier.height(80.dp))
                s == null -> EmptyState(
                    title = "Could not reach the server",
                    hint = "Check your connection and try again.",
                    action = { OutlinedButton(onClick = { scope.launch { shell.ai.refresh() } }) { Text("Retry") } },
                )
                !s.available -> EmptyState(
                    title = "No AI provider on this server",
                    hint = "The server operator can set TERMOSO_AI__* to a Chutes key or any OpenAI-compatible endpoint.",
                )
                !s.enabled -> {
                    Text(
                        "Describe what you want in plain words and get one shell command back. " +
                            "You read it and press Enter yourself — nothing runs on its own.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Disclosure(providerLabel = aiProviderLabel(s), confidential = s.confidential, context = aiContextLabel(target))
                    Button(
                        onClick = {
                            scope.launch {
                                runCatching { shell.ai.setEnabled(true) }.onFailure { shell.notify(it.userMessage()) }
                            }
                        },
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text("Turn on for my account") }
                }
                else -> {
                    OutlinedTextField(
                        value = prompt,
                        onValueChange = { if (it.length <= AI_MAX_PROMPT_CHARS) prompt = it },
                        label = { Text("What should the command do?") },
                        placeholder = { Text("find files over 100 MB changed this week") },
                        supportingText = { Text("${prompt.length}/$AI_MAX_PROMPT_CHARS · sends: ${aiContextLabel(target)}") },
                        minLines = 2,
                        maxLines = 4,
                        enabled = !busy,
                        modifier = Modifier.fillMaxWidth(),
                        keyboardOptions = KeyboardOptions(
                            capitalization = KeyboardCapitalization.Sentences,
                            imeAction = ImeAction.Send,
                        ),
                        keyboardActions = KeyboardActions(onSend = { ask() }),
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            aiProviderLabel(s) + if (s.confidential) " · confidential compute" else "",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.weight(1f),
                        )
                        if (busy) {
                            CircularProgressIndicator(Modifier.height(20.dp), strokeWidth = 2.dp)
                        } else {
                            Button(onClick = ::ask, enabled = prompt.isNotBlank()) { Text("Suggest") }
                        }
                    }

                    failure?.let { f ->
                        Text(f.text, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                        if (f.retry) TextButton(onClick = ::ask, enabled = prompt.isNotBlank()) { Text("Try again") }
                    }

                    answer?.let { a ->
                        if (a.command.isBlank()) {
                            Text(
                                a.explanation ?: "The model had no command for that.",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        } else {
                            Text(
                                a.command,
                                fontFamily = FontFamily.Monospace,
                                style = MaterialTheme.typography.bodyMedium,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .background(MaterialTheme.colorScheme.surfaceContainerHigh, RoundedCornerShape(8.dp))
                                    .padding(12.dp)
                                    .horizontalScroll(rememberScrollState()),
                            )
                            a.explanation?.let {
                                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Button(
                                    onClick = { onInsert(a.command); onClose() },
                                    enabled = connected,
                                    modifier = Modifier.weight(1f),
                                ) { Text("Insert") }
                                OutlinedButton(
                                    onClick = {
                                        copyToClipboard(context, a.command)
                                        shell.notify("Command copied")
                                    },
                                    modifier = Modifier.weight(1f),
                                ) { Text("Copy") }
                            }
                            Text(
                                "Insert puts the command at the prompt without running it.",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

/** What leaves the phone, in the words we would want to read ourselves. */
@Composable
private fun Disclosure(providerLabel: String, confidential: Boolean, context: String) {
    Column(
        Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceContainerHigh, RoundedCornerShape(12.dp))
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text("What leaves this phone", style = MaterialTheme.typography.labelLarge)
        Text(
            "Only your request text plus a label for the OS family (from the saved host, not the terminal) — " +
                "currently: $context. Never the terminal output, command history, host address, credentials or vault contents.",
            style = MaterialTheme.typography.bodySmall,
        )
        Text(
            "Provider: $providerLabel." + if (confidential) {
                " Runs in confidential compute: the operator cannot read your request, but the model does — this is not end-to-end encryption."
            } else {
                " The provider sees the request text."
            },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
