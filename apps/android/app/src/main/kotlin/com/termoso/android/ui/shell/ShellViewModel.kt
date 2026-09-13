package com.termoso.android.ui.shell

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.VaultRepository
import com.termoso.core.VaultInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** Cross-tab state: the vault currently shown in Vaults → Hosts, plus one-shot notices. */
class ShellViewModel(val repo: VaultRepository) : ViewModel() {
    private val _vaults = MutableStateFlow<List<VaultInfo>>(emptyList())
    val vaults: StateFlow<List<VaultInfo>> = _vaults.asStateFlow()

    private val _selectedVaultId = MutableStateFlow<String?>(null)
    val selectedVaultId: StateFlow<String?> = _selectedVaultId.asStateFlow()

    private val _notice = MutableStateFlow<String?>(null)
    val notice: StateFlow<String?> = _notice.asStateFlow()

    init {
        viewModelScope.launch {
            repo.revision.collect { reload() }
        }
    }

    private suspend fun reload() {
        val list = runCatching { repo.read { vaults() } }.getOrDefault(emptyList())
        _vaults.value = list
        if (_selectedVaultId.value == null || list.none { it.id == _selectedVaultId.value }) {
            _selectedVaultId.value = list.firstOrNull()?.id
        }
    }

    fun selectVault(id: String) {
        _selectedVaultId.value = id
    }

    fun notify(message: String) {
        _notice.value = message
    }

    fun noticeShown() {
        _notice.value = null
    }
}
