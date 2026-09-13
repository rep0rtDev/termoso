package com.termoso.android.data

import android.content.Context
import com.termoso.android.service.SessionService
import com.termoso.core.HostItem
import com.termoso.core.PromptAnswer
import com.termoso.core.PromptRequest
import com.termoso.core.QuickTarget
import com.termoso.core.SessionListener
import com.termoso.core.SessionState
import com.termoso.core.SshSession
import com.termoso.core.TerminalOptions
import com.termoso.core.TerminalPalette
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext

/** A prompt the user has to answer before the connection can continue. */
data class PendingPrompt(val id: ULong, val request: PromptRequest)

/** One-shot signals from the remote that the UI reacts to once. */
sealed interface SessionEvent {
    data object Bell : SessionEvent

    /** OSC 52: the remote asked to place [text] on the clipboard. */
    data class Clipboard(val text: String) : SessionEvent
}

/**
 * Receives Rust callbacks (from tokio threads) and republishes them as flows the
 * UI collects on the main thread. Renders are conflated: the UI pulls one fresh
 * frame per tick it observes, no matter how many arrived in between.
 */
class SessionBridge : SessionListener {
    private val _state = MutableStateFlow<SessionState>(SessionState.Connecting("Connecting…"))
    val state: StateFlow<SessionState> = _state.asStateFlow()

    private val _frameTick = MutableStateFlow(0L)
    val frameTick: StateFlow<Long> = _frameTick.asStateFlow()

    private val _title = MutableStateFlow<String?>(null)
    val title: StateFlow<String?> = _title.asStateFlow()

    private val _prompt = MutableStateFlow<PendingPrompt?>(null)
    val prompt: StateFlow<PendingPrompt?> = _prompt.asStateFlow()

    private val _osName = MutableStateFlow<String?>(null)
    val osName: StateFlow<String?> = _osName.asStateFlow()

    private val _events = MutableSharedFlow<SessionEvent>(extraBufferCapacity = 16)
    val events: SharedFlow<SessionEvent> = _events.asSharedFlow()

    override fun onState(state: SessionState) {
        _state.value = state
        if (state !is SessionState.Connecting) _prompt.value = null
    }

    override fun onRender() {
        _frameTick.update { it + 1 }
    }

    override fun onPrompt(promptId: ULong, request: PromptRequest) {
        _prompt.value = PendingPrompt(promptId, request)
    }

    override fun onTitle(title: String?) {
        _title.value = title?.takeIf { it.isNotBlank() }
    }

    override fun onBell() {
        _events.tryEmit(SessionEvent.Bell)
    }

    override fun onClipboard(text: String) {
        _events.tryEmit(SessionEvent.Clipboard(text))
    }

    override fun onOsDetected(osName: String) {
        _osName.value = osName
    }

    fun promptAnswered() {
        _prompt.value = null
    }
}

/** A live terminal: the Rust session plus everything the UI needs to show it. */
class TerminalSession(
    val id: String,
    val label: String,
    val target: String,
    val hostId: String?,
    /** Set for quick-connect sessions (no saved host). */
    val quick: QuickTarget?,
    /** OS saved on the host; [detectedOs] supersedes it once the shell opens. */
    val savedOsName: String?,
    val palette: TerminalPalette?,
    val rust: SshSession,
    private val bridge: SessionBridge,
) {
    val state: StateFlow<SessionState> get() = bridge.state
    val frameTick: StateFlow<Long> get() = bridge.frameTick
    val title: StateFlow<String?> get() = bridge.title
    val prompt: StateFlow<PendingPrompt?> get() = bridge.prompt
    val events: SharedFlow<SessionEvent> get() = bridge.events
    val detectedOs: StateFlow<String?> get() = bridge.osName

    suspend fun answer(prompt: PendingPrompt, answer: PromptAnswer): Boolean =
        withContext(Dispatchers.IO) { rust.answer(prompt.id, answer) }.also { bridge.promptAnswered() }
}

/**
 * Owns every open terminal for one unlocked vault. Connecting runs the Rust
 * connect on the IO dispatcher; the returned session drives itself from tokio.
 * The foreground service mirrors the session count so Android keeps the process
 * (and the sockets) alive while the user is in another app.
 */
class SessionManager(private val context: Context, private val repo: VaultRepository) {
    private val _sessions = MutableStateFlow<List<TerminalSession>>(emptyList())
    val sessions: StateFlow<List<TerminalSession>> = _sessions.asStateFlow()

    private val _activeId = MutableStateFlow<String?>(null)
    val activeId: StateFlow<String?> = _activeId.asStateFlow()

    val active: TerminalSession? get() = _sessions.value.firstOrNull { it.id == _activeId.value }

    fun find(id: String): TerminalSession? = _sessions.value.firstOrNull { it.id == id }

    fun setActive(id: String) {
        if (_sessions.value.any { it.id == id }) _activeId.value = id
    }

    suspend fun connectHost(hostId: String, palette: TerminalPalette?): TerminalSession {
        val host: HostItem = repo.read { host(hostId) }
        val bridge = SessionBridge()
        val rust = repo.read { connectHost(hostId, options(palette), bridge) }
        val user = host.username.takeIf { it.isNotBlank() }?.let { "$it@" } ?: ""
        val target = "$user${host.address}:${host.port}"
        return register(
            TerminalSession(
                rust.id(),
                host.label.ifBlank { host.address },
                target,
                hostId,
                null,
                host.osName,
                palette,
                rust,
                bridge,
            ),
        )
    }

    suspend fun connectQuick(target: QuickTarget, palette: TerminalPalette?): TerminalSession {
        val bridge = SessionBridge()
        val rust = repo.read { connectQuick(target, options(palette), bridge) }
        val text = "${target.username}@${target.host}:${target.port}"
        return register(TerminalSession(rust.id(), target.host, text, null, target, null, palette, rust, bridge))
    }

    /** Replace a closed/failed session with a fresh connection to the same target. */
    suspend fun reconnect(id: String): TerminalSession? {
        val old = find(id) ?: return null
        val fresh = when {
            old.hostId != null -> connectHost(old.hostId, old.palette)
            old.quick != null -> connectQuick(old.quick, old.palette)
            else -> return null
        }
        _sessions.update { list -> list.filterNot { it.id == fresh.id }.map { if (it.id == id) fresh else it } }
        withContext(Dispatchers.IO) { runCatching { old.rust.disconnect() } }
        SessionService.sync(context, _sessions.value.size)
        return fresh
    }

    private fun options(palette: TerminalPalette?) = TerminalOptions(
        cols = 80u,
        rows = 24u,
        termType = "",
        palette = palette,
    )

    private fun register(session: TerminalSession): TerminalSession {
        _sessions.update { it + session }
        _activeId.value = session.id
        SessionService.sync(context, _sessions.value.size)
        return session
    }

    /** Disconnect and drop the tab. */
    suspend fun close(id: String) {
        val session = find(id) ?: return
        _sessions.update { list -> list.filterNot { it.id == id } }
        if (_activeId.value == id) _activeId.value = _sessions.value.lastOrNull()?.id
        withContext(Dispatchers.IO) { runCatching { session.rust.disconnect() } }
        SessionService.sync(context, _sessions.value.size)
    }

    suspend fun closeAll() {
        val list = _sessions.value
        _sessions.value = emptyList()
        _activeId.value = null
        withContext(Dispatchers.IO) { list.forEach { runCatching { it.rust.disconnect() } } }
        SessionService.sync(context, 0)
    }
}
