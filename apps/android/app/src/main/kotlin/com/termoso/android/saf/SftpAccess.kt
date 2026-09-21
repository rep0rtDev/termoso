package com.termoso.android.saf

import com.termoso.android.R
import com.termoso.android.data.AppContainer
import com.termoso.android.data.PendingPrompt
import com.termoso.android.data.SftpConnection
import com.termoso.android.data.SftpManager
import com.termoso.android.data.VaultState
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.core.FileProtocol
import com.termoso.core.HostItem
import com.termoso.core.MobileException
import com.termoso.core.PromptAnswer
import com.termoso.core.PromptRequest
import com.termoso.core.SessionState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull
import java.io.FileNotFoundException

/** A failure the Files UI can show as-is: the message is already for the user. */
class ProviderException(message: String, val locked: Boolean = false) : FileNotFoundException(message)

/**
 * Where the documents provider gets its vault and file connections (SFTP or
 * WebDAV). Nothing here can show UI, so the rules are strict: the vault opens
 * only when that needs no one present (see [AppContainer.unlockSilently]), an
 * in-use app lock counts as locked, and any connection that stops to ask
 * something — password, passphrase, an unknown host key or certificate — is
 * cancelled with a message telling the user to connect from Termoso once
 * instead.
 */
class SftpAccess(private val container: AppContainer) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val perHost = mutableMapOf<String, Mutex>()

    /** Connections this class opened itself, by connection id, with their last use. */
    private val owned = mutableMapOf<String, Long>()
    private var reaper: Job? = null

    /** The open vault; [ProviderException] with `locked = true` when it cannot be opened silently. */
    fun vault(): VaultState.Open = runBlocking {
        if (container.gated.value || container.backgroundLockDue()) throw locked()
        try {
            container.unlockSilently() ?: throw locked()
        } catch (e: ProviderException) {
            throw e
        } catch (e: Exception) {
            throw ProviderException(e.userMessage())
        }
        container.vault.value as? VaultState.Open ?: throw locked()
    }

    /** True when roots can be served without anyone unlocking anything. */
    fun available(): Boolean =
        container.hasProfile() && !container.gated.value && !container.backgroundLockDue() &&
            (container.vault.value is VaultState.Open || !container.masterKeys.authRequired())

    /** Saved hosts with something to browse (an SSH or a WebDAV section), every vault. */
    fun hosts(vault: VaultState.Open): List<HostItem> = runBlocking {
        runCatching { vault.repo.read { hosts(null) } }
            .getOrElse { throw ProviderException(it.userMessage()) }
            .filter { SftpManager.hasFiles(it) }
    }

    /** The host behind a share; not found when it no longer has that section. */
    fun host(vault: VaultState.Open, id: String, protocol: FileProtocol): HostItem = runBlocking {
        try {
            vault.repo.read { host(id) }.also { if (!offers(it, protocol)) throw notFound() }
        } catch (e: MobileException.NotFound) {
            throw notFound()
        } catch (e: MobileException) {
            throw ProviderException(e.userMessage())
        }
    }

    /**
     * A connected session to the share behind [id]: one the app already has
     * open, or a fresh one that reached `Connected` without a prompt. Blocks
     * the calling binder thread for at most [CONNECT_TIMEOUT_MS].
     */
    fun connection(id: DocumentId): SftpConnection = connection(id.hostId, id.protocol)

    fun connection(hostId: String, protocol: FileProtocol): SftpConnection {
        val vault = vault()
        return runBlocking {
            val mutex = synchronized(perHost) { perHost.getOrPut(DocumentId.rootId(hostId, protocol)) { Mutex() } }
            mutex.withLock {
                vault.sftp.forHost(hostId)
                    .firstOrNull { it.protocol == protocol && it.state.value is SessionState.Connected }
                    ?.also { touch(it) }
                    ?: open(vault, hostId, protocol)
            }
        }
    }

    private suspend fun open(vault: VaultState.Open, hostId: String, protocol: FileProtocol): SftpConnection {
        val conn = try {
            if (!offers(vault.repo.read { host(hostId) }, protocol)) throw notFound()
            vault.sftp.openHost(hostId, protocol)
        } catch (e: MobileException.NotFound) {
            throw notFound()
        } catch (e: MobileException) {
            throw ProviderException(e.userMessage())
        }
        val outcome = withTimeoutOrNull(CONNECT_TIMEOUT_MS) {
            merge(
                conn.state.filter { it !is SessionState.Connecting }.map { Outcome.Settled(it) },
                conn.prompt.filterNotNull().map { Outcome.Asked(it.request, it) },
            ).first()
        }
        when (outcome) {
            is Outcome.Settled -> when (val s = outcome.state) {
                is SessionState.Connected -> {
                    synchronized(owned) { owned[conn.id] = System.currentTimeMillis() }
                    scheduleReaper(vault)
                    return conn
                }
                is SessionState.Failed -> fail(vault, conn, s.message)
                is SessionState.Closed -> fail(vault, conn, s.reason ?: str(R.string.connection_closed))
                is SessionState.Connecting -> fail(vault, conn, str(R.string.files_connection_timed_out))
            }
            is Outcome.Asked -> {
                runCatching { conn.answer(outcome.pending, PromptAnswer.Cancel) }
                fail(vault, conn, promptMessage(outcome.request, conn.label))
            }
            null -> fail(vault, conn, str(R.string.files_connection_timed_out))
        }
    }

    private suspend fun fail(vault: VaultState.Open, conn: SftpConnection, message: String): Nothing {
        vault.sftp.close(conn.id)
        throw ProviderException(message)
    }

    private fun touch(conn: SftpConnection) {
        synchronized(owned) { if (conn.id in owned) owned[conn.id] = System.currentTimeMillis() }
    }

    /** Provider-opened connections nobody used for [IDLE_TIMEOUT_MS] are closed, so the foreground service can stop. */
    private fun scheduleReaper(vault: VaultState.Open) {
        synchronized(owned) {
            if (reaper?.isActive == true) return
            reaper = scope.launch {
                while (true) {
                    delay(IDLE_TIMEOUT_MS / 2)
                    val now = System.currentTimeMillis()
                    val stale = synchronized(owned) {
                        val ids = owned.filterValues { now - it >= IDLE_TIMEOUT_MS }.keys.toList()
                        ids.forEach { owned.remove(it) }
                        ids
                    }
                    val live = vault.sftp.connections.value.filter { it.id in stale && it.state.value is SessionState.Connected }
                    if (live.isNotEmpty()) vault.sftp.closeMany(live.map { it.id })
                    if (synchronized(owned) { owned.isEmpty() }) return@launch
                }
            }
        }
    }

    private sealed interface Outcome {
        data class Settled(val state: SessionState) : Outcome
        data class Asked(val request: PromptRequest, val pending: PendingPrompt) : Outcome
    }

    private fun locked() = ProviderException(str(R.string.files_unlock_termoso_first), locked = true)

    private fun notFound() = ProviderException(str(R.string.files_host_no_longer_exists))

    private fun promptMessage(request: PromptRequest, label: String): String = when (request) {
        is PromptRequest.HostKeyUnknown, is PromptRequest.HostKeyChanged -> str(R.string.files_host_key_needs_confirmation, label)
        is PromptRequest.Certificate -> str(R.string.files_host_certificate_needs_confirmation, label)
        is PromptRequest.Password, is PromptRequest.Passphrase, is PromptRequest.KeyboardInteractive ->
            str(R.string.files_host_needs_credentials, label)
        is PromptRequest.SecurityKeyPin, is PromptRequest.SecurityKeyInsert -> str(R.string.files_host_needs_security_key, label)
    }

    private fun offers(host: HostItem, protocol: FileProtocol): Boolean = when (protocol) {
        FileProtocol.SFTP -> host.protocol == "ssh"
        FileProtocol.WEBDAV -> host.webdavUrl != null
    }

    private companion object {
        const val CONNECT_TIMEOUT_MS = 45_000L
        const val IDLE_TIMEOUT_MS = 5 * 60_000L
    }
}
