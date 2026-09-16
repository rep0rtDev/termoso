import SwiftUI

struct MainTabView: View {
    let repository: VaultRepository
    @Environment(AppContainer.self) private var container
    @State private var vaults: VaultsModel
    @State private var settings: SettingsModel
    @State private var sessions: SessionStore

    init(repository: VaultRepository) {
        self.repository = repository
        _vaults = State(initialValue: VaultsModel(repository: repository))
        _settings = State(initialValue: SettingsModel(repository: repository))
        _sessions = State(initialValue: SessionStore(repository: repository))
    }

    var body: some View {
        TabView {
            VaultsView(model: vaults)
                .tabItem { Label("Vaults", systemImage: "lock.rectangle.stack") }
                .accessibilityIdentifier("tab.vaults")
            ConnectionsView(store: sessions)
                .tabItem { Label("Connections", systemImage: "bolt.horizontal") }
                .badge(sessions.liveCount)
                .accessibilityIdentifier("tab.connections")
            SettingsView(model: settings)
                .tabItem { Label("Profile", systemImage: "person.crop.circle") }
                .accessibilityIdentifier("tab.settings")
        }
        .tint(Theme.terminalKeyActive)
        .environment(sessions)
        .preferredColorScheme(settings.colorScheme)
        .fullScreenCover(isPresented: $sessions.terminalPresented) {
            TerminalScreen(store: sessions, settings: settings, uiTestMode: container.uiTestMode)
        }
        .task {
            settings.load()
            vaults.reload()
        }
    }
}
