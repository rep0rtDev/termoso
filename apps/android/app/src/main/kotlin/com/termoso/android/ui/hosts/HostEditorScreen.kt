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
import androidx.compose.material.icons.filled.CloudQueue
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material.icons.filled.Terminal
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.R
import com.termoso.android.data.VaultRepository
import com.termoso.android.str
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.PickerRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.keychain.SshIdRows
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.terminal.copyToClipboard
import com.termoso.android.ui.vault.vaultLabel
import com.termoso.core.HostDraft
import com.termoso.core.IdentityItem
import com.termoso.core.InheritedInfo
import com.termoso.core.QuickTarget
import com.termoso.core.TagItem
import com.termoso.core.TelnetDraft
import com.termoso.core.VaultInfo
import com.termoso.core.WebDavDraft
import com.termoso.core.moshDefaultServerCommand

/**
 * New / edit host form. The `⋯` menu saves pending edits before it connects,
 * so what the terminal dials is what the form shows.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HostEditorScreen(
    shell: ShellViewModel,
    hostId: String?,
    groupId: String?,
    onClose: () -> Unit,
    prefill: QuickTarget? = null,
    onConnect: (String) -> Unit = {},
    onSftp: (String) -> Unit = {},
    onWebdav: (String) -> Unit = {},
    onForward: (String) -> Unit = {},
) {
    val initialVault by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vm: HostEditorViewModel = viewModel(key = "host/${hostId ?: "new"}") {
        HostEditorViewModel(shell.repo, hostId, initialVault, groupId, prefill)
    }
    val state by vm.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    var menu by remember { mutableStateOf(false) }
    var confirmRemove by remember { mutableStateOf(false) }
    // What to do with the saved host once the write lands ("Connect" from the menu).
    var afterSave by remember { mutableStateOf<((String) -> Unit)?>(null) }

    LaunchedEffect(state.error) {
        state.error?.let { shell.notify(it); vm.errorShown() }
    }
    LaunchedEffect(state.saved) {
        if (!state.saved) return@LaunchedEffect
        onClose()
        val id = state.savedId
        val next = afterSave
        if (id != null && next != null) next(id)
    }

    fun saveThen(action: (String) -> Unit) {
        if (!state.canSave) return
        afterSave = action
        vm.save()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(if (hostId == null) stringResource(R.string.new_host) else state.draft?.label?.ifBlank { null } ?: stringResource(R.string.edit_host)) },
                navigationIcon = {
                    IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close)) }
                },
                actions = {
                    IconButton(onClick = vm::save, enabled = state.canSave) {
                        if (state.saving) {
                            CircularProgressIndicator(modifier = Modifier.height(20.dp), strokeWidth = 2.dp)
                        } else {
                            Icon(Icons.Filled.Check, contentDescription = stringResource(R.string.save))
                        }
                    }
                    val draft = state.draft
                    val savedId = draft?.id
                    if (draft != null) {
                        Box {
                            IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.more)) }
                            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                                if (draft.ssh || draft.telnet != null) {
                                    DropdownMenuItem(
                                        text = { Text(if (savedId == null) stringResource(R.string.save_and_connect) else stringResource(R.string.connect)) },
                                        leadingIcon = { Icon(Icons.Filled.Terminal, null) },
                                        enabled = state.canSave,
                                        onClick = { menu = false; saveThen(onConnect) },
                                    )
                                }
                                if (draft.webdav != null) {
                                    DropdownMenuItem(
                                        text = { Text(if (savedId == null) stringResource(R.string.save_and_open_webdav) else stringResource(R.string.webdav_files)) },
                                        leadingIcon = { Icon(Icons.Filled.CloudQueue, null) },
                                        enabled = state.canSave,
                                        onClick = { menu = false; saveThen(onWebdav) },
                                    )
                                }
                                if (draft.ssh) {
                                    DropdownMenuItem(
                                        text = { Text(if (savedId == null) stringResource(R.string.save_and_open_sftp) else "SFTP") },
                                        leadingIcon = { Icon(Icons.Filled.FolderOpen, null) },
                                        enabled = state.canSave,
                                        onClick = { menu = false; saveThen(onSftp) },
                                    )
                                    if (savedId != null) {
                                        DropdownMenuItem(
                                            text = { Text(stringResource(R.string.port_forwarding_2)) },
                                            leadingIcon = { Icon(Icons.Filled.SwapHoriz, null) },
                                            onClick = { menu = false; onForward(savedId) },
                                        )
                                    }
                                }
                                DropdownMenuItem(
                                    text = { Text(stringResource(R.string.copy_link)) },
                                    leadingIcon = { Icon(Icons.Filled.Link, null) },
                                    enabled = draft.address.isNotBlank(),
                                    onClick = {
                                        menu = false
                                        copyToClipboard(context, draftLink(draft))
                                        shell.notify(str(R.string.link_copied))
                                    },
                                )
                                if (savedId != null) {
                                    DropdownMenuItem(
                                        text = { Text(stringResource(R.string.duplicate)) },
                                        leadingIcon = { Icon(Icons.Filled.ContentCopy, null) },
                                        enabled = !state.saving,
                                        onClick = { menu = false; vm.duplicate() },
                                    )
                                    DropdownMenuItem(
                                        text = { Text(stringResource(R.string.remove_2), color = MaterialTheme.colorScheme.error) },
                                        leadingIcon = { Icon(Icons.Filled.Delete, null, tint = MaterialTheme.colorScheme.error) },
                                        enabled = !state.saving,
                                        onClick = { menu = false; confirmRemove = true },
                                    )
                                }
                            }
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
        val vault = state.vaults.firstOrNull { it.id == draft.vaultId }
        val presence = rememberVaultPresence(shell, vault)
        val viewers = remember(presence, draft.id) { draft.id?.let { viewersByHost(presence)[it] } ?: emptyList() }
        HostForm(
            repo = shell.repo,
            state = state,
            draft = draft,
            vm = vm,
            viewers = viewers,
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        )
    }

    if (confirmRemove) {
        ConfirmDialog(
            title = stringResource(R.string.remove_host),
            text = stringResource(R.string.the_host_is_removed_from_this_vault_keys),
            confirm = stringResource(R.string.remove_2),
            onConfirm = { confirmRemove = false; vm.delete() },
            onDismiss = { confirmRemove = false },
        )
    }
}

/** `ssh://user@host[:port]` from the form as it stands (Telnet-only hosts give `telnet://`, WebDAV-only the share URL). */
fun draftLink(d: HostDraft): String {
    val telnet = d.telnet
    val webdav = d.webdav
    return when {
        d.ssh -> hostLink("ssh", d.username, d.address.trim(), d.port?.toInt() ?: 22)
        telnet != null -> hostLink("telnet", "", d.address.trim(), telnet.port?.toInt() ?: 23)
        webdav != null -> webdav.url.trim()
        else -> hostLink("ssh", d.username, d.address.trim(), d.port?.toInt() ?: 22)
    }
}

