import SwiftUI
import TermosoCore

/// Active and recently closed terminal sessions. Tapping a row brings the
/// terminal back; closed sessions can be reopened or removed.
struct ConnectionsView: View {
    @Bindable var store: SessionStore
    @State private var newConnection = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    if store.sessions.isEmpty {
                        emptyState
                    } else {
                        sessionsCard
                    }
                    newConnectionCard
                    Text("iOS gives background apps only a short grace period, so sessions live while Termoso is on screen.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 20)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
            .pageBackground()
            .navigationTitle("Connections")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        newConnection = true
                    } label: {
                        Image(systemName: "plus")
                    }
                    .accessibilityIdentifier("connections.new")
                }
            }
            .sheet(isPresented: $newConnection) {
                NewConnectionSheet(store: store)
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 10) {
            Image(systemName: "bolt.horizontal")
                .font(.system(size: 30))
                .foregroundStyle(.secondary)
            Text("No active connections")
                .font(.headline)
            Text("Tap a host in Vaults to open a terminal, or start a quick connection below.")
                .font(.footnote)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 28)
        .card()
        .accessibilityIdentifier("connections.empty")
    }

    private var sessionsCard: some View {
        CardSection("Terminals") {
            ForEach(Array(store.sessions.reversed().enumerated()), id: \.element.id) { index, session in
                if index > 0 { RowDivider() }
                Button {
                    store.show(session)
                } label: {
                    SessionRow(session: session)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("connections.session.\(session.title)")
                .contextMenu {
                    if session.isLive {
                        Button("Disconnect", systemImage: "bolt.slash", role: .destructive) { store.disconnect(session) }
                    } else {
                        Button("Reopen", systemImage: "arrow.clockwise") {
                            store.remove(session)
                            store.reopen(session)
                        }
                    }
                    Button("Remove", systemImage: "xmark", role: .destructive) { store.remove(session) }
                }
            }
        } accessory: {
            if store.sessions.contains(where: { !$0.isLive }) {
                Button("Clear finished") { store.clearFinished() }
                    .font(.footnote)
                    .accessibilityIdentifier("connections.clearFinished")
            }
        }
        .accessibilityIdentifier("connections.sessions")
    }

    private var newConnectionCard: some View {
        CardSection("New connection") {
            Button {
                newConnection = true
            } label: {
                CardRow(title: "Quick connect", subtitle: "user@host:port", chevron: true) {
                    SymbolTile(systemImage: "bolt.horizontal")
                }
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("connections.quickConnect")
            #if targetEnvironment(simulator)
                RowDivider()
                Button {
                    store.connectLocal()
                } label: {
                    CardRow(title: "Local shell", subtitle: "Simulator only") {
                        SymbolTile(systemImage: "terminal", tint: .secondary)
                    }
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("connections.localShell")
            #endif
        }
    }
}

struct SessionRow: View {
    let session: TerminalSession

    var body: some View {
        HStack(spacing: 12) {
            SymbolTile(systemImage: icon, tint: session.isLive ? Theme.terminalKeyActive : .secondary)
            VStack(alignment: .leading, spacing: 2) {
                Text(session.title)
                    .font(.body)
                    .lineLimit(1)
                Text(subtitle)
                    .font(.footnote)
                    .foregroundStyle(session.isLive ? .secondary : .tertiary)
                    .lineLimit(1)
            }
            Spacer()
            Circle()
                .fill(dotColor)
                .frame(width: 8, height: 8)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .contentShape(Rectangle())
    }

    private var icon: String {
        switch session.origin {
        case .local: "terminal"
        case .quick: "bolt.horizontal"
        case let .host(_, _, transport): transport == .mosh ? "antenna.radiowaves.left.and.right" : "server.rack"
        }
    }

    private var subtitle: String {
        let time = session.startedAt.formatted(date: .omitted, time: .shortened)
        return "\(session.stateLabel) · \(time)"
    }

    private var dotColor: Color {
        switch session.state {
        case .connecting: .yellow
        case .connected: Theme.terminalKeyActive
        case .closed: .secondary
        case .failed: .red
        }
    }
}

/// `user@host:port` quick connection (SSH); Telnet via the `telnet://` prefix.
struct NewConnectionSheet: View {
    let store: SessionStore
    @Environment(\.dismiss) private var dismiss
    @State private var target = ""
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("user@host:port", text: $target)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .focused($focused)
                        .submitLabel(.go)
                        .onSubmit(connect)
                        .accessibilityIdentifier("quickConnect.target")
                } footer: {
                    Text("Connects over SSH with the default port 22. Add `telnet://` for Telnet. The host is not saved — use Vaults for that.")
                }
                Section {
                    Button("Connect", action: connect)
                        .disabled(parsed == nil)
                        .accessibilityIdentifier("quickConnect.connect")
                }
                #if targetEnvironment(simulator)
                    Section {
                        Button("Local shell") {
                            store.connectLocal()
                            dismiss()
                        }
                        .accessibilityIdentifier("quickConnect.localShell")
                    }
                #endif
            }
            .navigationTitle("Quick connect")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
            .onAppear { focused = true }
        }
        .presentationDetents([.medium])
    }

    private var parsed: QuickTarget? {
        QuickTargetParser.parse(target)
    }

    private func connect() {
        guard let parsed else { return }
        store.connectQuick(parsed)
        dismiss()
    }
}

enum QuickTargetParser {
    /// `[ssh://|telnet://][user@]host[:port]`; IPv6 hosts in brackets.
    static func parse(_ raw: String) -> QuickTarget? {
        var text = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return nil }
        var proto = "ssh"
        for (prefix, name) in [("ssh://", "ssh"), ("telnet://", "telnet")] where text.lowercased().hasPrefix(prefix) {
            proto = name
            text = String(text.dropFirst(prefix.count))
        }
        var username = ""
        if let at = text.lastIndex(of: "@") {
            username = String(text[..<at])
            text = String(text[text.index(after: at)...])
        }
        var host = text
        var port: UInt16 = proto == "telnet" ? 23 : 22
        if host.hasPrefix("[") {
            guard let close = host.firstIndex(of: "]") else { return nil }
            let rest = host[host.index(after: close)...]
            host = String(host[host.index(after: host.startIndex) ..< close])
            if rest.hasPrefix(":") {
                guard let p = UInt16(rest.dropFirst()), p > 0 else { return nil }
                port = p
            } else if !rest.isEmpty {
                return nil
            }
        } else if let colon = host.lastIndex(of: ":"), host.filter({ $0 == ":" }).count == 1 {
            guard let p = UInt16(host[host.index(after: colon)...]), p > 0 else { return nil }
            port = p
            host = String(host[..<colon])
        }
        guard !host.isEmpty, !host.contains(" ") else { return nil }
        return QuickTarget(host: host, port: port, username: username, protocol: proto)
    }
}
