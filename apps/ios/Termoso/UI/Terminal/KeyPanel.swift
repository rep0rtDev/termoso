import SwiftUI
import TermosoCore

/// One button of the extra-keys bar.
enum PanelKey: Hashable, Identifiable {
    case special(SpecialKey, label: String)
    case text(String, label: String? = nil)
    case ctrl
    case alt
    case shift

    var id: String {
        switch self {
        case let .special(key, _): "special.\(key)"
        case let .text(text, _): "text.\(text)"
        case .ctrl: "ctrl"
        case .alt: "alt"
        case .shift: "shift"
        }
    }

    var label: String {
        switch self {
        case let .special(_, label): label
        case let .text(text, label): label ?? text
        case .ctrl: "ctrl"
        case .alt: "alt"
        case .shift: "shift"
        }
    }

    var isModifier: Bool {
        switch self {
        case .ctrl, .alt, .shift: true
        default: false
        }
    }

    static let main: [PanelKey] = [
        .special(.escape, label: "esc"),
        .special(.tab, label: "tab"),
        .ctrl,
        .alt,
        .special(.up, label: "↑"),
        .special(.down, label: "↓"),
        .special(.left, label: "←"),
        .special(.right, label: "→"),
        .text("-"),
        .text("/"),
        .text("|"),
        .text("~"),
        .text(":"),
        .text("'"),
        .text("\""),
    ]

    static let extra: [PanelKey] = [
        .special(.home, label: "home"),
        .special(.end, label: "end"),
        .special(.pageUp, label: "pgup"),
        .special(.pageDown, label: "pgdn"),
        .special(.insert, label: "ins"),
        .special(.delete, label: "del"),
        .shift,
        .text("`"),
        .text("^"),
        .text("&"),
        .text("*"),
        .text(";"),
        .text("\\"),
        .text("{"),
        .text("}"),
        .text("["),
        .text("]"),
        .text("<"),
        .text(">"),
    ]

    static let functionRow: [PanelKey] = [
        .special(.f1, label: "F1"), .special(.f2, label: "F2"), .special(.f3, label: "F3"),
        .special(.f4, label: "F4"), .special(.f5, label: "F5"), .special(.f6, label: "F6"),
        .special(.f7, label: "F7"), .special(.f8, label: "F8"), .special(.f9, label: "F9"),
        .special(.f10, label: "F10"), .special(.f11, label: "F11"), .special(.f12, label: "F12"),
    ]
}

/// Sticky modifiers: tapping ctrl/alt/shift arms it for the next key.
struct StickyMods: Equatable {
    var ctrl = false
    var alt = false
    var shift = false

    var keyMods: KeyMods { KeyMods(ctrl: ctrl, alt: alt, shift: shift) }
    var any: Bool { ctrl || alt || shift }

    mutating func toggle(_ key: PanelKey) {
        switch key {
        case .ctrl: ctrl.toggle()
        case .alt: alt.toggle()
        case .shift: shift.toggle()
        default: break
        }
    }

    func isOn(_ key: PanelKey) -> Bool {
        switch key {
        case .ctrl: ctrl
        case .alt: alt
        case .shift: shift
        default: false
        }
    }

    mutating func clear() {
        self = StickyMods()
    }
}

/// Extra-keys bar above the software keyboard: modifiers, navigation keys and
/// symbols that are awkward on a phone keyboard, plus keyboard/paste/more
/// controls. `expanded` adds the symbol and function rows.
struct KeyPanel: View {
    @Binding var mods: StickyMods
    @Binding var expanded: Bool
    let keyboardShown: Bool
    let onKey: (PanelKey) -> Void
    let onToggleKeyboard: () -> Void
    let onPaste: () -> Void
    let onMore: () -> Void

    var body: some View {
        VStack(spacing: 6) {
            if expanded {
                keyRow(PanelKey.functionRow, identifierPrefix: "terminal.fkey")
                keyRow(PanelKey.extra, identifierPrefix: "terminal.key")
            }
            HStack(spacing: 6) {
                controlButton(systemImage: keyboardShown ? "keyboard.chevron.compact.down" : "keyboard", identifier: "terminal.keyboard", action: onToggleKeyboard)
                keyRow(PanelKey.main, identifierPrefix: "terminal.key")
                controlButton(systemImage: expanded ? "chevron.down" : "chevron.up", identifier: "terminal.moreKeys") {
                    withAnimation(.easeInOut(duration: 0.15)) { expanded.toggle() }
                }
                controlButton(systemImage: "doc.on.clipboard", identifier: "terminal.paste", action: onPaste)
                controlButton(systemImage: "ellipsis", identifier: "terminal.more", action: onMore)
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(Theme.terminalPanel)
        .accessibilityIdentifier("terminal.keyPanel")
    }

    private func keyRow(_ keys: [PanelKey], identifierPrefix: String) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(keys) { key in
                    let on = mods.isOn(key)
                    Button {
                        onKey(key)
                    } label: {
                        Text(key.label)
                            .font(.system(size: 14, weight: .medium, design: .monospaced))
                            .foregroundStyle(on ? Color.black : Theme.terminalText)
                            .frame(minWidth: 34)
                            .frame(height: 34)
                            .padding(.horizontal, 6)
                            .background(on ? Theme.terminalKeyActive : Theme.terminalKey, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("\(identifierPrefix).\(key.id)")
                    .accessibilityAddTraits(on ? .isSelected : [])
                }
            }
        }
    }

    private func controlButton(systemImage: String, identifier: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(Theme.terminalText)
                .frame(width: 34, height: 34)
                .background(Theme.terminalKey, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(identifier)
    }
}

/// Autocomplete chips shown above the key panel while a command is typed.
struct SuggestionStrip: View {
    let suggestions: [SuggestionItem]
    let onPick: (SuggestionItem) -> Void

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(Array(suggestions.prefix(12).enumerated()), id: \.offset) { _, item in
                    Button {
                        onPick(item)
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: symbol(for: item.kind))
                                .font(.system(size: 10, weight: .semibold))
                                .foregroundStyle(Theme.terminalMuted)
                            Text(item.label)
                                .font(.system(size: 13, design: .monospaced))
                                .foregroundStyle(Theme.terminalText)
                                .lineLimit(1)
                        }
                        .padding(.horizontal, 10)
                        .frame(height: 30)
                        .background(Theme.terminalKey, in: Capsule())
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("terminal.suggestion.\(item.label)")
                }
            }
            .padding(.horizontal, 8)
        }
        .frame(height: 38)
        .background(Theme.terminalChrome)
        .accessibilityIdentifier("terminal.suggestions")
    }

    private func symbol(for kind: SuggestionKind) -> String {
        switch kind {
        case .command: "terminal"
        case .option: "minus"
        case .subcommand: "arrow.turn.down.right"
        case .path: "folder"
        case .history: "clock"
        case .snippet: "text.badge.plus"
        }
    }
}
