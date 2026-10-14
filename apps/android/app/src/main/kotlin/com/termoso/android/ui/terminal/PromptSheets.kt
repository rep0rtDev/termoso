package com.termoso.android.ui.terminal

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Cable
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Security
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.ModalBottomSheetProperties
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.PendingPrompt
import com.termoso.android.ui.keychain.LocalFido2
import com.termoso.android.ui.keychain.SecurityKeyListening
import com.termoso.android.ui.keychain.SecurityKeyPicker
import com.termoso.android.ui.keychain.waitingHint
import com.termoso.core.HostKeyChoice
import com.termoso.core.PromptAnswer
import com.termoso.core.PromptRequest
import kotlinx.coroutines.delay

/** Where in the connection a prompt sits; drives the step line at the top of the sheet. */
private enum class AuthStep { CONNECT, HOST_KEY, AUTH, SHELL }

private fun stepOf(request: PromptRequest): AuthStep = when (request) {
    is PromptRequest.HostKeyUnknown, is PromptRequest.HostKeyChanged, is PromptRequest.Certificate -> AuthStep.HOST_KEY
    else -> AuthStep.AUTH
}

/**
 * Routes a Rust prompt to the matching sheet; every path ends in exactly one [onAnswer].
 *
 * Prompts are bottom sheets, not dialogs: the terminal stays visible behind them,
 * the text field is focused with the keyboard up, and a tap on the scrim does
 * nothing — only Cancel gives up the connection.
 */
@Composable
fun PromptDialog(pending: PendingPrompt, onAnswer: (PromptAnswer) -> Unit) {
    when (val req = pending.request) {
        is PromptRequest.HostKeyUnknown -> HostKeySheet(
            title = stringResource(R.string.unknown_host),
            host = req.host,
            keyType = req.keyType,
            fingerprints = listOf(stringResource(R.string.fingerprint) to req.fingerprint),
            warning = null,
            onAnswer = onAnswer,
        )
        is PromptRequest.HostKeyChanged -> HostKeySheet(
            title = stringResource(R.string.host_key_changed),
            host = req.host,
            keyType = req.keyType,
            fingerprints = listOf(stringResource(R.string.saved_3) to req.oldFingerprint, stringResource(R.string.received) to req.newFingerprint),
            warning = stringResource(R.string.the_key_this_server_presents_differs_from_the),
            onAnswer = onAnswer,
        )
        is PromptRequest.Username -> SecretSheet(
            title = stringResource(R.string.username),
            subtitle = stringResource(R.string.for_, req.host),
            retry = req.retry,
            retryText = stringResource(R.string.enter_a_username_to_continue),
            secret = false,
            onAnswer = onAnswer,
        )
        is PromptRequest.Password -> SecretSheet(
            title = stringResource(R.string.password),
            subtitle = stringResource(R.string.for_, req.username),
            retry = req.retry,
            onAnswer = onAnswer,
        )
        is PromptRequest.Passphrase -> SecretSheet(
            title = stringResource(R.string.key_passphrase),
            subtitle = req.keyLabel,
            retry = req.retry,
            onAnswer = onAnswer,
        )
        is PromptRequest.KeyboardInteractive -> InteractiveSheet(req, onAnswer)
        is PromptRequest.SecurityKeyPin -> SecurityKeyPinSheet(req, onAnswer)
        is PromptRequest.SecurityKeyInsert -> SecurityKeyInsertSheet(req, onAnswer)
        is PromptRequest.Certificate -> HostKeySheet(
            title = stringResource(R.string.untrusted_certificate),
            host = req.host,
            keyType = stringResource(R.string.tls_certificate_sha_256),
            fingerprints = listOf(stringResource(R.string.fingerprint) to req.fingerprint),
            warning = null,
            onAnswer = onAnswer,
        )
    }
}

