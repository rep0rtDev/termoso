package com.termoso.android.ui.keychain

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.TouchApp
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SecretField
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.Fido2Transport

/**
 * Create an OpenSSH `sk-*` key on a FIDO2 security key. The private scalar is
 * born inside the token and never leaves it; the vault stores only the public
 * key and the key handle.
 */
@Composable
fun Fido2GenerateScreen(shell: ShellViewModel, onClose: () -> Unit, onSaved: (String) -> Unit) {
    val fido2 = LocalFido2.current
    val initialVault by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vm: Fido2GenerateViewModel = viewModel { Fido2GenerateViewModel(shell.repo, fido2, initialVault) }
    val s by vm.state.collectAsStateWithLifecycle()
    val devices by fido2.devices.collectAsStateWithLifecycle()
    val pending by fido2.usbPending.collectAsStateWithLifecycle()
    val device = s.device(devices)

    SecurityKeyListening(fido2)
    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }
    LaunchedEffect(s.savedId) { s.savedId?.let(onSaved) }

    EditorScaffold("New FIDO2 key", s.working, s.canSave(devices), onClose, vm::generate) {
        VaultPicker(s.vaults, s.vaultId) { id -> vm.update { it.copy(vaultId = id) } }

        SectionLabel("Security key")
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
                FormField(s.label, { v -> vm.update { it.copy(label = v) } }, "Label", placeholder = "YubiKey 5")
                FormField(s.comment, { v -> vm.update { it.copy(comment = v) } }, "Comment", placeholder = "user@host")
            }
        }

        SectionLabel("Algorithm")
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
                if (s.kind == SkKind.ED25519 && device?.ed25519 == false) "This security key cannot do Ed25519 — pick ECDSA P-256." else s.kind.hint,
                style = MaterialTheme.typography.bodySmall,
                color = if (s.kind == SkKind.ED25519 && device?.ed25519 == false) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            )
        }

        SectionLabel("Options")
        SectionCard {
            SwitchRow(
                title = "Require touch",
                subtitle = "Every signature needs a tap on the key (recommended)",
                checked = s.userPresence,
                onCheckedChange = { v -> vm.update { it.copy(userPresence = v) } },
            )
            RowDivider()
            SwitchRow(
                title = "Require PIN on every use",
                subtitle = "User verification: the PIN is asked on every connection",
                checked = s.userVerification,
                onCheckedChange = { v -> vm.update { it.copy(userVerification = v) } },
            )
            RowDivider()
            SwitchRow(
                title = "Resident key",
                subtitle = when {
                    device?.residentKeys == false -> "This security key cannot store resident keys"
                    else -> "Stored on the key itself, can be loaded on another device with the PIN"
                },
                checked = s.resident && device?.residentKeys != false,
                onCheckedChange = { v -> vm.update { it.copy(resident = v) } },
                enabled = device?.residentKeys != false,
            )
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (s.resident) {
                    FormField(s.user, { v -> vm.update { it.copy(user = v) } }, "User name on the key", placeholder = "termoso")
                }
                FormField(s.application, { v -> vm.update { it.copy(application = v) } }, "Application", placeholder = "ssh:")
                SecretField(
                    s.pin,
                    { v -> vm.update { it.copy(pin = v) } },
                    if (device?.pinSet == true || s.resident || s.userVerification) "Security key PIN" else "Security key PIN (if set)",
                )
            }
        }

        SectionLabel("Handle passphrase")
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SecretField(s.passphrase, { v -> vm.update { it.copy(passphrase = v) } }, "Passphrase (optional)")
                if (s.passphrase.isNotEmpty()) {
                    SecretField(s.confirm, { v -> vm.update { it.copy(confirm = v) } }, "Confirm passphrase")
                    if (s.passphraseMismatch) {
                        Text("Passphrases do not match", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
            if (s.passphrase.isNotEmpty()) {
                RowDivider()
                SwitchRow(
                    title = "Remember passphrase",
                    subtitle = "Stored encrypted in the vault so connections do not prompt",
                    checked = s.remember,
                    onCheckedChange = { v -> vm.update { it.copy(remember = v) } },
                )
            }
        }
        Text(
            "The private key is created inside the security key and cannot be copied out. " +
                "The vault keeps the public key and a handle; the passphrase only protects that handle.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
    }

    if (s.working) TouchDialog(touch = s.touch, transportNfc = device?.transport == Fido2Transport.NFC)
}

/** Import the `ssh:` resident credentials already stored on a security key. */
@Composable
fun Fido2LoadScreen(shell: ShellViewModel, onClose: () -> Unit, onLoaded: (List<String>) -> Unit) {
    val fido2 = LocalFido2.current
    val initialVault by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vm: Fido2LoadViewModel = viewModel { Fido2LoadViewModel(shell.repo, fido2, initialVault) }
    val s by vm.state.collectAsStateWithLifecycle()
    val devices by fido2.devices.collectAsStateWithLifecycle()
    val pending by fido2.usbPending.collectAsStateWithLifecycle()
    val device = s.device(devices)

    SecurityKeyListening(fido2)
    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }
    LaunchedEffect(s.loaded) {
        val ids = s.loaded ?: return@LaunchedEffect
        if (ids.isEmpty()) shell.notify("No new SSH keys on this security key.")
        onLoaded(ids)
    }

    EditorScaffold("Load from security key", s.working, s.canSave(devices), onClose, vm::load) {
        VaultPicker(s.vaults, s.vaultId) { id -> vm.update { it.copy(vaultId = id) } }

        SectionLabel("Security key")
        SecurityKeyPicker(
            devices = devices,
            selected = s.deviceId,
            pending = pending,
            hint = fido2.waitingHint(),
            onSelect = { id -> vm.update { it.copy(deviceId = id) } },
            onRefresh = fido2::refreshUsb,
        )
        if (device != null && !device.residentKeys) {
            Text(
                "This security key cannot store resident keys, so there is nothing to load.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(horizontal = 4.dp),
            )
        }

        SectionLabel("PIN")
        SectionCard {
            Column(Modifier.padding(16.dp)) {
                SecretField(s.pin, { v -> vm.update { it.copy(pin = v) } }, "Security key PIN")
            }
        }

        SectionLabel("Handle passphrase")
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SecretField(s.passphrase, { v -> vm.update { it.copy(passphrase = v) } }, "Passphrase (optional)")
                if (s.passphrase.isNotEmpty()) {
                    SecretField(s.confirm, { v -> vm.update { it.copy(confirm = v) } }, "Confirm passphrase")
                    if (s.passphraseMismatch) {
                        Text("Passphrases do not match", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
            if (s.passphrase.isNotEmpty()) {
                RowDivider()
                SwitchRow(
                    title = "Remember passphrase",
                    subtitle = "Stored encrypted in the vault so connections do not prompt",
                    checked = s.remember,
                    onCheckedChange = { v -> vm.update { it.copy(remember = v) } },
                )
            }
        }
        Text(
            "Lists the resident SSH credentials on the key (PIN required) and adds the ones this vault does not have yet.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
    }

    if (s.working) TouchDialog(touch = s.touch, transportNfc = device?.transport == Fido2Transport.NFC)
}

/** Modal shown while Rust talks to the token; flips to "touch now" on the listener callback. */
@Composable
fun TouchDialog(touch: Boolean, transportNfc: Boolean, onCancel: (() -> Unit)? = null) {
    AlertDialog(
        onDismissRequest = {},
        confirmButton = {
            if (onCancel != null) TextButton(onClick = onCancel) { Text("Cancel") }
        },
        icon = {
            if (touch) {
                Icon(Icons.Filled.TouchApp, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(32.dp))
            } else {
                CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 3.dp)
            }
        },
        title = { Text(if (touch) "Touch your security key" else "Talking to the security key…") },
        text = {
            Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
                Text(
                    when {
                        touch && transportNfc -> "Keep the key against the back of the phone until it is done."
                        touch -> "Tap the button or sensor on the key to confirm."
                        transportNfc -> "Hold the key still against the back of the phone."
                        else -> "Do not unplug the key."
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
    )
}
