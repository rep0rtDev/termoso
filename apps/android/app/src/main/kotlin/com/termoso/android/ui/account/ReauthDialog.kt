package com.termoso.android.ui.account

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.AccountManager
import com.termoso.android.data.ReauthRequest
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.SecretField
import com.termoso.android.ui.keychain.LocalFido2
import com.termoso.android.ui.keychain.SecurityKeyListening
import com.termoso.android.ui.keychain.SecurityKeyPicker
import com.termoso.android.ui.keychain.TouchDialog
import com.termoso.android.ui.keychain.waitingHint
import com.termoso.core.Fido2Listener
import com.termoso.core.Fido2Transport
import com.termoso.core.MfaMethod
import com.termoso.core.MobileException
import com.termoso.core.ReauthOutcome
import kotlinx.coroutines.launch

/**
 * Hosts the step-up prompt for the whole shell: whenever a sensitive account
 * change comes back with `ReauthRequired`, [AccountManager.withReauth] parks
 * the action here until the user confirms the password (and second factor).
 */
@Composable
fun ReauthHost(account: AccountManager) {
    val request by account.reauthRequest.collectAsStateWithLifecycle()
    request?.let { ReauthDialog(account, it) }
}

private sealed interface ReauthStage {
    data object Password : ReauthStage
    data class Mfa(val methods: List<MfaMethod>) : ReauthStage
    data class EmailCode(val emailHint: String) : ReauthStage
}

