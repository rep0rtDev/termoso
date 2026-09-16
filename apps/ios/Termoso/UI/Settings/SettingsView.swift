import SwiftUI
import TermosoCore

struct SettingsView: View {
    @Environment(AppContainer.self) private var container
    let model: SettingsModel

    var body: some View {
        NavigationStack {
            Form {
                accountSection
                appearanceSection
                terminalSection
                sshSection
                securitySection
                aboutSection
            }
            .navigationTitle("Settings")
            .task { model.load() }
        }
    }

    private var accountSection: some View {
        Section {
            LabeledContent("Server", value: serverLabel)
            Label("Sign in", systemImage: "person.crop.circle")
                .foregroundStyle(.tertiary)
        } header: {
            Text("Account")
        } footer: {
            Text("Sign-in, end-to-end encrypted sync and teams arrive in the account milestone. Your vault already works offline.")
        }
    }

    private var serverLabel: String {
        switch container.preferences.serverChoice {
        case .cloud: "Termoso Cloud"
        case .selfHosted: container.preferences.selfHostedURL ?? "Self-hosted"
        case .offline, .none: "Offline"
        }
    }

    private var appearanceSection: some View {
        Section("Appearance") {
            Picker("Theme", selection: model.binding(\.appTheme, default: "system")) {
                Text("System").tag("system")
                Text("Dark").tag("dark")
                Text("Light").tag("light")
            }
            .accessibilityIdentifier("settings.theme")
            Picker("Hosts view", selection: model.binding(\.hostsView, default: "list")) {
                Text("List").tag("list")
                Text("Grid").tag("grid")
            }
        }
    }

    private var terminalSection: some View {
        Section("Terminal") {
            Stepper(value: model.binding(\.terminalFontSize, default: 14), in: 8 ... 32) {
                LabeledContent("Font size", value: "\(model.settings?.terminalFontSize ?? 14) pt")
            }
            .accessibilityIdentifier("settings.fontSize")
            Picker("Cursor", selection: model.binding(\.cursorStyle, default: "block")) {
                Text("Block").tag("block")
                Text("Underline").tag("underline")
                Text("Beam").tag("beam")
            }
            Toggle("Cursor blink", isOn: model.binding(\.cursorBlink, default: true))
            Toggle("Terminal bell", isOn: model.binding(\.terminalBell, default: true))
            Toggle("Haptic feedback", isOn: model.binding(\.hapticFeedback, default: true))
            Toggle("Keep screen on", isOn: model.binding(\.keepScreenOn, default: true))
            Toggle("Autocomplete", isOn: model.binding(\.autocomplete, default: true))
        }
    }

    private var sshSection: some View {
        Section {
            Toggle("Post-quantum key exchange", isOn: model.binding(\.postQuantumKex, default: true))
                .accessibilityIdentifier("settings.pqKex")
            Toggle("Detect OS after connecting", isOn: model.binding(\.detectOs, default: true))
            Stepper(value: model.binding(\.keepAliveSeconds, default: 30), in: 0 ... 300, step: 5) {
                LabeledContent(
                    "Keep-alive",
                    value: (model.settings?.keepAliveSeconds ?? 30) == 0 ? "Off" : "\(model.settings?.keepAliveSeconds ?? 30) s"
                )
            }
        } header: {
            Text("SSH")
        } footer: {
            Text("Hybrid ML-KEM + X25519 key exchange is used when the server offers it, with a classical fallback otherwise.")
        }
    }

    private var securitySection: some View {
        Section {
            LabeledContent("Master key", value: "Keychain · this device only")
            Label("Lock with Face ID / Touch ID", systemImage: "faceid")
                .foregroundStyle(.tertiary)
        } header: {
            Text("Security")
        } footer: {
            Text("The vault is an encrypted SQLite database; its key lives in this device's Keychain (never synced, not included in backups). Biometric lock arrives in the next update.")
        }
    }

    private var aboutSection: some View {
        Section {
            LabeledContent("Version", value: appVersion)
            LabeledContent("Core", value: container.coreVersion)
                .accessibilityIdentifier("settings.coreVersion")
            Link(destination: URL(string: "https://github.com/rep0rtDev/termoso")!) {
                Label("Source code", systemImage: "chevron.left.forwardslash.chevron.right")
            }
            Link(destination: URL(string: "https://github.com/rep0rtDev/termoso/blob/main/docs/IOS_SIDELOAD.md")!) {
                Label("Installing & re-signing (7-day sideload)", systemImage: "arrow.triangle.2.circlepath")
            }
        } header: {
            Text("About")
        } footer: {
            Text("Termoso is free software with no telemetry, no analytics and no paywalls. It reports to you — not on you.")
        }
    }

    private var appVersion: String {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? "dev"
        let build = info?["CFBundleVersion"] as? String ?? "0"
        return "\(version) (\(build))"
    }
}
