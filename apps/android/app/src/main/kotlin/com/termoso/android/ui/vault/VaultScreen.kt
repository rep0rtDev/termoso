package com.termoso.android.ui.vault

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Cloud
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.CloudQueue
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material.icons.filled.SyncProblem
import androidx.compose.material.icons.filled.Videocam
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.AccountManager
import com.termoso.android.plural
import com.termoso.android.str
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.hosts.PresenceStack
import com.termoso.android.ui.hosts.rememberVaultPresence
import com.termoso.android.ui.hosts.viewersByHost
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
    val history: Int = 0,
    val logs: Int = 0,
)

/**
 * Vaults tab: the current vault's sections — Hosts, Keychain, Port forwarding,
 * Snippets, History, Recordings — plus the device-wide Known hosts. The vault
 * is switched from the centred title, like in Termius.
 */
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
    val selectedVaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vault = vaults.firstOrNull { it.id == selectedVaultId }
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val accountStatus by account.status.collectAsStateWithLifecycle()

    var counts by remember { mutableStateOf(VaultCounts()) }
    var known by remember { mutableStateOf(0) }
    LaunchedEffect(vault, revision) {
        val loaded = runCatching {
            shell.repo.read {
                val perVault = if (vault == null || vault.locked) {
                    VaultCounts()
                } else {
                    VaultCounts(
                        hosts = hosts(vault.id).size,
                        keys = keys(vault.id).size,
                        identities = identities(vault.id).size,
                        forwards = pfRules(vault.id).size,
                        snippets = snippets(vault.id).size,
                        history = history(200u).count { it.belongsTo(vault) },
                        logs = sessionLogs().count { it.vaultId == vault.id },
                    )
                }
                knownHosts().size to perVault
            }
        }.getOrNull() ?: return@LaunchedEffect
        known = loaded.first
        counts = loaded.second
    }

    Scaffold(
        topBar = {
            CenterAlignedTopAppBar(
                title = { VaultPickerTitle(vaults, selectedVaultId, shell::selectVault) },
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
            if (vault != null) {
                VaultHeader(shell, vault)
                VaultSections(
                    vault = vault,
                    counts = counts,
                    onOpenHosts = onOpenHosts,
                    onOpenKeychain = onOpenKeychain,
                    onOpenForwarding = onOpenForwarding,
                    onOpenSnippets = onOpenSnippets,
                    onOpenHistory = onOpenHistory,
                    onOpenLogs = onOpenLogs,
                )
            }
            Spacer(Modifier.height(16.dp))
            SectionCard {
                ChevronRow(
                    title = stringResource(R.string.known_hosts),
                    subtitle = stringResource(R.string.known_hosts_shared_by_all_vaults),
                    badge = known.toString(),
                    leading = { IconTile(Icons.Filled.Fingerprint) },
                    modifier = Modifier.clickable(onClick = onOpenKnownHosts),
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

/** What kind of vault this is and, for team vaults, who is connected right now. */
@Composable
private fun VaultHeader(shell: ShellViewModel, v: VaultInfo) {
    val presence = rememberVaultPresence(shell, v)
    val online = remember(presence) { viewersByHost(presence).values.flatten().sortedBy { it.since } }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(start = 4.dp, end = 4.dp, top = 12.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            if (v.locked) stringResource(R.string.waiting_for_a_key) else vaultHint(v),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Spacer(Modifier.width(10.dp))
        if (v.locked) {
            Icon(Icons.Filled.Lock, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
        } else {
            PresenceStack(shell.repo, online, size = 26)
        }
    }
}

@Composable
private fun VaultSections(
    vault: VaultInfo,
    counts: VaultCounts,
    onOpenHosts: () -> Unit,
    onOpenKeychain: () -> Unit,
    onOpenForwarding: () -> Unit,
    onOpenSnippets: () -> Unit,
    onOpenHistory: () -> Unit,
    onOpenLogs: () -> Unit,
) {
    fun Modifier.section(cb: () -> Unit) = clickable(enabled = !vault.locked, onClick = cb)
    SectionCard {
        ChevronRow(
            title = stringResource(R.string.hosts),
            badge = counts.hosts.toString(),
            leading = { IconTile(Icons.Filled.Dns) },
            modifier = Modifier.section(onOpenHosts),
        )
        RowDivider()
        ChevronRow(
            title = stringResource(R.string.keychain),
            subtitle = keychainSubtitle(counts.keys, counts.identities),
            leading = { IconTile(Icons.Filled.Key) },
            modifier = Modifier.section(onOpenKeychain),
        )
        RowDivider()
        ChevronRow(
            title = stringResource(R.string.port_forwarding_3),
            badge = counts.forwards.toString(),
            leading = { IconTile(Icons.Filled.SwapHoriz) },
            modifier = Modifier.section(onOpenForwarding),
        )
        RowDivider()
        ChevronRow(
            title = stringResource(R.string.snippets),
            badge = counts.snippets.toString(),
            leading = { IconTile(Icons.Filled.Code) },
            modifier = Modifier.section(onOpenSnippets),
        )
        RowDivider()
        ChevronRow(
            title = stringResource(R.string.history),
            badge = counts.history.toString(),
            leading = { IconTile(Icons.Filled.History) },
            modifier = Modifier.section(onOpenHistory),
        )
        RowDivider()
        ChevronRow(
            title = stringResource(R.string.recordings_2),
            badge = counts.logs.toString(),
            leading = { IconTile(Icons.Filled.Videocam) },
            modifier = Modifier.section(onOpenLogs),
        )
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

private fun vaultHint(vault: VaultInfo): String = when (vault.kind) {
    VaultKind.TEAM -> str(R.string.vault_hint_team)
    VaultKind.PERSONAL -> str(R.string.vault_hint_personal)
    VaultKind.LOCAL -> str(R.string.vault_hint_local)
}

fun vaultLabel(v: VaultInfo): String = when (v.kind) {
    VaultKind.LOCAL -> str(R.string.local_vault)
    VaultKind.PERSONAL -> str(R.string.personal_vault)
    VaultKind.TEAM -> v.name
}
