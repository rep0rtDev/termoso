package com.termoso.android.ui.snippets

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.R
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.PickerRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.vault.vaultLabel
import com.termoso.core.HostItem
import com.termoso.core.SnippetDraft
import com.termoso.core.SnippetPackageItem
import com.termoso.core.VaultInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SnippetEditorState(
    val loading: Boolean = true,
    val draft: SnippetDraft? = null,
    val vaults: List<VaultInfo> = emptyList(),
    val packages: List<SnippetPackageItem> = emptyList(),
    val hosts: List<HostItem> = emptyList(),
    /** `{{names}}` found in the script, from Rust, in order of first appearance. */
    val variables: List<String> = emptyList(),
    val working: Boolean = false,
    val done: Boolean = false,
    val error: String? = null,
) {
    val canSave: Boolean get() = draft != null && draft.label.isNotBlank() && draft.script.isNotBlank() && !working
}

/** New / edit snippet form over [SnippetDraft]; validation and persistence stay in Rust. */
class SnippetEditorViewModel(
    private val repo: VaultRepository,
    private val snippetId: String?,
    private val initialVault: String?,
    private val initialPackage: String?,
) : ViewModel() {
    private val _state = MutableStateFlow(SnippetEditorState())
    val state: StateFlow<SnippetEditorState> = _state.asStateFlow()

    init {
        viewModelScope.launch { load() }
    }

    private suspend fun load() {
        runCatching {
            repo.read {
                val vaults = vaults().filter { !it.locked }
                val draft = if (snippetId != null) {
                    val s = snippet(snippetId)
                    SnippetDraft(
                        id = s.id,
                        vaultId = s.vaultId,
                        label = s.label,
                        script = s.script,
                        packageId = s.packageId,
                        closeAfterRun = s.closeAfterRun,
                        targetHostIds = s.targetHostIds,
                    )
                } else {
                    SnippetDraft(
                        id = null,
                        vaultId = initialVault ?: vaults.first().id,
                        label = "",
                        script = "",
                        packageId = initialPackage,
                        closeAfterRun = false,
                        targetHostIds = emptyList(),
                    )
                }
                SnippetEditorState(
                    loading = false,
                    draft = draft,
                    vaults = vaults,
                    packages = snippetPackages(draft.vaultId),
                    hosts = hosts(draft.vaultId),
                    variables = snippetVariables(draft.script),
                )
            }
        }.onSuccess { _state.value = it }
            .onFailure { e -> _state.update { it.copy(loading = false, error = e.userMessage()) } }
    }

    fun update(transform: (SnippetDraft) -> SnippetDraft) {
        _state.update { s -> s.draft?.let { s.copy(draft = transform(it)) } ?: s }
    }

    fun setScript(script: String) {
        update { it.copy(script = script) }
        viewModelScope.launch {
            val vars = runCatching { repo.read { snippetVariables(script) } }.getOrDefault(emptyList())
            _state.update { s -> if (s.draft?.script == script) s.copy(variables = vars) else s }
        }
    }

    /** Switching vault (new snippets only) drops the package and targets, which are per-vault. */
    fun setVault(vaultId: String) {
        val current = _state.value.draft ?: return
        if (current.vaultId == vaultId || current.id != null) return
        viewModelScope.launch {
            runCatching {
                repo.read {
                    _state.value.copy(
                        draft = current.copy(vaultId = vaultId, packageId = null, targetHostIds = emptyList()),
                        packages = snippetPackages(vaultId),
                        hosts = hosts(vaultId),
                    )
                }
            }.onSuccess { _state.value = it }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun errorShown() = _state.update { it.copy(error = null) }

    fun save() {
        val s = _state.value
        val draft = s.draft ?: return
        if (!s.canSave) return
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching { repo.write { saveSnippet(draft.copy(label = draft.label.trim())) } }
                .onSuccess { _state.update { it.copy(working = false, done = true) } }
                .onFailure { e -> _state.update { it.copy(working = false, error = e.userMessage()) } }
        }
    }

    fun delete() {
        val id = _state.value.draft?.id ?: return
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching { repo.write { deleteSnippet(id) } }
                .onSuccess { _state.update { it.copy(working = false, done = true) } }
                .onFailure { e -> _state.update { it.copy(working = false, error = e.userMessage()) } }
        }
    }
}

