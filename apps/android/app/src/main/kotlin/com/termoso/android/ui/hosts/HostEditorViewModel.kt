package com.termoso.android.ui.hosts

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.userMessage
import com.termoso.android.data.VaultRepository
import com.termoso.core.EnvVar
import com.termoso.core.GroupItem
import com.termoso.core.HostDraft
import com.termoso.core.IdentityItem
import com.termoso.core.InheritedInfo
import com.termoso.core.KeyItem
import com.termoso.core.SnippetItem
import com.termoso.core.TagItem
import com.termoso.core.TelnetDraft
import com.termoso.core.VaultInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class HostEditorState(
    val loading: Boolean = true,
    val draft: HostDraft? = null,
    val vaults: List<VaultInfo> = emptyList(),
    val groups: List<GroupItem> = emptyList(),
    val tags: List<TagItem> = emptyList(),
    val keys: List<KeyItem> = emptyList(),
    val identities: List<IdentityItem> = emptyList(),
    val snippets: List<SnippetItem> = emptyList(),
    val inherited: InheritedInfo? = null,
    val saving: Boolean = false,
    val saved: Boolean = false,
    val error: String? = null,
) {
    val canSave: Boolean get() = draft?.address?.isNotBlank() == true && !saving
}

/**
 * New/edit host form over [HostDraft]. The draft is the only thing that holds a
 * password, and only when the user typed one; `null` keeps the stored secret.
 */
class HostEditorViewModel(
    private val repo: VaultRepository,
    private val hostId: String?,
    private val initialVault: String?,
    private val groupId: String?,
) : ViewModel() {
    private val _state = MutableStateFlow(HostEditorState())
    val state: StateFlow<HostEditorState> = _state.asStateFlow()

    init {
        viewModelScope.launch { load() }
    }

    private suspend fun load() {
        runCatching {
            repo.read {
                val vaults = vaults().filter { !it.locked }
                val draft = if (hostId != null) {
                    hostDraft(hostId)
                } else {
                    val vault = initialVault ?: vaults.first().id
                    newHostDraft(vault, groupId)
                }
                HostEditorState(
                    loading = false,
                    draft = draft,
                    vaults = vaults,
                    groups = groups(draft.vaultId),
                    tags = tags(draft.vaultId),
                    keys = keys(draft.vaultId),
                    identities = identities(draft.vaultId),
                    snippets = snippets(draft.vaultId),
                    inherited = inherited(draft.groupId),
                )
            }
        }.onSuccess { s -> _state.value = s }
            .onFailure { e -> _state.update { it.copy(loading = false, error = e.userMessage()) } }
    }

    fun update(transform: (HostDraft) -> HostDraft) {
        _state.update { s -> s.draft?.let { s.copy(draft = transform(it)) } ?: s }
    }

    /** Edit the Telnet section; no-op while the host has none. */
    fun updateTelnet(transform: (TelnetDraft) -> TelnetDraft) = update { d ->
        d.telnet?.let { d.copy(telnet = transform(it)) } ?: d
    }

    /** Switching vault (new hosts only) resets group/tags/key/identity, which are per-vault. */
    fun setVault(vaultId: String) {
        val current = _state.value.draft ?: return
        if (current.vaultId == vaultId || current.id != null) return
        viewModelScope.launch {
            runCatching {
                repo.read {
                    _state.value.copy(
                        draft = current.copy(
                            vaultId = vaultId,
                            groupId = null,
                            tagIds = emptyList(),
                            sshKeyId = null,
                            identityId = null,
                            startupSnippetId = null,
                        ),
                        groups = groups(vaultId),
                        tags = tags(vaultId),
                        keys = keys(vaultId),
                        identities = identities(vaultId),
                        snippets = snippets(vaultId),
                        inherited = inherited(null),
                    )
                }
            }.onSuccess { _state.value = it }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun setGroup(groupId: String?) {
        update { it.copy(groupId = groupId) }
        viewModelScope.launch {
            runCatching { repo.read { inherited(groupId) } }
                .onSuccess { inh -> _state.update { it.copy(inherited = inh) } }
        }
    }

    fun toggleTag(tagId: String) = update {
        it.copy(tagIds = if (tagId in it.tagIds) it.tagIds - tagId else it.tagIds + tagId)
    }

    fun createTag(label: String) {
        val draft = _state.value.draft ?: return
        viewModelScope.launch {
            runCatching { repo.write { createTag(draft.vaultId, label.trim()) } }
                .onSuccess { tag ->
                    _state.update { s ->
                        s.copy(
                            tags = (s.tags + tag).distinctBy { it.id },
                            draft = s.draft?.copy(tagIds = s.draft.tagIds + tag.id),
                        )
                    }
                }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun setEnv(index: Int, name: String, value: String) = update {
        it.copy(envVariables = it.envVariables.toMutableList().apply { this[index] = EnvVar(name, value) })
    }

    fun addEnv() = update { it.copy(envVariables = it.envVariables + EnvVar("", "")) }

    fun removeEnv(index: Int) = update {
        it.copy(envVariables = it.envVariables.toMutableList().apply { removeAt(index) })
    }

    fun save() {
        val draft = _state.value.draft ?: return
        if (!_state.value.canSave) return
        _state.update { it.copy(saving = true) }
        viewModelScope.launch {
            val clean = draft.copy(
                label = draft.label.trim(),
                address = draft.address.trim(),
                username = draft.username.trim(),
                telnet = draft.telnet?.let { it.copy(username = it.username.trim()) },
                envVariables = draft.envVariables.filter { it.name.isNotBlank() },
            )
            runCatching { repo.write { saveHost(clean) } }
                .onSuccess { _state.update { it.copy(saving = false, saved = true) } }
                .onFailure { e -> _state.update { it.copy(saving = false, error = e.userMessage()) } }
        }
    }

    fun errorShown() = _state.update { it.copy(error = null) }
}
