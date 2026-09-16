import SwiftUI
import TermosoCore

/// Large rounded-square host tile: the detected distro glyph in its brand
/// colour on a tinted background, or a generic server symbol until the OS is
/// known. `icon` (manual override) wins over the detected `osName`.
struct HostAvatar: View {
    let host: HostItem
    var size: CGFloat = 44

    var body: some View {
        let icon = DistroIcons.forOs(host.icon) ?? DistroIcons.forOs(host.osName)
        ZStack {
            RoundedRectangle(cornerRadius: size * 0.28, style: .continuous)
                .fill((icon?.color ?? Color.accentColor).opacity(0.14))
            if let icon {
                icon.image
                    .resizable()
                    .renderingMode(.template)
                    .aspectRatio(contentMode: .fit)
                    .foregroundStyle(icon.color)
                    .padding(size * 0.22)
            } else {
                Image(systemName: fallbackSymbol)
                    .font(.system(size: size * 0.42, weight: .semibold))
                    .foregroundStyle(Color.accentColor)
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }

    private var fallbackSymbol: String {
        switch host.protocol {
        case "telnet": "network"
        case "serial": "cable.connector"
        default: host.useMosh ? "antenna.radiowaves.left.and.right" : "server.rack"
        }
    }
}

/// Group tile: folder glyph on the accent tint.
struct GroupAvatar: View {
    var size: CGFloat = 44

    var body: some View {
        Image(systemName: "folder.fill")
            .font(.system(size: size * 0.42, weight: .semibold))
            .foregroundStyle(Color.accentColor)
            .frame(width: size, height: size)
            .background(Color.accentColor.opacity(0.14), in: RoundedRectangle(cornerRadius: size * 0.28, style: .continuous))
            .accessibilityHidden(true)
    }
}
