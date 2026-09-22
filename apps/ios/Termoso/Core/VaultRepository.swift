import Foundation
import TermosoCore

/// Thin Swift face over the Rust `TermosoApp` object: the encrypted vault
/// store plus everything built on it. One instance per opened profile.
///
/// Calls are synchronous and cheap (local SQLite); anything that hits the
/// network or does key derivation is dispatched off the main actor by callers.
struct VaultRepository: Sendable {
    let app: TermosoCore.TermosoApp

    /// Opens (or creates) the vault at `profileDir` with a 32-byte master key.
    /// The key bytes are zeroed after the core has copied them. The directory
    /// is excluded from device backups: without the device-bound key the
    /// ciphertext would be useless anyway.
    static func open(profileDir: URL, masterKey: Data) throws -> VaultRepository {
        var key = masterKey
        defer { key.resetBytes(in: 0 ..< key.count) }
        try FileManager.default.createDirectory(at: profileDir, withIntermediateDirectories: true)
        var dir = profileDir
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try dir.setResourceValues(values)
        let app = try TermosoCore.TermosoApp.open(profileDir: profileDir.path, masterKey: key)
        return VaultRepository(app: app)
    }

    // MARK: Vaults

    func vaults() throws -> [VaultInfo] {
        try app.vaults()
    }

    func localVault() throws -> VaultInfo {
        try app.localVault()
    }

    // MARK: Hosts, groups, tags

    func hosts(vaultId: String?) throws -> [HostItem] {
        try app.hosts(vaultId: vaultId)
    }

    func groups(vaultId: String?) throws -> [GroupItem] {
        try app.groups(vaultId: vaultId)
    }

    func tags(vaultId: String?) throws -> [TagItem] {
        try app.tags(vaultId: vaultId)
    }

    func hostDraft(id: String) throws -> HostDraft {
        try app.hostDraft(id: id)
    }

    func newHostDraft(vaultId: String, groupId: String?) throws -> HostDraft {
        try app.newHostDraft(vaultId: vaultId, groupId: groupId)
    }

    @discardableResult
    func saveHost(_ draft: HostDraft) throws -> HostItem {
        try app.saveHost(draft: draft)
    }

    func deleteHost(id: String) throws {
        try app.deleteHost(id: id)
    }

    @discardableResult
    func saveGroup(vaultId: String, id: String?, label: String, parentId: String?) throws -> GroupItem {
        try app.saveGroup(vaultId: vaultId, id: id, label: label, parentId: parentId)
    }

    func deleteGroup(id: String) throws {
        try app.deleteGroup(id: id)
    }

    @discardableResult
    func createTag(vaultId: String, label: String) throws -> TagItem {
        try app.createTag(vaultId: vaultId, label: label)
    }

    // MARK: Server key pins

    func hostKeyPins(host: String, port: UInt16) throws -> [HostKeyPinItem] {
        try app.hostKeyPins(host: host, port: port)
    }

    func pinHostKey(vaultId: String, host: String, port: UInt16, publicKey: String?) throws -> [HostKeyPinItem] {
        try app.pinHostKey(vaultId: vaultId, host: host, port: port, publicKey: publicKey)
    }

    func unpinHostKey(id: String) throws {
        try app.unpinHostKey(id: id)
    }

    // MARK: Settings

    func settings() throws -> MobileSettings {
        try app.settings()
    }

    func saveSettings(_ settings: MobileSettings) throws {
        try app.saveSettings(settings: settings)
    }
}
