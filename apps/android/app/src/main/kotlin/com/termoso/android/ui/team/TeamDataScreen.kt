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
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.RotationCard
import com.termoso.core.TeamDataCard

/** Read-only: data region, scheduled deletion and secrets due for rotation.
 *  Export, deletion and rotation jobs live in the web cabinet and the desktop app. */
@Composable
fun TeamDataScreen(shell: ShellViewModel, teamId: String, canRotation: Boolean, onBack: () -> Unit) {
    var data by remember { mutableStateOf<TeamDataCard?>(null) }
    var rotation by remember { mutableStateOf<RotationCard?>(null) }
    LaunchedEffect(teamId) {
        runCatching { shell.repo.read { teamData(teamId) } }
            .onSuccess { data = it }
            .onFailure { shell.notify(it.userMessage()) }
        if (canRotation) {
            runCatching { shell.repo.read { teamRotation(teamId) } }.onSuccess { rotation = it }
        }
    }
    SubScreen(title = stringResource(R.string.team_data), onBack = onBack) { padding ->
        val d = data
        if (d == null) {
            Column(Modifier.fillMaxSize().padding(padding), horizontalAlignment = Alignment.CenterHorizontally) {
                Spacer(Modifier.height(48.dp)); CircularProgressIndicator()
            }
            return@SubScreen
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding)) {
            item { InfoRow(stringResource(R.string.data_region), d.region ?: stringResource(R.string.not_specified)) }
            item { InfoRow(stringResource(R.string.created), d.createdAt.take(10)) }
            d.deletionScheduledAt?.let { at ->
                item {
                    Text(
                        stringResource(R.string.deletion_scheduled_for, at.take(16).replace('T', ' ')),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            }
            item {
                Text(
                    stringResource(R.string.team_data_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(16.dp),
                )
            }
            rotation?.let { r ->
                item {
                    Text(
                        stringResource(R.string.due_for_rotation),
                        style = MaterialTheme.typography.titleSmall,
                        modifier = Modifier.padding(start = 16.dp, top = 16.dp, bottom = 8.dp),
                    )
                }
                if (r.keyMaxAgeDays == 0 && r.passwordMaxAgeDays == 0) {
                    item { Text(stringResource(R.string.rotation_not_configured), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 16.dp)) }
                } else if (r.due.isEmpty()) {
                    item { Text(stringResource(R.string.nothing_is_due), modifier = Modifier.padding(horizontal = 16.dp)) }
                }
                items(r.due, key = { it.entityId }) { e ->
                    InfoRow(
                        stringResource(
                            when (e.kind) {
                                "key" -> R.string.ssh_key
                                "certificate" -> R.string.certificate
                                "identity" -> R.string.identity
                                else -> R.string.host_password
                            },
                        ) + " · " + e.entityId.take(8),
                        stringResource(R.string.days_n, e.ageDays) + if (e.jobQueued) " · " + stringResource(R.string.job_queued) else "",
                    )
                }
                if (r.pendingJobs > 0) {
                    item { Text(stringResource(R.string.rotation_jobs_pending, r.pendingJobs), style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(16.dp)) }
                }
            }
            item { Spacer(Modifier.height(24.dp)) }
        }
    }
}

@Composable
private fun InfoRow(k: String, v: String) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {
        Text(k, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
        Text(v, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
    HorizontalDivider(Modifier.padding(start = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
}
