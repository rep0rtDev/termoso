package com.termoso.android.data

import com.termoso.core.AiStatusCard
import com.termoso.core.AiSuggestionCard
import com.termoso.core.AiTarget
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

/**
 * "Ask AI" state for the account: whether the server offers suggestions,
 * whether this account opted in and how much of today's quota is left.
 * Requests go through Rust, which sends the text plus an OS label and nothing
 * else; the answer is only ever shown or pasted, never typed with Enter.
 */
class AiManager(
    private val repo: VaultRepository,
    private val account: AccountManager,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /** Latest status; `null` until loaded or while signed out. */
    private val _status = MutableStateFlow<AiStatusCard?>(null)
    val status: StateFlow<AiStatusCard?> = _status.asStateFlow()

    init {
        scope.launch {
            account.status.map { it.account?.userId }.distinctUntilChanged().collect { user ->
                if (user == null) _status.value = null else refresh()
            }
        }
    }

    val signedIn: Boolean get() = account.signedIn

    suspend fun refresh(): AiStatusCard? {
        if (!account.signedIn) return null
        return runCatching { repo.read { aiStatus() } }.getOrNull()?.also { _status.value = it }
    }

    suspend fun setEnabled(enabled: Boolean): AiStatusCard {
        return repo.read { setAiEnabled(enabled) }.also { _status.value = it }
    }

    /** One suggestion; on success the quota shown in [status] follows the answer. */
    suspend fun ask(prompt: String, target: AiTarget): AiSuggestionCard {
        return repo.read { aiAsk(prompt, target) }.also { r ->
            _status.value = _status.value?.let { s ->
                s.copy(usedToday = usedAfter(s.dailyQuota, r.remainingToday))
            }
        }
    }

    fun close() {
        scope.cancel()
    }
}

/** Requests spent today once a reply said [remaining] of [dailyQuota] are left. */
fun usedAfter(dailyQuota: UInt, remaining: UInt): UInt =
    if (remaining >= dailyQuota) 0u else dailyQuota - remaining
