import Foundation
import TermosoCore
@testable import Termoso
import XCTest

final class CellGridTests: XCTestCase {
    /// Packs one cell the way the Rust side does: code point, flags<<24 | fg, bg.
    private func cell(_ scalar: UInt32, fg: UInt32 = 0xD7DEE9, bg: UInt32 = 0x101722, flags: UInt8 = 0) -> [UInt8] {
        var bytes: [UInt8] = []
        for value in [scalar, (UInt32(flags) << 24) | (fg & 0x00FF_FFFF), bg & 0x00FF_FFFF] {
            bytes.append(UInt8(value & 0xFF))
            bytes.append(UInt8((value >> 8) & 0xFF))
            bytes.append(UInt8((value >> 16) & 0xFF))
            bytes.append(UInt8((value >> 24) & 0xFF))
        }
        return bytes
    }

    func testDecodesCharactersColorsAndFlags() {
        var bytes: [UInt8] = []
        bytes += cell(UInt32(UnicodeScalar("h").value), flags: CellFlag.bold)
        bytes += cell(UInt32(UnicodeScalar("i").value), fg: 0x2BB884, flags: CellFlag.underline | CellFlag.italic)
        bytes += cell(0) // empty
        bytes += cell(0x4E16, flags: CellFlag.wide) // 世
        bytes += cell(0, flags: CellFlag.wideSpacer)
        bytes += cell(UInt32(UnicodeScalar("x").value), flags: CellFlag.hidden)
        let grid = CellGrid(cols: 3, rows: 2, cells: Data(bytes))

        XCTAssertEqual(grid.character(0, 0), "h")
        XCTAssertEqual(grid.character(1, 0), "i")
        XCTAssertNil(grid.character(2, 0))
        XCTAssertEqual(grid.flags(0, 0) & CellFlag.bold, CellFlag.bold)
        XCTAssertEqual(grid.flags(1, 0) & CellFlag.underline, CellFlag.underline)
        XCTAssertEqual(grid.foreground(1, 0), 0x2BB884)
        XCTAssertEqual(grid.background(0, 0), 0x101722)
        XCTAssertEqual(grid.character(0, 1), "世")
        XCTAssertNil(grid.character(1, 1), "wide spacer must not render")
        XCTAssertNil(grid.character(2, 1), "hidden cell must not render")
        XCTAssertEqual(grid.rowText(0), "hi")
        XCTAssertEqual(grid.rowText(1), "世")
    }

    func testShortBufferIsZeroPadded() {
        let grid = CellGrid(cols: 4, rows: 2, cells: Data(cell(UInt32(UnicodeScalar("a").value))))
        XCTAssertEqual(grid.character(0, 0), "a")
        XCTAssertNil(grid.character(3, 1))
        XCTAssertEqual(grid.rowText(1), "")
    }
}

final class QuickTargetParserTests: XCTestCase {
    func testUserHostPort() {
        let t = QuickTargetParser.parse("deploy@pq.example.org:2222")
        XCTAssertEqual(t?.username, "deploy")
        XCTAssertEqual(t?.host, "pq.example.org")
        XCTAssertEqual(t?.port, 2222)
        XCTAssertEqual(t?.protocol, "ssh")
    }

    func testDefaultsAndTelnet() {
        XCTAssertEqual(QuickTargetParser.parse("example.org")?.port, 22)
        XCTAssertEqual(QuickTargetParser.parse("example.org")?.username, "")
        let telnet = QuickTargetParser.parse("telnet://bbs.example.org")
        XCTAssertEqual(telnet?.protocol, "telnet")
        XCTAssertEqual(telnet?.port, 23)
        XCTAssertEqual(QuickTargetParser.parse("ssh://root@10.0.0.1")?.username, "root")
    }

    func testIPv6AndInvalid() {
        let v6 = QuickTargetParser.parse("[2001:db8::1]:2200")
        XCTAssertEqual(v6?.host, "2001:db8::1")
        XCTAssertEqual(v6?.port, 2200)
        XCTAssertEqual(QuickTargetParser.parse("2001:db8::1")?.host, "2001:db8::1")
        XCTAssertNil(QuickTargetParser.parse(""))
        XCTAssertNil(QuickTargetParser.parse("host:notaport"))
        XCTAssertNil(QuickTargetParser.parse("host:0"))
        XCTAssertNil(QuickTargetParser.parse("[2001:db8::1"))
        XCTAssertNil(QuickTargetParser.parse("two words"))
    }
}

final class StickyModsTests: XCTestCase {
    func testToggleAndClear() {
        var mods = StickyMods()
        XCTAssertFalse(mods.any)
        mods.toggle(.ctrl)
        XCTAssertTrue(mods.isOn(.ctrl))
        XCTAssertTrue(mods.keyMods.ctrl)
        XCTAssertFalse(mods.keyMods.alt)
        mods.toggle(.ctrl)
        XCTAssertFalse(mods.any)
        mods.toggle(.alt)
        mods.toggle(.shift)
        mods.clear()
        XCTAssertFalse(mods.any)
    }
}
