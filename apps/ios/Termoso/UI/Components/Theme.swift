import SwiftUI

/// Shared look: pale grouped page background, white rounded cards, one green
/// accent, and a dark navy terminal shell. Colours adapt to dark mode.
enum Theme {
    static let cardRadius: CGFloat = 16
    static let tileRadius: CGFloat = 12

    static let page = Color(uiColor: .systemGroupedBackground)
    static let card = Color(uiColor: .secondarySystemGroupedBackground)
    static let cardInset = Color(uiColor: .tertiarySystemGroupedBackground)
    static let separator = Color(uiColor: .separator)

    /// Terminal chrome (tabs, key panel) — darker than the default palette
    /// background so the grid reads as the focused surface.
    static let terminalChrome = Color(red: 0x10 / 255, green: 0x17 / 255, blue: 0x22 / 255)
    static let terminalPanel = Color(red: 0x18 / 255, green: 0x22 / 255, blue: 0x32 / 255)
    static let terminalKey = Color(red: 0x24 / 255, green: 0x30 / 255, blue: 0x44 / 255)
    static let terminalKeyActive = Color(red: 0x2B / 255, green: 0xB8 / 255, blue: 0x84 / 255)
    static let terminalText = Color(red: 0xD7 / 255, green: 0xDE / 255, blue: 0xE9 / 255)
    static let terminalMuted = Color(red: 0x8A / 255, green: 0x97 / 255, blue: 0xAA / 255)

    static func color(rgb: UInt32) -> Color {
        Color(
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255
        )
    }
}

/// White rounded card used for list groups on the pale page background.
struct CardModifier: ViewModifier {
    var padding: CGFloat = 0

    func body(content: Content) -> some View {
        content
            .padding(padding)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous))
    }
}

extension View {
    func card(padding: CGFloat = 0) -> some View {
        modifier(CardModifier(padding: padding))
    }

    /// Page chrome shared by the tab roots: pale grouped background behind a
    /// plain scroll view of cards.
    func pageBackground() -> some View {
        background(Theme.page.ignoresSafeArea())
    }
}

/// Rounded search field placed under the large title, like the address-book
/// search on the hosts screen.
struct SearchField: View {
    let prompt: String
    @Binding var text: String
    var identifier: String

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(.secondary)
            TextField(prompt, text: $text)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .accessibilityIdentifier(identifier)
            if !text.isEmpty {
                Button {
                    text = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("\(identifier).clear")
            }
        }
        .padding(.horizontal, 12)
        .frame(height: 40)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: Theme.tileRadius, style: .continuous))
    }
}

/// Row inside a card: leading tile, title/subtitle, optional trailing view,
/// chevron for navigation rows.
struct CardRow<Leading: View, Trailing: View>: View {
    let title: String
    var subtitle: String? = nil
    var chevron = false
    @ViewBuilder var leading: Leading
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 12) {
            leading
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 8)
            trailing
            if chevron {
                Image(systemName: "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(.tertiary)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .contentShape(Rectangle())
    }
}

extension CardRow where Trailing == EmptyView {
    init(title: String, subtitle: String? = nil, chevron: Bool = false, @ViewBuilder leading: () -> Leading) {
        self.title = title
        self.subtitle = subtitle
        self.chevron = chevron
        self.leading = leading()
        self.trailing = EmptyView()
    }
}

/// Square rounded tile with an SF Symbol, used as the leading glyph of
/// section rows (Hosts, Keychain, Snippets…).
struct SymbolTile: View {
    let systemImage: String
    var tint: Color = .accentColor
    var size: CGFloat = 36

    var body: some View {
        Image(systemName: systemImage)
            .font(.system(size: size * 0.45, weight: .semibold))
            .foregroundStyle(tint)
            .frame(width: size, height: size)
            .background(tint.opacity(0.14), in: RoundedRectangle(cornerRadius: size * 0.28, style: .continuous))
    }
}

/// Section caption above a card, with an optional trailing accessory.
struct CardHeader<Accessory: View>: View {
    let text: String
    @ViewBuilder var accessory: Accessory

    init(_ text: String, @ViewBuilder accessory: () -> Accessory) {
        self.text = text
        self.accessory = accessory()
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(text.uppercased())
                .font(.footnote.weight(.semibold))
                .foregroundStyle(.secondary)
            Spacer()
            accessory
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 6)
    }
}

extension CardHeader where Accessory == EmptyView {
    init(_ text: String) {
        self.text = text
        self.accessory = EmptyView()
    }
}

/// Caption + card, the building block of every tab root.
struct CardSection<Accessory: View, Content: View>: View {
    let title: String?
    @ViewBuilder var accessory: Accessory
    @ViewBuilder var content: Content

    init(_ title: String?, @ViewBuilder content: () -> Content, @ViewBuilder accessory: () -> Accessory) {
        self.title = title
        self.accessory = accessory()
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let title {
                CardHeader(title) { accessory }
            }
            VStack(spacing: 0) {
                content
            }
            .card()
        }
    }
}

extension CardSection where Accessory == EmptyView {
    init(_ title: String?, @ViewBuilder content: () -> Content) {
        self.title = title
        self.accessory = EmptyView()
        self.content = content()
    }
}

/// Hairline between card rows, indented past the leading tile.
struct RowDivider: View {
    var inset: CGFloat = 62

    var body: some View {
        Divider().padding(.leading, inset)
    }
}
