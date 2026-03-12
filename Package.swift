// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "macland",
    platforms: [
        .macOS(.v14),
    ],
    products: [
        .library(name: "MaclandHostSupport", targets: ["MaclandHostSupport"]),
        .executable(name: "macland-host", targets: ["macland-host"]),
        .executable(name: "macland-host-selftest", targets: ["macland-host-selftest"]),
    ],
    targets: [
        .target(
            name: "MaclandHostSupport",
            path: "Sources/MaclandHostSupport"
        ),
        .executableTarget(
            name: "macland-host",
            dependencies: ["MaclandHostSupport"],
            path: "Sources/macland-host"
        ),
        .executableTarget(
            name: "macland-host-selftest",
            dependencies: ["MaclandHostSupport"],
            path: "Sources/macland-host-selftest"
        ),
    ]
)
