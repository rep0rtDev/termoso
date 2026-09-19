package com.termoso.android.ui.keychain

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.R
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.PickerRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SecretField
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.vault.vaultLabel
import com.termoso.core.IdentityDraft
import com.termoso.core.IdentityItem
import com.termoso.core.KeyItem
import com.termoso.core.SshIdKeyKind
import com.termoso.core.VaultInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class IdentityEditorState(
    val loading: Boolean = true,
    val existing: IdentityItem? = null,
    val vaults: List<VaultInfo> = emptyList(),
    val keys: List<KeyItem> = emptyList(),
    val vaultId: String? = null,
    val label: String = "",
    val username: String = "",
    /** null = keep the stored password; "" = clear it. */
    val password: String? = null,
    val sshKeyId: String? = null,
    val sshId: Boolean = false,
    val sshIdKeyType: SshIdKeyKind? = null,
    val working: Boolean = false,
    val done: Boolean = false,
    val error: String? = null,
) {
    val canSave get() = !working && vaultId != null && (label.isNotBlank() || username.isNotBlank() || sshId)
}

class IdentityEditorViewModel(
    private val repo: VaultRepository,
    private val identityId: String?,
    private val initialVault: String?,
) : ViewModel() {
    private val _state = MutableStateFlow(IdentityEditorState())
    val state: StateFlow<IdentityEditorState> = _state.asStateFlow()

    init {
        viewModelScope.launch { load() }
    }

    private suspend fun load() {
        runCatching {
            repo.read {
                val vaults = vaults().filter { !it.locked }
                val existing = identityId?.let { id -> identities(null).firstOrNull { it.id == id } ?: error(str(R.string.identity_not_found)) }
                val vault = existing?.vaultId ?: initialVault ?: vaults.first().id
                IdentityEditorState(
                    loading = false,
                    existing = existing,
                    vaults = vaults,
                    keys = keys(vault),
                    vaultId = vault,
                    label = existing?.label ?: "",
                    username = existing?.username ?: "",
                    sshKeyId = existing?.sshKeyId,
                    sshId = existing?.sshId ?: false,
                    sshIdKeyType = existing?.sshIdKeyType,
                )
            }
        }.onSuccess { _state.value = it }
            .onFailure { e -> _state.update { it.copy(loading = false, error = e.userMessage()) } }
    }

    fun update(transform: (IdentityEditorState) -> IdentityEditorState) = _state.update(transform)

    fun setVault(id: String) {
        viewModelScope.launch {
            val keys = runCatching { repo.read { keys(id) } }.getOrDefault(emptyList())
            _state.update { it.copy(vaultId = id, keys = keys, sshKeyId = it.sshKeyId?.takeIf { k -> keys.any { key -> key.id == k } }) }
        }
    }

    fun errorShown() = _state.update { it.copy(error = null) }

    fun save() {
        val s = _state.value
        val vault = s.vaultId ?: return
        if (!s.canSave) return
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching {
                repo.write {
                    saveIdentity(
                        IdentityDraft(
                            id = s.existing?.id,
                            vaultId = vault,
                            label = s.label.trim().ifBlank { s.username.trim() }.ifBlank { str(R.string.ssh_id) },
                            username = s.username.trim(),
                            password = s.password,
                            sshKeyId = s.sshKeyId,
                            sshId = s.sshId,
                            sshIdKeyType = s.sshIdKeyType.takeIf { s.sshId },
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
            runCatching { repo.write { deleteIdentity(id) } }
                .onSuccess { _state.update { it.copy(working = false, done = true) } }
                .onFailure { e -> _state.update { it.copy(working = false, error = e.userMessage()) } }
        }
    }
}

/** New / edit identity: label, username, password, SSH key, SSH ID. The certificate is preserved by Rust. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun IdentityEditorScreen(shell: ShellViewModel, identityId: String?, onClose: () -> Unit) {
    val initialVault by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vm: IdentityEditorViewModel = viewModel(key = "identity/${identityId ?: "new"}") {
        IdentityEditorViewModel(shell.repo, identityId, initialVault)
    }
    val s by vm.state.collectAsStateWithLifecycle()
    var confirmDelete by remember { mutableStateOf(false) }

    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }
    LaunchedEffect(s.done) { if (s.done) onClose() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(if (identityId == null) stringResource(R.string.new_identity) else s.label.ifBlank { stringResource(R.string.edit_identity) }) },
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
            if (s.existing == null && s.vaults.size > 1) {
                PickerRow(
                    label = stringResource(R.string.vault),
                    value = s.vaults.firstOrNull { it.id == s.vaultId }?.let(::vaultLabel) ?: "",
                    options = s.vaults.map { it.id to vaultLabel(it) },
                    selected = s.vaultId,
                    onPick = { id -> id?.let(vm::setVault) },
                    empty = null,
                )
            }
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    FormField(s.label, { v -> vm.update { it.copy(label = v) } }, stringResource(R.string.label), placeholder = s.username.ifBlank { stringResource(R.string.work_account) })
                    FormField(s.username, { v -> vm.update { it.copy(username = v) } }, stringResource(R.string.username))
                }
            }

            SectionLabel(stringResource(R.string.credentials))
            SectionCard {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    SshIdRows(
                        sshId = s.sshId,
                        keyType = s.sshIdKeyType,
                        onSshId = { v -> vm.update { it.copy(sshId = v) } },
                        onKeyType = { v -> vm.update { it.copy(sshIdKeyType = v) } },
                        usernameHint = s.username.isBlank(),
                    )
                    val keepStored = s.password == null && s.existing?.hasPassword == true
                    SecretField(
                        value = s.password ?: "",
                        onChange = { v -> vm.update { it.copy(password = v) } },
                        label = stringResource(R.string.password),
                        placeholder = if (keepStored) stringResource(R.string.saved) else null,
                        leadingActions = if (keepStored) {
                            { TextButton(onClick = { vm.update { it.copy(password = "") } }) { Text(stringResource(R.string.clear)) } }
                        } else {
                            null
                        },
                    )
                    val keyLabel = s.keys.firstOrNull { it.id == s.sshKeyId }?.label ?: stringResource(R.string.none)
                    PickerRow(
                        label = stringResource(R.string.ssh_key),
                        value = keyLabel,
                        options = listOf<Pair<String?, String>>(null to stringResource(R.string.none)) + s.keys.map { it.id to it.label },
                        selected = s.sshKeyId,
                        onPick = { id -> vm.update { it.copy(sshKeyId = id) } },
                        empty = stringResource(R.string.no_keys_in_this_vault_yet),
                    )
                }
                if (s.existing?.hasCertificate == true) {
                    RowDivider()
                    ListRow(title = stringResource(R.string.certificate), subtitle = stringResource(R.string.attached_on_desktop_kept_as_is))
                }
            }

            if (s.existing != null) {
                SectionLabel(" ")
                SectionCard {
                    ListRow(
                        title = stringResource(R.string.delete_identity),
                        titleColor = MaterialTheme.colorScheme.error,
                        modifier = Modifier.clickable { confirmDelete = true },
                    )
                }
            }
        }
    }

    if (confirmDelete) {
        ConfirmDialog(
            title = stringResource(R.string.delete_identity_2),
            text = stringResource(R.string.hosts_using_will_lose_these_credentials, s.label),
            confirm = stringResource(R.string.delete),
            onConfirm = { confirmDelete = false; vm.delete() },
            onDismiss = { confirmDelete = false },
        )
    }
}
