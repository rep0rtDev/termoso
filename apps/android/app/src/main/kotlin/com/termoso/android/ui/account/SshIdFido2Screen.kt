package com.termoso.android.ui.account

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
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
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SecretField
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.keychain.EditorScaffold
import com.termoso.android.ui.keychain.LocalFido2
import com.termoso.android.ui.keychain.SecurityKeyListening
import com.termoso.android.ui.keychain.SecurityKeyPicker
import com.termoso.android.ui.keychain.SkKind
import com.termoso.android.ui.keychain.TouchDialog
import com.termoso.android.ui.keychain.waitingHint
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.Fido2DeviceCard
import com.termoso.core.Fido2GenerateDraft
import com.termoso.core.Fido2Listener
import com.termoso.core.Fido2Transport
import com.termoso.core.MobileException
import com.termoso.core.SshIdKeyKind
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SshIdFido2State(
    val deviceId: String? = null,
    val label: String = "",
    val kind: SkKind = SkKind.ED25519,
    val resident: Boolean = false,
    val userPresence: Boolean = true,
    val userVerification: Boolean = false,
    val pin: String = "",
    val working: Boolean = false,
    val touch: Boolean = false,
    val done: Boolean = false,
    val error: String? = null,
) {
    fun device(devices: List<Fido2DeviceCard>): Fido2DeviceCard? =
        devices.firstOrNull { it.id == deviceId } ?: devices.singleOrNull()

    fun canSave(devices: List<Fido2DeviceCard>): Boolean {
        val d = device(devices) ?: return false
        if (working) return false
        if (kind == SkKind.ED25519 && !d.ed25519) return false
        if (resident && !d.residentKeys) return false
        return true
    }
}

/**
 * Drives `sshid_add_fido2`: one call creates the credential on the token and
 * publishes its public key under the handle. The vault (personal one, so the
 * handle follows the account) and the passphrase policy are decided in Rust;
 * the PIN is only held for the duration of the call.
 */
class SshIdFido2ViewModel(
    private val repo: VaultRepository,
    private val account: AccountManager,
    val fido2: Fido2Manager,
) : ViewModel() {
    private val _state = MutableStateFlow(SshIdFido2State())
    val state: StateFlow<SshIdFido2State> = _state.asStateFlow()

    fun update(transform: (SshIdFido2State) -> SshIdFido2State) = _state.update(transform)

    fun errorShown() = _state.update { it.copy(error = null) }

    fun generate() {
        val s = _state.value
        val devices = fido2.devices.value
        if (!s.canSave(devices)) return
        val device = s.device(devices) ?: return
        _state.update { it.copy(working = true, touch = false) }
        val listener = object : Fido2Listener {
            override fun onTouch() = _state.update { it.copy(touch = true) }
        }
        viewModelScope.launch {
            runCatching {
                account.withReauth {
                    repo.write {
                        sshidAddFido2(
                            Fido2GenerateDraft(
                                vaultId = "",
                                deviceId = device.id,
                                label = s.label.trim().ifBlank { str(R.string.security_key_2, s.kind.label) },
                                algorithm = s.kind.algorithm,
                                application = "ssh:",
                                resident = s.resident,
                                userPresence = s.userPresence,
                                userVerification = s.userVerification,
                                pin = s.pin.takeIf { it.isNotEmpty() },
                                user = "",
                                comment = "",
                                passphrase = null,
                                rememberPassphrase = false,
                            ),
                            listener,
                        )
                    }
                }
            }.onSuccess { _state.update { it.copy(working = false, touch = false, pin = "", done = true) } }
                .onFailure { e ->
                    _state.update {
                        it.copy(
                            working = false,
                            touch = false,
                            pin = if (e is MobileException.SecurityKey && e.kind.startsWith("fido2_pin")) "" else it.pin,
                            error = e.takeUnless { it is ReauthCancelled }?.userMessage(),
                        )
                    }
                }
        }
    }
}

