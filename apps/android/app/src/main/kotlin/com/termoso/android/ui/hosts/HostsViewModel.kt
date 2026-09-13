package com.termoso.android.ui.hosts

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.userMessage
import com.termoso.android.data.VaultRepository
import com.termoso.core.GroupItem
import com.termoso.core.HostItem
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

enum class HostSort(val label: String) {
    NAME("Name"),
    LAST_CONNECTED("Last connected"),
    UPDATED("Recently updated"),
}

data class HostsUiState(
    val loading: Boolean = true,
    val group: GroupItem? = null,
    val groups: List<GroupItem> = emptyList(),
    val hosts: List<HostItem> = emptyList(),
    val allGroups: List<GroupItem> = emptyList(),
    val query: String = "",
    val sort: HostSort = HostSort.NAME,
    val selected: Set<String> = emptySet(),
    val error: String? = null,
) {
    val selecting: Boolean get() = selected.isNotEmpty()

    val visibleGroups: List<GroupItem>
        get() = groups
            .filter { query.isBlank() || it.label.contains(query, ignoreCase = true) }
            .sortedBy { it.label.lowercase() }

    val visibleHosts: List<HostItem>
        get() {
            val q = query.trim()
            val filtered = if (q.isBlank()) {
                hosts
            } else {
                hosts.filter {
                    it.label.contains(q, true) || it.address.contains(q, true) ||
                        it.username.contains(q, true) || it.tags.any { t -> t.contains(q, true) }
                }
            }
            return when (sort) {
                HostSort.NAME -> filtered.sortedBy { it.label.ifBlank { it.address }.lowercase() }
                HostSort.LAST_CONNECTED -> filtered.sortedByDescending { it.lastConnected ?: Long.MIN_VALUE }
                HostSort.UPDATED -> filtered.sortedByDescending { it.updatedAt }
            }
        }
}

/**
 * One level of the host tree (a vault root or a group). Reloads whenever the
 * repository reports a mutation or the selected vault changes.
 */
class HostsViewModel(
    private val repo: VaultRepository,
    vaultId: StateFlow<String?>,
    private val groupId: String?,
) : ViewModel() {
    private val _state = MutableStateFlow(HostsUiState())
    val state: StateFlow<HostsUiState> = _state.asStateFlow()
    private var currentVault: String? = null

    init {
        viewModelScope.launch {
            combine(repo.revision, vaultId) { _, v -> v }.collect { v ->
                currentVault = v
                reload(v)
            }
        }
    }

    private suspend fun reload(vaultId: String?) {
        if (vaultId == null) return
        runCatching {
            repo.read {
                val all = groups(vaultId)
                Triple(
                    all,
                    all.filter { it.parentId == groupId },
                    hosts(vaultId).filter { it.groupId == groupId },
                )
            }
        }.onSuccess { (all, groups, hosts) ->
            _state.update {
                it.copy(
                    loading = false,
                    group = all.firstOrNull { g -> g.id == groupId },
                    allGroups = all,
                    groups = groups,
                    hosts = hosts,
                    selected = it.selected.filterTo(HashSet()) { id -> hosts.any { h -> h.id == id } },
                    error = null,
                )
            }
        }.onFailure { e ->
            _state.update { it.copy(loading = false, error = e.userMessage()) }
        }
    }

    fun setQuery(q: String) = _state.update { it.copy(query = q) }

    fun setSort(sort: HostSort) = _state.update { it.copy(sort = sort) }

    fun toggle(id: String) = _state.update {
        it.copy(selected = if (id in it.selected) it.selected - id else it.selected + id)
    }

    fun selectAll() = _state.update { it.copy(selected = it.visibleHosts.map { h -> h.id }.toSet()) }

    fun clearSelection() = _state.update { it.copy(selected = emptySet()) }

    private fun mutate(block: suspend () -> Unit) {
        viewModelScope.launch {
            runCatching { block() }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
            clearSelection()
        }
    }

    fun deleteSelected() = mutate {
        val ids = _state.value.selected.toList()
        repo.write { deleteHosts(ids) }
    }

    fun duplicateSelected() = mutate {
        val ids = _state.value.selected.toList()
        repo.write { ids.forEach { duplicateHost(it) } }
    }

    fun moveSelected(targetGroup: String?) = mutate {
        val ids = _state.value.selected.toList()
        repo.write { moveHosts(ids, targetGroup) }
    }

    fun copySelected(vaultId: String, withCredentials: Boolean) = mutate {
        val ids = _state.value.selected.toList()
        repo.write { copyHostsToVault(ids, vaultId, withCredentials) }
    }

    fun createGroup(label: String) = mutate {
        val vault = currentVault ?: return@mutate
        repo.write { saveGroup(vault, null, label.trim(), groupId) }
    }

    fun renameGroup(group: GroupItem, label: String) = mutate {
        repo.write { saveGroup(group.vaultId, group.id, label.trim(), group.parentId) }
    }

    fun deleteGroup(group: GroupItem) = mutate {
        repo.write { deleteGroup(group.id) }
    }

    fun errorShown() = _state.update { it.copy(error = null) }
}