/** Termius snippet form: Vault, Name, Package, Script, targets, close after running. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SnippetEditorScreen(
    shell: ShellViewModel,
    snippetId: String?,
    vaultId: String?,
    packageId: String?,
    onClose: () -> Unit,
) {
    val selectedVault by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vm: SnippetEditorViewModel = viewModel(key = "snippet/${snippetId ?: "new"}") {
        SnippetEditorViewModel(shell.repo, snippetId, vaultId ?: selectedVault, packageId)
    }
    val s by vm.state.collectAsStateWithLifecycle()
    var confirmDelete by remember { mutableStateOf(false) }
    var pickTargets by remember { mutableStateOf(false) }

    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }
    LaunchedEffect(s.done) { if (s.done) onClose() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(if (snippetId == null) stringResource(R.string.new_snippet) else s.draft?.label?.ifBlank { stringResource(R.string.edit_snippet) } ?: stringResource(R.string.edit_snippet)) },
                navigationIcon = { IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close)) } },
                actions = {
                    if (snippetId != null) {
                        IconButton(onClick = { confirmDelete = true }, enabled = !s.working) {
                            Icon(Icons.Filled.Delete, contentDescription = stringResource(R.string.delete))
                        }
                    }
                    IconButton(onClick = vm::save, enabled = s.canSave) {
                        if (s.working) {
                            CircularProgressIndicator(modifier = Modifier.height(20.dp), strokeWidth = 2.dp)
                        } else {
                            Icon(Icons.Filled.Check, contentDescription = stringResource(R.string.save))
                        }
                    }
                },
            )
        },
    ) { padding ->
        val draft = s.draft
        if (s.loading || draft == null) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            return@Scaffold
        }
        Column(
            Modifier.fillMaxSize().padding(padding).imePadding().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (draft.id == null && s.vaults.size > 1) {
                        PickerRow(
                            label = stringResource(R.string.vault),
                            value = s.vaults.firstOrNull { it.id == draft.vaultId }?.let { vaultLabel(it) } ?: "",
                            options = s.vaults.map { it.id to vaultLabel(it) },
                            selected = draft.vaultId,
                            onPick = { id -> id?.let(vm::setVault) },
                            empty = null,
                        )
                    }
                    FormField(draft.label, { v -> vm.update { it.copy(label = v) } }, stringResource(R.string.name))
                    PickerRow(
                        label = stringResource(R.string.package_),
                        value = s.packages.firstOrNull { it.id == draft.packageId }?.let { packagePath(s.packages, it) } ?: stringResource(R.string.none),
                        options = listOf<Pair<String?, String>>(null to stringResource(R.string.none)) +
                            s.packages.sortedBy { packagePath(s.packages, it).lowercase() }.map { it.id to packagePath(s.packages, it) },
                        selected = draft.packageId,
                        onPick = { id -> vm.update { it.copy(packageId = id) } },
                        empty = null,
                    )
                }
            }

            SectionLabel(stringResource(R.string.script))
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        value = draft.script,
                        onValueChange = vm::setScript,
                        placeholder = { Text("sudo systemctl restart {{service}}") },
                        minLines = 6,
                        textStyle = TextStyle(fontFamily = FontFamily.Monospace),
                        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
                        modifier = Modifier.fillMaxWidth().heightIn(max = 320.dp),
                    )
                    Text(
                        if (s.variables.isEmpty()) {
                            stringResource(R.string.wrap_a_value_in_to_be_asked_for)
                        } else {
                            stringResource(R.string.variables) + s.variables.joinToString(", ")
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            SectionLabel(stringResource(R.string.run))
            SectionCard {
                ChevronRow(
                    title = stringResource(R.string.targets),
                    subtitle = targetSummary(draft.targetHostIds, s.hosts),
                    modifier = Modifier.clickable(enabled = s.hosts.isNotEmpty()) { pickTargets = true },
                )
                RowDivider()
                SwitchRow(
                    title = stringResource(R.string.close_sessions_after_running),
                    subtitle = stringResource(R.string.disconnect_each_terminal_once_the_script_was_sent),
                    checked = draft.closeAfterRun,
                    onCheckedChange = { v -> vm.update { it.copy(closeAfterRun = v) } },
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (pickTargets) {
        TargetsDialog(
            hosts = s.hosts,
            selected = s.draft?.targetHostIds ?: emptyList(),
            onSave = { ids -> vm.update { it.copy(targetHostIds = ids) }; pickTargets = false },
            onDismiss = { pickTargets = false },
        )
    }
    if (confirmDelete) {
        ConfirmDialog(
            title = stringResource(R.string.delete_snippet),
            text = stringResource(R.string.will_be_removed_from_this_vault, s.draft?.label ?: ""),
            confirm = stringResource(R.string.delete),
            onConfirm = { confirmDelete = false; vm.delete() },
            onDismiss = { confirmDelete = false },
        )
    }
}

@Composable
private fun TargetsDialog(hosts: List<HostItem>, selected: List<String>, onSave: (List<String>) -> Unit, onDismiss: () -> Unit) {
    var picked by remember { mutableStateOf(selected.toSet()) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.target_hosts)) },
        text = {
            Column(Modifier.heightIn(max = 400.dp).verticalScroll(rememberScrollState())) {
                Text(
                    stringResource(R.string.preselected_when_the_snippet_runs_hosts_without_an),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                hosts.forEach { h ->
                    Row(
                        Modifier.fillMaxWidth().clickable { picked = if (h.id in picked) picked - h.id else picked + h.id },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(checked = h.id in picked, onCheckedChange = { picked = if (it) picked + h.id else picked - h.id })
                        Spacer(Modifier.width(4.dp))
                        Column {
                            Text(h.label.ifBlank { h.address }, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(h.address, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = { onSave(hosts.map { it.id }.filter { it in picked }) }) { Text(stringResource(R.string.done)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

private fun packagePath(all: List<SnippetPackageItem>, pkg: SnippetPackageItem): String {
    val parts = ArrayDeque<String>()
    var cur: SnippetPackageItem? = pkg
    var guard = 0
    while (cur != null && guard++ < 32) {
        parts.addFirst(cur.label)
        cur = all.firstOrNull { it.id == cur?.parentId }
    }
    return parts.joinToString(" / ")
}

private fun targetSummary(ids: List<String>, hosts: List<HostItem>): String {
    if (hosts.isEmpty()) return str(R.string.no_hosts_in_this_vault_yet)
    val labels = ids.mapNotNull { id -> hosts.firstOrNull { it.id == id }?.let { it.label.ifBlank { it.address } } }
    return when {
        labels.isEmpty() -> str(R.string.ask_every_time)
        labels.size <= 2 -> labels.joinToString(", ")
        else -> "${labels.take(2).joinToString(", ")} +${labels.size - 2}"
    }
}
