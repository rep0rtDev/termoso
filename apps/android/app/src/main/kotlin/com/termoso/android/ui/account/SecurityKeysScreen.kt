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
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.data.AccountManager
import com.termoso.android.data.Fido2Manager
import com.termoso.android.data.userMessage
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
        val name = s.name.trim().ifBlank { device.product.ifBlank { "Security key" } }
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
                        it.copy(working = false, ceremony = false, touch = false, pin = keepPin(e, it.pin), error = e.userMessage())
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
            .onFailure { e -> _state.update { it.copy(working = false, error = e.userMessage()) } }
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
    var removing by remember { mutableStateOf<SecurityKeyCredential?>(null) }

    SecurityKeyListening(fido2)
    LaunchedEffect(Unit) { vm.reload() }
    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }

    SubScreen(title = "Security keys", onBack = onBack) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            val card = s.card
            SectionLabel("Registered keys")
            SectionCard {
                when {
                    s.loading -> Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                    }
                    card == null -> ListRow(
                        title = "Could not load two-factor settings",
                        subtitle = s.loadError,
                        titleColor = MaterialTheme.colorScheme.error,
                    ) {
                        TextButton(onClick = vm::reload) { Text("Retry") }
                    }
                    card.securityKeys.isEmpty() -> ListRow(
                        title = "No security keys yet",
                        subtitle = "Register a FIDO2 key below and sign-ins can be confirmed with a touch instead of a code.",
                        leading = { IconTile(Icons.Filled.Security) },
                    )
                    else -> card.securityKeys.forEachIndexed { i, k ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = k.name,
                            subtitle = "Added ${relative(k.createdAt)} · " +
                                (k.lastUsedAt?.let { "last used ${relative(it)}" } ?: "never used"),
                            leading = { IconTile(Icons.Filled.Security) },
                        ) {
                            IconButton(onClick = { removing = k }, enabled = !s.working) {
                                Icon(Icons.Filled.Delete, contentDescription = "Remove security key", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
            }
            if (card != null) {
                SectionLabel("Other factors")
                SectionCard {
                    ListRow(
                        title = "Authenticator app",
                        subtitle = if (card.totpEnabled) "On" else "Off — set it up in the web cabinet",
                        leading = { IconTile(Icons.Filled.Smartphone, selected = card.totpEnabled) },
                    )
                    RowDivider()
                    ListRow(
                        title = "Backup codes",
                        subtitle = when {
                            card.backupCodesRemaining == 0u && card.securityKeys.isEmpty() && !card.totpEnabled -> "Issued once a second factor is on"
                            card.backupCodesRemaining == 0u -> "None left — regenerate them in the web cabinet"
                            else -> "${card.backupCodesRemaining} unused"
                        },
                        leading = { IconTile(Icons.Filled.Pin) },
                    )
                }
            }

            SectionLabel("Add a security key")
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
                        "Name",
                        placeholder = device?.product?.ifBlank { null } ?: "YubiKey 5 NFC",
                        enabled = !s.working,
                    )
                    SecretField(
                        s.pin,
                        { v -> vm.update { it.copy(pin = v) } },
                        if (device?.pinSet == false) "Security key PIN (none set)" else "Security key PIN",
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
                            Text("Register this key")
                        }
                    }
                }
            }
            Text(
                "The key creates a credential for this server and proves it with a touch on every sign-in. " +
                    "The server stores only the public key; the PIN is sent to the key itself, never to the server. " +
                    "Registering the first factor issues backup codes — get them from the web cabinet.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 4.dp, vertical = 12.dp),
            )
            Spacer(Modifier.height(16.dp))
        }
    }

    if (s.ceremony) TouchDialog(touch = s.touch, transportNfc = device?.transport == Fido2Transport.NFC)

    removing?.let { k ->
        val last = s.card?.let { it.securityKeys.size == 1 && !it.totpEnabled } == true
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text("Remove ${k.name}?") },
            text = {
                Text(
                    if (last) {
                        "This is your only second factor. Removing it turns two-factor authentication off and discards the backup codes."
                    } else {
                        "This key can no longer confirm sign-ins. Other factors stay as they are."
                    },
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    removing = null
                    scope.launch { if (vm.remove(k.id)) shell.notify("${k.name} removed") }
                }) { Text("Remove", color = Danger) }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text("Cancel") } },
        )
    }
}

/** A wrong PIN is cleared so it cannot be resent by accident; any other failure keeps the form intact. */
private fun keepPin(e: Throwable, pin: String): String =
    if (e is MobileException.SecurityKey && e.kind.startsWith("fido2_pin")) "" else pin
