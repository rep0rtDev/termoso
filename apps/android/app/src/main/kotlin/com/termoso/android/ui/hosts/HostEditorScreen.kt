package com.termoso.android.ui.hosts

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.PickerRow
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.keychain.SshIdRows
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.vault.vaultLabel
import com.termoso.core.HostDraft
import com.termoso.core.InheritedInfo
import com.termoso.core.TagItem
import com.termoso.core.VaultInfo

/** New / edit host form. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HostEditorScreen(shell: ShellViewModel, hostId: String?, groupId: String?, onClose: () -> Unit) {
    val initialVault by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vm: HostEditorViewModel = viewModel(key = "host/${hostId ?: "new"}") {
        HostEditorViewModel(shell.repo, hostId, initialVault, groupId)
    }
    val state by vm.state.collectAsStateWithLifecycle()

    LaunchedEffect(state.error) {
        state.error?.let { shell.notify(it); vm.errorShown() }
    }
    LaunchedEffect(state.saved) {
        if (state.saved) onClose()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(if (hostId == null) "New host" else state.draft?.label?.ifBlank { null } ?: "Edit host") },
                navigationIcon = {
                    IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = "Close") }
                },
                actions = {
                    IconButton(onClick = vm::save, enabled = state.canSave) {
                        if (state.saving) {
                            CircularProgressIndicator(modifier = Modifier.height(20.dp), strokeWidth = 2.dp)
                        } else {
                            Icon(Icons.Filled.Check, contentDescription = "Save")
                        }
                    }
                },
            )
        },
    ) { padding ->
        val draft = state.draft
        if (state.loading || draft == null) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                if (state.loading) CircularProgressIndicator()
            }
            return@Scaffold
        }
        HostForm(
            state = state,
            draft = draft,
            vm = vm,
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        )
    }
}

@Composable
private fun HostForm(state: HostEditorState, draft: HostDraft, vm: HostEditorViewModel, modifier: Modifier) {
    var groupPicker by remember { mutableStateOf(false) }
    var tagPicker by remember { mutableStateOf(false) }
    var more by remember { mutableStateOf(hasAdvanced(draft)) }
    val inherited = state.inherited
    val groupPaths = remember(state.groups) { groupPaths(state.groups) }
    val identity = state.identities.firstOrNull { it.id == draft.identityId }

    Column(modifier) {
        if (draft.id == null && state.vaults.size > 1) {
            VaultRow(state.vaults, draft.vaultId, onSelect = vm::setVault)
        } else {
            Spacer(Modifier.height(12.dp))
        }

        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(draft.label, { v -> vm.update { it.copy(label = v) } }, "Alias")
                FormField(
                    draft.address,
                    { v -> vm.update { it.copy(address = v) } },
                    "Hostname or IP address",
                    keyboard = KeyboardType.Uri,
                )
            }
            RowDivider()
            ChevronRow(
                title = "Group",
                badge = draft.groupId?.let { groupPaths[it] } ?: "None",
                modifier = Modifier.clickable { groupPicker = true },
            )
            RowDivider()
            ChevronRow(
                title = "Tags",
                badge = tagSummary(draft, state.tags),
                modifier = Modifier.clickable { tagPicker = true },
            )
        }

        SectionLabel("SSH")
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(
                    draft.port?.toString() ?: "",
                    { v -> vm.update { it.copy(port = v.filter(Char::isDigit).take(5).toUIntOrNull()?.takeIf { p -> p in 1u..65535u }?.toUShort()) } },
                    "Port",
                    placeholder = inherited?.port?.toString() ?: "22",
                    keyboard = KeyboardType.Number,
                )
                IdentityRow(state, draft, onPick = { id -> vm.update { it.copy(identityId = id) } })
                if (identity == null) {
                    FormField(
                        draft.username,
                        { v -> vm.update { it.copy(username = v) } },
                        "Username",
                        placeholder = inherited?.username ?: if (draft.sshId || inherited?.sshId == true) "SSH ID handle" else "root",
                    )
                    PasswordField(draft, inherited, onChange = { v -> vm.update { it.copy(password = v) } })
                    KeyRow(state, draft, onPick = { id -> vm.update { it.copy(sshKeyId = id) } })
                    SshIdRows(
                        sshId = draft.sshId,
                        keyType = draft.sshIdKeyType,
                        onSshId = { v -> vm.update { it.copy(sshId = v) } },
                        onKeyType = { v -> vm.update { it.copy(sshIdKeyType = v) } },
                        usernameHint = draft.username.isBlank() && inherited?.username.isNullOrBlank(),
                    )
                } else {
                    Text(
                        if (identity.sshId) {
                            "Username, password, key and SSH ID come from the identity “${identity.label}”."
                        } else {
                            "Username, password and key come from the identity “${identity.label}”."
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            RowDivider()
            SwitchRow(
                title = "Agent forwarding",
                checked = draft.agentForwarding,
                onCheckedChange = { v -> vm.update { it.copy(agentForwarding = v) } },
            )
            if (state.snippets.isNotEmpty() || draft.startupSnippetId != null) {
                RowDivider()
                Box(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                    StartupSnippetRow(state, draft, onPick = { id -> vm.update { it.copy(startupSnippetId = id) } })
                }
            }
        }

        Row(
            Modifier
                .fillMaxWidth()
                .clickable { more = !more }
                .padding(vertical = 16.dp, horizontal = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(if (more) "Hide advanced" else "Show advanced", color = MaterialTheme.colorScheme.primary)
            Icon(
                if (more) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
            )
        }

        if (more) {
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    IpVersionRow(draft.ipVersion) { v -> vm.update { it.copy(ipVersion = v) } }
                    FormField(
                        draft.keepAliveInterval?.toString() ?: "",
                        { v -> vm.update { it.copy(keepAliveInterval = v.filter(Char::isDigit).take(5).toUIntOrNull()) } },
                        "Keep-alive interval, seconds",
                        placeholder = "App default",
                        keyboard = KeyboardType.Number,
                    )
                    FormField(
                        draft.timeout?.toString() ?: "",
                        { v -> vm.update { it.copy(timeout = v.filter(Char::isDigit).take(4).toUIntOrNull()) } },
                        "Connection timeout, seconds",
                        placeholder = "App default",
                        keyboard = KeyboardType.Number,
                    )
                }
                RowDivider()
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Environment variables", style = MaterialTheme.typography.labelLarge)
                    draft.envVariables.forEachIndexed { i, env ->
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            OutlinedTextField(
                                value = env.name,
                                onValueChange = { vm.setEnv(i, it, env.value) },
                                label = { Text("Name") },
                                singleLine = true,
                                modifier = Modifier.weight(1f),
                                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters, autoCorrectEnabled = false),
                            )
                            OutlinedTextField(
                                value = env.value,
                                onValueChange = { vm.setEnv(i, env.name, it) },
                                label = { Text("Value") },
                                singleLine = true,
                                modifier = Modifier.weight(1f),
                                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
                            )
                            IconButton(onClick = { vm.removeEnv(i) }) { Icon(Icons.Filled.Close, contentDescription = "Remove") }
                        }
                    }
                    TextButton(onClick = vm::addEnv) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                        Text("Add variable")
                    }
                }
                RowDivider()
                Column(Modifier.padding(16.dp)) {
                    OutlinedTextField(
                        value = draft.notes,
                        onValueChange = { v -> vm.update { it.copy(notes = v) } },
                        label = { Text("Notes") },
                        minLines = 3,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        }
        Spacer(Modifier.height(32.dp))
    }

    if (groupPicker) {
        GroupPickerDialog(
            title = "Group",
            groups = state.groups,
            current = draft.groupId,
            onPick = { vm.setGroup(it); groupPicker = false },
            onDismiss = { groupPicker = false },
        )
    }
    if (tagPicker) {
        TagPickerDialog(
            tags = state.tags,
            selected = draft.tagIds.toSet(),
            onToggle = vm::toggleTag,
            onCreate = vm::createTag,
            onDismiss = { tagPicker = false },
        )
    }
}

@Composable
private fun StartupSnippetRow(state: HostEditorState, draft: HostDraft, onPick: (String?) -> Unit) {
    val snippet = state.snippets.firstOrNull { it.id == draft.startupSnippetId }
    PickerRow(
        label = "Startup snippet",
        value = snippet?.label ?: if (draft.startupSnippetId != null) "Unknown snippet" else "None",
        options = listOf<Pair<String?, String>>(null to "None") + state.snippets.map { it.id to it.label },
        selected = draft.startupSnippetId,
        onPick = onPick,
        empty = null,
    )
}

private fun hasAdvanced(d: HostDraft) =
    d.envVariables.isNotEmpty() || d.keepAliveInterval != null || d.timeout != null || d.notes.isNotBlank() || d.ipVersion != "auto"

private fun tagSummary(draft: HostDraft, tags: List<TagItem>): String {
    val labels = draft.tagIds.mapNotNull { id -> tags.firstOrNull { it.id == id }?.label }
    return when {
        labels.isEmpty() -> "None"
        labels.size <= 2 -> labels.joinToString(", ")
        else -> "${labels.take(2).joinToString(", ")} +${labels.size - 2}"
    }
}

@Composable
private fun PasswordField(draft: HostDraft, inherited: InheritedInfo?, onChange: (String?) -> Unit) {
    var visible by remember { mutableStateOf(false) }
    val stored = draft.password == null && draft.hasPassword
    FormField(
        value = draft.password ?: "",
        onChange = { onChange(it) },
        label = if (stored) "Password · saved" else "Password",
        placeholder = when {
            stored -> "••••••••"
            inherited?.hasPassword == true -> "Inherited from group"
            else -> null
        },
        keyboard = KeyboardType.Password,
        visual = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
        trailing = {
            Row {
                if (draft.hasPassword) {
                    TextButton(onClick = { onChange(if (stored) "" else null) }) {
                        Text(if (stored) "Clear" else "Keep saved")
                    }
                }
                IconButton(onClick = { visible = !visible }) {
                    Icon(
                        if (visible) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                        contentDescription = if (visible) "Hide password" else "Show password",
                    )
                }
            }
        },
    )
}

@Composable
private fun KeyRow(state: HostEditorState, draft: HostDraft, onPick: (String?) -> Unit) {
    val key = state.keys.firstOrNull { it.id == draft.sshKeyId }
    PickerRow(
        label = "Key",
        value = key?.label ?: state.inherited?.sshKeyLabel?.let { "$it (inherited)" } ?: "None",
        options = listOf<Pair<String?, String>>(null to "None") + state.keys.map { it.id to "${it.label} · ${it.keyType}" },
        selected = draft.sshKeyId,
        onPick = onPick,
        empty = "No keys in this vault yet — add one in Keychain.",
    )
}

@Composable
private fun IdentityRow(state: HostEditorState, draft: HostDraft, onPick: (String?) -> Unit) {
    if (state.identities.isEmpty() && draft.identityId == null) return
    val identity = state.identities.firstOrNull { it.id == draft.identityId }
    PickerRow(
        label = "Identity",
        value = identity?.label ?: state.inherited?.identityLabel?.let { "$it (inherited)" } ?: "None",
        options = listOf<Pair<String?, String>>(null to "None") + state.identities.map { it.id to "${it.label} · ${it.username}" },
        selected = draft.identityId,
        onPick = onPick,
        empty = null,
    )
}

@Composable
private fun IpVersionRow(value: String, onPick: (String) -> Unit) {
    val options = listOf("auto" to "Auto", "ipv4" to "IPv4 only", "ipv6" to "IPv6 only")
    PickerRow(
        label = "IP version",
        value = options.firstOrNull { it.first == value }?.second ?: value,
        options = options.map { it.first to it.second },
        selected = value,
        onPick = { onPick(it ?: "auto") },
        empty = null,
    )
}

@Composable
private fun VaultRow(vaults: List<VaultInfo>, selected: String, onSelect: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(Modifier.fillMaxWidth()) {
        ListRow(
            title = vaults.firstOrNull { it.id == selected }?.let { vaultLabel(it) } ?: "Vault",
            subtitle = "Vault",
            modifier = Modifier.clickable { open = true },
        ) { Icon(Icons.Filled.ArrowDropDown, contentDescription = "Choose vault") }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            vaults.forEach { v ->
                DropdownMenuItem(text = { Text(vaultLabel(v)) }, onClick = { onSelect(v.id); open = false })
            }
        }
    }
}

@Composable
private fun TagPickerDialog(
    tags: List<TagItem>,
    selected: Set<String>,
    onToggle: (String) -> Unit,
    onCreate: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var newTag by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Tags") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                if (tags.isEmpty()) {
                    Text("No tags yet.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                tags.sortedBy { it.label.lowercase() }.forEach { tag ->
                    Row(
                        Modifier.fillMaxWidth().clickable { onToggle(tag.id) },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(checked = tag.id in selected, onCheckedChange = { onToggle(tag.id) })
                        Text(tag.label, Modifier.weight(1f))
                        Text("${tag.hosts}", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = newTag,
                    onValueChange = { newTag = it },
                    label = { Text("New tag") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                    trailingIcon = {
                        IconButton(
                            onClick = { onCreate(newTag); newTag = "" },
                            enabled = newTag.isNotBlank(),
                        ) { Icon(Icons.Filled.Add, contentDescription = "Add tag") }
                    },
                )
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } },
    )
}
