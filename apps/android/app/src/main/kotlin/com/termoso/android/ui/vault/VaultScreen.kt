package com.termoso.android.ui.vault

import com.termoso.android.plural
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Cloud
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.CloudQueue
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material.icons.filled.SyncProblem
import androidx.compose.material.icons.filled.Videocam
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.AccountManager
import com.termoso.android.str
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.SyncState
import com.termoso.core.VaultInfo
import com.termoso.core.VaultKind

private data class VaultCounts(
    val hosts: Int = 0,
    val keys: Int = 0,
    val identities: Int = 0,
    val forwards: Int = 0,
    val snippets: Int = 0,
    val known: Int = 0,
    val history: Int = 0,
    val logs: Int = 0,
)

/** Vaults tab: vault picker, cloud/sync indicator, sections (Hosts, Keychain, Port forwarding, Snippets, Known hosts, History). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VaultScreen(
    shell: ShellViewModel,
    account: AccountManager,
    onOpenAccount: () -> Unit,
    onSignIn: () -> Unit,
    onOpenHosts: () -> Unit,
    onOpenKeychain: () -> Unit,
    onOpenForwarding: () -> Unit,
    onOpenSnippets: () -> Unit,
    onOpenKnownHosts: () -> Unit,
    onOpenHistory: () -> Unit,
    onOpenLogs: () -> Unit,
) {
    val vaults by shell.vaults.collectAsStateWithLifecycle()
    val selectedId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val selected = vaults.firstOrNull { it.id == selectedId }
    val accountStatus by account.status.collectAsStateWithLifecycle()

    var counts by remember { mutableStateOf(VaultCounts()) }
    LaunchedEffect(selected, revision) {
        val vault = selected ?: return@LaunchedEffect
        val id = vault.id
        counts = runCatching {
            shell.repo.read {
                VaultCounts(
                    hosts = hosts(id).size,
                    keys = keys(id).size,
                    identities = identities(id).size,
                    forwards = pfRules(id).size,
                    snippets = snippets(id).size,
                    known = knownHosts().size,
                    history = history(200u).count { it.belongsTo(vault) },
                    logs = sessionLogs().count { it.vaultId == id },
                )
            }
        }.getOrDefault(VaultCounts())
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { VaultPicker(vaults, selected, onSelect = shell::selectVault) },
                actions = {
                    val signedIn = accountStatus.account != null
                    val sync = accountStatus.sync
                    IconButton(onClick = if (signedIn) onOpenAccount else onSignIn) {
                        Icon(
                            when {
                                !signedIn -> Icons.Filled.CloudQueue
                                sync.state == SyncState.OFFLINE -> Icons.Filled.CloudOff
                                sync.state == SyncState.ERROR -> Icons.Filled.SyncProblem
                                else -> Icons.Filled.Cloud
                            },
                            contentDescription = when {
                                !signedIn -> stringResource(R.string.sign_in_to_sync)
                                sync.state == SyncState.SYNCING -> stringResource(R.string.syncing_3)
                                sync.state == SyncState.OFFLINE -> stringResource(R.string.offline)
                                sync.state == SyncState.ERROR -> stringResource(R.string.sync_failed)
                                else -> stringResource(R.string.synced_2)
                            },
                            tint = when {
                                !signedIn -> MaterialTheme.colorScheme.onSurfaceVariant
                                sync.state == SyncState.ERROR -> MaterialTheme.colorScheme.error
                                else -> MaterialTheme.colorScheme.primary
                            },
                        )
                    }
                },
            )
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            SectionCard {
                ChevronRow(
                    title = stringResource(R.string.hosts),
                    badge = counts.hosts.toString(),
                    leading = { IconTile(Icons.Filled.Dns) },
                    modifier = Modifier.clickable(onClick = onOpenHosts),
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.keychain),
                    subtitle = keychainSubtitle(counts.keys, counts.identities),
                    leading = { IconTile(Icons.Filled.Key) },
                    modifier = Modifier.clickable(onClick = onOpenKeychain),
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.port_forwarding_3),
                    badge = counts.forwards.toString(),
                    leading = { IconTile(Icons.Filled.SwapHoriz) },
                    modifier = Modifier.clickable(onClick = onOpenForwarding),
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.snippets),
                    badge = counts.snippets.toString(),
                    leading = { IconTile(Icons.Filled.Code) },
                    modifier = Modifier.clickable(onClick = onOpenSnippets),
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.known_hosts),
                    badge = counts.known.toString(),
                    leading = { IconTile(Icons.Filled.Fingerprint) },
                    modifier = Modifier.clickable(onClick = onOpenKnownHosts),
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.history),
                    badge = counts.history.toString(),
                    leading = { IconTile(Icons.Filled.History) },
                    modifier = Modifier.clickable(onClick = onOpenHistory),
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.recordings_2),
                    badge = counts.logs.toString(),
                    leading = { IconTile(Icons.Filled.Videocam) },
                    modifier = Modifier.clickable(onClick = onOpenLogs),
                )
            }
            Spacer(Modifier.height(16.dp))
            Text(
                vaultHint(selected),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 4.dp),
            )
            Spacer(Modifier.height(24.dp))
        }
    }
}

private fun keychainSubtitle(keys: Int, identities: Int): String? {
    if (keys == 0 && identities == 0) return null
    val parts = buildList {
        if (keys > 0) add(plural(R.plurals.n_keys, keys, keys))
        if (identities > 0) add(plural(R.plurals.n_identities, identities, identities))
    }
    return parts.joinToString(" · ")
}

private fun vaultHint(vault: VaultInfo?): String = when (vault?.kind) {
    VaultKind.TEAM -> str(R.string.team_vault_shared_with_your_team_end_to)
    VaultKind.PERSONAL -> str(R.string.personal_vault_synced_to_your_devices_end_to)
    else -> str(R.string.local_vault_stored_only_on_this_device_encrypted)
}

@Composable
private fun VaultPicker(vaults: List<VaultInfo>, selected: VaultInfo?, onSelect: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Row(
        Modifier.clickable(enabled = vaults.size > 1) { open = true },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(selected?.let { vaultLabel(it) } ?: stringResource(R.string.vault), fontWeight = FontWeight.SemiBold)
        if (vaults.size > 1) {
            Icon(Icons.Filled.ArrowDropDown, contentDescription = stringResource(R.string.choose_vault))
        }
    }
    DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
        vaults.forEach { v ->
            DropdownMenuItem(
                text = { Text(vaultLabel(v)) },
                trailingIcon = if (v.id == selected?.id) {
                    { Icon(Icons.Filled.Check, contentDescription = null) }
                } else {
                    null
                },
                onClick = { onSelect(v.id); open = false },
            )
        }
    }
}

fun vaultLabel(v: VaultInfo): String = when (v.kind) {
    VaultKind.LOCAL -> str(R.string.local_vault)
    VaultKind.PERSONAL -> str(R.string.personal_vault)
    VaultKind.TEAM -> v.name
}
