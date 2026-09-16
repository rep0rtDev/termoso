// Prints the CGWindow id of the frontmost on-screen window owned by the given
// pid (largest one if several), or exits 1 when there is none yet.
// usage: swift window.swift <pid>
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 2, let pid = Int(CommandLine.arguments[1]) else {
    FileHandle.standardError.write("usage: window.swift <pid>\n".data(using: .utf8)!)
    exit(2)
}

let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
    exit(1)
}

var best: (id: Int, area: Double)? = nil
for w in windows {
    guard let owner = w[kCGWindowOwnerPID as String] as? Int, owner == pid,
        let id = w[kCGWindowNumber as String] as? Int,
        let bounds = w[kCGWindowBounds as String] as? [String: Double],
        let width = bounds["Width"], let height = bounds["Height"]
    else { continue }
    let area = width * height
    if best == nil || area > best!.area {
        best = (id, area)
    }
}

if let best = best {
    print(best.id)
} else {
    exit(1)
}
