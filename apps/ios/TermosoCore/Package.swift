// swift-tools-version: 5.9
// Swift package wrapping the Rust `termoso-mobile` core for iOS.
//
// `Sources/TermosoCore/TermosoCore.swift` and `TermosoCoreFFI.xcframework`
// are produced by `apps/ios/build-core.sh` and are not checked in.
import PackageDescription

let package = Package(
    name: "TermosoCore",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "TermosoCore", targets: ["TermosoCore"]),
    ],
    targets: [
        .binaryTarget(
            name: "TermosoCoreFFI",
            path: "TermosoCoreFFI.xcframework"
        ),
        .target(
            name: "TermosoCore",
            dependencies: ["TermosoCoreFFI"],
            path: "Sources/TermosoCore",
            linkerSettings: [
                .linkedFramework("Security"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("SystemConfiguration"),
            ]
        ),
    ]
)
