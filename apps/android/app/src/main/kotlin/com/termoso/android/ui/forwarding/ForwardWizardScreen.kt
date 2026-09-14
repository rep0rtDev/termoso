package com.termoso.android.ui.forwarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.PickerRow
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.vault.vaultLabel
import com.termoso.core.PfKind

/**
 * Termius-style first step: pick Local / Remote / Dynamic and the vault, read
 * what the kind does, then continue to the rule form (or skip straight to it).
 */
@Composable
fun ForwardWizardScreen(shell: ShellViewModel, onBack: () -> Unit, onContinue: (PfKind, String) -> Unit) {
    val vaults by shell.vaults.collectAsStateWithLifecycle()
    val selectedVault by shell.selectedVaultId.collectAsStateWithLifecycle()
    val open = vaults.filter { !it.locked }
    var kind by rememberSaveable { mutableStateOf(PfKind.LOCAL) }
    var vaultId by rememberSaveable { mutableStateOf(selectedVault ?: open.firstOrNull()?.id) }
    val vault = vaultId?.takeIf { id -> open.any { it.id == id } } ?: open.firstOrNull()?.id

    SubScreen(title = "Port Forwarding", onBack = onBack) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 12.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                PfKind.entries.forEachIndexed { i, k ->
                    SegmentedButton(
                        selected = kind == k,
                        onClick = { kind = k },
                        shape = SegmentedButtonDefaults.itemShape(index = i, count = PfKind.entries.size),
                    ) { Text(shortTitle(k)) }
                }
            }
            if (open.size > 1) {
                Spacer(Modifier.height(16.dp))
                PickerRow(
                    label = "Vault",
                    value = open.firstOrNull { it.id == vault }?.let(::vaultLabel) ?: "",
                    options = open.map { it.id to vaultLabel(it) },
                    selected = vault,
                    onPick = { id -> vaultId = id },
                    empty = null,
                )
            }
            Spacer(Modifier.height(40.dp))
            IconTile(kindIcon(kind), size = 96, tint = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.height(24.dp))
            Text(kindTitle(kind), style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.height(12.dp))
            Text(
                explanation(kind),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(horizontal = 8.dp),
            )
            Spacer(Modifier.height(40.dp))
            Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Button(onClick = { vault?.let { onContinue(kind, it) } }, enabled = vault != null, modifier = Modifier.fillMaxWidth()) {
                    Text("Continue")
                }
                TextButton(onClick = { vault?.let { onContinue(PfKind.LOCAL, it) } }, enabled = vault != null, modifier = Modifier.fillMaxWidth()) {
                    Text("Skip wizard")
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

private fun shortTitle(kind: PfKind): String = when (kind) {
    PfKind.LOCAL -> "Local"
    PfKind.REMOTE -> "Remote"
    PfKind.DYNAMIC -> "Dynamic"
}

private fun explanation(kind: PfKind): String = when (kind) {
    PfKind.LOCAL ->
        "Opens a port on this device and sends everything that connects to it through the SSH host to a " +
            "destination the host can reach — a database or web console on a private network, for example."
    PfKind.REMOTE ->
        "Opens a port on the SSH server and sends everything that connects to it back to this device — " +
            "or another address reachable from here — so a local service becomes reachable from the server side."
    PfKind.DYNAMIC ->
        "Opens a SOCKS5 proxy on this device. Apps pointed at it reach any address through the SSH host, " +
            "the destination is chosen per connection."
}
