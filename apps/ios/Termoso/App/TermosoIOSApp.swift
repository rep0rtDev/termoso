import SwiftUI
import TermosoCore

@main
struct TermosoIOSApp: App {
    @State private var container: AppContainer

    init() {
        let launch = LaunchOptions.current
        initLogging(verbose: launch.verboseLogging)
        _container = State(initialValue: AppContainer(launch: launch))
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(container)
        }
    }
}

/// Process arguments understood by the app. UI tests pass `--ui-test` to get
/// a throw-away profile (temporary directory, ephemeral master key, welcome
/// shown) so they never touch the real vault or Keychain.
struct LaunchOptions: Sendable {
    var ephemeralProfile = false
    var verboseLogging = false

    static let current = LaunchOptions(arguments: CommandLine.arguments)

    init(arguments: [String]) {
        ephemeralProfile = arguments.contains("--ui-test")
        verboseLogging = arguments.contains("--verbose") || ephemeralProfile
    }
}
