import Foundation
import TermosoCore

/// Cell attribute bits packed by the core into the top byte of the
/// foreground word (`termoso_mobile::terminal::flag`).
enum CellFlag {
    static let bold: UInt8 = 1 << 0
    static let italic: UInt8 = 1 << 1
    static let underline: UInt8 = 1 << 2
    static let strikeout: UInt8 = 1 << 3
    static let dim: UInt8 = 1 << 4
    static let wide: UInt8 = 1 << 5
    static let wideSpacer: UInt8 = 1 << 6
    static let hidden: UInt8 = 1 << 7
}

/// Read-only view over a packed `GridFrame`: `cols × rows` little-endian
/// triples of `u32` — `[code point, flags << 24 | fg RGB, bg RGB]`, 12 bytes
/// per cell, row-major.
struct CellGrid {
    static let cellBytes = 12

    let cols: Int
    let rows: Int
    private let bytes: [UInt8]

    init(frame: GridFrame) {
        self.init(cols: Int(frame.cols), rows: Int(frame.rows), cells: frame.cells)
    }

    init(cols: Int, rows: Int, cells: Data) {
        self.cols = cols
        self.rows = rows
        var bytes = [UInt8](cells)
        let needed = cols * rows * Self.cellBytes
        if bytes.count < needed {
            bytes.append(contentsOf: [UInt8](repeating: 0, count: needed - bytes.count))
        }
        self.bytes = bytes
    }

    var isEmpty: Bool { cols == 0 || rows == 0 }

    @inline(__always)
    private func word(_ col: Int, _ row: Int, _ offset: Int) -> UInt32 {
        let base = (row * cols + col) * Self.cellBytes + offset
        return UInt32(bytes[base])
            | UInt32(bytes[base + 1]) << 8
            | UInt32(bytes[base + 2]) << 16
            | UInt32(bytes[base + 3]) << 24
    }

    func codePoint(_ col: Int, _ row: Int) -> UInt32 {
        word(col, row, 0)
    }

    /// 0xRRGGBB
    func foreground(_ col: Int, _ row: Int) -> UInt32 {
        word(col, row, 4) & 0x00FF_FFFF
    }

    /// 0xRRGGBB
    func background(_ col: Int, _ row: Int) -> UInt32 {
        word(col, row, 8) & 0x00FF_FFFF
    }

    func flags(_ col: Int, _ row: Int) -> UInt8 {
        UInt8(truncatingIfNeeded: word(col, row, 4) >> 24)
    }

    /// The character drawn in the cell, or nil for blanks/spacers/hidden.
    func character(_ col: Int, _ row: Int) -> Character? {
        let flags = flags(col, row)
        if flags & (CellFlag.wideSpacer | CellFlag.hidden) != 0 { return nil }
        let cp = codePoint(col, row)
        if cp == 0 || cp == 0x20 { return nil }
        guard let scalar = Unicode.Scalar(cp) else { return nil }
        return Character(scalar)
    }

    /// Text of one row with trailing blanks trimmed (spacers skipped).
    func rowText(_ row: Int) -> String {
        var text = ""
        for col in 0 ..< cols {
            let flags = flags(col, row)
            if flags & CellFlag.wideSpacer != 0 { continue }
            let cp = flags & CellFlag.hidden != 0 ? 0 : codePoint(col, row)
            if let scalar = Unicode.Scalar(cp == 0 ? 0x20 : cp) {
                text.unicodeScalars.append(scalar)
            }
        }
        while text.last == " " { text.removeLast() }
        return text
    }
}
