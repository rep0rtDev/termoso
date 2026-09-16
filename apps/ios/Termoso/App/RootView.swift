import SwiftUI

/// Locked → opening → (welcome on first run) → main shell.
struct RootView: View {
    @Environment(AppContainer.self) private var container

    var body: some View {
        Group {
            switch container.vault {
            case let .locked(error):
                LockedView(error: error) {
                    Task { await container.unlock() }
                }
            case .opening:
                ProgressView("Opening vault…")
                    .accessibilityIdentifier("root.opening")
            case let .open(repository):
                if container.welcomeSeen {
                    MainTabView(repository: repository)
                } else {
                    WelcomeView()
                }
            }
        }
        .task { await container.unlock() }
    }
}

struct LockedView: View {
    let error: String?
    let retry: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: error == nil ? "lock.fill" : "lock.trianglebadge.exclamationmark")
                .font(.system(size: 44))
                .foregroundStyle(.secondary)
            Text(error == nil ? "Vault locked" : "Couldn't open the vault")
                .font(.title3.weight(.semibold))
            if let error {
                Text(error)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
                    .accessibilityIdentifier("root.lockedError")
            }
            Button("Try again", action: retry)
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("root.retry")
        }
        .padding()
    }
}
