import SwiftUI
import TermosoCore

/// The Vaults tab: which vault is selected and what lives in it. Only Hosts
/// is wired in this increment; the other sections show where later
/// milestones plug in.
struct VaultsView: View {
    @Bindable var model: VaultsModel

    var body: some View {
        NavigationStack {
            List {
                Section {
                    vaultPicker
                } footer: {
                    if let vault = model.selectedVault {
                        Text(vaultDescription(vault))
                    }
                }

                Section("Address book") {
                    NavigationLink {
                        HostsView(model: model, groupId: nil)
                    } label: {
                        sectionRow("Hosts", systemImage: "server.rack", count: model.hosts.count)
                    }
                    .accessibilityIdentifier("vaults.hosts")
                    sectionRow("Groups", systemImage: "folder", count: model.groups.count)
                        .foregroundStyle(.secondary)
                    sectionRow("Tags", systemImage: "tag", count: model.tags.count)
                        .foregroundStyle(.secondary)
                }

                Section {
                    plannedRow("Keychain", systemImage: "key")
                    plannedRow("Port forwarding", systemImage: "arrow.left.arrow.right")
                    plannedRow("Snippets", systemImage: "text.badge.plus")
                    plannedRow("Known hosts", systemImage: "checkmark.seal")
                    plannedRow("History", systemImage: "clock")
                } header: {
                    Text("Coming next")
                } footer: {
                    Text("These sections exist in the Rust core already and arrive on iOS in the following updates.")
                }

                if let error = model.error {
                    Section {
                        Label(error, systemImage: "exclamationmark.triangle")
                            .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle("Vaults")
            .refreshable { model.reload() }
        }
    }

    private var vaultPicker: some View {
        Picker(selection: $model.selectedVaultId) {
            ForEach(model.vaults, id: \.id) { vault in
                Label(vault.name, systemImage: vaultIcon(vault))
                    .tag(Optional(vault.id))
            }
        } label: {
            Label("Vault", systemImage: "lock.rectangle.stack")
        }
        .accessibilityIdentifier("vaults.picker")
    }

    private func sectionRow(_ title: String, systemImage: String, count: Int) -> some View {
        HStack {
            Label(title, systemImage: systemImage)
            Spacer()
            Text("\(count)")
                .foregroundStyle(.secondary)
                .monospacedDigit()
        }
    }

    private func plannedRow(_ title: String, systemImage: String) -> some View {
        Label(title, systemImage: systemImage)
            .foregroundStyle(.tertiary)
    }

    private func vaultIcon(_ vault: VaultInfo) -> String {
        switch vault.kind {
        case .local: "iphone"
        case .personal: "person"
        case .team: "person.3"
        }
    }

    private func vaultDescription(_ vault: VaultInfo) -> String {
        switch vault.kind {
        case .local:
            return "Stored only on this device, encrypted with a key from the Keychain."
        case .personal:
            return "Synced end-to-end encrypted with your account."
        case .team:
            let access = switch vault.access {
            case .view: "view"
            case .edit: "edit"
            case .manage: "manage"
            }
            return "Shared with your team · \(access) access" + (vault.locked ? " · key pending" : "")
        }
    }
}
