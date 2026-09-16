import Foundation
import TermosoCore
@testable import Termoso
import XCTest

/// End-to-end over the real Rust core: opens an encrypted vault in a
/// temporary directory and drives the address book through the FFI.
final class VaultRepositoryTests: XCTestCase {
    private var dir: URL!
    private var key: Data!

    override func setUpWithError() throws {
        dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("termoso-tests-\(UUID().uuidString)", isDirectory: true)
        key = KeychainMasterKeyStore.randomKey()
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: dir)
    }

    private func open() throws -> VaultRepository {
        try VaultRepository.open(profileDir: dir, masterKey: key)
    }

    func testOpenCreatesLocalVault() throws {
        let repo = try open()
        let vaults = try repo.vaults()
        XCTAssertEqual(vaults.count, 1)
        XCTAssertEqual(vaults[0].kind, .local)
        XCTAssertEqual(try repo.localVault().id, vaults[0].id)
        XCTAssertTrue(try repo.hosts(vaultId: nil).isEmpty)
        XCTAssertFalse(coreVersion().isEmpty)
    }

    func testHostRoundTrip() throws {
        let repo = try open()
        let vault = try repo.localVault()

        var draft = try repo.newHostDraft(vaultId: vault.id, groupId: nil)
        XCTAssertNil(draft.id)
        XCTAssertTrue(draft.ssh)
        draft.label = "pqssh"
        draft.address = "pq.example.org"
        draft.username = "ubuntu"
        draft.port = 2222
        draft.password = "hunter2"
        draft.notes = "test box"

        let saved = try repo.saveHost(draft)
        XCTAssertEqual(saved.label, "pqssh")
        XCTAssertEqual(saved.address, "pq.example.org")
        XCTAssertEqual(saved.port, 2222)
        XCTAssertEqual(saved.username, "ubuntu")

        let hosts = try repo.hosts(vaultId: vault.id)
        XCTAssertEqual(hosts.map(\.id), [saved.id])

        let reloaded = try repo.hostDraft(id: saved.id)
        XCTAssertEqual(reloaded.id, saved.id)
        XCTAssertTrue(reloaded.hasPassword, "stored password is reported but never returned")
        XCTAssertNil(reloaded.password)
        XCTAssertEqual(reloaded.notes, "test box")

        try repo.deleteHost(id: saved.id)
        XCTAssertTrue(try repo.hosts(vaultId: vault.id).isEmpty)
        XCTAssertThrowsError(try repo.hostDraft(id: saved.id))
    }

    func testGroupsAndTags() throws {
        let repo = try open()
        let vault = try repo.localVault()

        let prod = try repo.saveGroup(vaultId: vault.id, id: nil, label: "prod", parentId: nil)
        let eu = try repo.saveGroup(vaultId: vault.id, id: nil, label: "eu", parentId: prod.id)
        let web = try repo.createTag(vaultId: vault.id, label: "web")

        var draft = try repo.newHostDraft(vaultId: vault.id, groupId: eu.id)
        XCTAssertEqual(draft.groupId, eu.id)
        draft.address = "10.0.0.1"
        draft.tagIds = [web.id]
        let host = try repo.saveHost(draft)
        XCTAssertEqual(host.groupPath, ["prod", "eu"])
        XCTAssertEqual(host.tags, ["web"])

        let groups = try repo.groups(vaultId: vault.id)
        XCTAssertEqual(Set(groups.map(\.label)), ["prod", "eu"])
        XCTAssertEqual(groups.first { $0.id == prod.id }?.groupCount, 1)
        XCTAssertEqual(groups.first { $0.id == eu.id }?.hostCount, 1)

        let tags = try repo.tags(vaultId: vault.id)
        XCTAssertEqual(tags.map(\.label), ["web"])
        XCTAssertEqual(tags.first?.hosts, 1)
    }

    func testPersistsAcrossReopen() throws {
        do {
            let repo = try open()
            var draft = try repo.newHostDraft(vaultId: try repo.localVault().id, groupId: nil)
            draft.label = "keep"
            draft.address = "keep.example"
            try repo.saveHost(draft)
        }
        let again = try open()
        XCTAssertEqual(try again.hosts(vaultId: nil).map(\.label), ["keep"])
    }

    func testWrongKeyFails() throws {
        _ = try open()
        let other = KeychainMasterKeyStore.randomKey()
        XCTAssertThrowsError(try VaultRepository.open(profileDir: dir, masterKey: other))
    }

    func testSettingsRoundTrip() throws {
        let repo = try open()
        var settings = try repo.settings()
        XCTAssertEqual(settings.appTheme, "system")
        settings.appTheme = "dark"
        settings.terminalFontSize = 18
        try repo.saveSettings(settings)
        let reloaded = try repo.settings()
        XCTAssertEqual(reloaded.appTheme, "dark")
        XCTAssertEqual(reloaded.terminalFontSize, 18)
    }

    func testErrorMessagesAreHuman() throws {
        let repo = try open()
        do {
            _ = try repo.hostDraft(id: "does-not-exist")
            XCTFail("expected an error")
        } catch {
            let text = userMessage(for: error)
            XCTAssertFalse(text.contains("MobileError"), "reflection leaked: \(text)")
            XCTAssertFalse(text.isEmpty)
        }
    }
}
