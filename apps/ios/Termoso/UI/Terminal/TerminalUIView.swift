import TermosoCore
import UIKit

/// UIKit terminal surface: draws the packed grid with UIKit string drawing,
/// owns the software keyboard (`UIKeyInput`) and hardware key presses, and
/// turns pan gestures into scrolling. SwiftUI drives it through
/// `TerminalSurface`.
final class TerminalUIView: UIView, UIKeyInput {
    // MARK: Configuration

    var session: TerminalSession? {
        didSet { if session !== oldValue { lastReported = nil; reload() } }
    }

    /// Sticky modifiers from the key panel, applied to the next key.
    var stickyMods: () -> KeyMods = { KeyMods(ctrl: false, alt: false, shift: false) }
    /// Called after any key/text reached the session (panel clears stickies).
    var onKeySent: (() -> Void)?
    var onLongPress: (() -> Void)?
    /// Pinch scale relative to the gesture start.
    var onPinch: ((CGFloat) -> Void)?
    var onSizeChange: ((UInt16, UInt16) -> Void)?
    var onFocusChange: ((Bool) -> Void)?

    var fontSize: CGFloat = 13 {
        didSet {
            guard fontSize != oldValue else { return }
            rebuildFonts()
            setNeedsLayout()
            setNeedsDisplay()
        }
    }

    var cursorBlink = true {
        didSet { restartBlink() }
    }

    /// Mirrors the visible text into `accessibilityValue` for XCUITest.
    var mirrorTextForTests = false

    // MARK: State

    private var frameData: GridFrame?
    private var grid: CellGrid?
    private var regularFont = UIFont.monospacedSystemFont(ofSize: 13, weight: .regular)
    private var boldFont = UIFont.monospacedSystemFont(ofSize: 13, weight: .bold)
    private var italicFont = UIFont.monospacedSystemFont(ofSize: 13, weight: .regular)
    private var boldItalicFont = UIFont.monospacedSystemFont(ofSize: 13, weight: .bold)
    private(set) var cellSize = CGSize(width: 8, height: 16)
    private var lastReported: (cols: UInt16, rows: UInt16)?
    private var panRemainder: CGFloat = 0
    private var cursorOn = true
    private var blinkTimer: Timer?
    private var suppressInsertUntil = Date.distantPast

    // MARK: UITextInputTraits

    var keyboardType: UIKeyboardType = .asciiCapable
    var autocorrectionType: UITextAutocorrectionType = .no
    var autocapitalizationType: UITextAutocapitalizationType = .none
    var spellCheckingType: UITextSpellCheckingType = .no
    var smartQuotesType: UITextSmartQuotesType = .no
    var smartDashesType: UITextSmartDashesType = .no
    var smartInsertDeleteType: UITextSmartInsertDeleteType = .no
    var keyboardAppearance: UIKeyboardAppearance = .dark
    var returnKeyType: UIReturnKeyType = .default
    var enablesReturnKeyAutomatically = false

    // MARK: Init

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = true
        contentMode = .redraw
        backgroundColor = .black
        isAccessibilityElement = true
        accessibilityIdentifier = "terminal.canvas"
        accessibilityLabel = "Terminal"
        rebuildFonts()

