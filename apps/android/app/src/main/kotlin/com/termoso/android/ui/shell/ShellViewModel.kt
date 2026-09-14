package com.termoso.android.ui.shell

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.SessionManager
import com.termoso.android.data.ForwardManager
import com.termoso.android.data.SftpConnection
import com.termoso.android.data.SftpManager
import com.termoso.android.data.TerminalSession
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.core.QuickTarget
import com.termoso.core.SessionState
import com.termoso.core.SnippetRun
import com.termoso.core.VaultInfo
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/** Time for the shell to print its prompt before a snippet is typed into a fresh session. */
private const val SHELL_READY_DELAY_MS = 400L

/** Time for the last line to reach the server before a close-after-run snippet drops the session. */
private const val CLOSE_AFTER_RUN_DELAY_MS = 1_000L

/** Cross-tab state: the vault currently shown in Vaults → Hosts, open terminals, one-shot notices. */
class ShellViewModel(
    val repo: VaultRepository,
    val sessions: SessionManager,
    val sftp: SftpManager,
    val forwards: ForwardManager,
) : ViewModel() {
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

    /** Join a `termoso://join/…` share as a viewer; the link is validated in Rust. */
    suspend fun joinLive(link: String): TerminalSession? =
        runCatching { sessions.joinLive(link.trim()) }
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

    /**
     * Expand the snippet in Rust and type it into the given open terminals.
     * `paste` leaves out the trailing newline. Sessions the snippet asks to
     * close are dropped shortly after the text went out.
     */
    suspend fun runSnippet(
        snippetId: String,
        targets: List<TerminalSession>,
        vars: Map<String, String>,
        paste: Boolean,
    ): SnippetRun? {
        val run = runCatching { repo.read { runSnippet(snippetId, targets.map { it.rust }, vars, paste) } }
            .onFailure { notify(it.userMessage()) }
            .getOrNull() ?: return null
        if (run.closeAfterRun) {
            viewModelScope.launch {
                delay(CLOSE_AFTER_RUN_DELAY_MS)
                run.sessionIds.forEach { sessions.close(it) }
            }
        }
        return run
    }

    /** Open a terminal to a saved host and run the snippet once its shell is up. */
    suspend fun connectAndRunSnippet(
        hostId: String,
        snippetId: String,
        vars: Map<String, String>,
        paste: Boolean,
    ): TerminalSession? {
        val session = connectHost(hostId) ?: return null
        viewModelScope.launch {
            val state = session.state.first { it !is SessionState.Connecting }
            if (state is SessionState.Connected && sessions.find(session.id) != null) {
                delay(SHELL_READY_DELAY_MS)
                runSnippet(snippetId, listOf(session), vars, paste)
            }
        }
        return session
    }

    fun noticeShown() {
        _notice.value = null
    }
}
