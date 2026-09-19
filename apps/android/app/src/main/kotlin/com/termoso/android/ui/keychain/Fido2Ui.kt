package com.termoso.android.ui.keychain

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Nfc
import androidx.compose.material.icons.filled.Usb
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import com.termoso.android.data.Fido2Manager
import com.termoso.android.str
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.security.findFragmentActivity
import com.termoso.core.Fido2DeviceCard
import com.termoso.core.Fido2Transport

/** The process-wide [Fido2Manager]; provided at the root of the composition. */
val LocalFido2 = staticCompositionLocalOf<Fido2Manager> { error("Fido2Manager not provided") }

/**
 * While this is in the composition the app listens for security keys: USB
 * plug events and permission prompts, and NFC reader mode on the activity.
 */
@Composable
fun SecurityKeyListening(fido2: Fido2Manager = LocalFido2.current) {
    val context = LocalContext.current
    DisposableEffect(fido2) {
        fido2.start()
        fido2.refreshUsb()
        val activity = context.findFragmentActivity()
        if (activity != null) fido2.enableNfc(activity)
        onDispose { if (activity != null) fido2.disableNfc(activity) }
    }
}

/** Hint that matches the hardware the phone actually has. */
fun Fido2Manager.waitingHint(): String = when {
    usbHost && nfcHardware && nfcEnabled -> str(R.string.plug_a_security_key_into_the_usb_port)
    usbHost && nfcHardware -> str(R.string.plug_a_security_key_into_the_usb_port_2)
    usbHost -> str(R.string.plug_a_security_key_into_the_usb_port_3)
    nfcHardware && nfcEnabled -> str(R.string.hold_the_security_key_to_the_back_of)
    nfcHardware -> str(R.string.turn_on_nfc_and_hold_the_security_key)
    else -> str(R.string.this_phone_has_neither_usb_host_nor_nfc)
}

/**
 * Attached tokens with their capabilities; tapping selects one. With a single
 * token nothing needs selecting and the row is informational. [pending] USB
 * devices are waiting on the system permission dialog.
 */
@Composable
fun SecurityKeyPicker(
    devices: List<Fido2DeviceCard>,
    selected: String?,
    pending: Int,
    hint: String,
    onSelect: (String?) -> Unit,
    onRefresh: () -> Unit,
) {
    SectionCard {
        if (devices.isEmpty()) {
            Row(
                Modifier.fillMaxWidth().padding(16.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.waiting_for_a_security_key), style = MaterialTheme.typography.bodyLarge)
                    Text(
                        if (pending > 0) stringResource(R.string.allow_termoso_to_use_the_usb_device_in) else hint,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
        devices.forEachIndexed { i, d ->
            if (i > 0) RowDivider()
            val chosen = selected == d.id || (selected == null && devices.size == 1)
            ListRow(
                title = d.product.ifBlank { if (d.transport == Fido2Transport.NFC) stringResource(R.string.nfc_security_key) else stringResource(R.string.usb_security_key) },
                subtitle = deviceSubtitle(d),
                leading = { IconTile(if (d.transport == Fido2Transport.NFC) Icons.Filled.Nfc else Icons.Filled.Usb, selected = chosen) },
                modifier = Modifier.clickable(enabled = devices.size > 1) { onSelect(if (selected == d.id) null else d.id) },
            )
        }
        RowDivider()
        Row(Modifier.padding(horizontal = 8.dp, vertical = 4.dp)) {
            TextButton(onClick = onRefresh) { Text(stringResource(R.string.rescan_usb)) }
        }
    }
}

fun deviceSubtitle(d: Fido2DeviceCard): String = listOfNotNull(
    when (d.pinSet) {
        true -> str(R.string.pin_set)
        false -> str(R.string.no_pin)
        null -> null
    },
    if (d.residentKeys) str(R.string.resident_keys) else null,
    if (d.ed25519) "Ed25519" else str(R.string.ecdsa_only),
    d.versions.firstOrNull { it.startsWith("FIDO_2") }?.replace("FIDO_2_", "CTAP 2.")?.replace("FIDO_2", "CTAP 2"),
).joinToString(" · ")
