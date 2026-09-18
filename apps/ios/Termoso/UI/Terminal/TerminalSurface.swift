import SwiftUI
import TermosoCore

/// SwiftUI face of `TerminalUIView`. `tick` is the session's `frameTick`,
/// read by the parent so a new frame always re-runs `updateUIView`.
struct TerminalSurface: UIViewRepresentable {
    let session: TerminalSession
    let tick: Int
    let fontSize: CGFloat
    let cursorBlink: Bool
    let mods: KeyMods
    /// Bump to (re)show the software keyboard; negative values hide it.
    let focusRequest: Int
    let mirrorTextForTests: Bool
    let onKeySent: () -> Void
    let onLongPress: () -> Void
    let onPinch: (CGFloat) -> Void
    let onFocusChange: (Bool) -> Void

    final class Coordinator {
        var mods = KeyMods(ctrl: false, alt: false, shift: false)
        var lastFocusRequest = 0
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeUIView(context: Context) -> TerminalUIView {
        let view = TerminalUIView(frame: .zero)
        let coordinator = context.coordinator
        view.stickyMods = { coordinator.mods }
        view.onKeySent = onKeySent
        view.onLongPress = onLongPress
        view.onPinch = onPinch
        view.onFocusChange = onFocusChange
        view.mirrorTextForTests = mirrorTextForTests
        view.fontSize = fontSize
        view.cursorBlink = cursorBlink
        view.session = session
        return view
    }

    func updateUIView(_ view: TerminalUIView, context: Context) {
        context.coordinator.mods = mods
        view.onKeySent = onKeySent
        view.onLongPress = onLongPress
        view.onPinch = onPinch
        view.onFocusChange = onFocusChange
        view.fontSize = fontSize
        view.cursorBlink = cursorBlink
        if view.session !== session {
            view.session = session
        } else {
            view.reload()
        }
        if context.coordinator.lastFocusRequest != focusRequest {
            context.coordinator.lastFocusRequest = focusRequest
            DispatchQueue.main.async {
                if focusRequest >= 0 {
                    view.becomeFirstResponder()
                } else {
                    view.resignFirstResponder()
                }
            }
        }
        _ = tick
    }
}
