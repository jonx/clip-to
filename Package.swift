// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "ClipTo",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(
            name: "ct",
            path: "Sources/ct",
            linkerSettings: [.linkedFramework("AppKit"), .linkedFramework("Carbon")]
        )
    ]
)
