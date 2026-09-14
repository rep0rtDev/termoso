package com.termoso.android.ui.account

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.data.AccountManager
import com.termoso.android.data.CLOUD_URL
import com.termoso.android.data.ServerChoice
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.SecretField
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.keychain.LocalFido2
import com.termoso.android.ui.keychain.SecurityKeyListening
import com.termoso.android.ui.keychain.SecurityKeyPicker
import com.termoso.android.ui.keychain.TouchDialog
import com.termoso.android.ui.keychain.copyText
import com.termoso.android.ui.keychain.waitingHint
import com.termoso.android.ui.theme.Emerald
import com.termoso.core.Fido2Transport
import com.termoso.core.MfaMethod

/**
 * Sign in / create account against Termoso Cloud or a self-hosted server,
 * including MFA, device approval and the one-time recovery phrase.
 * [onDone] fires once the account is signed in and (for sign-up) the
 * recovery phrase was acknowledged.
 */
@Composable
fun SignInScreen(account: AccountManager, mode: AuthMode, onBack: () -> Unit, onDone: () -> Unit) {
    val vm: SignInViewModel = viewModel(key = "signIn") { SignInViewModel(account, mode) }
    val status by account.status.collectAsStateWithLifecycle()
    LaunchedEffect(mode) { if (vm.step == AuthStep.Form) vm.switchMode(mode) }

    val title = when (val step = vm.step) {
        AuthStep.Form -> if (vm.mode == AuthMode.SignIn) "Sign in" else "Create account"
        is AuthStep.Mfa -> "Two-factor authentication"
        is AuthStep.DeviceApproval -> "Approve this device"
        is AuthStep.Recovery -> "Recovery phrase"
    }
    val back: () -> Unit = when (vm.step) {
        is AuthStep.Mfa, is AuthStep.DeviceApproval -> ({ vm.cancelPending() })
        is AuthStep.Recovery -> ({})
        AuthStep.Form -> onBack
    }

    SubScreen(title = title, onBack = back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            when (val step = vm.step) {
                AuthStep.Form -> {
                    if (status.account != null) {
                        Text(
                            "Already signed in as ${status.account?.email}. Sign out from Settings → Account first.",
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    } else {
                        CredentialsForm(vm, onDone)
                    }
                }
                is AuthStep.Mfa -> MfaForm(vm, step, onDone)
                is AuthStep.DeviceApproval -> ApprovalForm(vm, step, onDone)
                is AuthStep.Recovery -> RecoveryPhrase(vm, step, onDone)
            }
            vm.error?.let { ErrorLine(it) }
            Spacer(Modifier.height(16.dp))
        }
    }
}

