package com.termoso.android.ui.account

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Pin
import androidx.compose.material.icons.filled.Security
import androidx.compose.material.icons.filled.Smartphone
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.R
import com.termoso.android.data.AccountManager
import com.termoso.android.data.Fido2Manager
import com.termoso.android.data.ReauthCancelled
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SecretField
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.keychain.LocalFido2
import com.termoso.android.ui.keychain.SecurityKeyListening
import com.termoso.android.ui.keychain.SecurityKeyPicker
import com.termoso.android.ui.keychain.TouchDialog
import com.termoso.android.ui.keychain.waitingHint
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.theme.Danger
import com.termoso.core.Fido2DeviceCard
import com.termoso.core.Fido2Listener
import com.termoso.core.Fido2Transport
import com.termoso.core.MfaCard
import com.termoso.core.MobileException
import com.termoso.core.SecurityKeyCredential
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SecurityKeysState(
    val loading: Boolean = true,
    val card: MfaCard? = null,
    val loadError: String? = null,
    /** Registration form. */
    val name: String = "",
    val deviceId: String? = null,
    val pin: String = "",
    val working: Boolean = false,
    /** A makeCredential ceremony is running against the token (as opposed to a plain API call). */
    val ceremony: Boolean = false,
    val touch: Boolean = false,
    val error: String? = null,
) {
    fun device(devices: List<Fido2DeviceCard>): Fido2DeviceCard? =
        devices.firstOrNull { it.id == deviceId } ?: devices.singleOrNull()

    fun canRegister(devices: List<Fido2DeviceCard>): Boolean = !working && card != null && device(devices) != null
}

/**
 * Settings → Account → Security keys. Registration and removal go through
 * the Rust façade: the WebAuthn attestation is produced by the token over
 * USB/NFC and the server keeps only the public key; the PIN goes to Rust and
 * is cleared here as soon as the ceremony ends.
 */
class SecurityKeysViewModel(private val account: AccountManager, private val fido2: Fido2Manager) : ViewModel() {
    private val _state = MutableStateFlow(SecurityKeysState())
    val state: StateFlow<SecurityKeysState> = _state.asStateFlow()

    fun update(transform: (SecurityKeysState) -> SecurityKeysState) = _state.update(transform)

    fun errorShown() = _state.update { it.copy(error = null) }

    fun reload() {
        _state.update { it.copy(loading = it.card == null, loadError = null) }
        viewModelScope.launch {
            runCatching { account.mfaStatus() }
                .onSuccess { c -> _state.update { it.copy(loading = false, card = c) } }
                .onFailure { e -> _state.update { it.copy(loading = false, loadError = e.userMessage()) } }
        }
    }

    fun register() {
        val s = _state.value
        val device = s.device(fido2.devices.value) ?: return
        if (!s.canRegister(fido2.devices.value)) return
        val name = s.name.trim().ifBlank { device.product.ifBlank { str(R.string.security_key) } }
        _state.update { it.copy(working = true, ceremony = true, touch = false, error = null) }
        val listener = object : Fido2Listener {
            override fun onTouch() = _state.update { it.copy(touch = true) }
        }
        viewModelScope.launch {
            runCatching { account.registerSecurityKey(name, device.id, s.pin.takeIf { it.isNotEmpty() }, listener) }
                .onSuccess { added ->
                    _state.update {
                        it.copy(
                            working = false,
                            ceremony = false,
                            touch = false,
                            name = "",
                            pin = "",
                            card = it.card?.let { c -> c.copy(securityKeys = c.securityKeys + added) },
                        )
                    }
                    reload()
                }
                .onFailure { e ->
                    _state.update {
                        it.copy(
                            working = false,
                            ceremony = false,
                            touch = false,
                            pin = keepPin(e, it.pin),
                            error = e.takeUnless { it is ReauthCancelled }?.userMessage(),
                        )
                    }
                }
        }
    }

    suspend fun remove(id: String): Boolean {
        _state.update { it.copy(working = true, error = null) }
        return runCatching { account.removeSecurityKey(id) }
            .onSuccess {
                _state.update { s ->
                    s.copy(working = false, card = s.card?.let { c -> c.copy(securityKeys = c.securityKeys.filterNot { it.id == id }) })
                }
                reload()
            }
            .onFailure { e -> _state.update { it.copy(working = false, error = e.takeUnless { it is ReauthCancelled }?.userMessage()) } }
            .isSuccess
    }
}

