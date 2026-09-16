import SwiftUI
import TermosoCore
import UIKit

/// Full-screen terminal: session tabs on top, the grid, autocomplete chips and
/// the extra-keys bar. One screen hosts every open session; switching tabs
/// swaps the surface without touching the Rust sessions.
struct TerminalScreen: View {
    @Bindable var store: SessionStore
    let settings: SettingsModel
    let uiTestMode: Bool

    @Environment(\.dismiss) private var dismiss
    @State private var mods = StickyMods()
    @State private var panelExpanded = false
    @State private var keyboardShown = false
    @State private var focusRequest = 1
    @State private var fontScale: CGFloat = 1
    @State private var suggestions: [SuggestionItem] = []
    @State private var suggestionGeneration = 0
    @State private var showMore = false
    @State private var showLongPress = false
    @State private var newSession = false

    var body: some View {
        VStack(spacing: 0) {
            tabBar
            ZStack {
                if let session = store.active {
                    surface(session)
                        .id(session.id)
                    stateOverlay(session)
                } else {
                    ContentUnavailableView("No sessions", systemImage: "terminal")
                        .foregroundStyle(Theme.terminalText)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            .padding(.horizontal, 6)
            .padding(.bottom, 6)
            if let session = store.active, session.isLive {
                if !suggestions.isEmpty, settings.settings?.autocomplete ?? true {
                    SuggestionStrip(suggestions: suggestions) { item in
                        session.send(text: item.insert)
                        suggestions = []
                    }
                }
                KeyPanel(
                    mods: $mods,
                    expanded: $panelExpanded,
                    keyboardShown: keyboardShown,
                    onKey: { key in press(key, in: session) },
                    onToggleKeyboard: { focusRequest = keyboardShown ? -(abs(focusRequest) + 1) : abs(focusRequest) + 1 },
                    onPaste: { paste(into: session) },
                    onMore: { showMore = true }
                )
            }
        }
        .background(Theme.terminalChrome.ignoresSafeArea())
        .preferredColorScheme(.dark)
        .sheet(item: promptBinding) { prompt in
            if let session = store.active {
                PromptSheet(session: session, prompt: prompt)
            }
        }
        .sheet(isPresented: $newSession) {
            NewConnectionSheet(store: store)
        }
        .confirmationDialog("Session", isPresented: $showMore, titleVisibility: .hidden) {
            moreActions
        }
        .confirmationDialog("Terminal", isPresented: $showLongPress, titleVisibility: .hidden) {
            if let session = store.active {
                Button("Paste") { paste(into: session) }
                Button("Copy screen text") { UIPasteboard.general.string = session.visibleText.joined(separator: "\n") }
                Button("Clear screen (Ctrl+L)") { session.send(text: "l", mods: KeyMods(ctrl: true, alt: false, shift: false)) }
                Button("Jump to bottom") { session.scrollToBottom() }
            }
        }
        .onChange(of: store.active?.frameTick ?? 0) { _, _ in refreshSuggestions() }
        .onChange(of: store.active?.id) { _, _ in
            suggestions = []
            mods.clear()
        }
        .onChange(of: store.active?.bellTick ?? 0) { _, _ in
            if settings.settings?.hapticFeedback ?? true {
                UIImpactFeedbackGenerator(style: .light).impactOccurred()
            }
        }
        .onChange(of: store.active?.lastClipboard) { _, text in
            if let text { UIPasteboard.general.string = text }
        }
        .onAppear {
            UIApplication.shared.isIdleTimerDisabled = settings.settings?.keepScreenOn ?? true
        }
        .onDisappear {
            UIApplication.shared.isIdleTimerDisabled = false
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("terminal.screen")
    }

    // MARK: Tabs

    private var tabBar: some View {
        HStack(spacing: 8) {
            Button {
                dismiss()
            } label: {
                Image(systemName: "chevron.down")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(Theme.terminalText)
                    .frame(width: 32, height: 32)
            }
            .accessibilityIdentifier("terminal.close")

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(store.sessions) { session in
                        tab(session)
                    }
                }
                .padding(.vertical, 2)
            }

            Button {
                newSession = true
            } label: {
                Image(systemName: "plus")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(Theme.terminalText)
                    .frame(width: 32, height: 32)
            }
            .accessibilityIdentifier("terminal.newTab")
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
    }

    private func tab(_ session: TerminalSession) -> some View {
        let active = session.id == store.active?.id
        return Button {
            store.activeId = session.id
        } label: {
            HStack(spacing: 6) {
                Circle()
                    .fill(dotColor(session))
                    .frame(width: 7, height: 7)
                Text(session.title)
                    .font(.system(size: 13, weight: active ? .semibold : .regular))
                    .foregroundStyle(active ? Color.white : Theme.terminalMuted)
                    .lineLimit(1)
                if active {
                    Button {
                        store.remove(session)
                        if store.sessions.isEmpty { dismiss() }
                    } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 10, weight: .bold))
                            .foregroundStyle(Theme.terminalMuted)
                            .frame(width: 18, height: 18)
                    }
                    .accessibilityIdentifier("terminal.tab.close")
                }
            }
            .padding(.leading, 10)
            .padding(.trailing, active ? 4 : 10)
            .frame(height: 30)
            .background(active ? Theme.terminalKey : Color.clear, in: Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("terminal.tab.\(session.title)")
    }

    private func dotColor(_ session: TerminalSession) -> Color {
        switch session.state {
        case .connecting: .yellow
        case .connected: Theme.terminalKeyActive
        case .closed: Theme.terminalMuted
        case .failed: .red
        }
    }

    // MARK: Surface

    private func surface(_ session: TerminalSession) -> some View {
        let base = CGFloat(settings.settings?.terminalFontSize ?? 13)
        return TerminalSurface(
            session: session,
            tick: session.frameTick,
            fontSize: max(8, min(32, base * fontScale)),
            cursorBlink: settings.settings?.cursorBlink ?? true,
            mods: mods.keyMods,
            focusRequest: focusRequest,
            mirrorTextForTests: uiTestMode,
            onKeySent: { if mods.any { mods.clear() } },
            onLongPress: { showLongPress = true },
            onPinch: { scale in
                guard settings.settings?.pinchZoom ?? true else { return }
                fontScale = max(0.6, min(2.4, fontScale * scale))
            },
            onFocusChange: { keyboardShown = $0 }
        )
    }

    @ViewBuilder
    private func stateOverlay(_ session: TerminalSession) -> some View {
        switch session.state {
        case let .connecting(detail):
            VStack(spacing: 14) {
                ProgressView()
                    .tint(Theme.terminalKeyActive)
                    .controlSize(.large)
                Text(session.origin.title)
                    .font(.headline)
                    .foregroundStyle(.white)
                Text(detail.isEmpty ? "Connecting…" : detail)
                    .font(.footnote)
                    .foregroundStyle(Theme.terminalMuted)
                    .accessibilityIdentifier("terminal.state.connecting")
                Button("Cancel") {
                    store.remove(session)
                    if store.sessions.isEmpty { dismiss() }
                }
                .buttonStyle(.bordered)
                .tint(Theme.terminalMuted)
            }
            .padding(24)
            .background(Theme.terminalPanel.opacity(0.92), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        case .connected:
            EmptyView()
        case let .closed(reason):
            finishedOverlay(session, title: "Session closed", detail: reason ?? "The connection was closed.", identifier: "terminal.state.closed")
        case let .failed(_, message):
            finishedOverlay(session, title: "Connection failed", detail: message, identifier: "terminal.state.failed")
        }
    }

    private func finishedOverlay(_ session: TerminalSession, title: String, detail: String, identifier: String) -> some View {
        VStack(spacing: 12) {
            Image(systemName: title == "Session closed" ? "bolt.slash" : "exclamationmark.triangle")
                .font(.system(size: 28))
                .foregroundStyle(Theme.terminalMuted)
            Text(title)
                .font(.headline)
                .foregroundStyle(.white)
            Text(detail)
                .font(.footnote)
                .foregroundStyle(Theme.terminalMuted)
                .multilineTextAlignment(.center)
                .accessibilityIdentifier(identifier)
            HStack(spacing: 10) {
                Button("Reconnect") {
                    store.remove(session)
                    store.reopen(session)
                }
                .buttonStyle(.borderedProminent)
                .tint(Theme.terminalKeyActive)
                .accessibilityIdentifier("terminal.reconnect")
                Button("Close") {
                    store.remove(session)
                    if store.sessions.isEmpty { dismiss() }
                }
                .buttonStyle(.bordered)
                .tint(Theme.terminalMuted)
                .accessibilityIdentifier("terminal.closeSession")
            }
        }
        .padding(24)
        .frame(maxWidth: 320)
        .background(Theme.terminalPanel.opacity(0.94), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
    }

    // MARK: Actions

    @ViewBuilder
    private var moreActions: some View {
        if let session = store.active {
            if session.isLive {
                Button("Disconnect", role: .destructive) { store.disconnect(session) }
                    .accessibilityIdentifier("terminal.disconnect")
            } else {
                Button("Reconnect") {
                    store.remove(session)
                    store.reopen(session)
                }
            }
            Button("Close tab") {
                store.remove(session)
                if store.sessions.isEmpty { dismiss() }
            }
            Button("Copy screen text") { UIPasteboard.general.string = session.visibleText.joined(separator: "\n") }
            Button("Reset text size") { fontScale = 1 }
        }
    }

    private func press(_ key: PanelKey, in session: TerminalSession) {
        if key.isModifier {
            mods.toggle(key)
            return
        }
        let current = mods.keyMods
        switch key {
        case let .special(special, _):
            session.send(key: special, mods: current)
        case let .text(text, _):
            session.send(text: text, mods: current)
        default:
            break
        }
        session.scrollToBottom()
        mods.clear()
        if settings.settings?.hapticFeedback ?? true {
            UIImpactFeedbackGenerator(style: .rigid).impactOccurred(intensity: 0.6)
        }
    }

    private func paste(into session: TerminalSession) {
        guard let text = UIPasteboard.general.string, !text.isEmpty else { return }
        session.paste(text)
    }

    private var promptBinding: Binding<TerminalSession.PendingPrompt?> {
        Binding(
            get: { store.active?.prompt },
            set: { newValue in if newValue == nil { store.active?.prompt = nil } }
        )
    }

    /// Suggestions come from the core and may consult the remote shell, so
    /// they are fetched off the main actor; stale results are dropped.
    private func refreshSuggestions() {
        guard settings.settings?.autocomplete ?? true, let session = store.active, session.isLive,
              let handle = session.handle else {
            if !suggestions.isEmpty { suggestions = [] }
            return
        }
        guard let typed = session.typedLine, !typed.isEmpty else {
            if !suggestions.isEmpty { suggestions = [] }
            return
        }
        suggestionGeneration &+= 1
        let generation = suggestionGeneration
        Task { @MainActor in
            let items = await Self.fetchSuggestions(handle)
            guard generation == suggestionGeneration else { return }
            suggestions = items
        }
    }

    private nonisolated static func fetchSuggestions(_ handle: SshSession) async -> [SuggestionItem] {
        await Task.detached(priority: .userInitiated) { handle.suggestions() }.value
    }
}
