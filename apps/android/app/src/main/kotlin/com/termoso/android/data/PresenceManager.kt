package com.termoso.android.data

import com.termoso.core.TeamPresenceCard
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Who is connected to team-vault hosts right now, per team. Snapshots come over
 * REST and are reloaded when the server pushes a presence change for the team,
 * when the account changes and on a slow timer for teams a screen is watching.
 * Nothing here is published; the Rust side reports this device's own sessions.
 */
class PresenceManager(
    private val repo: VaultRepository,
    private val account: AccountManager,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val lock = Mutex()

    private val _byTeam = MutableStateFlow<Map<String, TeamPresenceCard>>(emptyMap())

    /** Latest snapshot per team id; absent until the first successful load. */
    val byTeam: StateFlow<Map<String, TeamPresenceCard>> = _byTeam.asStateFlow()

    /** Own visibility (`null` until loaded / while signed out). */
    private val _hidden = MutableStateFlow<Boolean?>(null)
    val hidden: StateFlow<Boolean?> = _hidden.asStateFlow()

    /** Team ids some screen is currently showing, with the number of watchers. */
    private val watched = MutableStateFlow<Map<String, Int>>(emptyMap())

    init {
        scope.launch { account.presenceChanges.collect { refresh(it) } }
        scope.launch {
            account.status.map { it.account?.userId }.distinctUntilChanged().collect { user ->
                if (user == null) {
                    _byTeam.value = emptyMap()
                    _hidden.value = null
                } else {
                    loadHidden()
                    watched.value.keys.forEach { refresh(it) }
                }
            }
        }
        scope.launch {
            while (true) {
                delay(POLL_MS)
                if (account.signedIn) watched.value.keys.forEach { refresh(it) }
            }
        }
    }

    /** Start following a team; pairs with [unwatch]. Loads the snapshot if missing. */
    fun watch(teamId: String) {
        watched.update { it + (teamId to (it[teamId] ?: 0) + 1) }
        if (account.signedIn && teamId !in _byTeam.value) scope.launch { refresh(teamId) }
    }

    fun unwatch(teamId: String) {
        watched.update {
            val n = (it[teamId] ?: 1) - 1
            if (n <= 0) it - teamId else it + (teamId to n)
        }
    }

    suspend fun refresh(teamId: String) {
        if (!account.signedIn) return
        val card = runCatching { repo.read { teamPresence(teamId) } }.getOrNull() ?: return
        lock.withLock { _byTeam.update { it + (teamId to card) } }
    }

    suspend fun loadHidden() {
        runCatching { repo.read { presenceHidden() } }.onSuccess { _hidden.value = it }
    }

    suspend fun setHidden(hidden: Boolean) {
        _hidden.value = repo.read { setPresenceHidden(hidden) }
    }

    fun close() {
        scope.cancel()
    }

    private companion object {
        const val POLL_MS = 60_000L
    }
}
