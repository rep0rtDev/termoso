import Foundation
import Observation
import TermosoCore

/// Vault selection plus the address book of the selected vault. Re-reads
/// from the core after every mutation — the store is local and fast.
@Observable
@MainActor
final class VaultsModel {
    let repository: VaultRepository

    private(set) var vaults: [VaultInfo] = []
    private(set) var hosts: [HostItem] = []
    private(set) var groups: [GroupItem] = []
    private(set) var tags: [TagItem] = []
    private(set) var error: String?

    var selectedVaultId: String? {
        didSet { if oldValue != selectedVaultId { reloadAddressBook() } }
    }

    init(repository: VaultRepository) {
        self.repository = repository
    }

    var selectedVault: VaultInfo? {
        vaults.first { $0.id == selectedVaultId }
    }

    func reload() {
        do {
            vaults = try repository.vaults()
            if selectedVaultId == nil || !vaults.contains(where: { $0.id == selectedVaultId }) {
                selectedVaultId = vaults.first { $0.kind == .local }?.id ?? vaults.first?.id
            }
            reloadAddressBook()
        } catch {
            self.error = userMessage(for: error)
        }
    }

    func reloadAddressBook() {
        guard let vaultId = selectedVaultId else {
            hosts = []
            groups = []
            tags = []
            return
        }
        do {
            hosts = try repository.hosts(vaultId: vaultId)
            groups = try repository.groups(vaultId: vaultId)
            tags = try repository.tags(vaultId: vaultId)
            error = nil
        } catch {
            self.error = userMessage(for: error)
        }
    }

    // MARK: Queries

    func hosts(inGroup groupId: String?, matching search: String, tag: String?) -> [HostItem] {
        let needle = search.trimmingCharacters(in: .whitespaces).lowercased()
        return hosts.filter { host in
            if let tag, !host.tags.contains(tag) { return false }
            if needle.isEmpty {
                return host.groupId == groupId
            }
            return host.label.lowercased().contains(needle)
                || host.address.lowercased().contains(needle)
                || host.username.lowercased().contains(needle)
                || host.tags.contains { $0.lowercased().contains(needle) }
        }
        .sorted { $0.label.localizedCaseInsensitiveCompare($1.label) == .orderedAscending }
    }

    func groups(inGroup parentId: String?) -> [GroupItem] {
        groups.filter { $0.parentId == parentId }
            .sorted { $0.label.localizedCaseInsensitiveCompare($1.label) == .orderedAscending }
    }

    func group(_ id: String) -> GroupItem? {
        groups.first { $0.id == id }
    }

    // MARK: Mutations

    func newHostDraft(groupId: String?) throws -> HostDraft {
        guard let vaultId = selectedVaultId else {
            throw MobileError.Invalid(detail: "No vault selected")
        }
        return try repository.newHostDraft(vaultId: vaultId, groupId: groupId)
    }

    func hostDraft(id: String) throws -> HostDraft {
        try repository.hostDraft(id: id)
    }

    func saveHost(_ draft: HostDraft) throws {
        try repository.saveHost(draft)
        reloadAddressBook()
    }

    func deleteHost(id: String) {
        run { try repository.deleteHost(id: id) }
    }

    func createGroup(label: String, parentId: String?) {
        guard let vaultId = selectedVaultId else { return }
        run { try repository.saveGroup(vaultId: vaultId, id: nil, label: label, parentId: parentId) }
    }

    func deleteGroup(id: String) {
        run { try repository.deleteGroup(id: id) }
    }

    func hostKeyPins(host: String, port: UInt16) throws -> [HostKeyPinItem] {
        try repository.hostKeyPins(host: host, port: port)
    }

    func pinHostKey(vaultId: String, host: String, port: UInt16, publicKey: String?) throws -> [HostKeyPinItem] {
        try repository.pinHostKey(vaultId: vaultId, host: host, port: port, publicKey: publicKey)
    }

    func unpinHostKey(id: String) throws {
        try repository.unpinHostKey(id: id)
    }

    func createTag(label: String) throws -> TagItem {
        guard let vaultId = selectedVaultId else {
            throw MobileError.Invalid(detail: "No vault selected")
        }
        let tag = try repository.createTag(vaultId: vaultId, label: label)
        tags = try repository.tags(vaultId: vaultId)
        return tag
    }

    private func run(_ body: () throws -> Void) {
        do {
            try body()
            reloadAddressBook()
        } catch {
            self.error = userMessage(for: error)
        }
    }
}
