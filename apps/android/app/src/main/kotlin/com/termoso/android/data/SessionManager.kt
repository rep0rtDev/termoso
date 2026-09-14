package com.termoso.android.data

import com.termoso.core.HostItem
import com.termoso.core.LiveEndReason
import com.termoso.core.LiveListener
import com.termoso.core.LiveParticipantCard
import com.termoso.core.LiveShare
import com.termoso.core.PromptAnswer
import com.termoso.core.PromptRequest
import com.termoso.core.QuickTarget
import com.termoso.core.SessionListener
import com.termoso.core.SessionState
import com.termoso.core.SshSession
import com.termoso.core.TerminalOptions
import com.termoso.core.TerminalPalette
import com.termoso.core.Transport
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

    /** Drop [id] from the UI; a newer prompt Rust already raised stays untouched. */
    fun promptAnswered(id: ULong) {
        _prompt.update { if (it?.id == id) null else it }
    }
}

/** One-shot multiplayer signals (host and viewer side). */
sealed interface LiveEvent {
    /** Viewer: the host granted ([canWrite]) or took back control. */
    data class Control(val canWrite: Boolean) : LiveEvent

    /** The share is over; the viewer's terminal closes right after. */
    data class Ended(val reason: LiveEndReason, val message: String) : LiveEvent
}

/**
 * Multiplayer state of one terminal: who is connected, whether we may type
 * (always true for the host) and the host's share handle while sharing. Rust
 * calls in from tokio threads; the UI collects the flows.
 */
class LiveBridge(viewer: Boolean = false) : LiveListener {
    private val _participants = MutableStateFlow<List<LiveParticipantCard>>(emptyList())
    val participants: StateFlow<List<LiveParticipantCard>> = _participants.asStateFlow()

    private val _canWrite = MutableStateFlow(!viewer)
    val canWrite: StateFlow<Boolean> = _canWrite.asStateFlow()

    private val _share = MutableStateFlow<LiveShare?>(null)
    val share: StateFlow<LiveShare?> = _share.asStateFlow()

    private val _ended = MutableStateFlow<LiveEvent.Ended?>(null)
    val ended: StateFlow<LiveEvent.Ended?> = _ended.asStateFlow()

    private val _events = MutableSharedFlow<LiveEvent>(extraBufferCapacity = 16)
    val events: SharedFlow<LiveEvent> = _events.asSharedFlow()

    override fun onParticipants(participants: List<LiveParticipantCard>) {
        _participants.value = participants
    }

    override fun onControl(canWrite: Boolean) {
        _canWrite.value = canWrite
        _events.tryEmit(LiveEvent.Control(canWrite))
    }

    override fun onEnded(reason: LiveEndReason, message: String) {
        val ev = LiveEvent.Ended(reason, message)
        _ended.value = ev
        _share.value = null
        _participants.value = emptyList()
        _events.tryEmit(ev)
    }

    internal fun started(share: LiveShare) {
        _share.value = share
        _participants.value = share.participants()
    }

    internal fun stopped() {
        _share.value = null
        _participants.value = emptyList()
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
    /** How the shell was opened; a reconnect keeps it. */
    val transport: Transport,
    /** OS saved on the host; [detectedOs] supersedes it once the shell opens. */
    val savedOsName: String?,
    val rust: SshSession,
    private val bridge: SessionBridge,
    /** True for a terminal joined from somebody's share: read-only until granted. */
    val isView: Boolean = false,
    private val live: LiveBridge = LiveBridge(),
) {
    val state: StateFlow<SessionState> get() = bridge.state
    val frameTick: StateFlow<Long> get() = bridge.frameTick
    val title: StateFlow<String?> get() = bridge.title
    val prompt: StateFlow<PendingPrompt?> get() = bridge.prompt
    val events: SharedFlow<SessionEvent> get() = bridge.events
    val detectedOs: StateFlow<String?> get() = bridge.osName

    val participants: StateFlow<List<LiveParticipantCard>> get() = live.participants
    val canWrite: StateFlow<Boolean> get() = live.canWrite
    val share: StateFlow<LiveShare?> get() = live.share
    val liveEnded: StateFlow<LiveEvent.Ended?> get() = live.ended
    val liveEvents: SharedFlow<LiveEvent> get() = live.events

    /** Can be reconnected by us (not a view, has a target). */
    val reconnectable: Boolean get() = !isView && (hostId != null || quick != null)

    internal val liveListener: LiveBridge get() = live

    suspend fun answer(prompt: PendingPrompt, answer: PromptAnswer): Boolean {
        bridge.promptAnswered(prompt.id)
        return withContext(Dispatchers.IO) { rust.answer(prompt.id, answer) }
    }
}

/**
 * Owns every open terminal for one unlocked vault. Connecting runs the Rust
 * connect on the IO dispatcher; the returned session drives itself from tokio.
 * The foreground service mirrors the session count so Android keeps the process
 * (and the sockets) alive while the user is in another app.
 */
class SessionManager(private val repo: VaultRepository, private val keepAlive: KeepAlive) {
    private val _sessions = MutableStateFlow<List<TerminalSession>>(emptyList())
    val sessions: StateFlow<List<TerminalSession>> = _sessions.asStateFlow()

