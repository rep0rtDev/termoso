import SwiftUI

struct MainTabView: View {
    let repository: VaultRepository
    @State private var vaults: VaultsModel
    @State private var settings: SettingsModel

    init(repository: VaultRepository) {
        self.repository = repository
        _vaults = State(initialValue: VaultsModel(repository: repository))
        _settings = State(initialValue: SettingsModel(repository: repository))
    }

    var body: some View {
        TabView {
            VaultsView(model: vaults)
                .tabItem { Label("Vaults", systemImage: "lock.rectangle.stack") }
                .accessibilityIdentifier("tab.vaults")
            ConnectionsView()
                .tabItem { Label("Connections", systemImage: "bolt.horizontal") }
                .accessibilityIdentifier("tab.connections")
            SettingsView(model: settings)
                .tabItem { Label("Settings", systemImage: "gearshape") }
                .accessibilityIdentifier("tab.settings")
        }
        .preferredColorScheme(settings.colorScheme)
        .task {
            settings.load()
            vaults.reload()
        }
    }
}
