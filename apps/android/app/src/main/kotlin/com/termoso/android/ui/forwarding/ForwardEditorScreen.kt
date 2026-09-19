package com.termoso.android.ui.forwarding

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.R
import com.termoso.android.data.ForwardManager
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.PickerRow
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.HostItem
import com.termoso.core.PfKind
import com.termoso.core.PfRuleDraft
import com.termoso.core.PfRuleItem
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class ForwardEditorState(
    val loading: Boolean = true,
    val existing: PfRuleItem? = null,
    val vaultId: String = "",
    /** SSH hosts of the vault; Telnet-only hosts cannot forward. */
    val hosts: List<HostItem> = emptyList(),
    val kind: PfKind = PfKind.LOCAL,
    val label: String = "",
    val hostId: String? = null,
    val bind: String = "",
    val localPort: String = "",
    val remoteHost: String = "",
    val remotePort: String = "",
    val autoStart: Boolean = false,
    val working: Boolean = false,
    val done: Boolean = false,
    val error: String? = null,
) {
    val canSave: Boolean
        get() = !working && hostId != null && localPort.toUShortOrNull()?.let { it > 0u } == true &&
            when (kind) {
                PfKind.LOCAL -> remoteHost.isNotBlank() && remotePort.toUShortOrNull()?.let { it > 0u } == true
                PfKind.REMOTE -> remotePort.toUShortOrNull()?.let { it > 0u } == true
                PfKind.DYNAMIC -> true
            }
}

class ForwardEditorViewModel(
    private val repo: VaultRepository,
    private val forwards: ForwardManager,
    private val ruleId: String?,
    private val initialKind: PfKind,
    private val initialVault: String?,
    private val initialHost: String?,
) : ViewModel() {
    private val _state = MutableStateFlow(ForwardEditorState())
    val state: StateFlow<ForwardEditorState> = _state.asStateFlow()

    init {
        viewModelScope.launch { load() }
    }

    private suspend fun load() {
        runCatching {
            repo.read {
                val existing = ruleId?.let { pfRule(it) }
                val preset = initialHost?.let { host(it) }
                val vault = existing?.vaultId ?: preset?.vaultId ?: initialVault ?: localVault().id
                val hosts = hosts(vault).filter { it.protocol.equals("ssh", ignoreCase = true) }
                ForwardEditorState(
                    loading = false,
                    existing = existing,
                    vaultId = vault,
                    hosts = hosts,
                    kind = existing?.kind ?: initialKind,
                    label = existing?.label ?: "",
                    hostId = existing?.hostId?.takeIf { !existing.hostMissing } ?: preset?.id ?: hosts.singleOrNull()?.id,
                    bind = existing?.boundAddress ?: "",
                    localPort = existing?.localPort?.toString() ?: "",
                    remoteHost = existing?.remoteHost ?: "",
                    remotePort = existing?.remotePort?.takeIf { it > 0u }?.toString() ?: "",
                    autoStart = existing?.autoStart ?: false,
                )
            }
        }.onSuccess { _state.value = it }
            .onFailure { e -> _state.update { it.copy(loading = false, error = e.userMessage()) } }
    }

    fun update(transform: (ForwardEditorState) -> ForwardEditorState) = _state.update(transform)

    fun errorShown() = _state.update { it.copy(error = null) }

    /** Save the rule; a running tunnel is stopped so the next start uses the new settings. */
    fun save() {
        val s = _state.value
        val host = s.hostId ?: return
        if (!s.canSave) return
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching {
                s.existing?.let { forwards.stop(it.id) }
                repo.write {
                    savePfRule(
                        PfRuleDraft(
                            id = s.existing?.id,
                            vaultId = s.vaultId,
                            label = s.label.trim(),
                            hostId = host,
                            kind = s.kind,
                            boundAddress = s.bind.trim(),
                            localPort = s.localPort.toUShort(),
                            remoteHost = if (s.kind == PfKind.DYNAMIC) "" else s.remoteHost.trim(),
                            remotePort = if (s.kind == PfKind.DYNAMIC) 0u else s.remotePort.toUShort(),
                            autoStart = s.autoStart,
                        ),
                    )
                }
            }.onSuccess { _state.update { it.copy(working = false, done = true) } }
                .onFailure { e -> _state.update { it.copy(working = false, error = e.userMessage()) } }
        }
    }

    fun delete() {
        val id = _state.value.existing?.id ?: return
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching {
                forwards.stop(id)
                repo.write { deletePfRule(id) }
            }.onSuccess { _state.update { it.copy(working = false, done = true) } }
                .onFailure { e -> _state.update { it.copy(working = false, error = e.userMessage()) } }
        }
    }
}