@Composable
private fun CredentialsForm(vm: SignInViewModel, onDone: () -> Unit) {
    val modes = listOf(AuthMode.SignIn to "Sign in", AuthMode.Register to "Create account")
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
        modes.forEachIndexed { i, (m, label) ->
            SegmentedButton(
                selected = vm.mode == m,
                onClick = { vm.switchMode(m) },
                shape = SegmentedButtonDefaults.itemShape(i, modes.size),
                enabled = !vm.busy,
            ) { Text(label) }
        }
    }

    Text("Server", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
    val servers = listOf(ServerChoice.Cloud to "Termoso Cloud", ServerChoice.SelfHosted to "Self-hosted")
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
        servers.forEachIndexed { i, (s, label) ->
            SegmentedButton(
                selected = vm.server == s,
                onClick = { vm.chooseServer(s) },
                shape = SegmentedButtonDefaults.itemShape(i, servers.size),
                enabled = !vm.busy,
            ) { Text(label) }
        }
    }
    when (vm.server) {
        ServerChoice.Cloud -> Text(
            "${CLOUD_URL.removePrefix("https://")} · free forever, no limits, no plans. Everything is end-to-end encrypted — the server never sees your hosts, keys or passwords.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        ServerChoice.SelfHosted -> {
            FormField(
                value = vm.serverUrl,
                onChange = vm::editServerUrl,
                label = "Server address",
                placeholder = "https://termoso.example.com",
                keyboard = KeyboardType.Uri,
                enabled = !vm.busy,
            )
            ServerProbeLine(vm.probe)
        }
    }

    Text("Account", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
    FormField(value = vm.email, onChange = { vm.email = it }, label = "Email", keyboard = KeyboardType.Email, enabled = !vm.busy)
    SecretField(
        value = vm.password,
        onChange = { vm.password = it },
        label = if (vm.mode == AuthMode.Register) "Master password" else "Password",
        enabled = !vm.busy,
    )
    if (vm.mode == AuthMode.Register) {
        SecretField(value = vm.confirm, onChange = { vm.confirm = it }, label = "Confirm password", enabled = !vm.busy)
        OutlinedTextField(
            value = vm.displayName,
            onValueChange = { vm.displayName = it },
            label = { Text("Display name (optional)") },
            singleLine = true,
            enabled = !vm.busy,
            modifier = Modifier.fillMaxWidth(),
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words, autoCorrectEnabled = false),
        )
        val registrationClosed = vm.server == ServerChoice.SelfHosted && vm.serverCard?.registrationOpen == false
        if (registrationClosed || vm.invite.isNotEmpty()) {
            FormField(value = vm.invite, onChange = { vm.invite = it }, label = "Invitation token", enabled = !vm.busy)
        }
        Text(
            "Your master password protects your encryption keys and is never sent to the server (OPAQUE). " +
                "It cannot be reset — you will get a recovery phrase on the next step.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }

    Spacer(Modifier.height(4.dp))
    BusyButton(
        text = if (vm.mode == AuthMode.SignIn) "Sign in" else "Create account",
        busy = vm.busy,
        onClick = { vm.submit(onDone) },
    )
}

@Composable
private fun ServerProbeLine(probe: ServerProbe) {
    when (probe) {
        ServerProbe.Idle -> {}
        ServerProbe.Checking -> Row(verticalAlignment = Alignment.CenterVertically) {
            CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 2.dp)
            Spacer(Modifier.width(8.dp))
            Text("Checking server…", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        is ServerProbe.Ok -> Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.Check, contentDescription = null, tint = Emerald, modifier = Modifier.size(16.dp))
            Spacer(Modifier.width(6.dp))
            Text(
                "${probe.card.name} ${probe.card.version} · " +
                    (if (probe.card.registrationOpen) "registration open" else "invite only") +
                    (if (probe.card.email) "" else " · no email"),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        is ServerProbe.Failed -> Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.ErrorOutline, contentDescription = null, tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(16.dp))
            Spacer(Modifier.width(6.dp))
            Text(probe.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
        }
    }
}

private fun MfaMethod.label(): String = when (this) {
    MfaMethod.TOTP -> "Authenticator app"
    MfaMethod.BACKUP_CODE -> "Backup code"
    MfaMethod.EMAIL -> "Email code"
    MfaMethod.WEBAUTHN -> "Security key"
}

@Composable
private fun MfaForm(vm: SignInViewModel, step: AuthStep.Mfa, onDone: () -> Unit) {
    Text(
        "Your account has two-factor authentication turned on. Choose how to confirm it's you.",
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        step.methods.forEach { m ->
            FilterChip(
                selected = vm.mfaMethod == m,
                onClick = { vm.pickMethod(m) },
                label = { Text(m.label()) },
                enabled = !vm.busy,
            )
        }
    }
    when (vm.mfaMethod) {
        null -> {}
        MfaMethod.WEBAUTHN -> SecurityKeyMfa(vm, onDone)
        MfaMethod.EMAIL -> {
            if (!vm.emailCodeSent) {
                OutlinedButton(onClick = vm::sendEmailCode, enabled = !vm.busy) { Text("Send code by email") }
            } else {
                Text("We emailed you a code.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                CodeField(vm, "Email code", KeyboardType.Number)
                TextButton(onClick = vm::sendEmailCode, enabled = !vm.busy) { Text("Send again") }
            }
        }
        MfaMethod.TOTP -> CodeField(vm, "6-digit code", KeyboardType.Number)
        MfaMethod.BACKUP_CODE -> CodeField(vm, "Backup code", KeyboardType.Ascii)
    }
    val canVerify = vm.mfaMethod != null && vm.mfaMethod != MfaMethod.WEBAUTHN &&
        (vm.mfaMethod != MfaMethod.EMAIL || vm.emailCodeSent)
    if (canVerify) {
        BusyButton(text = "Verify", busy = vm.busy, onClick = { vm.submitMfa(onDone) })
    }
    TextButton(onClick = vm::cancelPending, enabled = !vm.busy) { Text("Cancel") }
}

/**
 * Second factor with a FIDO2 key over USB or NFC. Nothing to type: the
 * attached token signs the server's challenge inside Rust; the only input is
 * the key's PIN when the server asks for user verification.
 */
@Composable
private fun SecurityKeyMfa(vm: SignInViewModel, onDone: () -> Unit) {
    val fido2 = LocalFido2.current
    val devices by fido2.devices.collectAsStateWithLifecycle()
    val pending by fido2.usbPending.collectAsStateWithLifecycle()
    val device = devices.firstOrNull { it.id == vm.skDeviceId } ?: devices.singleOrNull()

    SecurityKeyListening(fido2)
    SecurityKeyPicker(
        devices = devices,
        selected = vm.skDeviceId,
        pending = pending,
        hint = fido2.waitingHint(),
        onSelect = { vm.skDeviceId = it },
        onRefresh = fido2::refreshUsb,
    )
    SecretField(
        vm.skPin,
        { vm.skPin = it },
        if (device?.pinSet == false) "Security key PIN (none set)" else "Security key PIN",
        enabled = !vm.busy,
    )
    Text(
        "The key signs a one-time challenge from the server; the PIN is only ever sent to the key itself.",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    BusyButton(
        text = "Use security key",
        busy = vm.busy,
        enabled = devices.isNotEmpty(),
        onClick = { vm.submitSecurityKey(onDone) },
    )
    if (vm.busy) TouchDialog(touch = vm.skTouch, transportNfc = device?.transport == Fido2Transport.NFC)
}

@Composable
private fun ApprovalForm(vm: SignInViewModel, step: AuthStep.DeviceApproval, onDone: () -> Unit) {
    Text(
        "This is a new device for your account. We sent a confirmation code to ${step.emailHint}.",
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    CodeField(vm, "Code from the email", KeyboardType.Number)
    BusyButton(text = "Approve device", busy = vm.busy, onClick = { vm.submitApproval(onDone) })
    Row {
        TextButton(onClick = vm::resendApproval, enabled = !vm.busy) { Text("Send again") }
        Spacer(Modifier.weight(1f))
        TextButton(onClick = vm::cancelPending, enabled = !vm.busy) { Text("Cancel") }
    }
}

@Composable
private fun RecoveryPhrase(vm: SignInViewModel, step: AuthStep.Recovery, onDone: () -> Unit) {
    val context = LocalContext.current
    Text(
        "Write these words down and keep them somewhere safe. They are the only way to regain access " +
            "if you forget your master password — Termoso cannot reset it for you. This phrase is shown once.",
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    val words = step.phrase.trim().split(Regex("\\s+"))
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        words.chunked(3).forEachIndexed { row, chunk ->
            Row(Modifier.fillMaxWidth()) {
                chunk.forEachIndexed { col, word ->
                    Text(
                        "${row * 3 + col + 1}. $word",
                        modifier = Modifier.weight(1f),
                        style = MaterialTheme.typography.bodyMedium,
                        fontFamily = FontFamily.Monospace,
                    )
                }
            }
        }
    }
    OutlinedButton(onClick = { copyText(context, "Termoso recovery phrase", step.phrase, sensitive = true) }) {
        Icon(Icons.Filled.ContentCopy, contentDescription = null, modifier = Modifier.size(18.dp))
        Spacer(Modifier.width(8.dp))
        Text("Copy")
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(checked = vm.recoverySaved, onCheckedChange = { vm.recoverySaved = it })
        Text("I have saved my recovery phrase", modifier = Modifier.weight(1f))
    }
    Button(
        onClick = onDone,
        enabled = vm.recoverySaved,
        modifier = Modifier.fillMaxWidth().height(48.dp),
        shape = RoundedCornerShape(12.dp),
    ) { Text("Continue") }
}

@Composable
private fun CodeField(vm: SignInViewModel, label: String, keyboard: KeyboardType) {
    FormField(value = vm.code, onChange = { vm.code = it }, label = label, keyboard = keyboard, enabled = !vm.busy)
}

@Composable
private fun BusyButton(text: String, busy: Boolean, onClick: () -> Unit, enabled: Boolean = true) {
    Button(
        onClick = onClick,
        enabled = !busy && enabled,
        modifier = Modifier.fillMaxWidth().height(48.dp),
        shape = RoundedCornerShape(12.dp),
    ) {
        if (busy) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = MaterialTheme.colorScheme.onPrimary)
        } else {
            Text(text)
        }
    }
}

@Composable
private fun ErrorLine(text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.Filled.ErrorOutline, contentDescription = null, tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(18.dp))
        Spacer(Modifier.width(8.dp))
        Text(text, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium, textAlign = TextAlign.Start)
    }
}
