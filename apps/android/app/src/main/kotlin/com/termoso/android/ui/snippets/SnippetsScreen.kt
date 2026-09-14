package com.termoso.android.ui.snippets

import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.CreateNewFolder
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.SnippetItem
import com.termoso.core.SnippetPackageItem

/**
 * Snippets of the selected vault, one package level at a time (Termius: packages
 * are folders). Tap a snippet to edit, the play button to run it into open
 * terminals; long-press for the menu.
 */
@Composable
fun SnippetsScreen(
    shell: ShellViewModel,
    packageId: String?,
    onBack: () -> Unit,
    onOpenPackage: (String) -> Unit,
    onNewSnippet: (vaultId: String, packageId: String?) -> Unit,
    onEditSnippet: (String) -> Unit,
    onOpenTerminal: () -> Unit,
) {
    val vm: SnippetsViewModel = viewModel { SnippetsViewModel(shell.repo, shell.selectedVaultId) }
    val state by vm.state.collectAsStateWithLifecycle()
    var fabMenu by remember { mutableStateOf(false) }
    var packageDialog by remember { mutableStateOf<SnippetPackageItem?>(null) }
    var newPackage by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf<SnippetItem?>(null) }
    var confirmDeletePackage by remember { mutableStateOf<SnippetPackageItem?>(null) }
    var run by remember { mutableStateOf<SnippetItem?>(null) }

    LaunchedEffect(state.error) { state.error?.let { shell.notify(it); vm.errorShown() } }

    val here = state.packages.firstOrNull { it.id == packageId }
    val packages = state.packages.filter { it.parentId == packageId }
    val snippets = state.snippets.filter { it.packageId == packageId }
    val vaultId = state.vaultId

    SubScreen(
        title = here?.label ?: "Snippets",
        onBack = onBack,
        floating = {
            if (vaultId != null && (packages.isNotEmpty() || snippets.isNotEmpty())) {
                Box {
                    FloatingActionButton(onClick = { fabMenu = true }) { Icon(Icons.Filled.Add, contentDescription = "New") }
                    NewMenu(
                        expanded = fabMenu,
                        onDismiss = { fabMenu = false },
                        onSnippet = { onNewSnippet(vaultId, packageId) },
                        onPackage = { newPackage = true },
                    )
                }
            }
        },
    ) { padding ->
        when {
            state.loading -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            packages.isEmpty() && snippets.isEmpty() -> Column(
                Modifier.fillMaxSize().padding(padding),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                EmptyState(
                    title = if (here == null) "No snippets yet" else "Empty package",
                    hint = "Save commands you type often and run them into one or many terminals, with {{variables}} filled in on the way.",
                )
                if (vaultId != null) {
                    Box {
                        Button(onClick = { fabMenu = true }) { Text("Create") }
                        NewMenu(
                            expanded = fabMenu,
                            onDismiss = { fabMenu = false },
                            onSnippet = { onNewSnippet(vaultId, packageId) },
                            onPackage = { newPackage = true },
                        )
                    }
                }
            }
            else -> LazyColumn(
                Modifier.fillMaxSize().padding(padding),
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
            ) {
                if (packages.isNotEmpty()) {
                    item { SectionLabel("Packages") }
                    item {
                        SectionCard {
                            packages.forEachIndexed { i, p ->
                                if (i > 0) RowDivider()
                                PackageRow(
                                    pkg = p,
                                    onOpen = { onOpenPackage(p.id) },
                                    onRename = { packageDialog = p },
                                    onDelete = { confirmDeletePackage = p },
                                )
                            }
                        }
                    }
                    item { Spacer(Modifier.height(16.dp)) }
                }
                if (snippets.isNotEmpty()) {
                    item { SectionLabel("Snippets") }
                    item {
                        SectionCard {
                            snippets.forEachIndexed { i, s ->
                                if (i > 0) RowDivider()
                                SnippetRow(
                                    snippet = s,
                                    onEdit = { onEditSnippet(s.id) },
                                    onRun = { run = s },
                                    onDuplicate = { vm.duplicate(s.id) },
                                    onDelete = { confirmDelete = s },
                                )
                            }
                        }
                    }
                }
                item { Spacer(Modifier.height(88.dp)) }
            }
        }
    }

    if (newPackage && vaultId != null) {
        PackageDialog(
            existing = null,
            onSave = { label -> vm.savePackage(vaultId, null, label, packageId); newPackage = false },
            onDismiss = { newPackage = false },
        )
    }
    packageDialog?.let { p ->
        PackageDialog(
            existing = p,
            onSave = { label -> vm.savePackage(p.vaultId, p.id, label, p.parentId); packageDialog = null },
            onDismiss = { packageDialog = null },
        )
    }
    confirmDelete?.let { s ->
        ConfirmDialog(
            title = "Delete snippet?",
            text = "\"${s.label}\" will be removed from this vault.",
            confirm = "Delete",
            onConfirm = { vm.delete(s.id); confirmDelete = null },
            onDismiss = { confirmDelete = null },
        )
    }
    confirmDeletePackage?.let { p ->
        ConfirmDialog(
            title = "Delete package?",
            text = "\"${p.label}\" will be removed. Snippets and packages inside it move up one level.",
            confirm = "Delete",
            onConfirm = { vm.deletePackage(p.id); confirmDeletePackage = null },
            onDismiss = { confirmDeletePackage = null },
        )
    }
    run?.let { s ->
        RunSnippetDialog(
            shell = shell,
            snippet = s,
            preselect = emptyList(),
            onDismiss = { run = null },
            onLaunched = { launch ->
                run = null
                if (launch.sessions > 0 || launch.connected > 0) onOpenTerminal()
            },
        )
    }
}

