import Foundation
import TermosoCore

/// Where a terminal session came from — drives its title and how it can be
/// reopened after it closes.
enum SessionOrigin: Hashable {
    case host(id: String, label: String, transport: Transport)
    case local
    case quick(QuickTarget)

    var title: String {
        switch self {
        case let .host(_, label, _): label
        case .local: "Local shell"
        case let .quick(target): target.username.isEmpty ? target.host : "\(target.username)@\(target.host)"
        }
    }
}

/// One terminal session as the UI sees it. All mutation happens on the main
/// actor; the Rust `SshSession` behind `handle` is thread-safe.
@Observable
@MainActor
final class TerminalSession: Identifiable {
    struct PendingPrompt: Identifiable, Equatable {
        let id: UInt64
        let request: PromptRequest
    }

    let id = UUID().uuidString
    let origin: SessionOrigin
    let startedAt = Date()

    private(set) var handle: SshSession?
    var title: String
    var state: SessionState = .connecting(detail: "Starting…", stage: .connecting, hop: nil)
    /// Bumped by the core whenever the grid changed; the terminal view
    /// re-reads `frame()` on the next display cycle.
    var frameTick = 0
    var prompt: PendingPrompt?
    var osName: String?
    var bellTick = 0
    var lastClipboard: String?

    init(origin: SessionOrigin) {
        self.origin = origin
        self.title = origin.title
    }

    fileprivate func attach(_ handle: SshSession) {
        self.handle = handle
    }

    fileprivate func fail(_ message: String) {
        state = .failed(kind: "connect", message: message)
    }

    var isLive: Bool {
        switch state {
        case .connecting, .connected: true
        case .closed, .failed: false
        }
    }

    var stateLabel: String {
        switch state {
        case let .connecting(detail, _, _): detail.isEmpty ? "Connecting…" : detail
        case .connected: "Connected"
        case let .closed(reason): reason.map { "Closed · \($0)" } ?? "Closed"
        case let .failed(_, message): message
        }
    }

    // MARK: Input

    func send(text: String, mods: KeyMods = KeyMods(ctrl: false, alt: false, shift: false)) {
        handle?.sendText(text: text, mods: mods)
    }

    func send(key: SpecialKey, mods: KeyMods = KeyMods(ctrl: false, alt: false, shift: false)) {
        handle?.sendKey(key: key, mods: mods)
    }

    func paste(_ text: String) {
        handle?.paste(text: text)
        handle?.scrollToBottom()
    }

    func run(command: String) {
        handle?.runCommand(command: command)
        handle?.scrollToBottom()
    }

    func resize(cols: UInt16, rows: UInt16) {
        guard cols > 1, rows > 1 else { return }
        handle?.resize(cols: cols, rows: rows)
    }

    /// Positive = towards history. On the alternate screen (less, vim…) the
    /// gesture becomes arrow keys, like a mouse wheel would.
    func scroll(lines: Int32, altScreen: Bool) {
        guard let handle, lines != 0 else { return }
        if altScreen {
            let key: SpecialKey = lines > 0 ? .up : .down
            for _ in 0 ..< min(abs(Int(lines)), 200) {
                handle.sendKey(key: key, mods: KeyMods(ctrl: false, alt: false, shift: false))
            }
        } else {
            handle.scroll(lines: lines)
        }
    }

    func scrollToBottom() {
        handle?.scrollToBottom()
    }

    func answer(_ prompt: PendingPrompt, _ answer: PromptAnswer) {
        if self.prompt?.id == prompt.id { self.prompt = nil }
        _ = handle?.answer(promptId: prompt.id, answer: answer)
    }

    func disconnect() {
        handle?.disconnect()
    }

    var frame: GridFrame? { handle?.frame() }
    var visibleText: [String] { handle?.visibleText() ?? [] }
    var typedLine: String? { handle?.typedLine() }
    var suggestions: [SuggestionItem] { handle?.suggestions() ?? [] }
}

/// `SessionListener` for one session. The core calls these from its worker
/// threads; everything is forwarded to the main actor. Renders are coalesced:
/// at most one main-queue hop is in flight per burst of output.
final class SessionBridge: SessionListener, @unchecked Sendable {
    private weak var session: TerminalSession?
    private let lock = NSLock()
    private var renderQueued = false

    @MainActor
    init(session: TerminalSession) {
        self.session = session
    }

    private func onMain(_ body: @escaping @MainActor (TerminalSession) -> Void) {
        DispatchQueue.main.async { [weak session] in
            MainActor.assumeIsolated {
                guard let session else { return }
                body(session)
            }
        }
    }

    func onState(state: SessionState) {
        onMain { $0.state = state }
    }