        let tap = UITapGestureRecognizer(target: self, action: #selector(handleTap))
        addGestureRecognizer(tap)
        let pan = UIPanGestureRecognizer(target: self, action: #selector(handlePan(_:)))
        pan.maximumNumberOfTouches = 1
        addGestureRecognizer(pan)
        let long = UILongPressGestureRecognizer(target: self, action: #selector(handleLongPress(_:)))
        addGestureRecognizer(long)
        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(handlePinch(_:)))
        addGestureRecognizer(pinch)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("TerminalUIView is code-only")
    }

    deinit {
        blinkTimer?.invalidate()
    }

    // MARK: Fonts & metrics

    private func rebuildFonts() {
        regularFont = .monospacedSystemFont(ofSize: fontSize, weight: .regular)
        boldFont = .monospacedSystemFont(ofSize: fontSize, weight: .bold)
        italicFont = Self.italic(regularFont)
        boldItalicFont = Self.italic(boldFont)
        let advance = ("M" as NSString).size(withAttributes: [.font: regularFont]).width
        cellSize = CGSize(width: max(1, advance), height: max(1, ceil(regularFont.lineHeight)))
    }

    private static func italic(_ font: UIFont) -> UIFont {
        var traits = font.fontDescriptor.symbolicTraits
        traits.insert(.traitItalic)
        guard let descriptor = font.fontDescriptor.withSymbolicTraits(traits) else { return font }
        return UIFont(descriptor: descriptor, size: font.pointSize)
    }

    private func font(for flags: UInt8) -> UIFont {
        let bold = flags & CellFlag.bold != 0
        let italic = flags & CellFlag.italic != 0
        switch (bold, italic) {
        case (true, true): return boldItalicFont
        case (true, false): return boldFont
        case (false, true): return italicFont
        case (false, false): return regularFont
        }
    }

    // MARK: Data

    /// Re-read the grid from the session and redraw.
    func reload() {
        frameData = session?.frame
        grid = frameData.map(CellGrid.init(frame:))
        if let frameData {
            backgroundColor = Self.color(frameData.background)
        }
        if mirrorTextForTests, let session {
            accessibilityValue = session.visibleText.filter { !$0.isEmpty }.suffix(12).joined(separator: "\n")
        }
        setNeedsDisplay()
    }

    var altScreen: Bool { frameData?.altScreen ?? false }

    // MARK: Layout

    override func layoutSubviews() {
        super.layoutSubviews()
        let cols = UInt16(clamping: Int(bounds.width / cellSize.width))
        let rows = UInt16(clamping: Int(bounds.height / cellSize.height))
        guard cols >= 2, rows >= 2 else { return }
        if lastReported?.cols != cols || lastReported?.rows != rows {
            lastReported = (cols, rows)
            session?.resize(cols: cols, rows: rows)
            onSizeChange?(cols, rows)
        }
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil {
            blinkTimer?.invalidate()
            blinkTimer = nil
        } else {
            restartBlink()
        }
    }

    // MARK: Drawing

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        let base = frameData?.background ?? 0x1A_1D_28
        ctx.setFillColor(Self.color(base).cgColor)
        ctx.fill(rect)
        guard let grid, let frameData, !grid.isEmpty else { return }

        let cw = cellSize.width
        let ch = cellSize.height
        let firstRow = max(0, Int(rect.minY / ch))
        let lastRow = min(grid.rows - 1, Int(rect.maxY / ch))
        guard firstRow <= lastRow else { return }

        // Backgrounds: one fill per run of equal colour.
        for row in firstRow ... lastRow {
            var col = 0
            while col < grid.cols {
                let bg = grid.background(col, row)
                var end = col + 1
                while end < grid.cols, grid.background(end, row) == bg { end += 1 }
                if bg != base {
                    ctx.setFillColor(Self.color(bg).cgColor)
                    ctx.fill(CGRect(x: CGFloat(col) * cw, y: CGFloat(row) * ch, width: CGFloat(end - col) * cw, height: ch))
                }
                col = end
            }
        }

        // Text: runs of equal foreground/attributes, wide glyphs on their own.
        for row in firstRow ... lastRow {
            var col = 0
            while col < grid.cols {
                let flags = grid.flags(col, row)
                guard let first = grid.character(col, row) else {
                    col += 1
                    continue
                }
                let fg = grid.foreground(col, row)
                if flags & CellFlag.wide != 0 {
                    draw(String(first), fg: fg, flags: flags, x: CGFloat(col) * cw, y: CGFloat(row) * ch, width: cw * 2)
                    col += 2
                    continue
                }
                var text = String(first)
                var end = col + 1
                while end < grid.cols,
                      grid.flags(end, row) == flags,
                      grid.foreground(end, row) == fg,
                      let next = grid.character(end, row) {
                    text.append(next)
                    end += 1
                }
                draw(text, fg: fg, flags: flags, x: CGFloat(col) * cw, y: CGFloat(row) * ch, width: nil)
                col = end
            }
        }

        drawCursor(ctx, grid: grid, frame: frameData)
    }