/** Settings → Account → SSH ID → New security key. */
@Composable
fun SshIdFido2Screen(shell: ShellViewModel, account: AccountManager, onClose: () -> Unit, onDone: () -> Unit) {
    val fido2 = LocalFido2.current
    val vm: SshIdFido2ViewModel = viewModel { SshIdFido2ViewModel(shell.repo, account, fido2) }
    val s by vm.state.collectAsStateWithLifecycle()
    val devices by fido2.devices.collectAsStateWithLifecycle()
    val pending by fido2.usbPending.collectAsStateWithLifecycle()
    val device = s.device(devices)

    SecurityKeyListening(fido2)
    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }
    LaunchedEffect(s.done) { if (s.done) { shell.notify(str(R.string.security_key_published_under_your_ssh_id)); onDone() } }

    EditorScaffold(stringResource(R.string.security_key_for_ssh_id), s.working, s.canSave(devices), onClose, vm::generate) {
        Text(
            stringResource(R.string.a_new_credential_is_created_inside_the_security, sshidUrlName(s.kind)),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 4.dp),
        )

        SectionLabel(stringResource(R.string.security_key))
        SecurityKeyPicker(
            devices = devices,
            selected = s.deviceId,
            pending = pending,
            hint = fido2.waitingHint(),
            onSelect = { id -> vm.update { it.copy(deviceId = id) } },
            onRefresh = fido2::refreshUsb,
        )

        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(s.label, { v -> vm.update { it.copy(label = v) } }, stringResource(R.string.label), placeholder = stringResource(R.string.yubikey_5))
            }
        }

        SectionLabel(stringResource(R.string.algorithm))
        SectionCard {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                SkKind.entries.forEach { kind ->
                    FilterChip(
                        selected = s.kind == kind,
                        enabled = kind != SkKind.ED25519 || device?.ed25519 != false,
                        onClick = { vm.update { it.copy(kind = kind) } },
                        label = { Text(kind.label) },
                    )
                }
            }
            Text(
                if (s.kind == SkKind.ED25519 && device?.ed25519 == false) stringResource(R.string.this_security_key_cannot_do_ed25519_pick_ecdsa) else stringResource(s.kind.hint),
                style = MaterialTheme.typography.bodySmall,
                color = if (s.kind == SkKind.ED25519 && device?.ed25519 == false) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            )
        }

        SectionLabel(stringResource(R.string.options))
        SectionCard {
            SwitchRow(
                title = stringResource(R.string.require_touch),
                subtitle = stringResource(R.string.every_signature_needs_a_tap_on_the_key),
                checked = s.userPresence,
                onCheckedChange = { v -> vm.update { it.copy(userPresence = v) } },
            )
            RowDivider()
            SwitchRow(
                title = stringResource(R.string.require_pin_on_every_use),
                subtitle = stringResource(R.string.user_verification_the_pin_is_asked_on_every),
                checked = s.userVerification,
                onCheckedChange = { v -> vm.update { it.copy(userVerification = v) } },
            )
            RowDivider()
            SwitchRow(
                title = stringResource(R.string.resident_key),
                subtitle = when {
                    device?.residentKeys == false -> stringResource(R.string.this_security_key_cannot_store_resident_keys)
                    else -> stringResource(R.string.stored_on_the_key_itself_can_be_loaded)
                },
                checked = s.resident && device?.residentKeys != false,
                onCheckedChange = { v -> vm.update { it.copy(resident = v) } },
                enabled = device?.residentKeys != false,
            )
            Column(Modifier.padding(16.dp)) {
                SecretField(
                    s.pin,
                    { v -> vm.update { it.copy(pin = v) } },
                    if (device?.pinSet == true || s.resident || s.userVerification) stringResource(R.string.security_key_pin) else stringResource(R.string.security_key_pin_if_set),
                )
            }
        }
        Text(
            stringResource(R.string.the_handle_is_kept_in_your_personal_vault),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
    }

    if (s.working) TouchDialog(touch = s.touch, transportNfc = device?.transport == Fido2Transport.NFC)
}

private fun sshidUrlName(kind: SkKind): String =
    when (kind) {
        SkKind.ED25519 -> SshIdKeyKind.ED25519_SK
        SkKind.ECDSA -> SshIdKeyKind.ECDSA_SK
    }.label()
