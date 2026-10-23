package com.termoso.android.ui.keychain

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.clickable
import com.termoso.android.R
import com.termoso.android.data.KeyExportBridge
import com.termoso.android.data.KeyExportState
import com.termoso.android.data.VaultRepository
import com.termoso.android.ui.terminal.PromptDialog
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import com.termoso.core.HostItem
import com.termoso.core.KeyExport
import com.termoso.core.KeyItem

/**
 * ssh-copy-id: runs the export of [keyId] to [hostId] and shows progress,
 * connection prompts and the outcome. [onDone] receives the toast text.
 */
@Composable
fun KeyExportDialog(
    repo: VaultRepository,
    keyLabel: String,
    keyId: String,
    hostId: String,
    vaultId: String,
    onDone: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val bridge = remember { KeyExportBridge() }
    var rust by remember { mutableStateOf<KeyExport?>(null) }
    LaunchedEffect(keyId, hostId) {
        rust = withContext(Dispatchers.IO) {
            runCatching { repo.read { startKeyExport(keyId, hostId, vaultId, bridge) } }
                .onFailure { bridge.onFailed(it.message ?: it.toString()) }
                .getOrNull()
        }
    }
    DisposableEffect(Unit) { onDispose { rust?.cancel() } }

    val state by bridge.state.collectAsState()
    val prompt by bridge.prompt.collectAsState()
    val added = stringResource(R.string.key_added_to_authorized_keys, keyLabel)
    val present = stringResource(R.string.key_already_authorized, keyLabel)
    LaunchedEffect(state) {
        val s = state
        if (s is KeyExportState.Done) {
            onDone((if (s.result.added) added else present) + " " + s.result.target)
            onDismiss()
        }
    }

    prompt?.let { p ->
        PromptDialog(p) { answer ->
            bridge.promptAnswered(p.id)
            rust?.answer(p.id, answer)
        }
    }
    AlertDialog(
        onDismissRequest = {},
        title = { Text(stringResource(R.string.export_key_to_host)) },
        text = {
            when (val s = state) {
                is KeyExportState.Running -> Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp))
                    Spacer(Modifier.width(12.dp))
                    Text(s.detail.ifEmpty { stringResource(R.string.connecting) })
                }
                is KeyExportState.Failed -> Text(s.message, color = MaterialTheme.colorScheme.error)
                is KeyExportState.Done -> Text(s.result.target)
            }
        },
        confirmButton = {
            if (state is KeyExportState.Failed) TextButton(onClick = onDismiss) { Text(stringResource(R.string.close)) }
        },
        dismissButton = {
            if (state is KeyExportState.Running) TextButton(onClick = { rust?.cancel(); onDismiss() }) { Text(stringResource(R.string.cancel)) }
        },
    )
}

/** Pick one of the vault's hosts to authorize [key] on, then run the export. */
@Composable
fun ExportKeyToHostFlow(repo: VaultRepository, key: KeyItem, onNotify: (String) -> Unit, onDismiss: () -> Unit) {
    var host by remember { mutableStateOf<HostItem?>(null) }
    val hosts by produceState<List<HostItem>?>(null, key.vaultId) {
        value = withContext(Dispatchers.IO) { runCatching { repo.read { hosts(key.vaultId) } }.getOrDefault(emptyList()).filter { it.protocol == "ssh" } }
    }
    val picked = host
    if (picked == null) {
        PickerDialog(
            title = stringResource(R.string.export_key_to_host),
            hint = stringResource(R.string.export_key_to_host_hint),
            empty = stringResource(R.string.no_hosts_in_this_vault),
            items = hosts,
            label = { it.label },
            onPick = { host = it },
            onDismiss = onDismiss,
        )
    } else {
        KeyExportDialog(repo, key.label, key.id, picked.id, key.vaultId, onDone = onNotify, onDismiss = onDismiss)
    }
}

/** Pick one of the vault's keys to authorize on [host], then run the export. */
@Composable
fun AddKeyToHostFlow(repo: VaultRepository, host: HostItem, onNotify: (String) -> Unit, onDismiss: () -> Unit) {
    var key by remember { mutableStateOf<KeyItem?>(null) }
    val keys by produceState<List<KeyItem>?>(null, host.vaultId) {
        value = withContext(Dispatchers.IO) { runCatching { repo.read { keys(host.vaultId) } }.getOrDefault(emptyList()) }
    }
    val picked = key
    if (picked == null) {
        PickerDialog(
            title = stringResource(R.string.add_ssh_key_to_host),
            hint = stringResource(R.string.add_ssh_key_to_host_hint, host.label),
            empty = stringResource(R.string.no_keys_in_this_vault),
            items = keys,
            label = { it.label },
            onPick = { key = it },
            onDismiss = onDismiss,
        )
    } else {
        KeyExportDialog(repo, picked.label, picked.id, host.id, host.vaultId, onDone = onNotify, onDismiss = onDismiss)
    }
}

@Composable
private fun <T> PickerDialog(
    title: String,
    hint: String,
    empty: String,
    items: List<T>?,
    label: (T) -> String,
    onPick: (T) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column {
                Text(hint, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(bottom = 8.dp))
                when {
                    items == null -> CircularProgressIndicator(modifier = Modifier.size(20.dp))
                    items.isEmpty() -> Text(empty, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    else -> LazyColumn(modifier = Modifier.fillMaxWidth()) {
                        items(items) { it ->
                            ListItem(
                                headlineContent = { Text(label(it)) },
                                modifier = Modifier.clickable { onPick(it) },
                            )
                        }
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
