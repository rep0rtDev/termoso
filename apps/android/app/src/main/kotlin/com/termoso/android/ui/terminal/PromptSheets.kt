package com.termoso.android.ui.terminal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Security
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.PendingPrompt
import com.termoso.android.ui.keychain.LocalFido2
import com.termoso.android.ui.keychain.SecurityKeyListening
import com.termoso.android.ui.keychain.SecurityKeyPicker
import com.termoso.android.ui.keychain.waitingHint
import com.termoso.core.HostKeyChoice
import com.termoso.core.PromptAnswer
import com.termoso.core.PromptRequest

/** Routes a Rust prompt to the matching dialog; every path ends in exactly one [onAnswer]. */
@Composable
fun PromptDialog(pending: PendingPrompt, onAnswer: (PromptAnswer) -> Unit) {
    when (val req = pending.request) {
        is PromptRequest.HostKeyUnknown -> HostKeyDialog(
            title = "Unknown host",
            host = req.host,
            keyType = req.keyType,
            fingerprints = listOf("Fingerprint" to req.fingerprint),
            warning = null,
            onAnswer = onAnswer,
        )
        is PromptRequest.HostKeyChanged -> HostKeyDialog(
            title = "Host key changed",
            host = req.host,
            keyType = req.keyType,
            fingerprints = listOf("Saved" to req.oldFingerprint, "Received" to req.newFingerprint),
            warning = "The key this server presents differs from the one saved earlier. " +
                "This can mean a reinstalled server — or someone intercepting the connection. " +
                "Continue only if you know why it changed.",
            onAnswer = onAnswer,
        )
        is PromptRequest.Password -> SecretDialog(
            title = "Password",
            subtitle = "for ${req.username}",
            retry = req.retry,
            onAnswer = onAnswer,
        )
        is PromptRequest.Passphrase -> SecretDialog(
            title = "Key passphrase",
            subtitle = req.keyLabel,
            retry = req.retry,
            onAnswer = onAnswer,
        )
        is PromptRequest.KeyboardInteractive -> InteractiveDialog(req, onAnswer)
        is PromptRequest.SecurityKeyPin -> SecurityKeyPinDialog(req, onAnswer)
        is PromptRequest.SecurityKeyInsert -> SecurityKeyInsertDialog(req, onAnswer)
    }
}

