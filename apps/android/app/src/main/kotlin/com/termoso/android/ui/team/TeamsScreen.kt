package com.termoso.android.ui.team

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.GroupAdd
import androidx.compose.material.icons.filled.Link
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.R
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.keychain.pasteText
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.TeamCard
import kotlinx.coroutines.launch

/** Settings → Account → Teams: the teams you belong to; create one or join by invitation link. */
@Composable
fun TeamsScreen(
    shell: ShellViewModel,
    onBack: () -> Unit,
    onOpenTeam: (String) -> Unit,
    joinLink: String? = null,
    onJoinLinkShown: () -> Unit = {},
) {
    val vm: TeamsViewModel = viewModel { TeamsViewModel(shell.repo) }
    val state by vm.state.collectAsStateWithLifecycle()
    var menu by remember { mutableStateOf(false) }
    var creating by remember { mutableStateOf(false) }
    var joining by remember { mutableStateOf(false) }
    var prefill by remember { mutableStateOf("") }

    LaunchedEffect(Unit) { vm.reload() }
    LaunchedEffect(joinLink) {
        if (joinLink != null) {
            prefill = joinLink
            joining = true
            onJoinLinkShown()
        }
    }

    LaunchedEffect(state.error) { state.error?.let { shell.notify(it) } }

    SubScreen(
        title = stringResource(R.string.teams),
        onBack = onBack,
        actions = {
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.create_or_join)) }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.create_team)) },
                        leadingIcon = { Icon(Icons.Filled.GroupAdd, contentDescription = null) },
                        onClick = { menu = false; creating = true },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.join_with_invitation_link)) },
                        leadingIcon = { Icon(Icons.Filled.Link, contentDescription = null) },
                        onClick = { menu = false; joining = true },
                    )
                }
            }
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            Spacer(Modifier.height(8.dp))
            when {
                state.loading -> Box(Modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
                }
                state.teams.isEmpty() -> {
                    EmptyState(
                        title = stringResource(R.string.no_team_yet),
                        hint = stringResource(R.string.share_hosts_keys_and_snippets_through_end_to),
                        icon = Icons.Filled.Group,
                        action = {
                            Row(horizontalArrangement = Arrangement.Center) {
                                Button(onClick = { creating = true }) { Text(stringResource(R.string.create_team)) }
                                Spacer(Modifier.size(12.dp))
                                OutlinedButton(onClick = { joining = true }) { Text(stringResource(R.string.join_with_link)) }
                            }
                        },
                    )
                }
                else -> SectionCard {
                    state.teams.forEachIndexed { i, t ->
                        if (i > 0) RowDivider()
                        TeamRow(t, Modifier.clickable { onOpenTeam(t.id) })
                    }
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (creating) {
        CreateTeamDialog(
            onDismiss = { creating = false },
            onCreate = { name ->
                runCatching { vm.create(name) }
                    .onSuccess { shell.notify(str(R.string.team_created, it.name)); onOpenTeam(it.id) }
                    .onFailure { shell.notify(it.userMessage()) }
                    .isSuccess
            },
        )
    }
    if (joining) {
        JoinTeamDialog(
            initial = prefill,
            onDismiss = { joining = false; prefill = "" },
            onJoin = { link ->
                runCatching { vm.join(link) }
                    .onSuccess { shell.notify(str(R.string.joined, it.name)); onOpenTeam(it.id) }
                    .onFailure { shell.notify(it.userMessage()) }
                    .isSuccess
            },
        )
    }
}

@Composable
fun TeamRow(t: TeamCard, modifier: Modifier = Modifier) {
    ChevronRow(
        title = t.name,
        subtitle = "${t.myRole.label()} · ${members(t.memberCount)}",
        leading = { IconTile(Icons.Filled.Group) },
        modifier = modifier,
    )
}

fun members(n: UInt): String = if (n == 1u) str(R.string.s_1_member) else str(R.string.members_2, n)

@Composable
private fun CreateTeamDialog(onDismiss: () -> Unit, onCreate: suspend (String) -> Boolean) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(stringResource(R.string.create_team)) },
        text = {
            Column {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.team_name)) },
                    singleLine = true,
                    enabled = !busy,
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    stringResource(R.string.you_become_the_owner_a_first_team_vault),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    scope.launch {
                        busy = true
                        val ok = onCreate(name.trim())
                        busy = false
                        if (ok) onDismiss()
                    }
                },
                enabled = name.isNotBlank() && !busy,
            ) {
                if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text(stringResource(R.string.create))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun JoinTeamDialog(initial: String, onDismiss: () -> Unit, onJoin: suspend (String) -> Boolean) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var link by remember { mutableStateOf(initial) }
    var busy by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(stringResource(R.string.join_a_team)) },
        text = {
            Column {
                OutlinedTextField(
                    value = link,
                    onValueChange = { link = it },
                    label = { Text(stringResource(R.string.invitation_link_or_token)) },
                    singleLine = true,
                    enabled = !busy,
                    modifier = Modifier.fillMaxWidth(),
                    trailingIcon = {
                        TextButton(onClick = { pasteText(context)?.let { link = it.trim() } }, enabled = !busy) { Text(stringResource(R.string.paste_2)) }
                    },
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    stringResource(R.string.paste_the_link_from_the_invitation_email_or),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    scope.launch {
                        busy = true
                        val ok = onJoin(link.trim())
                        busy = false
                        if (ok) onDismiss()
                    }
                },
                enabled = link.isNotBlank() && !busy,
            ) {
                if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text(stringResource(R.string.join))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text(stringResource(R.string.cancel)) } },
    )
}