/** New / edit forwarding rule: kind, host, ports, bind address. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ForwardEditorScreen(
    shell: ShellViewModel,
    ruleId: String?,
    kind: PfKind,
    vaultId: String?,
    hostId: String?,
    onClose: () -> Unit,
) {
    val vm: ForwardEditorViewModel = viewModel(key = "pf/${ruleId ?: "new"}") {
        ForwardEditorViewModel(shell.repo, shell.forwards, ruleId, kind, vaultId, hostId)
    }
    val s by vm.state.collectAsStateWithLifecycle()
    var confirmDelete by remember { mutableStateOf(false) }

    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }
    LaunchedEffect(s.done) { if (s.done) onClose() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(if (ruleId == null) stringResource(R.string.new_rule) else s.label.ifBlank { stringResource(R.string.edit_rule) }) },
                navigationIcon = { IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close)) } },
                actions = {
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
        if (s.loading) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            return@Scaffold
        }
        Column(
            Modifier.fillMaxSize().padding(padding).imePadding().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                PfKind.entries.forEachIndexed { i, k ->
                    SegmentedButton(
                        selected = s.kind == k,
                        onClick = { vm.update { it.copy(kind = k) } },
                        shape = SegmentedButtonDefaults.itemShape(index = i, count = PfKind.entries.size),
                    ) { Text(kindTitle(k).substringBefore(" ")) }
                }
            }

            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    FormField(s.label, { v -> vm.update { it.copy(label = v) } }, stringResource(R.string.label), placeholder = stringResource(R.string.optional))
                    if (s.hosts.isEmpty()) {
                        Text(
                            stringResource(R.string.no_ssh_hosts_in_this_vault_yet_add),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.error,
                        )
                    } else {
                        PickerRow(
                            label = stringResource(R.string.host),
                            value = s.hosts.firstOrNull { it.id == s.hostId }?.let { it.label.ifBlank { it.address } } ?: "",
                            options = s.hosts.map { it.id to it.label.ifBlank { it.address } },
                            selected = s.hostId,
                            onPick = { id -> vm.update { it.copy(hostId = id) } },
                            empty = null,
                        )
                    }
                }
            }

            SectionLabel(
                when (s.kind) {
                    PfKind.LOCAL -> stringResource(R.string.listen_on_this_device)
                    PfKind.REMOTE -> stringResource(R.string.listen_on_the_server)
                    PfKind.DYNAMIC -> stringResource(R.string.socks5_proxy_on_this_device)
                },
            )
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    FormField(
                        s.bind,
                        { v -> vm.update { it.copy(bind = v) } },
                        stringResource(R.string.bind_address),
                        placeholder = if (s.kind == PfKind.REMOTE) stringResource(R.string.localhost_server_side) else "127.0.0.1",
                        keyboard = KeyboardType.Uri,
                    )
                    if (s.kind == PfKind.REMOTE) {
                        FormField(s.remotePort, { v -> vm.update { it.copy(remotePort = v.filter(Char::isDigit)) } }, stringResource(R.string.port_on_server), keyboard = KeyboardType.Number)
                    } else {
                        FormField(s.localPort, { v -> vm.update { it.copy(localPort = v.filter(Char::isDigit)) } }, stringResource(R.string.local_port), keyboard = KeyboardType.Number)
                    }
                }
            }

            if (s.kind != PfKind.DYNAMIC) {
                SectionLabel(if (s.kind == PfKind.LOCAL) stringResource(R.string.forward_to_reachable_from_the_host) else stringResource(R.string.forward_to_reachable_from_this_device))
                SectionCard {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        FormField(
                            s.remoteHost,
                            { v -> vm.update { it.copy(remoteHost = v) } },
                            stringResource(R.string.destination_host),
                            placeholder = if (s.kind == PfKind.REMOTE) "127.0.0.1" else "db.internal",
                            keyboard = KeyboardType.Uri,
                        )
                        if (s.kind == PfKind.LOCAL) {
                            FormField(s.remotePort, { v -> vm.update { it.copy(remotePort = v.filter(Char::isDigit)) } }, stringResource(R.string.destination_port), keyboard = KeyboardType.Number)
                        } else {
                            FormField(s.localPort, { v -> vm.update { it.copy(localPort = v.filter(Char::isDigit)) } }, stringResource(R.string.destination_port), keyboard = KeyboardType.Number)
                        }
                    }
                }
            }

            SectionLabel(stringResource(R.string.options))
            SectionCard {
                SwitchRow(
                    title = stringResource(R.string.auto_start),
                    subtitle = stringResource(R.string.start_when_the_vault_unlocks_also_honoured_by),
                    checked = s.autoStart,
                    onCheckedChange = { v -> vm.update { it.copy(autoStart = v) } },
                )
            }

            if (s.existing != null) {
                SectionLabel(" ")
                SectionCard {
                    ListRow(
                        title = stringResource(R.string.delete_rule),
                        titleColor = MaterialTheme.colorScheme.error,
                        modifier = Modifier.clickable { confirmDelete = true },
                    )
                }
            }
        }
    }

    if (confirmDelete) {
        ConfirmDialog(
            title = stringResource(R.string.delete_rule_2),
            text = stringResource(R.string.will_be_removed_and_its_tunnel_stopped, s.label.ifBlank { s.existing?.route ?: "" }),
            confirm = stringResource(R.string.delete),
            onConfirm = { confirmDelete = false; vm.delete() },
            onDismiss = { confirmDelete = false },
        )
    }
}
