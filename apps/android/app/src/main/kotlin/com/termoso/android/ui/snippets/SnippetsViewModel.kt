package com.termoso.android.ui.snippets

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.core.SnippetItem
import com.termoso.core.SnippetPackageItem
import com.termoso.core.VaultInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SnippetsUiState(
    val loading: Boolean = true,
    val vaultId: String? = null,
    val snippets: List<SnippetItem> = emptyList(),
    val packages: List<SnippetPackageItem> = emptyList(),
    val error: String? = null,
    val notice: String? = null,
) {
    fun packagePath(id: String?): List<SnippetPackageItem> {
        val out = ArrayDeque<SnippetPackageItem>()
        var cur = id
        while (cur != null) {
            val p = packages.firstOrNull { it.id == cur } ?: break
            out.addFirst(p)
            cur = p.parentId
        }
        return out.toList()
    }
}

/** Snippets and packages of the selected vault; every mutation goes through Rust and bumps the repo revision. */
class SnippetsViewModel(
    private val repo: VaultRepository,
    private val vaultId: StateFlow<String?>,
) : ViewModel() {
    private val _state = MutableStateFlow(SnippetsUiState())
    val state: StateFlow<SnippetsUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            combine(vaultId, repo.revision) { v, _ -> v }.collect { reload(it) }
        }
    }

    private suspend fun reload(vault: String?) {
        runCatching { repo.read { snippets(vault) to snippetPackages(vault) } }
            .onSuccess { (s, p) -> _state.update { it.copy(loading = false, vaultId = vault, snippets = s, packages = p) } }
            .onFailure { e -> _state.update { it.copy(loading = false, vaultId = vault, error = e.userMessage()) } }
    }

    fun errorShown() = _state.update { it.copy(error = null) }

    fun noticeShown() = _state.update { it.copy(notice = null) }

    private fun mutate(block: suspend () -> Unit) {
        viewModelScope.launch {
            runCatching { block() }.onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun duplicate(id: String) = mutate { repo.write { duplicateSnippet(id) } }

    fun delete(id: String) = mutate { repo.write { deleteSnippet(id) } }

    fun savePackage(vaultId: String, id: String?, label: String, parentId: String?) =
        mutate { repo.write { saveSnippetPackage(vaultId, id, label.trim(), parentId) } }

    fun deletePackage(id: String) = mutate { repo.write { deleteSnippetPackage(id) } }

    /** Copy or move a snippet into [destination]; Rust owns what travels and what stays. */
    fun copyToVault(snippet: SnippetItem, destination: VaultInfo, move: Boolean) = mutate {
        repo.write { copySnippetToVault(snippet.id, destination.id, move) }
        _state.update { it.copy(notice = transferNotice(snippet.label, destination, move)) }
    }

    /** Copy or move a package with its subtree into [destination]. */
    fun copyPackageToVault(pkg: SnippetPackageItem, destination: VaultInfo, move: Boolean) = mutate {
        repo.write { copySnippetPackageToVault(pkg.id, destination.id, move) }
        _state.update { it.copy(notice = transferNotice(pkg.label, destination, move)) }
    }
}
