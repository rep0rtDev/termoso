package com.termoso.android.data

import com.termoso.android.str
import com.termoso.android.R
import com.termoso.core.PfTunnel
import com.termoso.core.PromptAnswer
import com.termoso.core.PromptRequest
import com.termoso.core.TunnelListener
import com.termoso.core.TunnelState
import com.termoso.core.TunnelStats
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.concurrent.ConcurrentHashMap

/** Rust tunnel callbacks republished as flows for the UI. */
class TunnelBridge : TunnelListener {
    private val _state = MutableStateFlow<TunnelState>(TunnelState.Connecting(str(R.string.connecting_ellipsis)))
    val state: StateFlow<TunnelState> = _state.asStateFlow()

    private val _prompt = MutableStateFlow<PendingPrompt?>(null)
    val prompt: StateFlow<PendingPrompt?> = _prompt.asStateFlow()

    override fun onState(state: TunnelState) {
        _state.value = state
        if (state !is TunnelState.Connecting) _prompt.value = null
    }

    override fun onPrompt(promptId: ULong, request: PromptRequest) {
        _prompt.value = PendingPrompt(promptId, request)
    }

    /** Drop [id] from the UI; a newer prompt Rust already raised stays untouched. */
    fun promptAnswered(id: ULong) {
        _prompt.update { if (it?.id == id) null else it }
    }
}

/** A running (or starting / reconnecting) forwarding rule. */
class Tunnel(
    val ruleId: String,
    val rust: PfTunnel,
    private val bridge: TunnelBridge,
) {
    val state: StateFlow<TunnelState> get() = bridge.state
    val prompt: StateFlow<PendingPrompt?> get() = bridge.prompt

    private val _stats = MutableStateFlow(TunnelStats(0u, 0u, 0u, 0u))
    /** Counters polled from Rust while the tunnel is running. */
    val stats: StateFlow<TunnelStats> = _stats.asStateFlow()

    suspend fun answer(prompt: PendingPrompt, answer: PromptAnswer): Boolean {
        bridge.promptAnswered(prompt.id)
        return withContext(Dispatchers.IO) { rust.answer(prompt.id, answer) }
    }

    internal suspend fun refreshStats() {
        _stats.value = withContext(Dispatchers.IO) { rust.stats() }
    }
}

/**
 * Owns every live tunnel for one unlocked vault, keyed by rule id. Starting
 * returns at once; connection state and prompts arrive on the tunnel's flows.
 * A tunnel that fails or is stopped by the user is dropped from [tunnels] so
 * the rule card falls back to its switch-off state; [lastError] keeps the
 * reason for the card to show.
 */
class ForwardManager(private val repo: VaultRepository, private val keepAlive: KeepAlive) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val watchers = ConcurrentHashMap<String, Job>()

    private val _tunnels = MutableStateFlow<Map<String, Tunnel>>(emptyMap())
    val tunnels: StateFlow<Map<String, Tunnel>> = _tunnels.asStateFlow()

    private val _lastError = MutableStateFlow<Map<String, String>>(emptyMap())
    /** Failure message per rule id from the most recent attempt, cleared on the next start. */
    val lastError: StateFlow<Map<String, String>> = _lastError.asStateFlow()

    private var autoStarted = false

    fun find(ruleId: String): Tunnel? = _tunnels.value[ruleId]

    /** Start every rule flagged auto-start, once per unlock. Failures surface on the rule cards. */
    suspend fun autoStartOnce() {
        if (autoStarted) return
        autoStarted = true
        val rules = runCatching { repo.read { pfRules(null) } }.getOrDefault(emptyList())
        rules.filter { it.autoStart && !it.hostMissing }.forEach { rule ->
            runCatching { start(rule.id) }
                .onFailure { e -> _lastError.update { it + (rule.id to e.userMessage()) } }
        }
    }

    suspend fun start(ruleId: String): Tunnel {
        find(ruleId)?.let { return it }
        _lastError.update { it - ruleId }
        val bridge = TunnelBridge()
        val rust = repo.read { startPf(ruleId, bridge) }
        val tunnel = Tunnel(ruleId, rust, bridge)
        _tunnels.update { it + (ruleId to tunnel) }
        syncKeepAlive()
        watchers[ruleId] = scope.launch { watch(tunnel) }
        return tunnel
    }

    suspend fun stop(ruleId: String) {
        val tunnel = _tunnels.value[ruleId] ?: return
        remove(ruleId)
        withContext(Dispatchers.IO) { runCatching { tunnel.rust.stop() } }
    }

    suspend fun closeAll() {
        val list = _tunnels.value.values.toList()
        _tunnels.value = emptyMap()
        watchers.values.forEach { it.cancel() }
        watchers.clear()
        withContext(Dispatchers.IO) { list.forEach { runCatching { it.rust.stop() } } }
        keepAlive.forwards(0)
    }

    private fun remove(ruleId: String) {
        watchers.remove(ruleId)?.cancel()
        _tunnels.update { it - ruleId }
        syncKeepAlive()
    }

    private fun syncKeepAlive() = keepAlive.forwards(_tunnels.value.size)

    /** Poll counters while running; drop the tunnel once Rust reports a terminal state. */
    private suspend fun watch(tunnel: Tunnel) {
        val poll = scope.launch {
            while (isActive) {
                if (tunnel.state.value is TunnelState.Running) tunnel.refreshStats()
                delay(1_000)
            }
        }
        try {
            tunnel.state.collect { s ->
                when (s) {
                    is TunnelState.Failed -> {
                        _lastError.update { it + (tunnel.ruleId to s.message) }
                        remove(tunnel.ruleId)
                    }
                    is TunnelState.Stopped -> remove(tunnel.ruleId)
                    is TunnelState.Connecting, is TunnelState.Reconnecting, is TunnelState.Running -> Unit
                }
            }
        } finally {
            poll.cancel()
        }
    }
}