    private func attributes(fg: UInt32, flags: UInt8, color: UIColor? = nil) -> [NSAttributedString.Key: Any] {
        var fgColor = color ?? Self.color(fg)
        if flags & CellFlag.dim != 0 {
            fgColor = fgColor.withAlphaComponent(0.6)
        }
        var attrs: [NSAttributedString.Key: Any] = [
            .font: font(for: flags),
            .foregroundColor: fgColor,
        ]
        if flags & CellFlag.underline != 0 {
            attrs[.underlineStyle] = NSUnderlineStyle.single.rawValue
            attrs[.underlineColor] = fgColor
        }
        if flags & CellFlag.strikeout != 0 {
            attrs[.strikethroughStyle] = NSUnderlineStyle.single.rawValue
            attrs[.strikethroughColor] = fgColor
        }
        return attrs
    }

    private func draw(_ text: String, fg: UInt32, flags: UInt8, x: CGFloat, y: CGFloat, width: CGFloat?, color: UIColor? = nil) {
        let string = NSAttributedString(string: text, attributes: attributes(fg: fg, flags: flags, color: color))
        if let width {
            let size = string.size()
            string.draw(at: CGPoint(x: x + max(0, (width - size.width) / 2), y: y))
        } else {
            string.draw(at: CGPoint(x: x, y: y))
        }
    }

    private func drawCursor(_ ctx: CGContext, grid: CellGrid, frame: GridFrame) {
        guard frame.displayOffset == 0, frame.cursor != .hidden else { return }
        let col = Int(frame.cursorCol)
        let row = Int(frame.cursorRow)
        guard col < grid.cols, row < grid.rows else { return }
        let flags = grid.flags(col, row)
        let wide = flags & CellFlag.wide != 0 ? 2 : 1
        let rect = CGRect(
            x: CGFloat(col) * cellSize.width,
            y: CGFloat(row) * cellSize.height,
            width: cellSize.width * CGFloat(wide),
            height: cellSize.height
        )
        let fg = grid.foreground(col, row)
        let cursorColor = Self.color(fg)
        let focused = isFirstResponder
        let style: CursorStyle = focused ? frame.cursor : .hollowBlock
        if focused, cursorBlink, !cursorOn, style != .hollowBlock { return }

        switch style {
        case .block:
            ctx.setFillColor(cursorColor.cgColor)
            ctx.fill(rect)
            if let ch = grid.character(col, row) {
                draw(String(ch), fg: fg, flags: flags, x: rect.minX, y: rect.minY, width: wide == 2 ? rect.width : nil,
                     color: Self.color(grid.background(col, row)))
            }
        case .hollowBlock:
            ctx.setStrokeColor(cursorColor.cgColor)
            ctx.setLineWidth(1)
            ctx.stroke(rect.insetBy(dx: 0.5, dy: 0.5))
        case .underline:
            ctx.setFillColor(cursorColor.cgColor)
            ctx.fill(CGRect(x: rect.minX, y: rect.maxY - 2, width: rect.width, height: 2))
        case .beam:
            ctx.setFillColor(cursorColor.cgColor)
            ctx.fill(CGRect(x: rect.minX, y: rect.minY, width: 2, height: rect.height))
        case .hidden:
            break
        }
    }