@Composable
private fun HostKeyDialog(
    title: String,
    host: String,
    keyType: String,
    fingerprints: List<Pair<String, String>>,
    warning: String?,
    onAnswer: (PromptAnswer) -> Unit,
) {
    val danger = warning != null
    AlertDialog(
        onDismissRequest = { onAnswer(PromptAnswer.HostKey(HostKeyChoice.REJECT)) },
        icon = if (danger) {
            { Icon(Icons.Filled.Warning, contentDescription = null, tint = MaterialTheme.colorScheme.error) }
        } else {
            null
        },
        title = { Text(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(host, style = MaterialTheme.typography.titleSmall)
                Text(keyType, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                fingerprints.forEach { (label, fp) ->
                    Column {
                        Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Text(fp, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                    }
                }
                if (warning != null) {
                    Text(warning, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                TextButton(onClick = { onAnswer(PromptAnswer.HostKey(HostKeyChoice.ACCEPT_ONCE)) }) { Text("Once") }
                Button(onClick = { onAnswer(PromptAnswer.HostKey(HostKeyChoice.ACCEPT_AND_SAVE)) }) {
                    Text(if (danger) "Replace & continue" else "Trust & save")
                }
            }
        },
        dismissButton = {
            TextButton(onClick = { onAnswer(PromptAnswer.HostKey(HostKeyChoice.REJECT)) }) { Text("Reject") }
        },
    )
}

@Composable
private fun SecretDialog(title: String, subtitle: String, retry: Boolean, onAnswer: (PromptAnswer) -> Unit) {
    var value by remember { mutableStateOf("") }
    var remember by rememberSaveable { mutableStateOf(false) }
    var shown by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val submit = { onAnswer(PromptAnswer.Secret(value = value, remember = remember)) }

    AlertDialog(
        onDismissRequest = { onAnswer(PromptAnswer.Cancel) },
        title = { Text(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (retry) {
                    Text("That didn't work — try again.", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                }
                OutlinedTextField(
                    value = value,
                    onValueChange = { value = it },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().focusRequester(focus),
                    visualTransformation = if (shown) VisualTransformation.None else PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        imeAction = ImeAction.Done,
                        autoCorrectEnabled = false,
                        capitalization = KeyboardCapitalization.None,
                    ),
                    keyboardActions = KeyboardActions(onDone = { submit() }),
                    trailingIcon = {
                        IconButton(onClick = { shown = !shown }) {
                            Icon(
                                if (shown) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                                contentDescription = if (shown) "Hide" else "Show",
                            )
                        }
                    },
                )
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                    Checkbox(checked = remember, onCheckedChange = { remember = it })
                    Text("Save in vault", style = MaterialTheme.typography.bodyMedium)
                }
            }
        },
        confirmButton = { Button(onClick = submit) { Text("Continue") } },
        dismissButton = { TextButton(onClick = { onAnswer(PromptAnswer.Cancel) }) { Text("Cancel") } },
    )
}

/** PIN for a FIDO2 key; never offered to be saved — the token asks every time by design. */
@Composable
private fun SecurityKeyPinDialog(req: PromptRequest.SecurityKeyPin, onAnswer: (PromptAnswer) -> Unit) {
    var value by remember { mutableStateOf("") }
    var shown by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val submit = { if (value.isNotEmpty()) onAnswer(PromptAnswer.Secret(value = value, remember = false)) }

    AlertDialog(
        onDismissRequest = { onAnswer(PromptAnswer.Cancel) },
        title = { Text("Security key PIN") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "${req.keyLabel} needs the PIN of the security key to sign in.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (req.retry) {
                    val left = req.retries
                    Text(
                        when {
                            left == null -> "Wrong PIN — try again."
                            left <= 1 -> "Wrong PIN — last attempt before the key locks."
                            else -> "Wrong PIN — $left attempts left."
                        },
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                OutlinedTextField(
                    value = value,
                    onValueChange = { value = it },
                    singleLine = true,
                    label = { Text("PIN") },
                    modifier = Modifier.fillMaxWidth().focusRequester(focus),
                    visualTransformation = if (shown) VisualTransformation.None else PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        imeAction = ImeAction.Done,
                        autoCorrectEnabled = false,
                        capitalization = KeyboardCapitalization.None,
                    ),
                    keyboardActions = KeyboardActions(onDone = { submit() }),
                    trailingIcon = {
                        IconButton(onClick = { shown = !shown }) {
                            Icon(
                                if (shown) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                                contentDescription = if (shown) "Hide" else "Show",
                            )
                        }
                    },
                )
                Text(
                    "After the PIN, touch the key when it blinks.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = { Button(onClick = submit, enabled = value.isNotEmpty()) { Text("Continue") } },
        dismissButton = { TextButton(onClick = { onAnswer(PromptAnswer.Cancel) }) { Text("Cancel") } },
    )
}

/**
 * No token (or the wrong one) is attached. Keeps USB/NFC listening on while it
 * is shown and enables Retry as soon as a key shows up in the Rust registry.
 */
@Composable
private fun SecurityKeyInsertDialog(req: PromptRequest.SecurityKeyInsert, onAnswer: (PromptAnswer) -> Unit) {
    val fido2 = LocalFido2.current
    val devices by fido2.devices.collectAsStateWithLifecycle()
    val pending by fido2.usbPending.collectAsStateWithLifecycle()
    SecurityKeyListening(fido2)

    AlertDialog(
        onDismissRequest = { onAnswer(PromptAnswer.Cancel) },
        icon = { Icon(Icons.Filled.Security, contentDescription = null, tint = MaterialTheme.colorScheme.primary) },
        title = { Text(if (req.wrongDevice) "Wrong security key" else "Insert your security key") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    if (req.wrongDevice) {
                        "The attached security key does not hold the credential for ${req.keyLabel}. Attach the key it was created on."
                    } else {
                        "${req.keyLabel} lives on a FIDO2 security key. ${fido2.waitingHint()}"
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                SecurityKeyPicker(
                    devices = devices,
                    selected = null,
                    pending = pending,
                    hint = fido2.waitingHint(),
                    onSelect = {},
                    onRefresh = fido2::refreshUsb,
                )
            }
        },
        confirmButton = {
            Button(onClick = { onAnswer(PromptAnswer.Retry) }, enabled = devices.isNotEmpty()) { Text("Retry") }
        },
        dismissButton = { TextButton(onClick = { onAnswer(PromptAnswer.Cancel) }) { Text("Cancel") } },
    )
}

@Composable
private fun InteractiveDialog(req: PromptRequest.KeyboardInteractive, onAnswer: (PromptAnswer) -> Unit) {
    val answers = remember(req) { mutableStateOf(List(req.questions.size) { "" }) }
    val submit = { onAnswer(PromptAnswer.Answers(values = answers.value)) }
    AlertDialog(
        onDismissRequest = { onAnswer(PromptAnswer.Cancel) },
        title = { Text(req.name.ifBlank { "Authentication" }) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (req.instructions.isNotBlank()) {
                    Text(req.instructions, style = MaterialTheme.typography.bodySmall)
                }
                if (req.questions.isEmpty()) {
                    Text("The server only asks you to confirm.", style = MaterialTheme.typography.bodySmall)
                }
                req.questions.forEachIndexed { i, q ->
                    OutlinedTextField(
                        value = answers.value[i],
                        onValueChange = { v -> answers.value = answers.value.toMutableList().also { it[i] = v } },
                        label = { Text(q.prompt.trim().trimEnd(':')) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                        visualTransformation = if (q.echo) VisualTransformation.None else PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(
                            keyboardType = if (q.echo) KeyboardType.Text else KeyboardType.Password,
                            imeAction = if (i == req.questions.lastIndex) ImeAction.Done else ImeAction.Next,
                            autoCorrectEnabled = false,
                            capitalization = KeyboardCapitalization.None,
                        ),
                        keyboardActions = KeyboardActions(onDone = { submit() }),
                    )
                }
                Spacer(Modifier.height(4.dp))
            }
        },
        confirmButton = { Button(onClick = submit) { Text("Continue") } },
        dismissButton = { TextButton(onClick = { onAnswer(PromptAnswer.Cancel) }) { Text("Cancel") } },
    )
}

/** Password-style local input for the terminal: typed text is sent only on Send, never echoed here. */
@Composable
fun HiddenInputDialog(onSend: (String, Boolean) -> Unit, onDismiss: () -> Unit) {
    var value by remember { mutableStateOf("") }
    var withEnter by rememberSaveable { mutableStateOf(true) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Hidden input") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "Type a password or other secret; it is sent to the session as-is and not kept anywhere.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                OutlinedTextField(
                    value = value,
                    onValueChange = { value = it },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().focusRequester(focus),
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        imeAction = ImeAction.Send,
                        autoCorrectEnabled = false,
                        capitalization = KeyboardCapitalization.None,
                    ),
                    keyboardActions = KeyboardActions(onSend = { onSend(value, withEnter) }),
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = withEnter, onCheckedChange = { withEnter = it })
                    Text("Press Enter after", style = MaterialTheme.typography.bodyMedium)
                }
            }
        },
        confirmButton = { Button(onClick = { onSend(value, withEnter) }) { Text("Send") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
