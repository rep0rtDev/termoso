import SwiftUI
import TermosoCore

/// Answers one core prompt: host key trust, password/passphrase, security-key
/// PIN, keyboard-interactive questions. Dismissing without answering cancels
/// the connection.
struct PromptSheet: View {
    let session: TerminalSession
    let prompt: TerminalSession.PendingPrompt

    @Environment(\.dismiss) private var dismiss
    @State private var secret = ""
    @State private var remember = false
    @State private var answers: [String] = []
    @State private var answered = false
    @FocusState private var secretFocused: Bool

    var body: some View {
        NavigationStack {
            Form {
                content
            }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { answer(.cancel) }
                        .accessibilityIdentifier("prompt.cancel")
                }
            }
        }
        .presentationDetents([.medium, .large])
        .interactiveDismissDisabled()
        .onAppear {
            if case let .keyboardInteractive(_, _, questions) = prompt.request {
                answers = Array(repeating: "", count: questions.count)
            }
            secretFocused = true
        }
        .onDisappear {
            if !answered { session.answer(prompt, .cancel) }
        }
    }

    private var title: String {
        switch prompt.request {
        case .hostKeyUnknown: "New host key"
        case .hostKeyChanged: "Host key changed"
        case .password: "Password"
        case .passphrase: "Key passphrase"
        case .securityKeyPin: "Security key PIN"
        case .securityKeyInsert: "Security key"
        case let .keyboardInteractive(name, _, _): name.isEmpty ? "Authentication" : name
        }
    }

    @ViewBuilder
    private var content: some View {
        switch prompt.request {
        case let .hostKeyUnknown(host, keyType, fingerprint):
            Section {
                LabeledContent("Host", value: host)
                LabeledContent("Key type", value: keyType)
                fingerprintRow(fingerprint)
            } footer: {
                Text("This server is not in your known hosts yet. Compare the fingerprint with one you trust before saving it.")
            }
            Section {
                Button("Trust and save") { answer(.hostKey(decision: .acceptAndSave)) }
                    .accessibilityIdentifier("prompt.hostKey.save")
                Button("Connect once") { answer(.hostKey(decision: .acceptOnce)) }
                    .accessibilityIdentifier("prompt.hostKey.once")
            }

        case let .hostKeyChanged(host, keyType, oldFingerprint, newFingerprint):
            Section {
                Label("The key of \(host) differs from the one saved earlier. Someone could be intercepting the connection — or the server was reinstalled.", systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(.red)
                LabeledContent("Key type", value: keyType)
                fingerprintRow(oldFingerprint, label: "Saved")
                fingerprintRow(newFingerprint, label: "Offered")
            }
            Section {
                Button("Connect once") { answer(.hostKey(decision: .acceptOnce)) }
                Button("Replace saved key", role: .destructive) { answer(.hostKey(decision: .acceptAndSave)) }
                    .accessibilityIdentifier("prompt.hostKey.replace")
            }

        case let .password(username, retry):
            Section {
                SecureField("Password for \(username)", text: $secret)
                    .focused($secretFocused)
                    .submitLabel(.go)
                    .onSubmit { submitSecret() }
                    .accessibilityIdentifier("prompt.secret")
                Toggle("Remember in this host", isOn: $remember)
                    .accessibilityIdentifier("prompt.remember")
            } footer: {
                if retry { Text("The previous password was rejected.").foregroundStyle(.red) }
            }
            connectButton

        case let .passphrase(keyLabel, retry):
            Section {
                SecureField("Passphrase for \(keyLabel)", text: $secret)
                    .focused($secretFocused)
                    .submitLabel(.go)
                    .onSubmit { submitSecret() }
                    .accessibilityIdentifier("prompt.secret")
                Toggle("Remember with the key", isOn: $remember)
            } footer: {
                if retry { Text("The passphrase did not unlock the key.").foregroundStyle(.red) }
            }
            connectButton

        case let .securityKeyPin(keyLabel, retry, retries):
            Section {
                SecureField("PIN for \(keyLabel)", text: $secret)
                    .keyboardType(.numberPad)
                    .focused($secretFocused)
                    .accessibilityIdentifier("prompt.secret")
            } footer: {
                if retry {
                    Text(retries.map { "Wrong PIN — \($0) attempts left." } ?? "Wrong PIN.").foregroundStyle(.red)
                }
            }
            connectButton

        case let .securityKeyInsert(keyLabel, wrongDevice):
            Section {
                Label(
                    wrongDevice ? "A different security key is connected. Insert the one that holds \(keyLabel)."
                        : "Security keys over USB/NFC are not wired up on iOS yet, so \(keyLabel) cannot be used here.",
                    systemImage: "key.radiowaves.forward"
                )
            }
            Section {
                Button("Try again") { answer(.retry) }
            }

        case let .keyboardInteractive(_, instructions, questions):
            if !instructions.isEmpty {
                Section { Text(instructions) }
            }
            Section {
                ForEach(Array(questions.enumerated()), id: \.offset) { index, question in
                    if question.echo {
                        TextField(question.prompt, text: binding(index))
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .accessibilityIdentifier("prompt.answer.\(index)")
                    } else {
                        SecureField(question.prompt, text: binding(index))
                            .accessibilityIdentifier("prompt.answer.\(index)")
                    }
                }
            }
            Section {
                Button("Continue") { answer(.answers(values: answers)) }
                    .accessibilityIdentifier("prompt.submit")
            }
        }
    }

    private var connectButton: some View {
        Section {
            Button("Connect") { submitSecret() }
                .disabled(secret.isEmpty)
                .accessibilityIdentifier("prompt.submit")
        }
    }

    private func fingerprintRow(_ fingerprint: String, label: String = "Fingerprint") -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).font(.footnote).foregroundStyle(.secondary)
            Text(fingerprint)
                .font(.system(.footnote, design: .monospaced))
                .textSelection(.enabled)
        }
    }

    private func binding(_ index: Int) -> Binding<String> {
        Binding(
            get: { index < answers.count ? answers[index] : "" },
            set: { if index < answers.count { answers[index] = $0 } }
        )
    }

    private func submitSecret() {
        guard !secret.isEmpty else { return }
        answer(.secret(value: secret, remember: remember))
    }

    private func answer(_ answer: PromptAnswer) {
        guard !answered else { return }
        answered = true
        session.answer(prompt, answer)
        dismiss()
    }
}
