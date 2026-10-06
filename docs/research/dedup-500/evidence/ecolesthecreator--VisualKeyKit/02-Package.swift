// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "VisualKeyKit",
    platforms: [
        .iOS(.v15),
        .macOS(.v12)
    ],
    products: [
        .library(
            name: "VisualKeyKit",
            targets: ["VisualKeyKit"]
        ),
    ],
    targets: [
        .target(
            name: "VisualKeyKit",
            dependencies: []
        ),
        .testTarget(
            name: "VisualKeyKitTests",
            dependencies: ["VisualKeyKit"]
        ),
    ]
)