    private func restartBlink() {
        blinkTimer?.invalidate()
        blinkTimer = nil
        cursorOn = true
        guard cursorBlink, window != nil else { return }
        blinkTimer = Timer.scheduledTimer(withTimeInterval: 0.55, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.isFirstResponder else { return }
                self.cursorOn.toggle()
                self.setNeedsDisplay()
            }
        }
    }

    static func color(_ rgb: UInt32) -> UIColor {
        UIColor(
            red: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }

    // MARK: Focus

    override var canBecomeFirstResponder: Bool { true }

    @discardableResult
    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        if ok {
            cursorOn = true
            setNeedsDisplay()
            onFocusChange?(true)
        }
        return ok
    }

    @discardableResult
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok {
            setNeedsDisplay()
            onFocusChange?(false)
        }
        return ok
    }

    // MARK: UIKeyInput

    var hasText: Bool { true }

    func insertText(_ text: String) {
        guard Date() >= suppressInsertUntil else { return }
        let mods = stickyMods()
        if text == "\n" || text == "\r" {
            session?.send(key: .enter, mods: mods)
        } else if text == "\t" {
            session?.send(key: .tab, mods: mods)
        } else {
            session?.send(text: text, mods: mods)
        }
        session?.scrollToBottom()
        onKeySent?()
    }

    func deleteBackward() {
        session?.send(key: .backspace, mods: stickyMods())
        session?.scrollToBottom()
        onKeySent?()
    }

    // MARK: Hardware keyboard

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var handled = false
        for press in presses {
            guard let key = press.key else { continue }
            if handle(hardwareKey: key) { handled = true }
        }
        if !handled {
            super.pressesBegan(presses, with: event)
        }
    }

    private func handle(hardwareKey key: UIKey) -> Bool {
        var mods = stickyMods()
        let flags = key.modifierFlags
        mods.ctrl = mods.ctrl || flags.contains(.control)
        mods.alt = mods.alt || flags.contains(.alternate)
        mods.shift = mods.shift || flags.contains(.shift)

        if let special = Self.specialKey(for: key.keyCode) {
            session?.send(key: special, mods: mods)
            session?.scrollToBottom()
            suppressInsertUntil = Date().addingTimeInterval(0.05)
            onKeySent?()
            return true
        }
        let chars = key.charactersIgnoringModifiers
        if mods.ctrl || mods.alt, !chars.isEmpty, chars.unicodeScalars.allSatisfy({ $0.value >= 0x20 }) {
            session?.send(text: chars, mods: mods)
            session?.scrollToBottom()
            suppressInsertUntil = Date().addingTimeInterval(0.05)
            onKeySent?()
            return true
        }
        return false
    }

    /// Keys the text system would swallow or mistranslate; Enter, Tab and
    /// Backspace stay with `UIKeyInput` so they are not sent twice.
    static func specialKey(for code: UIKeyboardHIDUsage) -> SpecialKey? {
        switch code {
        case .keyboardUpArrow: .up
        case .keyboardDownArrow: .down
        case .keyboardLeftArrow: .left
        case .keyboardRightArrow: .right
        case .keyboardEscape: .escape
        case .keyboardHome: .home
        case .keyboardEnd: .end
        case .keyboardPageUp: .pageUp
        case .keyboardPageDown: .pageDown
        case .keyboardInsert: .insert
        case .keyboardDeleteForward: .delete
        case .keyboardF1: .f1
        case .keyboardF2: .f2
        case .keyboardF3: .f3
        case .keyboardF4: .f4
        case .keyboardF5: .f5
        case .keyboardF6: .f6
        case .keyboardF7: .f7
        case .keyboardF8: .f8
        case .keyboardF9: .f9
        case .keyboardF10: .f10
        case .keyboardF11: .f11
        case .keyboardF12: .f12
        default: nil
        }
    }

    // MARK: Gestures

    @objc private func handleTap() {
        if !isFirstResponder {
            becomeFirstResponder()
        }
    }

    @objc private func handlePan(_ gesture: UIPanGestureRecognizer) {
        switch gesture.state {
        case .began:
            panRemainder = 0
        case .changed:
            let dy = gesture.translation(in: self).y + panRemainder
            let lines = Int(dy / cellSize.height)
            if lines != 0 {
                session?.scroll(lines: Int32(clamping: lines), altScreen: altScreen)
                panRemainder = dy - CGFloat(lines) * cellSize.height
                gesture.setTranslation(.zero, in: self)
            } else {
                panRemainder = dy
                gesture.setTranslation(.zero, in: self)
            }
        default:
            panRemainder = 0
        }
    }

    @objc private func handleLongPress(_ gesture: UILongPressGestureRecognizer) {
        guard gesture.state == .began else { return }
        onLongPress?()
    }

    @objc private func handlePinch(_ gesture: UIPinchGestureRecognizer) {
        guard gesture.state == .ended else { return }
        onPinch?(gesture.scale)
    }
}
