import Observation
import SwiftUI
import TermosoCore

/// `MobileSettings` from the encrypted vault; every change is written back
/// immediately so the Android and iOS clients share one settings shape.
@Observable
@MainActor
final class SettingsModel {
    let repository: VaultRepository
    private(set) var settings: MobileSettings?
    private(set) var error: String?

    init(repository: VaultRepository) {
        self.repository = repository
    }

    func load() {
        do {
            settings = try repository.settings()
            error = nil
        } catch {
            self.error = userMessage(for: error)
        }
    }

    func update(_ change: (inout MobileSettings) -> Void) {
        guard var current = settings else { return }
        change(&current)
        do {
            try repository.saveSettings(current)
            settings = current
            error = nil
        } catch {
            self.error = userMessage(for: error)
        }
    }

    var colorScheme: ColorScheme? {
        switch settings?.appTheme {
        case "dark": .dark
        case "light": .light
        default: nil
        }
    }

    /// Binding into one field of the settings record, saving on set.
    func binding<Value>(_ keyPath: WritableKeyPath<MobileSettings, Value>, default fallback: Value) -> Binding<Value> {
        Binding(
            get: { self.settings?[keyPath: keyPath] ?? fallback },
            set: { value in self.update { $0[keyPath: keyPath] = value } }
        )
    }
}
