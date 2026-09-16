import SwiftUI

/// First-run screen: pick how the app talks to a server. Sign-in itself
/// arrives with the account milestone; until then Cloud / Self-hosted only
/// remember the choice and continue into the offline vault.
struct WelcomeView: View {
    @Environment(AppContainer.self) private var container
    @State private var showSelfHosted = false
    @State private var selfHostedURL = ""

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 28) {
                    header
                    VStack(spacing: 12) {
                        choice(
                            title: "Termoso Cloud",
                            subtitle: "Free end-to-end encrypted sync. The server only ever stores ciphertext.",
                            systemImage: "icloud",
                            identifier: "welcome.cloud"
                        ) {
                            container.completeWelcome(choice: .cloud)
                        }
                        choice(
                            title: "Self-hosted server",
                            subtitle: "Your own Termoso instance — same app, your infrastructure.",
                            systemImage: "server.rack",
                            identifier: "welcome.selfHosted"
                        ) {
                            showSelfHosted = true
                        }
                        choice(
                            title: "Continue offline",
                            subtitle: "Encrypted local vault only. Connect an account any time in Settings.",
                            systemImage: "iphone.and.arrow.forward",
                            identifier: "welcome.offline"
                        ) {
                            container.completeWelcome(choice: .offline)
                        }
                    }
                    footer
                }
                .padding(24)
            }
            .navigationTitle("Welcome")
            .navigationBarTitleDisplayMode(.inline)
            .sheet(isPresented: $showSelfHosted) {
                SelfHostedSheet(url: $selfHostedURL) { url in
                    container.completeWelcome(choice: .selfHosted, selfHostedURL: url)
                }
                .presentationDetents([.medium])
            }
        }
    }

    private var header: some View {
        VStack(spacing: 10) {
            Image(systemName: "terminal.fill")
                .font(.system(size: 52))
                .foregroundStyle(Color.accentColor)
            Text("Termoso")
                .font(.largeTitle.weight(.bold))
            Text("SSH client that reports to you — not on you.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding(.top, 24)
    }

    private var footer: some View {
        VStack(spacing: 6) {
            Label("Open source · no telemetry · no lock-in", systemImage: "checkmark.shield")
                .font(.footnote)
                .foregroundStyle(.secondary)
            Text("Vault contents are encrypted on this device with a key kept in the Keychain.")
                .font(.caption2)
                .foregroundStyle(.tertiary)
                .multilineTextAlignment(.center)
        }
    }

    private func choice(
        title: String,
        subtitle: String,
        systemImage: String,
        identifier: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 14) {
                Image(systemName: systemImage)
                    .font(.title2)
                    .frame(width: 36)
                    .foregroundStyle(Color.accentColor)
                VStack(alignment: .leading, spacing: 3) {
                    Text(title).font(.headline)
                    Text(subtitle)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.leading)
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(.tertiary)
            }
            .padding(16)
            .background(.fill.tertiary, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(identifier)
    }
}

private struct SelfHostedSheet: View {
    @Binding var url: String
    let onContinue: (String) -> Void
    @Environment(\.dismiss) private var dismiss

    private var normalized: String? {
        var text = url.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.isEmpty { return nil }
        if !text.contains("://") { text = "https://" + text }
        guard let parsed = URL(string: text), let scheme = parsed.scheme,
              ["http", "https"].contains(scheme.lowercased()), parsed.host() != nil
        else { return nil }
        return text
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("https://termoso.example.com", text: $url)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .textContentType(.URL)
                        .accessibilityIdentifier("welcome.selfHosted.url")
                } header: {
                    Text("Server address")
                } footer: {
                    Text("Sign-in and sync arrive in a later update; the address is remembered for it.")
                }
            }
            .navigationTitle("Self-hosted")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Continue") {
                        if let normalized {
                            onContinue(normalized)
                            dismiss()
                        }
                    }
                    .disabled(normalized == nil)
                    .accessibilityIdentifier("welcome.selfHosted.continue")
                }
            }
        }
    }
}
