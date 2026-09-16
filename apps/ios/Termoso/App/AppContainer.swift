import Foundation
import Observation
import TermosoCore

/// Composition root: owns the profile location, the master-key store and the
/// opened vault. Mirrors `AppContainer` on Android.
@Observable
@MainActor
final class AppContainer {
    enum VaultState {
        /// Not opened yet, or opening failed (`error` set).
        case locked(error: String?)
        case opening
        case open(VaultRepository)

        var repository: VaultRepository? {
            if case let .open(repo) = self { return repo }
            return nil
        }
    }

    private(set) var vault: VaultState = .locked(error: nil)
    private(set) var welcomeSeen: Bool

    let profileDir: URL
    let preferences: AppPreferences
    let masterKeys: any MasterKeyStore
    let coreVersion: String = TermosoCore.coreVersion()
    /// Set under `--ui-test`: the terminal mirrors its visible text into the
    /// accessibility tree so XCUITest can assert on shell output.
    let uiTestMode: Bool

    init(launch: LaunchOptions) {
        uiTestMode = launch.ephemeralProfile
        if launch.ephemeralProfile {
            profileDir = FileManager.default.temporaryDirectory
                .appendingPathComponent("termoso-ui-test-\(UUID().uuidString)", isDirectory: true)
            preferences = .ephemeral()
            masterKeys = EphemeralMasterKeyStore()
        } else {
            profileDir = Self.defaultProfileDir()
            preferences = .standard()
            masterKeys = KeychainMasterKeyStore()
        }
        welcomeSeen = preferences.welcomeSeen
    }

    /// Explicit wiring for tests and previews.
    init(profileDir: URL, preferences: AppPreferences, masterKeys: any MasterKeyStore) {
        uiTestMode = false
        self.profileDir = profileDir
        self.preferences = preferences
        self.masterKeys = masterKeys
        welcomeSeen = preferences.welcomeSeen
    }

    /// `Application Support/Termoso/profile` — backed up only as ciphertext,
    /// the key stays in the Keychain (device-only).
    static func defaultProfileDir() -> URL {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return support.appendingPathComponent("Termoso", isDirectory: true)
            .appendingPathComponent("profile", isDirectory: true)
    }

    /// Loads (or creates) the master key and opens the encrypted vault off the
    /// main actor. Idempotent while open or opening.
    func unlock() async {
        switch vault {
        case .open, .opening:
            return
        case .locked:
            break
        }
        vault = .opening
        let dir = profileDir
        let keys = masterKeys
        do {
            let repo = try await Task.detached(priority: .userInitiated) {
                let key = try keys.loadOrCreate()
                return try VaultRepository.open(profileDir: dir, masterKey: key)
            }.value
            vault = .open(repo)
        } catch {
            vault = .locked(error: userMessage(for: error))
        }
    }

    func completeWelcome(choice: AppPreferences.ServerChoice, selfHostedURL: String? = nil) {
        preferences.serverChoice = choice
        preferences.selfHostedURL = selfHostedURL
        preferences.welcomeSeen = true
        welcomeSeen = true
    }
}
