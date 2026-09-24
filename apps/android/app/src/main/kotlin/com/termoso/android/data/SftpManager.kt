package com.termoso.android.data

import android.content.Context
import com.termoso.android.R
import com.termoso.android.ui.components.initialConnecting
import com.termoso.core.FileCapabilities
import com.termoso.core.FileProtocol
import com.termoso.core.HostItem
import com.termoso.core.PromptAnswer
import com.termoso.core.PromptRequest
import com.termoso.core.QuickTarget
import com.termoso.core.SessionState
import com.termoso.core.SftpListener
import com.termoso.core.SftpSession
import com.termoso.core.TransferCard
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext

/** Rust SFTP callbacks republished as flows for the UI. */
class SftpBridge : SftpListener {
    private val _state = MutableStateFlow<SessionState>(initialConnecting())
    val state: StateFlow<SessionState> = _state.asStateFlow()

    private val _prompt = MutableStateFlow<PendingPrompt?>(null)
    val prompt: StateFlow<PendingPrompt?> = _prompt.asStateFlow()

    private val _transfers = MutableStateFlow<List<TransferCard>>(emptyList())
    /** Every transfer of this connection, oldest first, live progress included. */
    val transfers: StateFlow<List<TransferCard>> = _transfers.asStateFlow()

    override fun onState(state: SessionState) {
        _state.value = state
        if (state !is SessionState.Connecting) _prompt.value = null
    }

    override fun onPrompt(promptId: ULong, request: PromptRequest) {
        _prompt.value = PendingPrompt(promptId, request)
    }

    override fun onTransfer(transfer: TransferCard) {
        _transfers.update { list ->
            if (list.any { it.id == transfer.id }) list.map { if (it.id == transfer.id) transfer else it } else list + transfer
        }
    }

    /** Drop [id] from the UI; a newer prompt Rust already raised stays untouched. */
    fun promptAnswered(id: ULong) {
        _prompt.update { if (it?.id == id) null else it }
    }

    fun dismissed(id: ULong) {
        _transfers.update { list -> list.filterNot { it.id == id } }
    }
}

/**
 * A live file connection (SFTP, WebDAV or the local shell's home): the Rust
 * session plus what the UI shows about it. [capabilities] says which
 * operations the protocol has.
 */
class SftpConnection(
    val id: String,
    val label: String,
    val target: String,
    val hostId: String?,
    /** Vault of [hostId]; a reconnect must find the host there again. */
    val vaultId: String?,
    val quick: QuickTarget?,
    val osName: String?,
    val rust: SftpSession,
    private val bridge: SftpBridge,
    /** Scratch directory for downloads on their way to SAF / "Open with" and for uploads on their way in. */
    val cacheDir: File,
) {
    val protocol: FileProtocol = rust.protocol()
    val capabilities: FileCapabilities = rust.capabilities()

    /** The local shell's home directory rather than a host. */
    val isLocal: Boolean get() = protocol == FileProtocol.LOCAL
    val state: StateFlow<SessionState> get() = bridge.state
    val prompt: StateFlow<PendingPrompt?> get() = bridge.prompt
    val transfers: StateFlow<List<TransferCard>> get() = bridge.transfers

    suspend fun answer(prompt: PendingPrompt, answer: PromptAnswer): Boolean {
        bridge.promptAnswered(prompt.id)
        return io { answer(prompt.id, answer) }
    }

    /** Run a blocking Rust call off the main thread. */
    suspend fun <T> io(block: SftpSession.() -> T): T = withContext(Dispatchers.IO) { rust.block() }

    /** Forget a finished card; `false` if it is still queued, running or paused. */
    suspend fun dismissTransfer(id: ULong): Boolean {
        val gone = io { dismissTransfer(id) }
        if (gone) bridge.dismissed(id)
        return gone
    }

    /** A fresh scratch path for one transfer, under this connection's cache. */
    fun scratch(transferKey: String, name: String): File =
        File(File(cacheDir, transferKey), name.ifBlank { "file" })
}

/**
 * Owns every SFTP connection for one unlocked vault. Connecting returns at
 * once; state and prompts arrive on the connection's flows. The scratch files
 * a connection used are wiped when it closes.
 */
