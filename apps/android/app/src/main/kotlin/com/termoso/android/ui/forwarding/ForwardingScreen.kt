package com.termoso.android.ui.forwarding

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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.CallMade
import androidx.compose.material.icons.filled.CallReceived
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Hub
import androidx.compose.material.icons.filled.SwapHoriz
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.R
import com.termoso.android.data.ForwardManager
import com.termoso.android.data.PendingPrompt
import com.termoso.android.data.Tunnel
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.components.TermosoSwitch
import com.termoso.android.ui.components.connectingLabel
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.terminal.PromptDialog
import com.termoso.core.PfKind
import com.termoso.core.PfRuleItem
import com.termoso.core.TunnelState
import com.termoso.core.TunnelStats
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class ForwardingUiState(
    val loading: Boolean = true,
    val rules: List<PfRuleItem> = emptyList(),
    val error: String? = null,
)

class ForwardingViewModel(
    private val repo: VaultRepository,
    private val forwards: ForwardManager,
    private val vaultId: StateFlow<String?>,
) : ViewModel() {
    private val _state = MutableStateFlow(ForwardingUiState())
    val state: StateFlow<ForwardingUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            combine(vaultId, repo.revision) { v, _ -> v }.collect { reload(it) }
        }
    }

    private suspend fun reload(vault: String?) {
        runCatching { repo.read { pfRules(vault) } }
            .onSuccess { list -> _state.update { it.copy(loading = false, rules = list) } }
            .onFailure { e -> _state.update { it.copy(loading = false, error = e.userMessage()) } }
    }

    fun errorShown() = _state.update { it.copy(error = null) }

    fun toggle(rule: PfRuleItem, on: Boolean) {
        viewModelScope.launch {
            runCatching { if (on) forwards.start(rule.id) else forwards.stop(rule.id) }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun duplicate(id: String) {
        viewModelScope.launch {
            runCatching { repo.write { duplicatePfRule(id) } }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun delete(id: String) {
        viewModelScope.launch {
            runCatching {
                forwards.stop(id)
                repo.write { deletePfRule(id) }
            }.onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }
}

/** Port forwarding rules of the selected vault: Local / Remote / Dynamic cards with a start–stop switch. */
@Composable
fun ForwardingScreen(
    shell: ShellViewModel,
    onBack: () -> Unit,
    onNewRule: () -> Unit,
    onEditRule: (String) -> Unit,
) {
    val vm: ForwardingViewModel = viewModel { ForwardingViewModel(shell.repo, shell.forwards, shell.selectedVaultId) }
    val state by vm.state.collectAsStateWithLifecycle()
    val tunnels by shell.forwards.tunnels.collectAsStateWithLifecycle()
    val errors by shell.forwards.lastError.collectAsStateWithLifecycle()
    var confirmDelete by remember { mutableStateOf<PfRuleItem?>(null) }

    LaunchedEffect(state.error) { state.error?.let { shell.notify(it); vm.errorShown() } }

    SubScreen(
        title = stringResource(R.string.port_forwarding),
        onBack = onBack,
        floating = {
            if (state.rules.isNotEmpty()) {
                FloatingActionButton(onClick = onNewRule) { Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.new_rule)) }
            }
        },
    ) { padding ->
        when {
            state.loading -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            state.rules.isEmpty() -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = stringResource(R.string.no_port_forwarding_rules),
                    hint = stringResource(R.string.reach_a_database_behind_a_host_expose_a),
                    icon = Icons.Filled.SwapHoriz,
                    action = { Button(onClick = onNewRule) { Text(stringResource(R.string.create_a_rule)) } },
                )
            }
            else -> LazyColumn(
                Modifier.fillMaxSize().padding(padding),
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                items(state.rules, key = { it.id }) { rule ->
                    RuleCard(
                        rule = rule,
                        tunnel = tunnels[rule.id],
                        lastError = errors[rule.id],
                        onToggle = { on -> vm.toggle(rule, on) },
                        onEdit = { onEditRule(rule.id) },
                        onDuplicate = { vm.duplicate(rule.id) },
                        onDelete = { confirmDelete = rule },
                    )
                }
                item { Spacer(Modifier.height(72.dp)) }
            }
        }
    }

    confirmDelete?.let { rule ->
        ConfirmDialog(
            title = stringResource(R.string.delete_rule_2),
            text = if (tunnels.containsKey(rule.id)) stringResource(R.string.will_be_removed_and_its_tunnel_stopped, rule.label.ifBlank { rule.route }) else stringResource(R.string.will_be_removed, rule.label.ifBlank { rule.route }),
            confirm = stringResource(R.string.delete),
            onConfirm = { vm.delete(rule.id); confirmDelete = null },
            onDismiss = { confirmDelete = null },
        )
    }
}

