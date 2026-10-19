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
import com.termoso.core.AccessRequestCard
import com.termoso.core.AccessRequestsCard
import androidx.compose.runtime.collectAsState
import com.termoso.android.data.AccountManager

/** Just-in-time access requests of a team: ask, withdraw, approve/deny/revoke. */
@Composable
fun TeamAccessScreen(shell: ShellViewModel, account: AccountManager, teamId: String, onBack: () -> Unit) {
    var card by remember { mutableStateOf<AccessRequestsCard?>(null) }
    var myId by remember { mutableStateOf<String?>(null) }
    val status by account.status.collectAsState()
    val teamVaults = status.vaults.filter { it.teamId == teamId }.map { it.id to it.name }
    var reload by remember { mutableIntStateOf(0) }
    var requesting by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val ctx = LocalContext.current
    LaunchedEffect(teamId, reload) {
        runCatching { shell.repo.read { teamAccessRequests(teamId) } }
            .onSuccess { card = it }
            .onFailure { shell.notify(it.userMessage()) }
        if (myId == null) {
            runCatching { shell.repo.read { teamMembers(teamId) } }
                .onSuccess { list -> myId = list.firstOrNull { it.me }?.userId }
        }
    }
    fun mutate(done: Int?, block: suspend () -> Unit) {
        scope.launch {
            runCatching { block() }
                .onSuccess { reload++; done?.let { shell.notify(ctx.getString(it)) } }
                .onFailure { shell.notify(it.userMessage()) }
        }
    }
    SubScreen(
        title = stringResource(R.string.access_requests),
        onBack = onBack,
        actions = { TextButton(onClick = { requesting = true }) { Text(stringResource(R.string.request_access)) } },
    ) { padding ->
        val c = card
        if (c == null) {
            Column(Modifier.fillMaxSize().padding(padding), horizontalAlignment = Alignment.CenterHorizontally) {
                Spacer(Modifier.height(48.dp)); CircularProgressIndicator()
            }
            return@SubScreen
        }
        val open = c.requests.filter { it.status == "pending" || it.status == "approved" }
        val closed = c.requests.filter { it.status != "pending" && it.status != "approved" }
        LazyColumn(Modifier.fillMaxSize().padding(padding)) {
            item {
                Text(
                    stringResource(R.string.access_requests_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(16.dp),
                )
            }
            if (open.isEmpty()) item { Text(stringResource(R.string.no_open_requests), modifier = Modifier.padding(16.dp)) }
            items(open, key = { it.id }) { r ->
                RequestRow(r, mine = r.userId == myId) {
                    Row {
                        if (r.status == "pending" && r.userId == myId) {
                            TextButton(onClick = { mutate(null) { shell.repo.read { teamCancelAccessRequest(teamId, r.id) } } }) { Text(stringResource(R.string.withdraw)) }
                        }
                        if (r.status == "pending" && c.canApprove) {
                            TextButton(onClick = { mutate(null) { shell.repo.read { teamDecideAccessRequest(teamId, r.id, "deny", null, null) } } }) {
                                Text(stringResource(R.string.deny), color = MaterialTheme.colorScheme.error)
                            }
                            Button(onClick = { mutate(R.string.access_granted_seal_hint) { shell.repo.read { teamDecideAccessRequest(teamId, r.id, "approve", null, null) } } }) {
                                Text(stringResource(R.string.approve))
                            }
                        }
                        if (r.status == "approved" && c.canApprove) {
                            TextButton(onClick = { mutate(null) { shell.repo.read { teamDecideAccessRequest(teamId, r.id, "revoke", null, null) } } }) {
                                Text(stringResource(R.string.revoke), color = MaterialTheme.colorScheme.error)
                            }
                        }
                    }
                }
            }
            if (closed.isNotEmpty()) {
                item { Text(stringResource(R.string.history), style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 8.dp)) }
                items(closed, key = { it.id }) { r -> RequestRow(r, mine = r.userId == myId) {} }
            }
            item { Spacer(Modifier.height(24.dp)) }
        }
    }
    if (requesting) {
        RequestAccessDialog(
            vaults = teamVaults,
            onDismiss = { requesting = false },
            onSubmit = { vaultId, role, hours, reason ->
                requesting = false
                mutate(R.string.request_sent) { shell.repo.read { teamRequestAccess(teamId, vaultId, role, hours, reason) } }
            },
        )
    }
}

@Composable
private fun statusLabel(s: String): String = stringResource(
    when (s) {
        "pending" -> R.string.pending
        "approved" -> R.string.active
        "denied" -> R.string.denied
        "revoked" -> R.string.revoked
        "expired" -> R.string.expired
        else -> R.string.cancelled_by_author
    },
)

@Composable
private fun RequestRow(r: AccessRequestCard, mine: Boolean, trailing: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {
        Text(
            "${if (mine) stringResource(R.string.you) else r.userEmail} → ${r.vaultName}",
            style = MaterialTheme.typography.bodyMedium,
        )
        Text(
            listOf(
                stringResource(if (r.role == "editor") R.string.editor else R.string.viewer),
                stringResource(R.string.hours_short, r.hours),
                statusLabel(r.status),
            ).joinToString(" · ") + (r.expiresAt?.takeIf { r.status == "approved" }?.let { " · " + stringResource(R.string.until_when, it.take(16).replace('T', ' ')) } ?: ""),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (r.reason.isNotBlank()) Text(r.reason, style = MaterialTheme.typography.bodySmall)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = androidx.compose.foundation.layout.Arrangement.End) { trailing() }
    }
    HorizontalDivider(Modifier.padding(start = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
}

@Composable
private fun RequestAccessDialog(
    vaults: List<Pair<String, String>>,
    onDismiss: () -> Unit,
    onSubmit: (vaultId: String, role: String, hours: Int, reason: String) -> Unit,
) {
    var vaultIdx by remember { mutableIntStateOf(0) }
    var editor by remember { mutableStateOf(false) }
    var hours by remember { mutableStateOf("8") }
    var reason by remember { mutableStateOf("") }
    val h = hours.toIntOrNull() ?: 0
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.request_access)) },
        text = {
            Column {
                if (vaults.size > 1) {
                    Row(Modifier.fillMaxWidth()) {
                        TextButton(onClick = { vaultIdx = (vaultIdx + 1) % vaults.size }) { Text(vaults.getOrNull(vaultIdx)?.second ?: "") }
                    }
                } else {
                    Text(vaults.firstOrNull()?.second ?: "", style = MaterialTheme.typography.bodyMedium)
                }
                Row(Modifier.fillMaxWidth()) {
                    TextButton(onClick = { editor = false }, enabled = editor) { Text(stringResource(R.string.viewer)) }
                    TextButton(onClick = { editor = true }, enabled = !editor) { Text(stringResource(R.string.editor)) }
                }
                OutlinedTextField(value = hours, onValueChange = { hours = it.filter(Char::isDigit).take(3) }, label = { Text(stringResource(R.string.hours)) }, singleLine = true)
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(value = reason, onValueChange = { reason = it }, label = { Text(stringResource(R.string.reason)) }, minLines = 2)
            }
        },
        confirmButton = {
            TextButton(
                enabled = vaults.isNotEmpty() && h in 1..720 && reason.isNotBlank(),
                onClick = { onSubmit(vaults[vaultIdx].first, if (editor) "editor" else "viewer", h, reason.trim()) },
            ) { Text(stringResource(R.string.request_access)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(android.R.string.cancel)) } },
    )
}
