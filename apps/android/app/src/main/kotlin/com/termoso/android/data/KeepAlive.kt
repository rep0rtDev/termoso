package com.termoso.android.data

import android.content.Context
import com.termoso.android.service.ActiveCounts
import com.termoso.android.service.SessionService
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
 * is live. Derived from the managers' state flows, so the notification can
 * never lag behind or run ahead of the lists the UI shows; tabs that already
 * closed or failed stay visible in the UI but are not counted.
 */
class KeepAlive(private val context: Context, sessions: SessionManager, files: SftpManager, forwards: ForwardManager) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    init {
        val terminals = sessions.sessions.liveCount { it.state }
        val fileConnections = files.connections.liveCount { it.state }
        val tunnels = forwards.tunnels.map { it.size }
        scope.launch {
            combine(terminals, fileConnections, tunnels) { t, f, p ->
                ActiveCounts(terminals = t, files = f, tunnels = p)
            }.distinctUntilChanged().collect { SessionService.sync(context, it) }
        }
    }

    /** Stop mirroring and drop the notification; the managers are expected to be empty by now. */
    fun close() {
        scope.cancel()
        SessionService.sync(context, ActiveCounts())
    }
}

private val SessionState.live: Boolean
    get() = this is SessionState.Connecting || this is SessionState.Connected

@OptIn(ExperimentalCoroutinesApi::class)
private fun <T> StateFlow<List<T>>.liveCount(state: (T) -> StateFlow<SessionState>): Flow<Int> =
    flatMapLatest { items ->
        if (items.isEmpty()) {
            flowOf(0)
        } else {
            combine(items.map(state)) { states -> states.count { it.live } }
        }
    }