@Composable
private fun RuleCard(
    rule: PfRuleItem,
    tunnel: Tunnel?,
    lastError: String?,
    onToggle: (Boolean) -> Unit,
    onEdit: () -> Unit,
    onDuplicate: () -> Unit,
    onDelete: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    // Starting a tunnel takes a moment before `tunnel` appears; show the
    // requested position right away and fall back to the real one if the
    // start never materialises.
    var pending by remember { mutableStateOf<Boolean?>(null) }
    LaunchedEffect(tunnel != null, lastError) { pending = null }
    LaunchedEffect(pending) {
        if (pending != null) {
            delay(4_000)
            pending = null
        }
    }
    val state = if (tunnel != null) tunnel.state.collectAsStateWithLifecycle().value else null
    val stats = if (tunnel != null) tunnel.stats.collectAsStateWithLifecycle().value else null
    val busy = state is TunnelState.Connecting || state is TunnelState.Reconnecting
    val statusColor = when {
        state is TunnelState.Running -> MaterialTheme.colorScheme.primary
        state is TunnelState.Reconnecting || (tunnel == null && lastError != null) -> MaterialTheme.colorScheme.error
        else -> MaterialTheme.colorScheme.onSurfaceVariant
    }

    Box {
        SectionCard(
            modifier = Modifier.combinedClickable(onClick = onEdit, onLongClick = { menu = true }),
        ) {
            Row(
                Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconTile(kindIcon(rule.kind))
                Spacer(Modifier.width(16.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        rule.label.ifBlank { kindTitle(rule.kind) },
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = FontWeight.Medium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        rule.route,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        statusLine(rule, state, stats, lastError),
                        style = MaterialTheme.typography.bodySmall,
                        color = statusColor,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Spacer(Modifier.width(8.dp))
                if (busy) {
                    CircularProgressIndicator(modifier = Modifier.padding(end = 12.dp).height(22.dp), strokeWidth = 2.dp)
                }
                TermosoSwitch(
                    checked = pending ?: (tunnel != null),
                    onCheckedChange = { on -> pending = on; onToggle(on) },
                    enabled = !rule.hostMissing,
                )
            }
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text(stringResource(R.string.edit)) }, leadingIcon = { Icon(Icons.Filled.Edit, null) }, onClick = { menu = false; onEdit() })
            DropdownMenuItem(
                text = { Text(stringResource(R.string.duplicate)) },
                leadingIcon = { Icon(Icons.Filled.ContentCopy, null) },
                onClick = { menu = false; onDuplicate() },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) },
                leadingIcon = { Icon(Icons.Filled.Delete, null, tint = MaterialTheme.colorScheme.error) },
                onClick = { menu = false; onDelete() },
            )
        }
    }
}

/**
 * Connection prompts (host key, password, …) raised by any live tunnel. Lives
 * in the shell so a tunnel started from the rules list can still ask after the
 * user navigated elsewhere.
 */
@Composable
fun TunnelPromptHost(forwards: ForwardManager) {
    val tunnels by forwards.tunnels.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val pending: List<Pair<Tunnel, State<PendingPrompt?>>> =
        tunnels.values.map { it to it.prompt.collectAsStateWithLifecycle() }
    val (tunnel, prompt) = pending.firstNotNullOfOrNull { (t, s) -> s.value?.let { t to it } } ?: return
    PromptDialog(prompt) { answer -> scope.launch { tunnel.answer(prompt, answer) } }
}

private fun statusLine(rule: PfRuleItem, state: TunnelState?, stats: TunnelStats?, lastError: String?): String {
    val host = if (rule.hostMissing) str(R.string.host_deleted) else rule.hostLabel
    return when (state) {
        null -> lastError?.let { str(R.string.stopped, it) } ?: str(R.string.host_stopped, host)
        is TunnelState.Connecting -> "$host · ${connectingLabel(state)}"
        is TunnelState.Running -> buildString {
            append(str(R.string.running_on, state.bound))
            if (stats != null) {
                append(str(R.string.sep_active_total, stats.active.toLong(), stats.connections.toLong()))
                if (stats.bytesIn > 0u || stats.bytesOut > 0u) {
                    append(" · ↓${bytes(stats.bytesIn)} ↑${bytes(stats.bytesOut)}")
                }
            }
        }
        is TunnelState.Reconnecting -> str(R.string.reconnecting_in_s, state.attempt, state.retryInSecs, state.reason)
        is TunnelState.Failed -> str(R.string.failed, state.message)
        is TunnelState.Stopped -> str(R.string.host_stopped, host)
    }
}

fun kindTitle(kind: PfKind): String = when (kind) {
    PfKind.LOCAL -> str(R.string.local_forwarding)
    PfKind.REMOTE -> str(R.string.remote_forwarding)
    PfKind.DYNAMIC -> str(R.string.dynamic_socks5)
}

fun kindIcon(kind: PfKind): ImageVector = when (kind) {
    PfKind.LOCAL -> Icons.Filled.CallMade
    PfKind.REMOTE -> Icons.Filled.CallReceived
    PfKind.DYNAMIC -> Icons.Filled.Hub
}

fun bytes(n: ULong): String {
    val v = n.toDouble()
    return when {
        v < 1024 -> str(R.string.size_b, n.toLong())
        v < 1024 * 1024 -> str(R.string.size_kb, v / 1024)
        v < 1024.0 * 1024 * 1024 -> str(R.string.size_mb, v / 1024 / 1024)
        else -> str(R.string.size_gb, v / 1024 / 1024 / 1024)
    }
}