@Composable
fun SecurityKeysScreen(shell: ShellViewModel, account: AccountManager, onBack: () -> Unit) {
    val fido2 = LocalFido2.current
    val vm: SecurityKeysViewModel = viewModel { SecurityKeysViewModel(account, fido2) }
    val s by vm.state.collectAsStateWithLifecycle()
    val devices by fido2.devices.collectAsStateWithLifecycle()
    val pending by fido2.usbPending.collectAsStateWithLifecycle()
    val device = s.device(devices)
    val scope = rememberCoroutineScope()
    val reauth by account.reauthRequest.collectAsStateWithLifecycle()
    var removing by remember { mutableStateOf<SecurityKeyCredential?>(null) }

    SecurityKeyListening(fido2)
    LaunchedEffect(Unit) { vm.reload() }
    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }

    SubScreen(title = stringResource(R.string.security_keys), onBack = onBack) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            val card = s.card
            SectionLabel(stringResource(R.string.registered_keys))
            SectionCard {
                when {
                    s.loading -> Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                    }
                    card == null -> ListRow(
                        title = stringResource(R.string.could_not_load_two_factor_settings),
                        subtitle = s.loadError,
                        titleColor = MaterialTheme.colorScheme.error,
                    ) {
                        TextButton(onClick = vm::reload) { Text(stringResource(R.string.retry)) }
                    }
                    card.securityKeys.isEmpty() -> ListRow(
                        title = stringResource(R.string.no_security_keys_yet),
                        subtitle = stringResource(R.string.register_a_fido2_key_below_and_sign_ins),
                        leading = { IconTile(Icons.Filled.Security) },
                    )
                    else -> card.securityKeys.forEachIndexed { i, k ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = k.name,
                            subtitle = stringResource(R.string.added, relative(k.createdAt)) +
                                (k.lastUsedAt?.let { stringResource(R.string.last_used, relative(it)) } ?: stringResource(R.string.never_used)),
                            leading = { IconTile(Icons.Filled.Security) },
                        ) {
                            IconButton(onClick = { removing = k }, enabled = !s.working) {
                                Icon(Icons.Filled.Delete, contentDescription = stringResource(R.string.remove_security_key), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
            }
            if (card != null) {
                SectionLabel(stringResource(R.string.other_factors))
                SectionCard {
                    ListRow(
                        title = stringResource(R.string.authenticator_app),
                        subtitle = if (card.totpEnabled) stringResource(R.string.on) else stringResource(R.string.off_set_it_up_in_the_web_cabinet),
                        leading = { IconTile(Icons.Filled.Smartphone, selected = card.totpEnabled) },
                    )
                    RowDivider()
                    ListRow(
                        title = stringResource(R.string.backup_codes),
                        subtitle = when {
                            card.backupCodesRemaining == 0u && card.securityKeys.isEmpty() && !card.totpEnabled -> stringResource(R.string.issued_once_a_second_factor_is_on)
                            card.backupCodesRemaining == 0u -> stringResource(R.string.none_left_regenerate_them_in_the_web_cabinet)
                            else -> stringResource(R.string.unused, card.backupCodesRemaining)
                        },
                        leading = { IconTile(Icons.Filled.Pin) },
                    )
                }
            }

            SectionLabel(stringResource(R.string.add_a_security_key))
            SecurityKeyPicker(
                devices = devices,
                selected = s.deviceId,
                pending = pending,
                hint = fido2.waitingHint(),
                onSelect = { id -> vm.update { it.copy(deviceId = id) } },
                onRefresh = fido2::refreshUsb,
            )
            Spacer(Modifier.height(12.dp))
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    FormField(
                        s.name,
                        { v -> vm.update { it.copy(name = v) } },
                        stringResource(R.string.name),
                        placeholder = device?.product?.ifBlank { null } ?: stringResource(R.string.yubikey_5_nfc),
                        enabled = !s.working,
                    )
                    SecretField(
                        s.pin,
                        { v -> vm.update { it.copy(pin = v) } },
                        if (device?.pinSet == false) stringResource(R.string.security_key_pin_none_set) else stringResource(R.string.security_key_pin),
                        enabled = !s.working,
                    )
                    Button(
                        onClick = vm::register,
                        enabled = s.canRegister(devices),
                        modifier = Modifier.fillMaxWidth().height(48.dp),
                        shape = RoundedCornerShape(12.dp),
                    ) {
                        if (s.working) {
                            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = MaterialTheme.colorScheme.onPrimary)
                        } else {
                            Text(stringResource(R.string.register_this_key))
                        }
                    }
                }
            }
            Text(
                stringResource(R.string.the_key_creates_a_credential_for_this_server),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 4.dp, vertical = 12.dp),
            )
            Spacer(Modifier.height(16.dp))
        }
    }

    if (s.ceremony && reauth == null) TouchDialog(touch = s.touch, transportNfc = device?.transport == Fido2Transport.NFC)

    removing?.let { k ->
        val last = s.card?.let { it.securityKeys.size == 1 && !it.totpEnabled } == true
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text(stringResource(R.string.remove, k.name)) },
            text = {
                Text(
                    if (last) {
                        stringResource(R.string.this_is_your_only_second_factor_removing_it)
                    } else {
                        stringResource(R.string.this_key_can_no_longer_confirm_sign_ins)
                    },
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    removing = null
                    scope.launch { if (vm.remove(k.id)) shell.notify(str(R.string.removed, k.name)) }
                }) { Text(stringResource(R.string.remove_2), color = Danger) }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/** A wrong PIN is cleared so it cannot be resent by accident; any other failure keeps the form intact. */
private fun keepPin(e: Throwable, pin: String): String =
    if (e is MobileException.SecurityKey && e.kind.startsWith("fido2_pin")) "" else pin