@Composable
private fun HostForm(
    repo: VaultRepository,
    state: HostEditorState,
    draft: HostDraft,
    vm: HostEditorViewModel,
    viewers: List<HostViewer>,
    modifier: Modifier,
) {
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

        if (viewers.isNotEmpty()) {
            ConnectedNowSection(repo, viewers)
            Spacer(Modifier.height(12.dp))
        }

        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(draft.label, { v -> vm.update { it.copy(label = v) } }, stringResource(R.string.alias))
                FormField(
                    draft.address,
                    { v -> vm.update { it.copy(address = v) } },
                    stringResource(R.string.hostname_or_ip_address),
                    placeholder = if (webdavOnly(draft)) stringResource(R.string.taken_from_the_webdav_url) else null,
                    keyboard = KeyboardType.Uri,
                )
            }
            RowDivider()
            ChevronRow(
                title = stringResource(R.string.group),
                badge = draft.groupId?.let { groupPaths[it] } ?: stringResource(R.string.none),
                modifier = Modifier.clickable { groupPicker = true },
            )
            RowDivider()
            ChevronRow(
                title = stringResource(R.string.tags),
                badge = tagSummary(draft, state.tags),
                modifier = Modifier.clickable { tagPicker = true },
            )
        }

        val telnet = draft.telnet
        val webdav = draft.webdav
        val sections = (if (draft.ssh) 1 else 0) + (if (telnet != null) 1 else 0) + (if (webdav != null) 1 else 0)
        if (draft.ssh) {
            ProtocolHeader("SSH", removable = sections > 1) { vm.update { it.copy(ssh = false) } }
            SshSection(state, draft, vm, inherited, identity)
        }

        if (telnet != null) {
            ProtocolHeader("Telnet", removable = sections > 1) { vm.update { it.copy(telnet = null) } }
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    FormField(
                        telnet.port?.toString() ?: "",
                        { v -> vm.updateTelnet { it.copy(port = v.filter(Char::isDigit).take(5).toUIntOrNull()?.takeIf { p -> p in 1u..65535u }?.toUShort()) } },
                        stringResource(R.string.port),
                        placeholder = "23",
                        keyboard = KeyboardType.Number,
                    )
                    FormField(telnet.username, { v -> vm.updateTelnet { it.copy(username = v) } }, stringResource(R.string.username))
                    PasswordField(
                        password = telnet.password,
                        hasPassword = telnet.hasPassword,
                        inheritedHint = false,
                        onChange = { v -> vm.updateTelnet { it.copy(password = v) } },
                    )
                    Text(
                        stringResource(R.string.telnet_is_not_encrypted_anything_typed_including_the),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        if (webdav != null) {
            ProtocolHeader("WebDAV", removable = sections > 1) { vm.update { it.copy(webdav = null) } }
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    FormField(
                        webdav.url,
                        { v -> vm.updateWebdav { it.copy(url = v) } },
                        stringResource(R.string.webdav_url),
                        placeholder = "https://cloud.example.com/remote.php/dav/files/user/",
                        keyboard = KeyboardType.Uri,
                    )
                    FormField(webdav.username, { v -> vm.updateWebdav { it.copy(username = v) } }, stringResource(R.string.username))
                    PasswordField(
                        password = webdav.password,
                        hasPassword = webdav.hasPassword,
                        inheritedHint = false,
                        onChange = { v -> vm.updateWebdav { it.copy(password = v) } },
                    )
                    FormField(
                        webdav.certificateFingerprint ?: "",
                        { v -> vm.updateWebdav { it.copy(certificateFingerprint = v.ifBlank { null }) } },
                        stringResource(R.string.certificate_fingerprint_sha_256),
                        placeholder = stringResource(R.string.system_trust_roots),
                        keyboard = KeyboardType.Ascii,
                    )
                    Text(
                        stringResource(R.string.webdav_credentials_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        if (!draft.ssh || telnet == null || webdav == null) {
            Row(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (!draft.ssh) {
                    TextButton(onClick = { vm.update { it.copy(ssh = true) } }) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                        Text(stringResource(R.string.add_ssh))
                    }
                }
                if (telnet == null) {
                    TextButton(onClick = { vm.update { it.copy(telnet = TelnetDraft(port = null, username = "", password = null, identityId = null, hasPassword = false)) } }) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                        Text(stringResource(R.string.add_telnet))
                    }
                }
                if (webdav == null) {
                    TextButton(onClick = { vm.update { it.copy(webdav = WebDavDraft(url = "", username = "", password = null, identityId = null, certificateFingerprint = null, hasPassword = false)) } }) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                        Text(stringResource(R.string.add_webdav))
                    }
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
            Text(if (more) stringResource(R.string.hide_advanced) else stringResource(R.string.show_advanced), color = MaterialTheme.colorScheme.primary)
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
                        stringResource(R.string.keep_alive_interval_seconds),
                        placeholder = stringResource(R.string.app_default),
                        keyboard = KeyboardType.Number,
                    )
                    FormField(
                        draft.timeout?.toString() ?: "",
                        { v -> vm.update { it.copy(timeout = v.filter(Char::isDigit).take(4).toUIntOrNull()) } },
                        stringResource(R.string.connection_timeout_seconds),
                        placeholder = stringResource(R.string.app_default),
                        keyboard = KeyboardType.Number,
                    )
                }
                RowDivider()
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.environment_variables), style = MaterialTheme.typography.labelLarge)
                    draft.envVariables.forEachIndexed { i, env ->
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            OutlinedTextField(
                                value = env.name,
                                onValueChange = { vm.setEnv(i, it, env.value) },
                                label = { Text(stringResource(R.string.name)) },
                                singleLine = true,
                                modifier = Modifier.weight(1f),
                                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters, autoCorrectEnabled = false),
                            )
                            OutlinedTextField(
                                value = env.value,
                                onValueChange = { vm.setEnv(i, env.name, it) },
                                label = { Text(stringResource(R.string.value_)) },
                                singleLine = true,
                                modifier = Modifier.weight(1f),
                                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
                            )
                            IconButton(onClick = { vm.removeEnv(i) }) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.remove_2)) }
                        }
                    }
                    TextButton(onClick = vm::addEnv) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                        Text(stringResource(R.string.add_variable))
                    }
                }
                RowDivider()
                Column(Modifier.padding(16.dp)) {
                    OutlinedTextField(
                        value = draft.notes,
                        onValueChange = { v -> vm.update { it.copy(notes = v) } },
                        label = { Text(stringResource(R.string.notes)) },
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
            title = stringResource(R.string.group),
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
private fun SshSection(
    state: HostEditorState,
    draft: HostDraft,
    vm: HostEditorViewModel,
    inherited: InheritedInfo?,
    identity: IdentityItem?,
) {
    SectionCard {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            FormField(
                draft.port?.toString() ?: "",
                { v -> vm.update { it.copy(port = v.filter(Char::isDigit).take(5).toUIntOrNull()?.takeIf { p -> p in 1u..65535u }?.toUShort()) } },
                stringResource(R.string.port),
                placeholder = inherited?.port?.toString() ?: "22",
                keyboard = KeyboardType.Number,
            )
            IdentityRow(state, draft, onPick = { id -> vm.update { it.copy(identityId = id) } })
            if (identity == null) {
                FormField(
                    draft.username,
                    { v -> vm.update { it.copy(username = v) } },
                    stringResource(R.string.username),
                    placeholder = inherited?.username ?: if (draft.sshId || inherited?.sshId == true) stringResource(R.string.ssh_id_handle) else "root",
                )
                PasswordField(
                    password = draft.password,
                    hasPassword = draft.hasPassword,
                    inheritedHint = inherited?.hasPassword == true,
                    onChange = { v -> vm.update { it.copy(password = v) } },
                )
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
                        stringResource(R.string.username_password_key_and_ssh_id_come_from, identity.label)
                    } else {
                        stringResource(R.string.username_password_and_key_come_from_the_identity, identity.label)
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        RowDivider()
        SwitchRow(
            title = stringResource(R.string.agent_forwarding),
            checked = draft.agentForwarding,
            onCheckedChange = { v -> vm.update { it.copy(agentForwarding = v) } },
        )
        RowDivider()
        SwitchRow(
            title = "Mosh",
            subtitle = stringResource(R.string.roaming_udp_session_ssh_only_starts_mosh_server),
            checked = draft.useMosh,
            onCheckedChange = { v -> vm.update { it.copy(useMosh = v) } },
        )
        if (draft.useMosh) {
            Box(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 16.dp)) {
                FormField(
                    draft.moshServerCommand ?: "",
                    { v -> vm.update { it.copy(moshServerCommand = v.takeIf { c -> c.isNotBlank() }) } },
                    stringResource(R.string.mosh_server_command),
                    placeholder = remember { moshDefaultServerCommand() },
                )
            }
        }
        if (state.snippets.isNotEmpty() || draft.startupSnippetId != null) {
            RowDivider()
            Box(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                StartupSnippetRow(state, draft, onPick = { id -> vm.update { it.copy(startupSnippetId = id) } })
            }
        }
    }
}

@Composable
private fun StartupSnippetRow(state: HostEditorState, draft: HostDraft, onPick: (String?) -> Unit) {
    val snippet = state.snippets.firstOrNull { it.id == draft.startupSnippetId }
    PickerRow(
        label = stringResource(R.string.startup_snippet),
        value = snippet?.label ?: if (draft.startupSnippetId != null) stringResource(R.string.unknown_snippet) else stringResource(R.string.none),
        options = listOf<Pair<String?, String>>(null to stringResource(R.string.none)) + state.snippets.map { it.id to it.label },
        selected = draft.startupSnippetId,
        onPick = onPick,
        empty = null,
    )
}

private fun webdavOnly(d: HostDraft) = !d.ssh && d.telnet == null && d.webdav != null

private fun hasAdvanced(d: HostDraft) =
    d.envVariables.isNotEmpty() || d.keepAliveInterval != null || d.timeout != null || d.notes.isNotBlank() || d.ipVersion != "auto"

private fun tagSummary(draft: HostDraft, tags: List<TagItem>): String {
    val labels = draft.tagIds.mapNotNull { id -> tags.firstOrNull { it.id == id }?.label }
    return when {
        labels.isEmpty() -> str(R.string.none)
        labels.size <= 2 -> labels.joinToString(", ")
        else -> "${labels.take(2).joinToString(", ")} +${labels.size - 2}"
    }
}

/** Section title with an optional "Remove" for the protocol it heads. */
@Composable
private fun ProtocolHeader(title: String, removable: Boolean, onRemove: () -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.weight(1f)) { SectionLabel(title) }
        if (removable) {
            TextButton(onClick = onRemove) { Text(stringResource(R.string.remove_2)) }
        }
    }
}

