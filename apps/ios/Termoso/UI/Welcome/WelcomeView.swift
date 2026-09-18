import SwiftUI

/// First-run screen: pick how the app talks to a server. Sign-in itself
/// arrives with the account milestone; until then Cloud / Self-hosted only
/// remember the choice and continue into the offline vault.
struct WelcomeView: View {
    @Environment(AppContainer.self) private var container
    @State private var showSelfHosted = false
    @State private var selfHostedURL = ""

    var body: some View {
        ZStack {
            Theme.terminalChrome.ignoresSafeArea()
            VStack(spacing: 0) {
                Spacer(minLength: 24)
                header
                Spacer(minLength: 24)
                VStack(spacing: 12) {
                    choice(
                        title: "Termoso Cloud",
                        subtitle: "Free end-to-end encrypted sync. The server only ever stores ciphertext.",
                        systemImage: "icloud",
                        identifier: "welcome.cloud",
                        prominent: true
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
                    Button {
                        container.completeWelcome(choice: .offline)
                    } label: {
                        Text("Continue offline")
                            .font(.body.weight(.medium))
                            .foregroundStyle(Theme.terminalText)
                            .frame(maxWidth: .infinity)
                            .frame(height: 48)
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("welcome.offline")
                }
                footer
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 16)
        }
        .preferredColorScheme(.dark)
        .sheet(isPresented: $showSelfHosted) {
            SelfHostedSheet(url: $selfHostedURL) { url in
                container.completeWelcome(choice: .selfHosted, selfHostedURL: url)
            }
            .presentationDetents([.medium])
        }
    }

    private var header: some View {
        VStack(spacing: 14) {
            Image(systemName: "terminal.fill")
                .font(.system(size: 40, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 92, height: 92)
                .background(
                    LinearGradient(colors: [Theme.terminalKeyActive, Color(red: 0x1C / 255, green: 0x8E / 255, blue: 0xA8 / 255)],
                                   startPoint: .topLeading, endPoint: .bottomTrailing),
                    in: RoundedRectangle(cornerRadius: 26, style: .continuous)
                )
            Text("Termoso")
                .font(.system(size: 34, weight: .bold))
                .foregroundStyle(.white)
            Text("SSH client that reports to you — not on you.")
                .font(.subheadline)
                .foregroundStyle(Theme.terminalMuted)
                .multilineTextAlignment(.center)
        }
    }

    private var footer: some View {
        VStack(spacing: 6) {
            Label("Open source · no telemetry · no lock-in", systemImage: "checkmark.shield")
                .font(.footnote)
                .foregroundStyle(Theme.terminalMuted)
            Text("Vault contents are encrypted on this device with a key kept in the Keychain.")
                .font(.caption2)
                .foregroundStyle(Theme.terminalMuted.opacity(0.8))
                .multilineTextAlignment(.center)
        }
        .padding(.top, 20)
    }

    private func choice(
        title: String,
        subtitle: String,
        systemImage: String,
        identifier: String,
        prominent: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 14) {
                Image(systemName: systemImage)
                    .font(.title3.weight(.semibold))
                    .frame(width: 32)
                    .foregroundStyle(prominent ? Color.black.opacity(0.75) : Theme.terminalKeyActive)
                VStack(alignment: .leading, spacing: 3) {
                    Text(title)
                        .font(.headline)
                        .foregroundStyle(prominent ? Color.black : Color.white)
                    Text(subtitle)
                        .font(.footnote)
                        .foregroundStyle(prominent ? Color.black.opacity(0.7) : Theme.terminalMuted)
                        .multilineTextAlignment(.leading)
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(prominent ? Color.black.opacity(0.5) : Theme.terminalMuted)
            }
            .padding(16)
            .background(prominent ? Theme.terminalKeyActive : Theme.terminalPanel, in: RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous))
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
