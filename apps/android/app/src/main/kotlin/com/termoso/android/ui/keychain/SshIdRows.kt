package com.termoso.android.ui.keychain

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.termoso.android.ui.account.label
import com.termoso.android.ui.components.PickerRow
import com.termoso.core.SshIdKeyKind
import com.termoso.core.sshidTypeIsHardware

private val softwareKinds = SshIdKeyKind.entries.filter { !sshidTypeIsHardware(it) }

/**
 * "Log in with SSH ID" switch plus the preferred passkey picker, shared by
 * the identity and host editors. The passkeys themselves live in Rust; the
 * choice here only orders which one is offered first.
 */
@Composable
fun SshIdRows(
    sshId: Boolean,
    keyType: SshIdKeyKind?,
    onSshId: (Boolean) -> Unit,
    onKeyType: (SshIdKeyKind?) -> Unit,
    usernameHint: Boolean,
) {
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("SSH ID", style = MaterialTheme.typography.bodyLarge)
                Text(
                    if (sshId) {
                        "Signs in with this phone's SSH ID passkeys" +
                            if (usernameHint) "; username defaults to your handle" else ""
                    } else {
                        "Use the passkeys published under your SSH ID handle"
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Switch(checked = sshId, onCheckedChange = { on -> onSshId(on); if (!on) onKeyType(null) })
        }
        if (sshId) {
            PickerRow(
                label = "Preferred key",
                value = keyType?.label() ?: "Any",
                options = listOf<Pair<String?, String>>(null to "Any") + softwareKinds.map { it.name to it.label() },
                selected = keyType?.name,
                onPick = { name -> onKeyType(name?.let(SshIdKeyKind::valueOf)) },
                empty = null,
            )
            Text(
                "Set up the handle in Settings → Account → SSH ID. Password and key below stay as fallbacks.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
    }
}