class SftpManager(
    private val context: Context,
    private val repo: VaultRepository,
    /** `HOME` of local shells, the directory [openLocal] browses. */
    private val localHome: File,
) {
    private val _connections = MutableStateFlow<List<SftpConnection>>(emptyList())
    val connections: StateFlow<List<SftpConnection>> = _connections.asStateFlow()

    fun find(id: String): SftpConnection? = _connections.value.firstOrNull { it.id == id }

    /** The one connection to the local home that is still usable, if any. */
    fun findLocal(): SftpConnection? =
        _connections.value.firstOrNull { it.isLocal && it.state.value !is SessionState.Closed && it.state.value !is SessionState.Failed }

    /**
     * Open a saved host's files. [vaultId] is the vault the caller took the
     * host from; Rust refuses a host of any other vault before reading
     * credentials. [protocol] picks the section when the host has both; `null`
     * means the primary one (WebDAV only for WebDAV-only hosts).
     */
    suspend fun openHost(hostId: String, vaultId: String, protocol: FileProtocol? = null): SftpConnection {
        val host: HostItem = repo.read { host(hostId) }
        val wanted = protocol ?: defaultProtocol(host)
        val bridge = SftpBridge()
        val rust = repo.read {
            if (wanted == FileProtocol.WEBDAV) webdavHost(hostId, vaultId, bridge) else sftpHost(hostId, vaultId, bridge)
        }
        val user = host.username.takeIf { it.isNotBlank() }?.let { "$it@" } ?: ""
        val target = if (wanted == FileProtocol.WEBDAV) host.webdavUrl ?: host.address else "$user${host.address}:${host.port}"
        return register(
            SftpConnection(
                id = rust.id(),
                label = host.label.ifBlank { host.address },
                target = target,
                hostId = hostId,
                vaultId = vaultId,
                quick = null,
                osName = host.osName,
                rust = rust,
                bridge = bridge,
                cacheDir = cacheFor(rust.id()),
            ),
        )
    }

    suspend fun openQuick(target: QuickTarget): SftpConnection {
        val bridge = SftpBridge()
        val rust = repo.read { sftpQuick(target, bridge) }
        return register(
            SftpConnection(
                id = rust.id(),
                label = target.host,
                target = listOf(target.username, "${target.host}:${target.port}").filter { it.isNotBlank() }.joinToString("@"),
                hostId = null,
                vaultId = null,
                quick = target,
                osName = null,
                rust = rust,
                bridge = bridge,
                cacheDir = cacheFor(rust.id()),
            ),
        )
    }

    /**
     * Browse the local shell's home directory (the same one local terminals
     * start in). Paths stay inside it; nothing else under the app's private
     * storage is reachable.
     */
    suspend fun openLocal(): SftpConnection {
        val bridge = SftpBridge()
        withContext(Dispatchers.IO) { localHome.mkdirs() }
        val rust = repo.read { localFiles(localHome.absolutePath, bridge) }
        return register(
            SftpConnection(
                id = rust.id(),
                label = context.getString(R.string.local_shell_files),
                target = context.getString(R.string.files_local_root_summary),
                hostId = null,
                vaultId = null,
                quick = null,
                osName = "android",
                rust = rust,
                bridge = bridge,
                cacheDir = cacheFor(rust.id()),
            ),
        )
    }

    /** Replace a closed/failed connection with a fresh one to the same target. */
    suspend fun reconnect(id: String): SftpConnection? {
        val old = find(id) ?: return null
        val fresh = when {
            old.isLocal -> openLocal()
            old.hostId != null && old.vaultId != null -> openHost(old.hostId, old.vaultId, old.protocol)
            old.quick != null -> openQuick(old.quick)
            else -> return null
        }
        _connections.update { list -> list.filterNot { it.id == fresh.id }.map { if (it.id == id) fresh else it } }
        dispose(old)
        return fresh
    }

    suspend fun close(id: String) {
        val conn = find(id) ?: return
        _connections.update { list -> list.filterNot { it.id == id } }
        dispose(conn)
    }

    suspend fun closeMany(ids: Collection<String>) {
        val wanted = ids.toSet()
        val closing = _connections.value.filter { it.id in wanted }
        if (closing.isEmpty()) return
        _connections.update { list -> list.filterNot { it.id in wanted } }
        closing.forEach { dispose(it) }
    }

    /** Every open file connection to a saved host. */
    fun forHost(hostId: String): List<SftpConnection> = _connections.value.filter { it.hostId == hostId }

    companion object {
        /** SFTP whenever the host has an SSH section; WebDAV only when that is all it has. */
        fun defaultProtocol(host: HostItem): FileProtocol =
            if (host.protocol.equals("webdav", true)) FileProtocol.WEBDAV else FileProtocol.SFTP

        /** Whether [host] can be browsed at all (SSH → SFTP, or a WebDAV section). */
        fun hasFiles(host: HostItem): Boolean = host.protocol.equals("ssh", true) || host.webdavUrl != null
    }

    suspend fun closeAll() {
        val list = _connections.value
        _connections.value = emptyList()
        list.forEach { dispose(it) }
    }

    private fun register(conn: SftpConnection): SftpConnection {
        _connections.update { it + conn }
        return conn
    }

    private suspend fun dispose(conn: SftpConnection) = withContext(Dispatchers.IO) {
        runCatching { conn.rust.disconnect() }
        runCatching { conn.cacheDir.deleteRecursively() }
    }

    private fun cacheFor(id: String): File = File(File(context.cacheDir, "sftp"), id).apply { mkdirs() }
}