/**
 * Frame shared by every prompt: title + Cancel, the connect → host key → auth →
 * shell step line with [step] lit, the body, and the action row. Not dismissable
 * by scrim or back — an accidental tap must not drop the connection.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PromptSheet(
    title: String,
    step: AuthStep,
    danger: Boolean,
    onCancel: () -> Unit,
    actions: @Composable RowScope.() -> Unit,
    content: @Composable ColumnScope.() -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(
        skipPartiallyExpanded = true,
        confirmValueChange = { it != SheetValue.Hidden },
    )
    ModalBottomSheet(
        onDismissRequest = {},
        sheetState = sheetState,
        properties = ModalBottomSheetProperties(shouldDismissOnBackPress = false),
        dragHandle = null,
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .padding(start = 20.dp, end = 20.dp, top = 16.dp, bottom = 16.dp)
                .navigationBarsPadding()
                .imePadding(),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (danger) {
                    Icon(Icons.Filled.Warning, contentDescription = null, tint = MaterialTheme.colorScheme.error)
                    Spacer(Modifier.width(8.dp))
                }
                Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                TextButton(onClick = onCancel) { Text(stringResource(R.string.cancel)) }
            }
            StepLine(step)
            content()
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically,
                content = actions,
            )
        }
    }
}

/** connect ─ host key ─ auth ─ shell: done and current steps in the accent, the rest muted. */
@Composable
private fun StepLine(current: AuthStep) {
    val steps = listOf(
        AuthStep.CONNECT to Icons.Filled.Cable,
        AuthStep.HOST_KEY to Icons.Filled.Security,
        AuthStep.AUTH to Icons.Filled.Key,
        AuthStep.SHELL to Icons.Filled.Terminal,
    )
    val accent = MaterialTheme.colorScheme.primary
    val muted = MaterialTheme.colorScheme.outlineVariant
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        steps.forEachIndexed { i, (step, icon) ->
            val reached = step.ordinal <= current.ordinal
            StepDot(icon, reached, step == current)
            if (i < steps.lastIndex) {
                val next = steps[i + 1].first
                Box(
                    Modifier
                        .weight(1f)
                        .height(2.dp)
                        .background(if (next.ordinal <= current.ordinal) accent else muted),
                )
            }
        }
    }
}

@Composable
private fun StepDot(icon: ImageVector, reached: Boolean, current: Boolean) {
    val accent = MaterialTheme.colorScheme.primary
    val bg = when {
        current -> accent
        reached -> accent.copy(alpha = 0.25f)
        else -> MaterialTheme.colorScheme.surfaceContainerHighest
    }
    val fg = when {
        current -> MaterialTheme.colorScheme.onPrimary
        reached -> accent
        else -> MaterialTheme.colorScheme.onSurfaceVariant
    }
    Box(Modifier.size(28.dp).clip(CircleShape).background(bg), contentAlignment = Alignment.Center) {
        Icon(icon, contentDescription = null, tint = fg, modifier = Modifier.size(16.dp))
    }
}

/** Focus [focus] once the sheet has slid in; focusing a hidden field is a no-op. */
@Composable
private fun FocusOnShow(focus: FocusRequester) {
    LaunchedEffect(Unit) {
        delay(200)
        focus.requestFocus()
    }
}