    private val _activeId = MutableStateFlow<String?>(null)
    val activeId: StateFlow<String?> = _activeId.asStateFlow()

    val active: TerminalSession? get() = _sessions.value.firstOrNull { it.id == _activeId.value }

    fun find(id: String): TerminalSession? = _sessions.value.firstOrNull { it.id == id }

    fun setActive(id: String) {
        if (_sessions.value.any { it.id == id }) _activeId.value = id
    }

    suspend fun connectHost(hostId: String, transport: Transport = Transport.AUTO): TerminalSession {
        val host: HostItem = repo.read { host(hostId) }
        val bridge = SessionBridge()
        val rust = repo.read { connectHost(hostId, options(transport), bridge) }
        val user = host.username.takeIf { it.isNotBlank() }?.let { "$it@" } ?: ""
        val mosh = transport == Transport.MOSH || (transport == Transport.AUTO && host.useMosh)
        val target = "$user${host.address}:${host.port}" + if (mosh) " · Mosh" else ""
        return register(
            TerminalSession(
                rust.id(),
                host.label.ifBlank { host.address },
                target,
                hostId,
                null,
                transport,
                host.osName,
                rust,
                bridge,
            ),
        )
    }

    suspend fun connectQuick(target: QuickTarget): TerminalSession {
        val bridge = SessionBridge()
        val rust = repo.read { connectQuick(target, options(Transport.SSH), bridge) }
        val text = "${target.username}@${target.host}:${target.port}"
        return register(TerminalSession(rust.id(), target.host, text, null, target, Transport.SSH, null, rust, bridge))
    }

    /**
     * Join a `termoso://join/…` link as a viewer. The link is parsed in Rust;
     * Kotlin never sees the secret in its fragment separately.
     */
    suspend fun joinLive(link: String): TerminalSession {
        val bridge = SessionBridge()
        val live = LiveBridge(viewer = true)
        val rust = repo.read { joinLive(link, options(Transport.SSH), bridge, live) }
        return register(
            TerminalSession(
                id = rust.id(),
                label = "Shared terminal",
                target = "Multiplayer",
                hostId = null,
                quick = null,
                transport = Transport.SSH,
                savedOsName = null,
                rust = rust,
                bridge = bridge,
                isView = true,
                live = live,
            ),
        )
    }

    /** Start sharing [id]; returns the opaque `termoso://join/…` link. */
    suspend fun share(id: String): String {
        val session = find(id) ?: throw IllegalStateException("no such session")
        val share = repo.read { shareSession(session.rust, session.label, session.liveListener) }
        session.liveListener.started(share)
        return share.link()
    }

    suspend fun stopShare(id: String) {
        val session = find(id) ?: return
        val share = session.share.value ?: return
        session.liveListener.stopped()
        withContext(Dispatchers.IO) { runCatching { share.stop() } }
    }

    suspend fun setControl(id: String, userId: String, enabled: Boolean) {
        val share = find(id)?.share?.value ?: return
        withContext(Dispatchers.IO) { share.setControl(userId, enabled) }
    }

    /** Account went away: stop every share we host and leave every view. */
    suspend fun endLive() {
        val list = _sessions.value
        list.filter { it.share.value != null }.forEach { stopShare(it.id) }
        list.filter { it.isView }.forEach { close(it.id) }
    }

    /** Replace a closed/failed session with a fresh connection to the same target. */
    suspend fun reconnect(id: String): TerminalSession? {
        val old = find(id) ?: return null
        val fresh = when {
            old.hostId != null -> connectHost(old.hostId, old.transport)
            old.quick != null -> connectQuick(old.quick)
            else -> return null
        }
        _sessions.update { list -> list.filterNot { it.id == fresh.id }.map { if (it.id == id) fresh else it } }
        withContext(Dispatchers.IO) { runCatching { old.rust.disconnect() } }
        keepAlive.terminals(_sessions.value.size)
        return fresh
    }

    /** Recolour every open terminal after the scheme changed in Settings. */
    suspend fun applyPalette(palette: TerminalPalette) = withContext(Dispatchers.IO) {
        _sessions.value.forEach { runCatching { it.rust.setPalette(palette) } }
    }

    private fun options(transport: Transport) = TerminalOptions(
        cols = 80u,
        rows = 24u,
        termType = "",
        palette = null,
        transport = transport,
    )

    private fun register(session: TerminalSession): TerminalSession {
        _sessions.update { it + session }
        _activeId.value = session.id
        keepAlive.terminals(_sessions.value.size)
        return session
    }

    /** Disconnect and drop the tab. */
    suspend fun close(id: String) {
        val session = find(id) ?: return
        _sessions.update { list -> list.filterNot { it.id == id } }
        if (_activeId.value == id) _activeId.value = _sessions.value.lastOrNull()?.id
        withContext(Dispatchers.IO) { runCatching { session.rust.disconnect() } }
        keepAlive.terminals(_sessions.value.size)
    }

    suspend fun closeAll() {
        val list = _sessions.value
        _sessions.value = emptyList()
        _activeId.value = null
        withContext(Dispatchers.IO) { list.forEach { runCatching { it.rust.disconnect() } } }
        keepAlive.terminals(0)
    }
}