/**
 * Password editor over the draft convention: `null` keeps the stored secret
 * ([hasPassword]), `""` clears it, anything else replaces it.
 */
@Composable
private fun PasswordField(password: String?, hasPassword: Boolean, inheritedHint: Boolean, onChange: (String?) -> Unit) {
    var visible by remember { mutableStateOf(false) }
    val stored = password == null && hasPassword
    FormField(
        value = password ?: "",
        onChange = { onChange(it) },
        label = if (stored) stringResource(R.string.password_saved) else stringResource(R.string.password),
        placeholder = when {
            stored -> "••••••••"
            inheritedHint -> stringResource(R.string.inherited_from_group)
            else -> null
        },
        keyboard = KeyboardType.Password,
        visual = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
        trailing = {
            Row {
                if (hasPassword) {
                    TextButton(onClick = { onChange(if (stored) "" else null) }) {
                        Text(if (stored) stringResource(R.string.clear) else stringResource(R.string.keep_saved))
                    }
                }
                IconButton(onClick = { visible = !visible }) {
                    Icon(
                        if (visible) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                        contentDescription = if (visible) stringResource(R.string.hide_password) else stringResource(R.string.show_password),
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
        label = stringResource(R.string.key),
        value = key?.label ?: state.inherited?.sshKeyLabel?.let { stringResource(R.string.inherited, it) } ?: stringResource(R.string.none),
        options = listOf<Pair<String?, String>>(null to stringResource(R.string.none)) + state.keys.map { it.id to "${it.label} · ${it.keyType}" },
        selected = draft.sshKeyId,
        onPick = onPick,
        empty = stringResource(R.string.no_keys_in_this_vault_yet_add_one),
    )
}

@Composable
private fun IdentityRow(state: HostEditorState, draft: HostDraft, onPick: (String?) -> Unit) {
    if (state.identities.isEmpty() && draft.identityId == null) return
    val identity = state.identities.firstOrNull { it.id == draft.identityId }
    PickerRow(
        label = stringResource(R.string.identity),
        value = identity?.label ?: state.inherited?.identityLabel?.let { stringResource(R.string.inherited, it) } ?: stringResource(R.string.none),
        options = listOf<Pair<String?, String>>(null to stringResource(R.string.none)) + state.identities.map { it.id to "${it.label} · ${it.username}" },
        selected = draft.identityId,
        onPick = onPick,
        empty = null,
    )
}

@Composable
private fun IpVersionRow(value: String, onPick: (String) -> Unit) {
    val options = listOf("auto" to stringResource(R.string.auto), "ipv4" to stringResource(R.string.ipv4_only), "ipv6" to stringResource(R.string.ipv6_only))
    PickerRow(
        label = stringResource(R.string.ip_version),
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
            title = vaults.firstOrNull { it.id == selected }?.let { vaultLabel(it) } ?: stringResource(R.string.vault),
            subtitle = stringResource(R.string.vault),
            modifier = Modifier.clickable { open = true },
        ) { Icon(Icons.Filled.ArrowDropDown, contentDescription = stringResource(R.string.choose_vault)) }
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
        title = { Text(stringResource(R.string.tags)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                if (tags.isEmpty()) {
                    Text(stringResource(R.string.no_tags_yet), color = MaterialTheme.colorScheme.onSurfaceVariant)
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
                    label = { Text(stringResource(R.string.new_tag)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                    trailingIcon = {
                        IconButton(
                            onClick = { onCreate(newTag); newTag = "" },
                            enabled = newTag.isNotBlank(),
                        ) { Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.add_tag)) }
                    },
                )
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.done)) } },
    )
}
