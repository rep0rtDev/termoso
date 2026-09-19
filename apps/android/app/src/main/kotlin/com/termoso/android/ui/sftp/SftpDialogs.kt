package com.termoso.android.ui.sftp

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Cancel
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Upload
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.IconTile
import com.termoso.core.SftpEntry
import com.termoso.core.TransferCard
import com.termoso.core.TransferDirection
import com.termoso.core.TransferStatus

/** chmod editor: three rows of r/w/x checkboxes plus the octal, kept in sync both ways. */
@Composable
fun PermissionsDialog(entry: SftpEntry, onConfirm: (UInt) -> Unit, onDismiss: () -> Unit) {
    var mode by remember { mutableStateOf((entry.mode ?: 0u) and 0x1FFu) }
    var octal by remember { mutableStateOf(mode.toString(8).padStart(3, '0')) }

    fun set(next: UInt) {
        mode = next and 0x1FFu
        octal = mode.toString(8).padStart(3, '0')
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.permissions)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(entry.name, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                Spacer(Modifier.height(4.dp))
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Spacer(Modifier.width(72.dp))
                    listOf(stringResource(R.string.read), stringResource(R.string.write), stringResource(R.string.exec)).forEach {
                        Text(it, Modifier.weight(1f), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                listOf(stringResource(R.string.owner) to 6, stringResource(R.string.group) to 3, stringResource(R.string.others) to 0).forEach { (label, shift) ->
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Text(label, Modifier.width(72.dp), style = MaterialTheme.typography.bodyMedium)
                        listOf(4u, 2u, 1u).forEach { bit ->
                            val mask = bit shl shift
                            Checkbox(
                                checked = mode and mask != 0u,
                                onCheckedChange = { on -> set(if (on) mode or mask else mode and mask.inv()) },
                                modifier = Modifier.weight(1f),
                            )
                        }
                    }
                }
                OutlinedTextField(
                    value = octal,
                    onValueChange = { v ->
                        octal = v.filter { it in '0'..'7' }.take(4)
                        octal.toUIntOrNull(8)?.let { mode = it and 0x1FFu }
                    },
                    label = { Text(stringResource(R.string.octal)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
            }
        },
        confirmButton = { Button(onClick = { onConfirm(mode) }) { Text(stringResource(R.string.apply)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Bottom sheet with every transfer of the connection: progress, speed, pause/resume/retry, cancel/dismiss. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TransfersSheet(
    transfers: List<TransferCard>,
    onPause: (ULong) -> Unit,
    onResume: (ULong) -> Unit,
    onCancel: (ULong) -> Unit,
    onDismissCard: (ULong) -> Unit,
    onClearFinished: () -> Unit,
    onClose: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onClose) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(stringResource(R.string.transfers), style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
            if (transfers.any { it.status.isFinished }) {
                TextButton(onClick = onClearFinished) { Text(stringResource(R.string.clear_finished)) }
            }
        }
        if (transfers.isEmpty()) {
            EmptyState(title = stringResource(R.string.no_transfers), hint = stringResource(R.string.downloads_and_uploads_from_this_connection_show_up), modifier = Modifier.padding(16.dp))
            Spacer(Modifier.height(24.dp))
        } else {
            LazyColumn(contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 32.dp)) {
                items(transfers.asReversed(), key = { it.id.toLong() }) { t ->
                    TransferRow(t, onPause = onPause, onResume = onResume, onCancel = onCancel, onDismiss = onDismissCard)
                }
            }
        }
    }
}

@Composable
private fun TransferRow(
    t: TransferCard,
    onPause: (ULong) -> Unit,
    onResume: (ULong) -> Unit,
    onCancel: (ULong) -> Unit,
    onDismiss: (ULong) -> Unit,
) {
    val total = t.total
    val fraction = if (total != null && total > 0uL) (t.done.toDouble() / total.toDouble()).toFloat().coerceIn(0f, 1f) else null
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconTile(if (t.direction == TransferDirection.DOWNLOAD) Icons.Filled.Download else Icons.Filled.Upload)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(t.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1)
            Text(
                t.remotePath,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
            )
            if (t.status.showsProgress) {
                if (fraction != null) {
                    LinearProgressIndicator(progress = { fraction }, modifier = Modifier.fillMaxWidth())
                } else if (t.status is TransferStatus.Running) {
                    LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                }
            }
            Text(
                t.statusLabel(),
                style = MaterialTheme.typography.labelMedium,
                color = if (t.status is TransferStatus.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 2,
            )
        }
        Spacer(Modifier.width(8.dp))
        t.status.actions.forEach { action ->
            when (action) {
                TransferAction.Pause -> IconButton(onClick = { onPause(t.id) }) { Icon(Icons.Filled.Pause, contentDescription = stringResource(R.string.pause)) }
                TransferAction.Resume -> IconButton(onClick = { onResume(t.id) }) { Icon(Icons.Filled.PlayArrow, contentDescription = stringResource(R.string.resume)) }
                TransferAction.Retry -> IconButton(onClick = { onResume(t.id) }) { Icon(Icons.Filled.Refresh, contentDescription = stringResource(R.string.retry)) }
                TransferAction.Cancel -> IconButton(onClick = { onCancel(t.id) }) { Icon(Icons.Filled.Cancel, contentDescription = stringResource(R.string.cancel)) }
                TransferAction.Dismiss -> IconButton(onClick = { onDismiss(t.id) }) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.dismiss)) }
            }
        }
    }
}