@Composable
private fun ReauthDialog(account: AccountManager, request: ReauthRequest) {
    val scope = rememberCoroutineScope()
    val fido2 = LocalFido2.current
    var stage by remember { mutableStateOf<ReauthStage>(ReauthStage.Password) }
    var password by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    var method by remember { mutableStateOf<MfaMethod?>(null) }
    var emailSent by remember { mutableStateOf(false) }
    var skDeviceId by remember { mutableStateOf<String?>(null) }
    var skPin by remember { mutableStateOf("") }
    var skTouch by remember { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }

    fun cancel() {
        scope.launch { runCatching { account.reauthCancel() } }
        request.finish(false)
    }

    fun handle(outcome: ReauthOutcome) {
        code = ""
        when (outcome) {
            is ReauthOutcome.Done -> request.finish(true)
            is ReauthOutcome.MfaRequired -> {
                stage = ReauthStage.Mfa(outcome.methods)
                method = outcome.methods.firstOrNull { it != MfaMethod.WEBAUTHN } ?: outcome.methods.firstOrNull()
            }
            is ReauthOutcome.EmailCodeRequired -> stage = ReauthStage.EmailCode(outcome.emailHint)
        }
    }

    fun run(block: suspend () -> ReauthOutcome) {
        if (busy) return
        busy = true
        error = null
        scope.launch {
            runCatching { block() }
                .onSuccess { handle(it) }
                .onFailure { e ->
                    if (e !is MobileException.SecurityKey || e.kind != "fido2_pin_invalid") skPin = ""
                    error = e.userMessage()
                }
            busy = false
            skTouch = false
        }
    }

    val s = stage
    val canSubmit = when (s) {
        ReauthStage.Password -> password.isNotEmpty()
        is ReauthStage.Mfa -> method != null && method != MfaMethod.WEBAUTHN && code.isNotBlank() &&
            (method != MfaMethod.EMAIL || emailSent)
        is ReauthStage.EmailCode -> code.isNotBlank()
    }

    fun submit() {
        when (val st = stage) {
            ReauthStage.Password -> run { account.reauthStart(password) }
            is ReauthStage.Mfa -> method?.let { m -> run { account.reauthMfa(m, code) } }
            is ReauthStage.EmailCode -> run { account.reauthEmailCode(code) }
        }
    }

    AlertDialog(
        onDismissRequest = { if (!busy) cancel() },
        title = {
            Text(
                when (s) {
                    ReauthStage.Password -> stringResource(R.string.confirm_its_you)
                    is ReauthStage.Mfa -> stringResource(R.string.two_factor_authentication)
                    is ReauthStage.EmailCode -> stringResource(R.string.check_your_email)
                },
            )
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                when (s) {
                    ReauthStage.Password -> {
                        Text(
                            stringResource(R.string.this_change_affects_the_security_of_your_account),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        SecretField(password, { password = it }, stringResource(R.string.master_password), enabled = !busy)
                    }
                    is ReauthStage.Mfa -> {
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            s.methods.forEach { m ->
                                FilterChip(
                                    selected = method == m,
                                    onClick = { method = m; code = ""; error = null },
                                    label = { Text(m.reauthLabel()) },
                                    enabled = !busy,
                                )
                            }
                        }
                        when (method) {
                            null -> {}
                            MfaMethod.TOTP -> FormField(code, { code = it }, stringResource(R.string.s_6_digit_code), keyboard = KeyboardType.Number, enabled = !busy)
                            MfaMethod.BACKUP_CODE -> FormField(code, { code = it }, stringResource(R.string.backup_code), keyboard = KeyboardType.Ascii, enabled = !busy)
                            MfaMethod.EMAIL -> {
                                if (!emailSent) {
                                    OutlinedButton(
                                        enabled = !busy,
                                        onClick = {
                                            scope.launch {
                                                runCatching { account.reauthSendMfaEmail() }
                                                    .onSuccess { emailSent = true }
                                                    .onFailure { error = it.userMessage() }
                                            }
                                        },
                                    ) { Text(stringResource(R.string.send_code_by_email)) }
                                } else {
                                    FormField(code, { code = it }, stringResource(R.string.email_code), keyboard = KeyboardType.Number, enabled = !busy)
                                }
                            }
                            MfaMethod.WEBAUTHN -> {
                                val devices by fido2.devices.collectAsStateWithLifecycle()
                                val pending by fido2.usbPending.collectAsStateWithLifecycle()
                                val device = devices.firstOrNull { it.id == skDeviceId } ?: devices.singleOrNull()
                                SecurityKeyListening(fido2)
                                SecurityKeyPicker(
                                    devices = devices,
                                    selected = skDeviceId,
                                    pending = pending,
                                    hint = fido2.waitingHint(),
                                    onSelect = { skDeviceId = it },
                                    onRefresh = fido2::refreshUsb,
                                )
                                SecretField(
                                    skPin,
                                    { skPin = it },
                                    if (device?.pinSet == false) stringResource(R.string.security_key_pin_none_set) else stringResource(R.string.security_key_pin),
                                    enabled = !busy,
                                )
                                OutlinedButton(
                                    enabled = !busy && devices.isNotEmpty(),
                                    modifier = Modifier.fillMaxWidth(),
                                    onClick = {
                                        val listener = object : Fido2Listener {
                                            override fun onTouch() {
                                                skTouch = true
                                            }
                                        }
                                        run { account.reauthSecurityKey(skDeviceId, skPin.takeIf { it.isNotEmpty() }, listener) }
                                    },
                                ) { Text(stringResource(R.string.use_security_key)) }
                                if (busy) TouchDialog(touch = skTouch, transportNfc = device?.transport == Fido2Transport.NFC)
                            }
                        }
                    }
                    is ReauthStage.EmailCode -> {
                        Text(
                            stringResource(R.string.your_account_has_no_password_so_we_sent, s.emailHint),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        FormField(code, { code = it }, stringResource(R.string.code_from_the_email), keyboard = KeyboardType.Number, enabled = !busy)
                    }
                }
                error?.let {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(
                            Icons.Filled.ErrorOutline,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.error,
                            modifier = Modifier.size(18.dp),
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                    }
                }
            }
        },
        confirmButton = {
            if (s !is ReauthStage.Mfa || method != MfaMethod.WEBAUTHN) {
                TextButton(onClick = ::submit, enabled = !busy && canSubmit) {
                    if (busy) {
                        CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                    } else {
                        Text(if (s is ReauthStage.Password) stringResource(R.string.continue_) else stringResource(R.string.verify))
                    }
                }
            }
        },
        dismissButton = { TextButton(onClick = ::cancel, enabled = !busy) { Text(stringResource(R.string.cancel)) } },
    )
}

private fun MfaMethod.reauthLabel(): String = when (this) {
    MfaMethod.TOTP -> str(R.string.authenticator_app)
    MfaMethod.BACKUP_CODE -> str(R.string.backup_code)
    MfaMethod.EMAIL -> str(R.string.email_code)
    MfaMethod.WEBAUTHN -> str(R.string.security_key)
}
