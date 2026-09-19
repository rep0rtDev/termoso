package com.termoso.android.ui.keychain

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import com.termoso.android.ui.account.label
import com.termoso.android.ui.components.PickerRow
import com.termoso.android.ui.components.TermosoSwitch
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
                Text(stringResource(R.string.ssh_id), style = MaterialTheme.typography.bodyLarge)
                Text(
                    if (sshId) {
                        stringResource(R.string.signs_in_with_this_phones_ssh_id_passkeys) +
                            if (usernameHint) stringResource(R.string.username_defaults_to_your_handle) else ""
                    } else {
                        stringResource(R.string.use_the_passkeys_published_under_your_ssh_id)
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            TermosoSwitch(checked = sshId, onCheckedChange = { on -> onSshId(on); if (!on) onKeyType(null) })
        }
        if (sshId) {
            PickerRow(
                label = stringResource(R.string.preferred_key),
                value = keyType?.label() ?: stringResource(R.string.any),
                options = listOf<Pair<String?, String>>(null to stringResource(R.string.any)) + softwareKinds.map { it.name to it.label() },
                selected = keyType?.name,
                onPick = { name -> onKeyType(name?.let(SshIdKeyKind::valueOf)) },
                empty = null,
            )
            Text(
                stringResource(R.string.set_up_the_handle_in_settings_account_ssh),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
    }
}