@Composable
private fun HostKeySheet(
    title: String,
    host: String,
    keyType: String,
    fingerprints: List<Pair<String, String>>,
    warning: String?,
    onAnswer: (PromptAnswer) -> Unit,
) {
    val danger = warning != null
    PromptSheet(
        title = title,
        step = AuthStep.HOST_KEY,
        danger = danger,
        onCancel = { onAnswer(PromptAnswer.HostKey(HostKeyChoice.REJECT)) },
        actions = {
            TextButton(onClick = { onAnswer(PromptAnswer.HostKey(HostKeyChoice.REJECT)) }) { Text(stringResource(R.string.reject)) }
            TextButton(onClick = { onAnswer(PromptAnswer.HostKey(HostKeyChoice.ACCEPT_ONCE)) }) { Text(stringResource(R.string.once)) }
            Button(onClick = { onAnswer(PromptAnswer.HostKey(HostKeyChoice.ACCEPT_AND_SAVE)) }) {
                Text(if (danger) stringResource(R.string.replace_continue) else stringResource(R.string.trust_save))
            }
        },
    ) {
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
}

@Composable
private fun SecretSheet(
    title: String,
    subtitle: String,
    retry: Boolean,
    onAnswer: (PromptAnswer) -> Unit,
    retryText: String = stringResource(R.string.that_didnt_work_try_again),
    /** `false` for a plain value (a username): shown as typed, no eye toggle. */
    secret: Boolean = true,
) {
    var value by remember { mutableStateOf("") }
    var remember by rememberSaveable { mutableStateOf(false) }
    var shown by remember { mutableStateOf(!secret) }
    val focus = remember { FocusRequester() }
    FocusOnShow(focus)
    val submit = { onAnswer(PromptAnswer.Secret(value = value, remember = remember)) }

    PromptSheet(
        title = title,
        step = AuthStep.AUTH,
        danger = false,
        onCancel = { onAnswer(PromptAnswer.Cancel) },
        actions = { Button(onClick = submit) { Text(stringResource(R.string.continue_)) } },
    ) {
        Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        if (retry) {
            Text(retryText, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
        OutlinedTextField(
            value = value,
            onValueChange = { value = it },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().focusRequester(focus),
            visualTransformation = if (shown) VisualTransformation.None else PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(
                keyboardType = if (secret) KeyboardType.Password else KeyboardType.Ascii,
                imeAction = ImeAction.Done,
                autoCorrectEnabled = false,
                capitalization = KeyboardCapitalization.None,
            ),
            keyboardActions = KeyboardActions(onDone = { submit() }),
            trailingIcon = if (!secret) {
                null
            } else {
                { EyeToggle(shown) { shown = !shown } }
            },
        )
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            Checkbox(checked = remember, onCheckedChange = { remember = it })
            Text(stringResource(R.string.save_in_vault), style = MaterialTheme.typography.bodyMedium)
        }
    }
}

@Composable
private fun EyeToggle(shown: Boolean, onToggle: () -> Unit) {
    IconButton(onClick = onToggle) {
        Icon(
            if (shown) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
            contentDescription = if (shown) stringResource(R.string.hide) else stringResource(R.string.show),
        )
    }
}

/** PIN for a FIDO2 key; never offered to be saved — the token asks every time by design. */
@Composable
private fun SecurityKeyPinSheet(req: PromptRequest.SecurityKeyPin, onAnswer: (PromptAnswer) -> Unit) {
    var value by remember { mutableStateOf("") }
    var shown by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    FocusOnShow(focus)
    val submit = { if (value.isNotEmpty()) onAnswer(PromptAnswer.Secret(value = value, remember = false)) }

    PromptSheet(
        title = stringResource(R.string.security_key_pin),
        step = AuthStep.AUTH,
        danger = false,
        onCancel = { onAnswer(PromptAnswer.Cancel) },
        actions = { Button(onClick = submit, enabled = value.isNotEmpty()) { Text(stringResource(R.string.continue_)) } },
    ) {
        Text(
            stringResource(R.string.needs_the_pin_of_the_security_key_to, req.keyLabel),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (req.retry) {
            val left = req.retries
            Text(
                when {
                    left == null -> stringResource(R.string.wrong_pin_try_again)
                    left <= 1 -> stringResource(R.string.wrong_pin_last_attempt_before_the_key_locks)
                    else -> stringResource(R.string.wrong_pin_attempts_left, left)
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
            trailingIcon = { EyeToggle(shown) { shown = !shown } },
        )
        Text(
            stringResource(R.string.after_the_pin_touch_the_key_when_it),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/**
 * No token (or the wrong one) is attached. Keeps USB/NFC listening on while it
 * is shown and enables Retry as soon as a key shows up in the Rust registry.
 */
@Composable
private fun SecurityKeyInsertSheet(req: PromptRequest.SecurityKeyInsert, onAnswer: (PromptAnswer) -> Unit) {
    val fido2 = LocalFido2.current
    val devices by fido2.devices.collectAsStateWithLifecycle()
    val pending by fido2.usbPending.collectAsStateWithLifecycle()
    SecurityKeyListening(fido2)

    PromptSheet(
        title = if (req.wrongDevice) stringResource(R.string.wrong_security_key) else stringResource(R.string.insert_your_security_key),
        step = AuthStep.AUTH,
        danger = false,
        onCancel = { onAnswer(PromptAnswer.Cancel) },
        actions = {
            Button(onClick = { onAnswer(PromptAnswer.Retry) }, enabled = devices.isNotEmpty()) { Text(stringResource(R.string.retry)) }
        },
    ) {
        Text(
            if (req.wrongDevice) {
                stringResource(R.string.the_attached_security_key_does_not_hold_the, req.keyLabel)
            } else {
                stringResource(R.string.lives_on_a_fido2_security_key, req.keyLabel, fido2.waitingHint())
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
}

@Composable
private fun InteractiveSheet(req: PromptRequest.KeyboardInteractive, onAnswer: (PromptAnswer) -> Unit) {
    val answers = remember(req) { mutableStateOf(List(req.questions.size) { "" }) }
    val focus = remember { FocusRequester() }
    if (req.questions.isNotEmpty()) FocusOnShow(focus)
    val submit = { onAnswer(PromptAnswer.Answers(values = answers.value)) }
    PromptSheet(
        title = req.name.ifBlank { stringResource(R.string.authentication) },
        step = AuthStep.AUTH,
        danger = false,
        onCancel = { onAnswer(PromptAnswer.Cancel) },
        actions = { Button(onClick = submit) { Text(stringResource(R.string.continue_)) } },
    ) {
        if (req.instructions.isNotBlank()) {
            Text(req.instructions, style = MaterialTheme.typography.bodySmall)
        }
        if (req.questions.isEmpty()) {
            Text(stringResource(R.string.the_server_only_asks_you_to_confirm), style = MaterialTheme.typography.bodySmall)
        }
        req.questions.forEachIndexed { i, q ->
            OutlinedTextField(
                value = answers.value[i],
                onValueChange = { v -> answers.value = answers.value.toMutableList().also { it[i] = v } },
                label = { Text(q.prompt.trim().trimEnd(':')) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().then(if (i == 0) Modifier.focusRequester(focus) else Modifier),
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
    }
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
        title = { Text(stringResource(R.string.hidden_input)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    stringResource(R.string.type_a_password_or_other_secret_it_is),
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
                    Text(stringResource(R.string.press_enter_after), style = MaterialTheme.typography.bodyMedium)
                }
            }
        },
        confirmButton = { Button(onClick = { onSend(value, withEnter) }) { Text(stringResource(R.string.send)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
