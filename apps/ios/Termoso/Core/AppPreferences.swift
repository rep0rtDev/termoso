import Foundation

/// Non-secret, device-local UI preferences. Everything that matters lives in
/// the encrypted vault (`MobileSettings`); this only remembers first-run
/// state so the shell knows what to show before the vault is open.
final class AppPreferences: @unchecked Sendable {
    enum ServerChoice: String {
        case cloud
        case selfHosted
        case offline
    }

    private let defaults: UserDefaults

    init(defaults: UserDefaults) {
        self.defaults = defaults
    }

    static func standard() -> AppPreferences {
        AppPreferences(defaults: .standard)
    }

    /// A throw-away suite that is wiped on creation — for UI tests.
    static func ephemeral() -> AppPreferences {
        let name = "com.termoso.ios.ephemeral.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name) ?? .standard
        defaults.removePersistentDomain(forName: name)
        return AppPreferences(defaults: defaults)
    }

    var welcomeSeen: Bool {
        get { defaults.bool(forKey: Keys.welcomeSeen) }
        set { defaults.set(newValue, forKey: Keys.welcomeSeen) }
    }

    var serverChoice: ServerChoice? {
        get { defaults.string(forKey: Keys.serverChoice).flatMap(ServerChoice.init(rawValue:)) }
        set { defaults.set(newValue?.rawValue, forKey: Keys.serverChoice) }
    }

    var selfHostedURL: String? {
        get { defaults.string(forKey: Keys.selfHostedURL) }
        set { defaults.set(newValue, forKey: Keys.selfHostedURL) }
    }

    private enum Keys {
        static let welcomeSeen = "welcome.seen"
        static let serverChoice = "server.choice"
        static let selfHostedURL = "server.selfHostedURL"
    }
}
