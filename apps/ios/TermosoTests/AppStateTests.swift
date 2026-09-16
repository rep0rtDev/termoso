import Foundation
import TermosoCore
@testable import Termoso
import XCTest

final class LaunchOptionsTests: XCTestCase {
    func testDefaults() {
        let options = LaunchOptions(arguments: ["Termoso"])
        XCTAssertFalse(options.ephemeralProfile)
        XCTAssertFalse(options.verboseLogging)
    }

    func testUITestFlagImpliesVerbose() {
        let options = LaunchOptions(arguments: ["Termoso", "--ui-test"])
        XCTAssertTrue(options.ephemeralProfile)
        XCTAssertTrue(options.verboseLogging)
    }
}

final class AppPreferencesTests: XCTestCase {
    func testEphemeralStartsClean() {
        let prefs = AppPreferences.ephemeral()
        XCTAssertFalse(prefs.welcomeSeen)
        XCTAssertNil(prefs.serverChoice)
        XCTAssertNil(prefs.selfHostedURL)
    }

    func testRoundTrip() {
        let prefs = AppPreferences.ephemeral()
        prefs.welcomeSeen = true
        prefs.serverChoice = .selfHosted
        prefs.selfHostedURL = "https://termoso.example"
        XCTAssertTrue(prefs.welcomeSeen)
        XCTAssertEqual(prefs.serverChoice, .selfHosted)
        XCTAssertEqual(prefs.selfHostedURL, "https://termoso.example")
    }
}

final class EphemeralMasterKeyStoreTests: XCTestCase {
    func testStableWithinProcessAndErasable() throws {
        let store = EphemeralMasterKeyStore()
        XCTAssertFalse(try store.exists())
        let first = try store.loadOrCreate()
        XCTAssertEqual(first.count, 32)
        XCTAssertEqual(try store.loadOrCreate(), first)
        XCTAssertTrue(try store.exists())
        try store.erase()
        XCTAssertFalse(try store.exists())
        XCTAssertNotEqual(try store.loadOrCreate(), first)
    }
}

final class KeychainMasterKeyStoreTests: XCTestCase {
    /// Uses a test-only service name so it never touches the app's real key.
    private let store = KeychainMasterKeyStore(service: "com.termoso.ios.tests", account: UUID().uuidString)

    override func tearDownWithError() throws {
        try store.erase()
    }

    func testCreateReadErase() throws {
        XCTAssertFalse(try store.exists())
        let key = try store.loadOrCreate()
        XCTAssertEqual(key.count, 32)
        XCTAssertEqual(try store.loadOrCreate(), key, "second load returns the persisted key")
        try store.erase()
        XCTAssertFalse(try store.exists())
        try store.erase()
    }
}

@MainActor
final class AppContainerTests: XCTestCase {
    func testUnlockOpensVaultAndWelcomeFlow() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("termoso-container-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let container = AppContainer(
            profileDir: dir,
            preferences: .ephemeral(),
            masterKeys: EphemeralMasterKeyStore()
        )
        XCTAssertFalse(container.welcomeSeen)
        XCTAssertNil(container.vault.repository)

        await container.unlock()
        let repo = try XCTUnwrap(container.vault.repository)
        XCTAssertEqual(try repo.vaults().count, 1)

        container.completeWelcome(choice: .selfHosted, selfHostedURL: "https://t.example")
        XCTAssertTrue(container.welcomeSeen)
        XCTAssertEqual(container.preferences.serverChoice, .selfHosted)
        XCTAssertEqual(container.preferences.selfHostedURL, "https://t.example")
    }
}

@MainActor
final class VaultsModelTests: XCTestCase {
    private var dir: URL!
    private var repo: VaultRepository!

    override func setUpWithError() throws {
        dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("termoso-model-\(UUID().uuidString)", isDirectory: true)
        repo = try VaultRepository.open(profileDir: dir, masterKey: KeychainMasterKeyStore.randomKey())
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: dir)
    }

    func testReloadSelectsLocalVaultAndFilters() throws {
        let model = VaultsModel(repository: repo)
        model.reload()
        XCTAssertEqual(model.selectedVault?.kind, .local)
        XCTAssertTrue(model.hosts.isEmpty)

        model.createGroup(label: "prod", parentId: nil)
        let prod = try XCTUnwrap(model.groups.first)
        let web = try model.createTag(label: "web")

        var a = try model.newHostDraft(groupId: nil)
        a.label = "alpha"
        a.address = "alpha.example"
        a.tagIds = [web.id]
        try model.saveHost(a)

        var b = try model.newHostDraft(groupId: prod.id)
        b.label = "bravo"
        b.address = "10.1.2.3"
        b.username = "root"
        try model.saveHost(b)

        XCTAssertEqual(model.hosts.count, 2)
        XCTAssertEqual(model.hosts(inGroup: nil, matching: "", tag: nil).map(\.label), ["alpha"])
        XCTAssertEqual(model.hosts(inGroup: prod.id, matching: "", tag: nil).map(\.label), ["bravo"])
        XCTAssertEqual(model.hosts(inGroup: nil, matching: "10.1", tag: nil).map(\.label), ["bravo"], "search spans groups")
        XCTAssertEqual(model.hosts(inGroup: nil, matching: "root", tag: nil).map(\.label), ["bravo"])
        XCTAssertEqual(model.hosts(inGroup: nil, matching: "", tag: "web").map(\.label), ["alpha"])
        XCTAssertEqual(model.groups(inGroup: nil).map(\.label), ["prod"])

        model.deleteHost(id: model.hosts.first { $0.label == "alpha" }!.id)
        XCTAssertEqual(model.hosts.map(\.label), ["bravo"])
        XCTAssertNil(model.error)
    }
}