    func onRender() {
        lock.lock()
        let queued = renderQueued
        renderQueued = true
        lock.unlock()
        if queued { return }
        DispatchQueue.main.async { [weak self, weak session] in
            guard let self else { return }
            self.lock.lock()
            self.renderQueued = false
            self.lock.unlock()
            MainActor.assumeIsolated {
                session?.frameTick &+= 1
            }
        }
    }

    func onPrompt(promptId: UInt64, request: PromptRequest) {
        onMain { $0.prompt = TerminalSession.PendingPrompt(id: promptId, request: request) }
    }

    func onTitle(title: String?) {
        onMain { session in
            let trimmed = title?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            session.title = trimmed.isEmpty ? session.origin.title : trimmed
        }
    }

    func onBell() {
        onMain { $0.bellTick &+= 1 }
    }

    func onClipboard(text: String) {
        onMain { $0.lastClipboard = text }
    }

    func onOsDetected(osName: String) {
        onMain { $0.osName = osName }
    }
}

/// All live terminal sessions of the opened profile. Sessions stay listed
/// after they close so the Connections tab can show why and offer reopen;
/// `remove` drops them.
@Observable
@MainActor
final class SessionStore {
    let repository: VaultRepository
    private(set) var sessions: [TerminalSession] = []
    var activeId: String?
    /// Whether the full-screen terminal is showing.
    var terminalPresented = false
    private var bridges: [String: SessionBridge] = [:]

    init(repository: VaultRepository) {
        self.repository = repository
    }

    var active: TerminalSession? {
        sessions.first { $0.id == activeId } ?? sessions.last
    }

    var liveCount: Int { sessions.filter { $0.isLive }.count }

    func session(id: String) -> TerminalSession? {
        sessions.first { $0.id == id }
    }

    // MARK: Connect

    @discardableResult
    func connect(host: HostItem, transport: Transport = .auto) -> TerminalSession {
        start(origin: .host(id: host.id, label: host.label.isEmpty ? host.address : host.label, transport: transport)) { listener in
            try repository.app.connectHost(hostId: host.id, options: Self.options(transport: transport), listener: listener)
        }
    }

    @discardableResult
    func connectLocal() -> TerminalSession {
        start(origin: .local) { listener in
            let home = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?.path ?? ""
            return try repository.app.connectLocal(
                shell: LocalShell(argv: [], home: home, env: ["TERM_PROGRAM=Termoso"]),
                options: Self.options(transport: .auto),
                listener: listener
            )
        }
    }

    @discardableResult
    func connectQuick(_ target: QuickTarget) -> TerminalSession {
        start(origin: .quick(target)) { listener in
            try repository.app.connectQuick(target: target, options: Self.options(transport: .auto), listener: listener)
        }
    }

    /// Start the same target again (closed sessions keep their tab until
    /// removed, like desktop Termius).
    @discardableResult
    func reopen(_ session: TerminalSession) -> TerminalSession {
        switch session.origin {
        case let .host(id, _, transport):
            start(origin: session.origin) { listener in
                try repository.app.connectHost(hostId: id, options: Self.options(transport: transport), listener: listener)
            }
        case .local:
            connectLocal()
        case let .quick(target):
            connectQuick(target)
        }
    }

    private func start(origin: SessionOrigin, _ launch: (SessionListener) throws -> SshSession) -> TerminalSession {
        let session = TerminalSession(origin: origin)
        let bridge = SessionBridge(session: session)
        bridges[session.id] = bridge
        sessions.append(session)
        activeId = session.id
        terminalPresented = true
        do {
            session.attach(try launch(bridge))
        } catch {
            session.fail(userMessage(for: error))
        }
        return session
    }

    private static func options(transport: Transport) -> TerminalOptions {
        TerminalOptions(cols: 80, rows: 24, termType: "", palette: nil, transport: transport)
    }

    // MARK: Lifecycle

    func disconnect(_ session: TerminalSession) {
        session.disconnect()
    }

    /// Disconnect (if live) and forget the session.
    func remove(_ session: TerminalSession) {
        session.disconnect()
        sessions.removeAll { $0.id == session.id }
        bridges[session.id] = nil
        if activeId == session.id {
            activeId = sessions.last?.id
        }
        if sessions.isEmpty {
            terminalPresented = false
        }
    }

    func show(_ session: TerminalSession) {
        activeId = session.id
        terminalPresented = true
    }

    /// Drop every closed/failed session from the list.
    func clearFinished() {
        for session in sessions where !session.isLive {
            bridges[session.id] = nil
        }
        sessions.removeAll { !$0.isLive }
        if let activeId, !sessions.contains(where: { $0.id == activeId }) {
            self.activeId = sessions.last?.id
        }
    }
}