@Composable
private fun NewMenu(expanded: Boolean, onDismiss: () -> Unit, onSnippet: () -> Unit, onPackage: () -> Unit) {
    DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
        DropdownMenuItem(
            text = { Text("New snippet") },
            leadingIcon = { Icon(Icons.Filled.Code, null) },
            onClick = { onDismiss(); onSnippet() },
        )
        DropdownMenuItem(
            text = { Text("New package") },
            leadingIcon = { Icon(Icons.Filled.CreateNewFolder, null) },
            onClick = { onDismiss(); onPackage() },
        )
    }
}

@Composable
private fun PackageRow(pkg: SnippetPackageItem, onOpen: () -> Unit, onRename: () -> Unit, onDelete: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    Box {
        ChevronRow(
            title = pkg.label,
            badge = pkg.snippetCount.toString(),
            leading = { IconTile(Icons.Filled.Folder) },
            modifier = Modifier.combinedClickable(onClick = onOpen, onLongClick = { menu = true }),
        )
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text("Rename") }, leadingIcon = { Icon(Icons.Filled.Edit, null) }, onClick = { menu = false; onRename() })
            DropdownMenuItem(
                text = { Text("Delete", color = MaterialTheme.colorScheme.error) },
                leadingIcon = { Icon(Icons.Filled.Delete, null, tint = MaterialTheme.colorScheme.error) },
                onClick = { menu = false; onDelete() },
            )
        }
    }
}

@Composable
private fun SnippetRow(
    snippet: SnippetItem,
    onEdit: () -> Unit,
    onRun: () -> Unit,
    onDuplicate: () -> Unit,
    onDelete: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    Box {
        ListRow(
            title = snippet.label,
            subtitle = snippetSubtitle(snippet),
            leading = { IconTile(Icons.Filled.Code) },
            modifier = Modifier.combinedClickable(onClick = onEdit, onLongClick = { menu = true }),
        ) {
            IconButton(onClick = onRun) { Icon(Icons.Filled.PlayArrow, contentDescription = "Run", tint = MaterialTheme.colorScheme.primary) }
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text("Run") }, leadingIcon = { Icon(Icons.Filled.PlayArrow, null) }, onClick = { menu = false; onRun() })
            DropdownMenuItem(text = { Text("Edit") }, leadingIcon = { Icon(Icons.Filled.Edit, null) }, onClick = { menu = false; onEdit() })
            DropdownMenuItem(
                text = { Text("Duplicate") },
                leadingIcon = { Icon(Icons.Filled.ContentCopy, null) },
                onClick = { menu = false; onDuplicate() },
            )
            DropdownMenuItem(
                text = { Text("Delete", color = MaterialTheme.colorScheme.error) },
                leadingIcon = { Icon(Icons.Filled.Delete, null, tint = MaterialTheme.colorScheme.error) },
                onClick = { menu = false; onDelete() },
            )
        }
    }
}

/** First script line plus the badges Termius shows: variable count, close-after-run. */
fun snippetSubtitle(snippet: SnippetItem): String {
    val first = snippet.script.lineSequence().firstOrNull { it.isNotBlank() }?.trim() ?: ""
    val extras = buildList {
        if (snippet.variables.isNotEmpty()) add("${snippet.variables.size} var" + if (snippet.variables.size == 1) "" else "s")
        if (snippet.targetHostIds.isNotEmpty()) add("${snippet.targetHostIds.size} target" + if (snippet.targetHostIds.size == 1) "" else "s")
        if (snippet.closeAfterRun) add("closes session")
    }
    return if (extras.isEmpty()) first else "$first · ${extras.joinToString(" · ")}"
}

@Composable
private fun PackageDialog(existing: SnippetPackageItem?, onSave: (String) -> Unit, onDismiss: () -> Unit) {
    var label by remember { mutableStateOf(existing?.label ?: "") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (existing == null) "New package" else "Rename package") },
        text = { FormField(label, { label = it }, "Name") },
        confirmButton = {
            TextButton(onClick = { onSave(label) }, enabled = label.isNotBlank()) { Text(if (existing == null) "Create" else "Save") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
