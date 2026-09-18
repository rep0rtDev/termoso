package com.termoso.android.ui.hosts

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material.icons.automirrored.filled.DriveFileMove
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.SortByAlpha
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.data.VaultRepository
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.HostAvatar
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.vault.vaultLabel
import com.termoso.core.GroupItem
import com.termoso.core.HostItem
import com.termoso.core.Transport
import com.termoso.core.VaultInfo

/** Host list for a vault root or a group: search, sort, per-host actions (long-press / ⋯), multi-select, FAB. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun HostsScreen(
    shell: ShellViewModel,
    groupId: String?,
    onBack: () -> Unit,
    onOpenGroup: (String) -> Unit,
    onNewHost: () -> Unit,
    onEditHost: (String) -> Unit,
    onConnect: (String) -> Unit,
    /** Connect over an explicit transport (Mosh, the Telnet section). */
    onConnectWith: (String, Transport) -> Unit,
    onSftp: (String) -> Unit,
    onOpenSftp: (String) -> Unit,
    onOpenTerminal: () -> Unit,
    onForward: (String) -> Unit,
) {
    val vm: HostsViewModel = viewModel(key = "hosts/${groupId ?: "root"}") {
        HostsViewModel(shell.repo, shell.selectedVaultId, groupId)
    }
    val state by vm.state.collectAsStateWithLifecycle()
    val vaults by shell.vaults.collectAsStateWithLifecycle()
    val selectedVaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vault = vaults.firstOrNull { it.id == selectedVaultId }
    val sessions by shell.sessions.sessions.collectAsStateWithLifecycle()
    val sftp by shell.sftp.connections.collectAsStateWithLifecycle()
    val openByHost = (sessions.mapNotNull { it.hostId } + sftp.mapNotNull { it.hostId }).groupingBy { it }.eachCount()
    val presence = rememberVaultPresence(shell, vault)
    val viewersByHost = remember(presence) { viewersByHost(presence) }

    LaunchedEffect(state.error) {
        state.error?.let { shell.notify(it); vm.errorShown() }
    }
    BackHandler(enabled = state.selecting) { vm.clearSelection() }

    var searching by remember { mutableStateOf(false) }
    var fabMenu by remember { mutableStateOf(false) }
    var dialog by remember { mutableStateOf<HostsDialog?>(null) }
    val otherVaults = vaults.filter { it.id != selectedVaultId && !it.locked }

    Scaffold(
        topBar = {
            if (state.selecting) {
                SelectionBar(
                    state = state,
                    vaults = otherVaults,
                    onClose = vm::clearSelection,
                    onSelectAll = vm::selectAll,
                    onEdit = { onEditHost(state.selected.first()) },
                    onSftp = {
                        val id = state.selected.first()
                        vm.clearSelection()
                        onSftp(id)
                    },
                    onConnectWith = { transport ->
                        val id = state.selected.first()
                        vm.clearSelection()
                        onConnectWith(id, transport)
                    },
                    onForward = {
                        val id = state.selected.first()
                        vm.clearSelection()
                        onForward(id)
                    },
                    onDuplicate = vm::duplicateSelected,
                    onMove = { dialog = HostsDialog.Move(state.selected.toList()) },
                    onCopy = { dialog = HostsDialog.Copy(state.selected.toList()) },
                    onDelete = { dialog = HostsDialog.Delete(state.selected.toList()) },
                )
            } else {
                TopAppBar(
                    title = {
                        if (searching) {
                            SearchField(state.query, vm::setQuery)
                        } else {
                            Text(state.group?.label ?: "Hosts")
                        }
                    },
                    navigationIcon = {
                        IconButton(onClick = { if (searching) { searching = false; vm.setQuery("") } else onBack() }) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                        }
                    },
                    actions = {
                        if (!searching) {
                            IconButton(onClick = { searching = true }) {
                                Icon(Icons.Filled.Search, contentDescription = "Search")
                            }
                        }
                        SortMenu(state.sort, vm::setSort)
                    },
                )
            }
        },
        floatingActionButton = {
            if (!state.selecting) {
                Box {
                    FloatingActionButton(onClick = { fabMenu = true }) {
                        Icon(Icons.Filled.Add, contentDescription = "Add")
                    }
                    DropdownMenu(expanded = fabMenu, onDismissRequest = { fabMenu = false }) {
                        DropdownMenuItem(text = { Text("New host") }, onClick = { fabMenu = false; onNewHost() })
                        DropdownMenuItem(
                            text = { Text("New group") },
                            onClick = { fabMenu = false; dialog = HostsDialog.NewGroup },
                        )
                    }
                }
            }
        },
    ) { padding ->
        when {
            state.loading -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            state.visibleGroups.isEmpty() && state.visibleHosts.isEmpty() -> Box(
                Modifier.fillMaxSize().padding(padding),
                contentAlignment = Alignment.Center,
            ) {
                if (state.query.isBlank()) {
                    EmptyState(
                        title = if (state.group == null) "No hosts yet" else "Empty group",
                        hint = "Save a server with its credentials to connect in one tap. " +
                            "Everything stays in the encrypted vault.",
                        icon = Icons.Filled.Dns,
                        action = { Button(onClick = onNewHost) { Text("Add host") } },
                    )
                } else {
                    EmptyState(
                        title = "Nothing found",
                        hint = "Try another name, address, user or tag.",
                        icon = Icons.Filled.Search,
                    )
                }
            }
            else -> HostList(
                repo = shell.repo,
                state = state,
                openByHost = openByHost,
                viewersByHost = viewersByHost,
                padding = padding,
                onOpenGroup = onOpenGroup,
                onGroupLongPress = { dialog = HostsDialog.GroupMenu(it) },
                onHostTap = { host ->
                    if (state.selecting) vm.toggle(host.id) else onConnect(host.id)
                },
                onHostLongPress = { host ->
                    if (state.selecting) vm.toggle(host.id) else dialog = HostsDialog.HostMenu(host)
                },
                onHostMenu = { dialog = HostsDialog.HostMenu(it) },
            )
        }
    }

    when (val d = dialog) {
        null -> Unit
        is HostsDialog.HostMenu -> HostActionsSheet(
            shell = shell,
            host = d.host,
            canCopyToVault = otherVaults.isNotEmpty(),
            onConnect = { transport -> if (transport == Transport.AUTO) onConnect(d.host.id) else onConnectWith(d.host.id, transport) },
            onSftp = { onSftp(d.host.id) },
            onOpenSftp = onOpenSftp,
            onOpenTerminal = onOpenTerminal,
            onForward = { onForward(d.host.id) },
            onEdit = { onEditHost(d.host.id) },
            onDuplicate = { vm.duplicate(listOf(d.host.id)) },
            onMove = { dialog = HostsDialog.Move(listOf(d.host.id)) },
            onCopy = { dialog = HostsDialog.Copy(listOf(d.host.id)) },
            onSelect = { vm.toggle(d.host.id) },
            onDelete = { dialog = HostsDialog.Delete(listOf(d.host.id)) },
            onClose = { dialog = null },
        )
        is HostsDialog.Delete -> ConfirmDialog(
            title = if (d.ids.size == 1) "Remove host?" else "Remove ${d.ids.size} hosts?",
            text = "Hosts are removed from this vault. Keys in the keychain stay.",
            confirm = "Remove",
            onConfirm = { vm.delete(d.ids); dialog = null },
            onDismiss = { dialog = null },
        )
        is HostsDialog.Move -> GroupPickerDialog(
            title = "Move to",
            groups = state.allGroups,
            current = groupId,
            onPick = { vm.move(d.ids, it); dialog = null },
            onDismiss = { dialog = null },
        )
        is HostsDialog.Copy -> CopyToVaultDialog(
            vaults = otherVaults,
            onCopy = { v, creds -> vm.copy(d.ids, v, creds); dialog = null },
            onDismiss = { dialog = null },
        )
        HostsDialog.NewGroup -> NameDialog(
            title = "New group",
            initial = "",
            onConfirm = { vm.createGroup(it); dialog = null },
            onDismiss = { dialog = null },
        )
        is HostsDialog.GroupMenu -> GroupMenuDialog(
            group = d.group,
            onRename = { dialog = HostsDialog.RenameGroup(d.group) },
            onDelete = { vm.deleteGroup(d.group); dialog = null },
            onDismiss = { dialog = null },
        )
        is HostsDialog.RenameGroup -> NameDialog(
            title = "Rename group",
            initial = d.group.label,
            onConfirm = { vm.renameGroup(d.group, it); dialog = null },
            onDismiss = { dialog = null },
        )
    }
}

