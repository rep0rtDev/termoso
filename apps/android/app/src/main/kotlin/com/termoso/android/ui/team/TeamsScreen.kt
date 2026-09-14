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
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.data.userMessage
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
        title = "Teams",
        onBack = onBack,
        actions = {
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Filled.Add, contentDescription = "Create or join") }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text("Create team") },
                        leadingIcon = { Icon(Icons.Filled.GroupAdd, contentDescription = null) },
                        onClick = { menu = false; creating = true },
                    )
                    DropdownMenuItem(
                        text = { Text("Join with invitation link") },
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
                        title = "No team yet",
                        hint = "Share hosts, keys and snippets through end-to-end encrypted team vaults. " +
                            "The server only ever stores ciphertext.",
                    )
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
                        Button(onClick = { creating = true }) { Text("Create team") }
                        Spacer(Modifier.size(12.dp))
                        OutlinedButton(onClick = { joining = true }) { Text("Join with link") }
                    }
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
                    .onSuccess { shell.notify("Team \"${it.name}\" created"); onOpenTeam(it.id) }
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
                    .onSuccess { shell.notify("Joined \"${it.name}\""); onOpenTeam(it.id) }
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

fun members(n: UInt): String = if (n == 1u) "1 member" else "$n members"

@Composable
private fun CreateTeamDialog(onDismiss: () -> Unit, onCreate: suspend (String) -> Boolean) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text("Create team") },
        text = {
            Column {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text("Team name") },
                    singleLine = true,
                    enabled = !busy,
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    "You become the owner. A first team vault named after the team is created with you as manager; " +
                        "its key is generated on this phone and sealed to each member you grant access to.",
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
                if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text("Create")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text("Cancel") } },
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
        title = { Text("Join a team") },
        text = {
            Column {
                OutlinedTextField(
                    value = link,
                    onValueChange = { link = it },
                    label = { Text("Invitation link or token") },
                    singleLine = true,
                    enabled = !busy,
                    modifier = Modifier.fillMaxWidth(),
                    trailingIcon = {
                        TextButton(onClick = { pasteText(context)?.let { link = it.trim() } }, enabled = !busy) { Text("Paste") }
                    },
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    "Paste the link from the invitation email or message. It must be for the account you are signed in with.",
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
                if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text("Join")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text("Cancel") } },
    )
}
