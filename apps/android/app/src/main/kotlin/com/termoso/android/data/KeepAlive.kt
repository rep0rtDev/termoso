package com.termoso.android.data

import android.content.Context
import com.termoso.android.R
import com.termoso.android.service.ActiveItem
import com.termoso.android.service.ActiveItems
import com.termoso.android.service.SessionControls
import com.termoso.android.service.SessionService
import com.termoso.android.str
import com.termoso.core.PfKind
import com.termoso.core.PfRuleItem
import com.termoso.core.SessionState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

/**
 * Mirrors what is connected (terminals, file connections, tunnels) into the
 * foreground service so the process survives in the background while anything
 * is live, and so the shade can list each connection with a Close button.
 * Derived from the managers' state flows, so the notifications can never lag
 * behind or run ahead of the lists the UI shows; tabs that already closed or
 * failed stay visible in the UI but are not listed.
 */
class KeepAlive(
    private val context: Context,
    private val sessions: SessionManager,
    private val files: SftpManager,
    private val forwards: ForwardManager,
    private val repo: VaultRepository,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    init {
        SessionService.controls = object : SessionControls {
            override fun closeTerminal(id: String) {
                scope.launch { runCatching { sessions.close(id) } }
            }

            override fun closeFile(id: String) {
                scope.launch { runCatching { files.close(id) } }
            }

            override fun stopTunnel(ruleId: String) {
                scope.launch { runCatching { forwards.stop(ruleId) } }
            }

            override fun closeAll() {
                scope.launch {
                    runCatching { sessions.closeAll() }
                    files.connections.value.forEach { runCatching { files.close(it.id) } }
                    forwards.tunnels.value.keys.forEach { runCatching { forwards.stop(it) } }
                }
            }
        }
        val terminals = sessions.sessions.live({ it.state }) { s ->
            ActiveItem(s.id, s.label, s.target, s.detectedOs.value ?: s.savedOsName)
        }
        // Connections the documents provider opened for another app stay out
        // of the shade: nothing to "close" from the user's point of view.
        val fileConnections = files.connections.live({ it.state }, include = { !it.background }) { c ->
            ActiveItem(c.id, str(R.string.sftp, c.label), c.target, c.osName)
        }
        val tunnels = forwards.tunnels.map { it.keys.toList() }.distinctUntilChanged().map { ids ->
            if (ids.isEmpty()) return@map emptyList()
            val rules = runCatching { repo.read { pfRules(null) } }.getOrDefault(emptyList())
            ids.map { id -> tunnelItem(id, rules.firstOrNull { it.id == id }) }
        }
        scope.launch {
            combine(terminals, fileConnections, tunnels) { t, f, p ->
                ActiveItems(terminals = t, files = f, tunnels = p)
            }.distinctUntilChanged().collect { SessionService.sync(context, it) }
        }
    }

    private fun tunnelItem(ruleId: String, rule: PfRuleItem?): ActiveItem {
        if (rule == null) return ActiveItem(ruleId, str(R.string.port_forwarding), ruleId)
        val endpoints = when (rule.kind) {
            PfKind.REMOTE -> "${rule.boundAddress.ifBlank { "localhost" }}:${rule.remotePort} → localhost:${rule.localPort}"
            PfKind.DYNAMIC -> "SOCKS ${rule.boundAddress}:${rule.localPort}"
            else -> "localhost:${rule.localPort} → ${rule.remoteHost}:${rule.remotePort}"
        }
        return ActiveItem(ruleId, str(R.string.notif_forwarding_for, rule.hostLabel), endpoints)
    }

    /** Stop mirroring and drop the notifications; the managers are expected to be empty by now. */
    fun close() {
        SessionService.controls = null
        scope.cancel()
        SessionService.sync(context, ActiveItems())
    }
}

private val SessionState.live: Boolean
    get() = this is SessionState.Connecting || this is SessionState.Connected

@OptIn(ExperimentalCoroutinesApi::class)
private fun <T> StateFlow<List<T>>.live(
    state: (T) -> StateFlow<SessionState>,
    include: (T) -> Boolean = { true },
    item: (T) -> ActiveItem,
): Flow<List<ActiveItem>> =
    flatMapLatest { all ->
        val items = all.filter(include)
        if (items.isEmpty()) {
            flowOf(emptyList())
        } else {
            combine(items.map(state)) { states -> items.filterIndexed { i, _ -> states[i].live }.map(item) }
        }
    }