private sealed interface HostsDialog {
    data class HostMenu(val host: HostItem) : HostsDialog
    data class Delete(val ids: List<String>) : HostsDialog
    data class Move(val ids: List<String>) : HostsDialog
    data class Copy(val ids: List<String>) : HostsDialog
    data object NewGroup : HostsDialog
    data class GroupMenu(val group: GroupItem) : HostsDialog
    data class RenameGroup(val group: GroupItem) : HostsDialog
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HostList(
    repo: VaultRepository,
    state: HostsUiState,
    openByHost: Map<String, Int>,
    viewersByHost: Map<String, List<HostViewer>>,
    padding: PaddingValues,
    onOpenGroup: (String) -> Unit,
    onGroupLongPress: (GroupItem) -> Unit,
    onHostTap: (HostItem) -> Unit,
    onHostLongPress: (HostItem) -> Unit,
    onHostMenu: (HostItem) -> Unit,
) {
    val groups = state.visibleGroups
    val hosts = state.visibleHosts
    LazyColumn(
        modifier = Modifier.fillMaxSize().padding(padding),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 96.dp),
    ) {
        if (groups.isNotEmpty()) {
            item { SectionLabel("Groups") }
            item {
                SectionCard {
                    groups.forEachIndexed { i, g ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = g.label,
                            subtitle = groupSubtitle(g),
                            leading = { IconTile(Icons.Filled.Folder) },
                            modifier = Modifier.combinedClickable(
                                onClick = { onOpenGroup(g.id) },
                                onLongClick = { onGroupLongPress(g) },
                            ),
                        )
                    }
                }
            }
        }
        if (hosts.isNotEmpty()) {
            item { SectionLabel("Hosts") }
            item {
                SectionCard {
                    hosts.forEachIndexed { i, h ->
                        if (i > 0) RowDivider()
                        val selected = h.id in state.selected
                        ListRow(
                            title = h.label.ifBlank { h.address },
                            subtitle = hostSubtitle(h),
                            leading = { HostAvatar(h.osName, selected = selected) },
                            modifier = Modifier.combinedClickable(
                                onClick = { onHostTap(h) },
                                onLongClick = { onHostLongPress(h) },
                            ),
                        ) {
                            if (h.tags.isNotEmpty()) {
                                Text(
                                    h.tags.take(2).joinToString(" · "),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.primary,
                                )
                            }
                            viewersByHost[h.id]?.let { PresenceStack(repo, it) }
                            openByHost[h.id]?.let { OpenSessionsBadge(it, onClick = { onHostMenu(h) }) }
                            if (!state.selecting) {
                                IconButton(onClick = { onHostMenu(h) }, modifier = Modifier.size(32.dp)) {
                                    Icon(
                                        Icons.Filled.MoreVert,
                                        contentDescription = "Host actions",
                                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** “N open” pill on a host card while it has terminal sessions, like the Active badge in Termius. */
@Composable
private fun OpenSessionsBadge(count: Int, onClick: () -> Unit) {
    Row(
        Modifier
            .padding(start = 8.dp)
            .clip(RoundedCornerShape(999.dp))
            .background(MaterialTheme.colorScheme.primaryContainer)
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        Box(Modifier.size(6.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
        Text(
            if (count == 1) "Active" else "$count active",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onPrimaryContainer,
        )
    }
}

private fun groupSubtitle(g: GroupItem): String? {
    val parts = buildList {
        if (g.hostCount > 0u) add("${g.hostCount} ${if (g.hostCount == 1u) "host" else "hosts"}")
        if (g.groupCount > 0u) add("${g.groupCount} ${if (g.groupCount == 1u) "group" else "groups"}")
    }
    return parts.joinToString(" · ").ifEmpty { null }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SelectionBar(
    state: HostsUiState,
    vaults: List<VaultInfo>,
    onClose: () -> Unit,
    onSelectAll: () -> Unit,
    onEdit: () -> Unit,
    onSftp: () -> Unit,
    onConnectWith: (Transport) -> Unit,
    onForward: () -> Unit,
    onDuplicate: () -> Unit,
    onMove: () -> Unit,
    onCopy: () -> Unit,
    onDelete: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    val single = state.visibleHosts.firstOrNull { it.id in state.selected }.takeIf { state.selected.size == 1 }
    val singleSsh = single?.protocol.equals("ssh", ignoreCase = true)
    val singleTelnet = singleSsh && single?.telnetPort != null
    TopAppBar(
        title = { Text("${state.selected.size} selected") },
        navigationIcon = {
            IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = "Cancel selection") }
        },
        actions = {
            if (state.selected.size == 1) {
                IconButton(onClick = onEdit) { Icon(Icons.Filled.Edit, contentDescription = "Edit") }
                if (singleSsh) {
                    IconButton(onClick = onSftp) { Icon(Icons.Filled.FolderOpen, contentDescription = "SFTP") }
                }
            }
            IconButton(onClick = onDelete) { Icon(Icons.Filled.Delete, contentDescription = "Remove") }
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = "More") }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text("Select all") },
                        leadingIcon = { Icon(Icons.Filled.Check, null) },
                        onClick = { menu = false; onSelectAll() },
                    )
                    DropdownMenuItem(
                        text = { Text("Duplicate") },
                        leadingIcon = { Icon(Icons.Filled.ContentCopy, null) },
                        onClick = { menu = false; onDuplicate() },
                    )
                    if (singleSsh) {
                        DropdownMenuItem(
                            text = { Text("Connect with Mosh") },
                            leadingIcon = { Icon(Icons.Filled.Bolt, null) },
                            onClick = { menu = false; onConnectWith(Transport.MOSH) },
                        )
                        if (singleTelnet) {
                            DropdownMenuItem(
                                text = { Text("Connect with Telnet") },
                                leadingIcon = { Icon(Icons.Filled.Terminal, null) },
                                onClick = { menu = false; onConnectWith(Transport.TELNET) },
                            )
                        }
                        DropdownMenuItem(
                            text = { Text("Port forwarding…") },
                            leadingIcon = { Icon(Icons.Filled.SwapHoriz, null) },
                            onClick = { menu = false; onForward() },
                        )
                    }
                    DropdownMenuItem(
                        text = { Text("Move to group…") },
                        leadingIcon = { Icon(Icons.AutoMirrored.Filled.DriveFileMove, null) },
                        onClick = { menu = false; onMove() },
                    )
                    if (vaults.isNotEmpty()) {
                        DropdownMenuItem(
                            text = { Text("Copy to vault…") },
                            leadingIcon = { Icon(Icons.Filled.ContentCopy, null) },
                            onClick = { menu = false; onCopy() },
                        )
                    }
                }
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
    )
}

@Composable
private fun SearchField(query: String, onChange: (String) -> Unit) {
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    OutlinedTextField(
        value = query,
        onValueChange = onChange,
        placeholder = { Text("Search hosts") },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().focusRequester(focus),
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
    )
}

@Composable
private fun SortMenu(sort: HostSort, onSort: (HostSort) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) { Icon(Icons.Filled.SortByAlpha, contentDescription = "Sort") }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            HostSort.entries.forEach { s ->
                DropdownMenuItem(
                    text = { Text(s.label) },
                    trailingIcon = if (s == sort) {
                        { Icon(Icons.Filled.Check, contentDescription = null) }
                    } else {
                        null
                    },
                    onClick = { onSort(s); open = false },
                )
            }
        }
    }
}

@Composable
fun ConfirmDialog(title: String, text: String, confirm: String, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Text(text) },
        confirmButton = {
            TextButton(onClick = onConfirm) { Text(confirm, color = MaterialTheme.colorScheme.error) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
private fun NameDialog(title: String, initial: String, onConfirm: (String) -> Unit, onDismiss: () -> Unit) {
    var value by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = value,
                onValueChange = { value = it },
                label = { Text("Name") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(value) }, enabled = value.isNotBlank()) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
private fun GroupMenuDialog(group: GroupItem, onRename: () -> Unit, onDelete: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(group.label) },
        text = {
            Column {
                Text(
                    "Deleting a group moves its hosts and sub-groups one level up.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = {
            Row {
                TextButton(onClick = onRename) { Text("Rename") }
                TextButton(onClick = onDelete) { Text("Delete", color = MaterialTheme.colorScheme.error) }
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

/** Flat list of groups with their path, plus “No group” at the top. */
@Composable
fun GroupPickerDialog(
    title: String,
    groups: List<GroupItem>,
    current: String?,
    onPick: (String?) -> Unit,
    onDismiss: () -> Unit,
) {
    val paths = remember(groups) { groupPaths(groups) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            LazyColumn(modifier = Modifier.fillMaxWidth().height(320.dp)) {
                item {
                    PickRow("No group", selected = current == null, onClick = { onPick(null) })
                }
                items(groups.sortedBy { paths[it.id] }, key = { it.id }) { g ->
                    PickRow(paths[g.id] ?: g.label, selected = g.id == current, onClick = { onPick(g.id) })
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

fun groupPaths(groups: List<GroupItem>): Map<String, String> {
    val byId = groups.associateBy { it.id }
    fun path(g: GroupItem): String {
        val parent = g.parentId?.let { byId[it] }
        return if (parent == null) g.label else "${path(parent)} / ${g.label}"
    }
    return groups.associate { it.id to path(it) }
}

@Composable
private fun PickRow(label: String, selected: Boolean, onClick: () -> Unit) {
    ListRow(
        title = label,
        modifier = Modifier.combinedClickable(onClick = onClick),
        titleColor = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
    ) {
        if (selected) Icon(Icons.Filled.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
    }
}

@Composable
private fun CopyToVaultDialog(vaults: List<VaultInfo>, onCopy: (String, Boolean) -> Unit, onDismiss: () -> Unit) {
    var target by remember { mutableStateOf(vaults.firstOrNull()?.id) }
    var withCredentials by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Copy to vault") },
        text = {
            Column {
                vaults.forEach { v ->
                    Row(
                        Modifier.fillMaxWidth().combinedClickable(onClick = { target = v.id }).padding(vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = target == v.id, onClick = { target = v.id })
                        Text(vaultLabel(v))
                    }
                }
                Spacer(Modifier.height(12.dp))
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("Include credentials", fontWeight = FontWeight.Medium)
                        Text(
                            if (withCredentials) {
                                "Passwords and keys are copied into the target vault."
                            } else {
                                "Only host details are copied; members use their own credentials."
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Switch(checked = withCredentials, onCheckedChange = { withCredentials = it })
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { target?.let { onCopy(it, withCredentials) } }, enabled = target != null) { Text("Copy") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
