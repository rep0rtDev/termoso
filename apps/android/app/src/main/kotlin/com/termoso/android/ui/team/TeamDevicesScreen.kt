package com.termoso.android.ui.team

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.shell.ShellViewModel
import kotlinx.coroutines.launch
import com.termoso.core.TeamDeviceCard
import com.termoso.core.TeamDevicesCard

/** Device review queue of a team (`devices.approve`). */
@Composable
fun TeamDevicesScreen(shell: ShellViewModel, teamId: String, onBack: () -> Unit) {
    var card by remember { mutableStateOf<TeamDevicesCard?>(null) }
    var reload by remember { mutableIntStateOf(0) }
    val scope = rememberCoroutineScope()
    val ctx = LocalContext.current
    LaunchedEffect(teamId, reload) {
        runCatching { shell.repo.read { teamDevices(teamId) } }
            .onSuccess { card = it }
            .onFailure { shell.notify(it.userMessage()) }
    }
    fun mutate(done: Int, block: suspend () -> Unit) {
        scope.launch {
            runCatching { block() }
                .onSuccess { reload++; shell.notify(ctx.getString(done)) }
                .onFailure { shell.notify(it.userMessage()) }
        }
    }
    SubScreen(title = stringResource(R.string.devices_awaiting_approval), onBack = onBack) { padding ->
        val c = card
        if (c == null) {
            Column(Modifier.fillMaxSize().padding(padding), horizontalAlignment = Alignment.CenterHorizontally) {
                Spacer(Modifier.height(48.dp)); CircularProgressIndicator()
            }
            return@SubScreen
        }
        val pending = c.devices.filter { it.status == "pending" }
        val decided = c.devices.filter { it.status != "pending" }
        LazyColumn(Modifier.fillMaxSize().padding(padding)) {
            item {
                Text(
                    stringResource(R.string.devices_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(16.dp),
                )
            }
            if (pending.isEmpty()) {
                item { Text(stringResource(R.string.no_pending_devices), modifier = Modifier.padding(16.dp)) }
            }
            items(pending, key = { it.deviceId }) { d ->
                DeviceRow(d) {
                    Row {
                        TextButton(onClick = { mutate(R.string.device_rejected) { shell.repo.read { teamRejectDevice(teamId, d.deviceId) } } }) {
                            Text(stringResource(R.string.reject), color = MaterialTheme.colorScheme.error)
                        }
                        Button(onClick = { mutate(R.string.device_approved) { shell.repo.read { teamApproveDevice(teamId, d.deviceId) } } }) {
                            Text(stringResource(R.string.approve))
                        }
                    }
                }
            }
            if (decided.isNotEmpty()) {
                item {
                    Text(
                        stringResource(R.string.reviewed_devices),
                        style = MaterialTheme.typography.titleSmall,
                        modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 8.dp),
                    )
                }
                items(decided, key = { it.deviceId }) { d ->
                    DeviceRow(d) {
                        Text(
                            stringResource(if (d.status == "approved") R.string.approved else R.string.rejected),
                            style = MaterialTheme.typography.labelMedium,
                            color = if (d.status == "approved") MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
                        )
                    }
                }
            }
            item { Spacer(Modifier.height(24.dp)) }
        }
    }
}

@Composable
private fun DeviceRow(d: TeamDeviceCard, trailing: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {
        Text(d.userEmail, style = MaterialTheme.typography.bodyMedium)
        Text(
            listOfNotNull(d.name, d.platform, d.appVersion, d.lastIp).joinToString(" · "),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = androidx.compose.foundation.layout.Arrangement.End) { trailing() }
    }
    HorizontalDivider(Modifier.padding(start = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
}
