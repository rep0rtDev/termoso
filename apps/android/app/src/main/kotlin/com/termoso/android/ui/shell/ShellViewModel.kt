package com.termoso.android.ui.shell

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.SessionManager
import com.termoso.android.data.SftpConnection
import com.termoso.android.data.SftpManager
import com.termoso.android.data.TerminalSession
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.core.QuickTarget
import com.termoso.core.VaultInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** Cross-tab state: the vault currently shown in Vaults → Hosts, open terminals, one-shot notices. */
class ShellViewModel(val repo: VaultRepository, val sessions: SessionManager, val sftp: SftpManager) : ViewModel() {
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

    /**
     * Open a terminal to a saved host. Returns null (after a notice) when Rust
     * refuses to even start — e.g. the host is missing; connection errors
     * themselves arrive later through the session state.
     */
    suspend fun connectHost(hostId: String): TerminalSession? =
        runCatching { sessions.connectHost(hostId) }
            .onFailure { notify(it.userMessage()) }
            .getOrNull()

    suspend fun connectQuick(target: QuickTarget): TerminalSession? =
        runCatching { sessions.connectQuick(target) }
            .onFailure { notify(it.userMessage()) }
            .getOrNull()

    suspend fun openSftpHost(hostId: String): SftpConnection? =
        runCatching { sftp.openHost(hostId) }
            .onFailure { notify(it.userMessage()) }
            .getOrNull()

    suspend fun openSftpQuick(target: QuickTarget): SftpConnection? =
        runCatching { sftp.openQuick(target) }
            .onFailure { notify(it.userMessage()) }
            .getOrNull()

    fun noticeShown() {
        _notice.value = null
    }
}
