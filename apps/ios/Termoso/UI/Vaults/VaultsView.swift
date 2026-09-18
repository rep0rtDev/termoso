import SwiftUI
import TermosoCore

/// The Vaults tab: which vault is selected and what lives in it. Only Hosts
/// is wired in this increment; the other sections show where later
/// milestones plug in.
struct VaultsView: View {
    @Bindable var model: VaultsModel

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    vaultCard
                    CardSection("Address book") {
                        NavigationLink {
                            HostsView(model: model, groupId: nil)
                        } label: {
                            CardRow(title: "Hosts", subtitle: nil, chevron: true) {
                                SymbolTile(systemImage: "server.rack")
                            } trailing: {
                                countBadge(model.hosts.count)
                            }
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("vaults.hosts")
                        RowDivider()
                        CardRow(title: "Groups", subtitle: nil) {
                            SymbolTile(systemImage: "folder", tint: .secondary)
                        } trailing: {
                            countBadge(model.groups.count)
                        }
                        RowDivider()
                        CardRow(title: "Tags", subtitle: nil) {
                            SymbolTile(systemImage: "tag", tint: .secondary)
                        } trailing: {
                            countBadge(model.tags.count)
                        }
                    }

                    CardSection("Coming next") {
                        plannedRow("Keychain", systemImage: "key")
                        RowDivider()
                        plannedRow("Port forwarding", systemImage: "arrow.left.arrow.right")
                        RowDivider()
                        plannedRow("Snippets", systemImage: "text.badge.plus")
                        RowDivider()
                        plannedRow("Known hosts", systemImage: "checkmark.seal")
                        RowDivider()
                        plannedRow("History", systemImage: "clock")
                    }
                    Text("These sections exist in the Rust core already and arrive on iOS in the following updates.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 16)

                    if let error = model.error {
                        Label(error, systemImage: "exclamationmark.triangle")
                            .foregroundStyle(.red)
                            .padding(14)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .card()
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
            .pageBackground()
            .navigationTitle("Vaults")
            .refreshable { model.reload() }
        }
    }

    private var vaultCard: some View {
        VStack(alignment: .leading, spacing: 10) {
            vaultPicker
                .pickerStyle(.menu)
                .tint(.primary)
            if let vault = model.selectedVault {
                Text(vaultDescription(vault))
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .card()
    }

    private func countBadge(_ count: Int) -> some View {
        Text("\(count)")
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .monospacedDigit()
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

    private func plannedRow(_ title: String, systemImage: String) -> some View {
        CardRow(title: title, subtitle: nil) {
            SymbolTile(systemImage: systemImage, tint: .secondary)
        }
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
